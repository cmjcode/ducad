#!/usr/bin/env python3
"""Eval harness DUCAD (P6.2) — hanya pustaka standar Python 3.

Menjalankan setiap tugas `tasks/<id>.json` N kali lewat perintah agent
(templat shell dengan {prompt_file} dan {workdir}), lalu menilai
`out/part.ducad` dengan `ducad-cli check` (fallback: `ducad-cli inspect`
+ pembanding Python untuk valid/body_count/volume/bbox_size/bbox_max).

Contoh:
  python3 evals/run_eval.py --tasks evals/tasks --runs 1 --agent-cmd "true"
  python3 evals/run_eval.py --agent-cmd 'claude -p "$(cat {prompt_file})" \\
      --mcp-config {workdir}/.mcp.json --permission-mode acceptEdits'
"""

import argparse
import datetime as dt
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
EDITOR = HERE.parent

# Check yang dinilai runner sendiri (tidak dikenal `ducad-cli check`).
RUNNER_ONLY = {"oplog_len"}


def find_binary(name, env_var):
    """Cari binary: variabel lingkungan → PATH → ~/.cargo/bin → target/."""
    if os.environ.get(env_var):
        return os.environ[env_var]
    found = shutil.which(name)
    if found:
        return found
    for cand in (
        Path.home() / ".cargo" / "bin" / name,
        EDITOR / "target" / "release" / name,
        EDITOR / "target" / "debug" / name,
    ):
        if cand.exists():
            return str(cand)
    return None


def run_cmd(args, cwd=None, timeout=120):
    try:
        p = subprocess.run(args, cwd=cwd, capture_output=True, text=True, timeout=timeout)
        return p.returncode, p.stdout, p.stderr
    except (OSError, subprocess.TimeoutExpired) as e:
        return None, "", str(e)


def cli_has_check(cli):
    code, _, _ = run_cmd([cli, "check", "--help"])
    return code == 0


def write_mcp_config(workdir, mcp_bin, vault_copy):
    servers = {"ducad": {"command": mcp_bin or "ducad-mcp", "args": ["--root", str(workdir)]}}
    if vault_copy:
        mnemonic = find_binary("mnemonic-cli", "DUCAD_MNEMONIC") or "mnemonic-cli"
        servers["mnemonic"] = {"command": mnemonic, "args": ["--vault", str(vault_copy), "mcp"]}
    (workdir / ".mcp.json").write_text(json.dumps({"mcpServers": servers}, indent=2) + "\n")


def fail_all(checks, message):
    return [
        {"index": i, "kind": c.get("check", "?"), "id": c.get("id"), "status": "error", "message": message}
        for i, c in enumerate(checks)
    ]


def pick_body(bodies, sel):
    if sel == "*":
        return bodies[0] if len(bodies) == 1 else None
    return next((b for b in bodies if b["name"] == sel), None)


def fallback_check(summary, check):
    """Penilai minimal berbasis `inspect --json` (sebelum P7 ada)."""
    bodies = summary.get("bodies", [])
    kind = check.get("check")
    if kind == "body_count":
        n = len(bodies)
        return n == check["expect"], n
    if kind == "valid":
        return bool(bodies) and all(b["valid"] for b in bodies), [b["valid"] for b in bodies]
    body = pick_body(bodies, check.get("body", "*"))
    if body is None:
        return None, "body tidak ditemukan / bukan satu-satunya"
    if kind == "volume":
        v = body["volume"]
        if check.get("expect") is not None:
            return abs(v - check["expect"]) / check["expect"] * 100 <= check.get("tol_pct", 1.0), v
        lo, hi = check.get("min"), check.get("max")
        return (lo is None or v >= lo) and (hi is None or v <= hi), v
    if kind == "bbox_size":
        s = body["size"]
        return all(abs(a - b) <= check.get("tol", 0.05) for a, b in zip(s, check["expect"])), s
    if kind == "bbox_max":
        s = body["size"]
        return all(a <= b + 1e-3 for a, b in zip(s, check["max"])), s
    return None, f"check '{kind}' butuh ducad-cli check (P7)"


def evaluate(cli, part, checks, workdir):
    """Nilai `part` terhadap `checks`; hasil = daftar dict per check."""
    if not part.exists():
        return fail_all(checks, "out/part.ducad tidak ada")
    if cli is None:
        return fail_all(checks, "ducad-cli tidak ditemukan (set DUCAD_CLI)")
    code, out, err = run_cmd([cli, "inspect", str(part), "--json"])
    if code != 0:
        return fail_all(checks, f"inspect gagal: {err.strip()[:300]}")
    summary = json.loads(out)
    results = [None] * len(checks)
    engine_checks = [(i, c) for i, c in enumerate(checks) if c.get("check") not in RUNNER_ONLY]
    for i, c in enumerate(checks):
        if c.get("check") == "oplog_len":
            n = summary.get("oplog_len")
            ok = n == c["expect"]
            results[i] = {"index": i, "kind": "oplog_len", "id": c.get("id"),
                          "status": "pass" if ok else "fail", "measured": n, "expected": c["expect"]}
    if engine_checks and cli_has_check(cli):
        checks_file = workdir / "eval_checks.json"
        checks_file.write_text(json.dumps([c for _, c in engine_checks]))
        code, out, err = run_cmd([cli, "check", str(part), "--checks", str(checks_file), "--json"])
        try:
            report = json.loads(out)
            for (i, _), r in zip(engine_checks, report["results"]):
                results[i] = dict(r, index=i)
        except (ValueError, KeyError):
            for i, c in engine_checks:
                results[i] = {"index": i, "kind": c.get("check"), "status": "error",
                              "message": f"check gagal (kode {code}): {err.strip()[:300]}"}
    else:
        for i, c in engine_checks:
            ok, measured = fallback_check(summary, c)
            status = "error" if ok is None else ("pass" if ok else "fail")
            results[i] = {"index": i, "kind": c.get("check"), "id": c.get("id"),
                          "status": status, "measured": measured}
    return results


def setup_task(task, workdir, cli):
    ops = task.get("setup_ops")
    if not ops:
        return None
    if cli is None:
        return "ducad-cli tidak ditemukan untuk setup"
    (workdir / "in").mkdir(exist_ok=True)
    code, _, err = run_cmd([cli, "run", str(HERE / ops), "--out", str(workdir / "in" / "part.ducad")])
    return None if code == 0 else f"setup gagal: {err.strip()[:300]}"


def run_one(task, run_idx, args, cli, mcp_bin):
    workdir = Path(tempfile.mkdtemp(prefix=f"ducad-eval-{task['id']}-{run_idx}-"))
    (workdir / "out").mkdir()
    vault_copy = None
    if args.with_memory:
        vault_copy = workdir / "vault"
        if Path(args.vault).exists():
            shutil.copytree(args.vault, vault_copy)
        else:
            vault_copy.mkdir()
    write_mcp_config(workdir, mcp_bin, vault_copy)
    prompt_file = workdir / "prompt.txt"
    prompt_file.write_text(task["prompt"] + "\n")

    record = {"task": task["id"], "run": run_idx, "workdir": str(workdir)}
    setup_err = setup_task(task, workdir, cli)
    start = time.monotonic()
    if setup_err:
        record.update(exit_code=None, wall_s=0.0, transcript_bytes=0,
                      results=fail_all(task["checks"], setup_err))
        return record
    cmd = args.agent_cmd.format(prompt_file=str(prompt_file), workdir=str(workdir))
    try:
        p = subprocess.run(cmd, shell=True, cwd=workdir, capture_output=True, text=True, timeout=args.timeout)
        exit_code, transcript = p.returncode, (p.stdout or "") + (p.stderr or "")
    except subprocess.TimeoutExpired as e:
        exit_code, transcript = "timeout", str(e.stdout or "")
    wall = time.monotonic() - start
    (workdir / "transcript.txt").write_text(transcript)
    record.update(exit_code=exit_code, wall_s=round(wall, 2), transcript_bytes=len(transcript.encode()),
                  results=evaluate(cli, workdir / "out" / "part.ducad", task["checks"], workdir))
    record["passed"] = all(r["status"] == "pass" for r in record["results"])
    if not args.keep_workdirs:
        shutil.rmtree(workdir, ignore_errors=True)
    return record


def write_summary(records, tasks, out_dir):
    per_task = {}
    fail_counts = {}
    for r in records:
        t = per_task.setdefault(r["task"], {"runs": 0, "passed": 0, "wall": []})
        t["runs"] += 1
        t["passed"] += 1 if r.get("passed") else 0
        t["wall"].append(r["wall_s"])
        for c in r["results"]:
            if c["status"] != "pass":
                key = c.get("kind", "?")
                fail_counts[key] = fail_counts.get(key, 0) + 1
    summary = {
        "tasks": {k: {"runs": v["runs"], "passed": v["passed"],
                      "pass_rate": v["passed"] / v["runs"] if v["runs"] else 0.0,
                      "mean_wall_s": sum(v["wall"]) / len(v["wall"]) if v["wall"] else 0.0}
                  for k, v in per_task.items()},
        "overall_pass_rate": (sum(1 for r in records if r.get("passed")) / len(records)) if records else 0.0,
        "most_failed_checks": sorted(fail_counts.items(), key=lambda kv: -kv[1]),
        "records": records,
    }
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")
    lines = ["# Hasil eval DUCAD", "",
             f"Pass-rate keseluruhan: **{summary['overall_pass_rate'] * 100:.0f}%** ({len(records)} run)", "",
             "| Tugas | Run | Lulus | Pass-rate | Rata-rata waktu (s) |", "|---|---|---|---|---|"]
    for t in tasks:
        s = summary["tasks"].get(t["id"])
        if s:
            lines.append(f"| {t['id']} | {s['runs']} | {s['passed']} | {s['pass_rate'] * 100:.0f}% | {s['mean_wall_s']:.1f} |")
    lines += ["", "## Check yang paling sering gagal", "", "| Check | Jumlah gagal/error |", "|---|---|"]
    lines += [f"| {k} | {n} |" for k, n in summary["most_failed_checks"]] or ["| – | 0 |"]
    (out_dir / "summary.md").write_text("\n".join(lines) + "\n")
    return summary


def main():
    ap = argparse.ArgumentParser(description="Eval harness DUCAD")
    ap.add_argument("--tasks", default=str(HERE / "tasks"))
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--agent-cmd", default=os.environ.get("DUCAD_EVAL_AGENT_CMD"))
    ap.add_argument("--timeout", type=int, default=600)
    ap.add_argument("--out", default=None)
    ap.add_argument("--with-memory", action="store_true", help="sertakan server mnemonic (salinan vault)")
    ap.add_argument("--vault", default=str(Path.home() / "DUCAD-Memory"))
    ap.add_argument("--keep-workdirs", action="store_true")
    ap.add_argument("--only", action="append", default=[], help="jalankan tugas tertentu saja (boleh diulang)")
    args = ap.parse_args()
    if not args.agent_cmd:
        ap.error("--agent-cmd (atau DUCAD_EVAL_AGENT_CMD) wajib diisi")
    if args.runs < 1:
        ap.error("--runs minimal 1")

    tasks = []
    for f in sorted(Path(args.tasks).glob("*.json")):
        t = json.loads(f.read_text())
        if not args.only or t["id"] in args.only:
            tasks.append(t)
    if not tasks:
        ap.error(f"tidak ada tugas di {args.tasks}")

    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    out_dir = Path(args.out) if args.out else HERE / "results" / stamp
    out_dir.mkdir(parents=True, exist_ok=True)
    cli = find_binary("ducad-cli", "DUCAD_CLI")
    mcp_bin = find_binary("ducad-mcp", "DUCAD_MCP")
    if cli is None:
        print("peringatan: ducad-cli tidak ditemukan; semua check akan error", file=sys.stderr)

    records = []
    for t in tasks:
        for i in range(args.runs):
            rec = run_one(t, i, args, cli, mcp_bin)
            status = "LULUS" if rec.get("passed") else "gagal"
            print(f"[{t['id']} #{i}] {status} ({rec['wall_s']} s)", file=sys.stderr)
            records.append(rec)
    summary = write_summary(records, tasks, out_dir)
    print(f"ringkasan: {out_dir / 'summary.md'} (pass-rate {summary['overall_pass_rate'] * 100:.0f}%)", file=sys.stderr)


if __name__ == "__main__":
    main()

//! Paket aturan (W1): kumpulan check bersitasi per proses/standar, disimpan
//! sebagai data di `rules/*.json` dan dikembangkan menjadi `CheckItem`
//! konkret. Deterministik dan tanpa LLM: agent cukup menyebut nama paket.
//!
//! Variabel paket ditulis `@nama` di dalam string JSON aturan. String yang
//! seluruhnya satu variabel digantikan nilai variabel itu apa adanya (angka
//! tetap angka); selain itu penggantian bersifat tekstual, sehingga
//! `"4 * (@thickness)"` menjadi ekspresi `Num` biasa.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::{CheckItem, Process, Severity};
use crate::error::{OpError, OpErrorCode, OpResult};

/// Dasar sebuah aturan: isi standar, atau pedoman praktik.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleBasis {
    Standard,
    Guideline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleVar {
    /// Nilai bawaan (angka atau string).
    pub value: Value,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub basis: RuleBasis,
    pub citation: String,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default)]
    pub note: String,
    /// `Check` dalam bentuk JSON, boleh memuat `@variabel`.
    pub check: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulePack {
    pub id: String,
    pub title: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,
    pub description: String,
    #[serde(default)]
    pub vars: BTreeMap<String, RuleVar>,
    pub rules: Vec<Rule>,
}

const SOURCES: &[&str] = &[
    include_str!("../../rules/iso-2768.json"),
    include_str!("../../rules/dfm-cnc.json"),
    include_str!("../../rules/dfm-fdm.json"),
    include_str!("../../rules/dfm-injection.json"),
    include_str!("../../rules/dfm-sheet-metal.json"),
    include_str!("../../rules/fea-safety.json"),
];

/// Semua paket aturan bawaan. Berkas yang rusak dilewati (dijaga tes
/// `every_builtin_pack_parses`).
pub fn rule_packs() -> Vec<RulePack> {
    SOURCES
        .iter()
        .filter_map(|s| serde_json::from_str::<RulePack>(s).ok())
        .collect()
}

pub fn rule_pack(id: &str) -> Option<RulePack> {
    rule_packs()
        .into_iter()
        .find(|p| p.id.eq_ignore_ascii_case(id))
}

fn star() -> String {
    "*".to_string()
}

/// A rule pack to expand into checks: either its id (`"dfm-cnc"`) or
/// `{"pack": "dfm-cnc", "body": "plate", "vars": {"min_wall": 1.5}}`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct RulePackRef {
    /// Pack id; list them with `get_schema {"rule_packs": true}`.
    pub pack: String,
    /// Body the rules apply to; `"*"` (default) = the only body.
    pub body: String,
    /// Overrides of the pack variables (number or string; a string may be an expression such
    /// as `"$t"`).
    pub vars: BTreeMap<String, Value>,
    /// Requirement id (see `set_brief`) every expanded check is traced to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
}

impl<'de> Deserialize<'de> for RulePackRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Full {
            pack: String,
            #[serde(default = "star")]
            body: String,
            #[serde(default)]
            vars: BTreeMap<String, Value>,
            #[serde(default)]
            requirement: Option<String>,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Id(String),
            Full(Full),
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Id(pack) => RulePackRef {
                pack,
                body: star(),
                vars: BTreeMap::new(),
                requirement: None,
            },
            Raw::Full(f) => RulePackRef {
                pack: f.pack,
                body: f.body,
                vars: f.vars,
                requirement: f.requirement,
            },
        })
    }
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn var_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Ganti `@nama` di dalam `text`. Variabel tak dikenal → `Err(nama)`.
fn substitute(text: &str, vars: &BTreeMap<String, Value>) -> Result<Value, String> {
    if let Some(name) = text.strip_prefix('@') {
        if !name.is_empty() && name.chars().all(is_ident) {
            return vars.get(name).cloned().ok_or_else(|| name.to_string());
        }
    }
    if !text.contains('@') {
        return Ok(Value::String(text.to_string()));
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('@') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let end = tail.find(|c| !is_ident(c)).unwrap_or(tail.len());
        let name = &tail[..end];
        if name.is_empty() {
            out.push('@');
        } else {
            out.push_str(&var_text(vars.get(name).ok_or_else(|| name.to_string())?));
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    Ok(Value::String(out))
}

fn substitute_value(v: &Value, vars: &BTreeMap<String, Value>) -> Result<Value, String> {
    Ok(match v {
        Value::String(s) => substitute(s, vars)?,
        Value::Array(a) => Value::Array(
            a.iter()
                .map(|x| substitute_value(x, vars))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| Ok((k.clone(), substitute_value(x, vars)?)))
                .collect::<Result<_, String>>()?,
        ),
        other => other.clone(),
    })
}

/// Kembangkan satu paket menjadi check konkret. Id check = `<paket>/<aturan>`.
pub fn expand_rule_pack(r: &RulePackRef) -> OpResult<Vec<CheckItem>> {
    let known: Vec<String> = rule_packs().into_iter().map(|p| p.id).collect();
    let pack = rule_pack(&r.pack).ok_or_else(|| {
        OpError::new(
            OpErrorCode::UnknownRef,
            format!(
                "rule pack '{}' does not exist (available: {known:?})",
                r.pack
            ),
        )
        .with_hint("list the packs and their variables with get_schema {\"rule_packs\": true}")
        .with_context(serde_json::json!({ "rule_pack": r.pack, "available": known }))
    })?;
    let mut vars: BTreeMap<String, Value> = pack
        .vars
        .iter()
        .map(|(k, v)| (k.clone(), v.value.clone()))
        .collect();
    for (k, v) in &r.vars {
        if !vars.contains_key(k) {
            let names: Vec<&String> = pack.vars.keys().collect();
            return Err(OpError::invalid(format!(
                "rule pack '{}' has no variable '{k}' (variables: {names:?})",
                pack.id
            ))
            .with_context(serde_json::json!({ "rule_pack": pack.id, "variables": names })));
        }
        if !(v.is_number() || v.is_string()) {
            return Err(OpError::invalid(format!(
                "variable '{k}' of rule pack '{}' must be a number or a string",
                pack.id
            )));
        }
        vars.insert(k.clone(), v.clone());
    }
    vars.insert("body".to_string(), Value::String(r.body.clone()));

    let mut out = Vec::with_capacity(pack.rules.len());
    for rule in &pack.rules {
        let check = substitute_value(&rule.check, &vars).map_err(|name| {
            OpError::invalid(format!(
                "rule '{}/{}' uses the undefined variable '@{name}'",
                pack.id, rule.id
            ))
        })?;
        let check = serde_json::from_value(check).map_err(|e| {
            OpError::invalid(format!(
                "rule '{}/{}' does not expand to a valid check: {e}",
                pack.id, rule.id
            ))
            .with_hint("check the variable values (numbers, or axis/class/process names)")
        })?;
        out.push(CheckItem {
            id: Some(format!("{}/{}", pack.id, rule.id)),
            note: (!rule.note.is_empty()).then(|| rule.note.clone()),
            requirement: r.requirement.clone(),
            citation: Some(rule.citation.clone()),
            severity: rule.severity,
            rule: Some(format!("{}@{}/{}", pack.id, pack.version, rule.id)),
            check,
        });
    }
    Ok(out)
}

/// Ringkasan paket untuk `get_schema {"rule_packs": true}`.
pub fn rule_pack_catalog() -> Value {
    Value::Array(
        rule_packs()
            .into_iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "title": p.title,
                    "version": p.version,
                    "process": p.process,
                    "description": p.description,
                    "vars": p.vars,
                    "rules": p.rules.iter().map(|r| serde_json::json!({
                        "id": r.id, "basis": r.basis, "severity": r.severity,
                        "check": r.check.get("check"), "citation": r.citation,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// Peringatan yang wajib menyertai setiap laporan kepatuhan.
pub const RULE_DISCLAIMER: &str = "Rule packs encode standards excerpts and shop guidelines as automatic checks. A passing report does not replace review and sign-off by a qualified engineer.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_pack_parses() {
        for src in SOURCES {
            let pack: RulePack = serde_json::from_str(src).expect("paket aturan harus valid");
            assert!(!pack.version.is_empty(), "{}", pack.id);
        }
        assert_eq!(rule_packs().len(), SOURCES.len());
    }

    #[test]
    fn every_rule_has_a_citation_and_expands_with_defaults() {
        for pack in rule_packs() {
            assert!(!pack.rules.is_empty(), "{}", pack.id);
            for rule in &pack.rules {
                assert!(
                    rule.citation.trim().len() >= 20,
                    "{}/{} tanpa sitasi yang berarti",
                    pack.id,
                    rule.id
                );
                // Pedoman tidak boleh menyamar sebagai standar.
                assert_eq!(
                    rule.citation.starts_with("Guideline:"),
                    rule.basis == RuleBasis::Guideline,
                    "{}/{}",
                    pack.id,
                    rule.id
                );
            }
            let items = expand_rule_pack(&RulePackRef {
                pack: pack.id.clone(),
                body: "*".into(),
                vars: BTreeMap::new(),
                requirement: Some("R1".into()),
            })
            .unwrap_or_else(|e| panic!("{}: {}", pack.id, e.message));
            assert_eq!(items.len(), pack.rules.len());
            for item in &items {
                assert!(item.citation.is_some() && item.rule.is_some());
                assert_eq!(item.requirement.as_deref(), Some("R1"));
            }
        }
    }

    #[test]
    fn vars_override_and_reject_unknown() {
        let r: RulePackRef = serde_json::from_str(
            r#"{"pack":"dfm-sheet-metal","body":"box","vars":{"thickness":"$t"}}"#,
        )
        .unwrap();
        let items = expand_rule_pack(&r).unwrap();
        let flange = items
            .iter()
            .find(|i| i.id.as_deref() == Some("dfm-sheet-metal/flange-length"))
            .unwrap();
        let json = serde_json::to_value(flange).unwrap();
        assert_eq!(json["min"], "4.0 * ($t)");
        assert_eq!(json["body"], "box");
        assert_eq!(json["rule"], "dfm-sheet-metal@1/flange-length");

        let short: RulePackRef = serde_json::from_str(r#""dfm-cnc""#).unwrap();
        assert_eq!(short.body, "*");
        let bad: RulePackRef =
            serde_json::from_str(r#"{"pack":"dfm-cnc","vars":{"nope":1}}"#).unwrap();
        assert!(expand_rule_pack(&bad).unwrap_err().message.contains("nope"));
        let missing: RulePackRef = serde_json::from_str(r#""dfm-laser""#).unwrap();
        assert_eq!(
            expand_rule_pack(&missing).unwrap_err().code,
            OpErrorCode::UnknownRef
        );
    }
}

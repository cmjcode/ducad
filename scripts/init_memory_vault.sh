#!/usr/bin/env bash
# Buat vault memori agent DUCAD (MNEMONIC) — idempoten: file yang sudah ada
# TIDAK ditimpa. Pemakaian: scripts/init_memory_vault.sh [DIR]
# (default: $HOME/DUCAD-Memory). Templat ada di scripts/memory-vault/.
set -euo pipefail

VAULT="${1:-$HOME/DUCAD-Memory}"
TEMPLATES="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/memory-vault"

mkdir -p "$VAULT"/{Standards,Preferences,Lessons,Projects,Sessions,BOM}

created=0
skipped=0
while IFS= read -r -d '' src; do
    rel="${src#"$TEMPLATES"/}"
    dst="$VAULT/$rel"
    if [ -e "$dst" ]; then
        skipped=$((skipped + 1))
        continue
    fi
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
    created=$((created + 1))
done < <(find "$TEMPLATES" -type f -name '*.md' -print0)

echo "vault: $VAULT (dibuat $created file, dilewati $skipped yang sudah ada)" >&2

#!/usr/bin/env bash
# Inlines WORKFLOW.md into README.md between <!-- WORKFLOW_START --> and <!-- WORKFLOW_END -->.
# Keeps README in sync with the canonical WORKFLOW.md. Run after editing WORKFLOW.md.
# Usage: scripts/inline-workflow-into-readme.sh [--check]
#   --check: only verify README is in sync (exit 1 if running the script would change README).

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WORKFLOW_FILE="${REPO_ROOT}/WORKFLOW.md"
README_FILE="${REPO_ROOT}/README.md"
CHECK_MODE=false

if [[ "${1:-}" == "--check" ]]; then
    CHECK_MODE=true
fi

if [[ ! -f "$WORKFLOW_FILE" ]]; then
    echo "Error: WORKFLOW.md not found at $WORKFLOW_FILE" >&2
    exit 1
fi
if [[ ! -f "$README_FILE" ]]; then
    echo "Error: README.md not found at $README_FILE" >&2
    exit 1
fi

START_MARKER='<!-- WORKFLOW_START -->'
END_MARKER='<!-- WORKFLOW_END -->'

# Check markers exist
if ! grep -qF "$START_MARKER" "$README_FILE"; then
    echo "Error: README.md does not contain $START_MARKER" >&2
    exit 1
fi
if ! grep -qF "$END_MARKER" "$README_FILE"; then
    echo "Error: README.md does not contain $END_MARKER" >&2
    exit 1
fi

# Build block in a temp file so we can safely embed multi-line content
block_file="$(mktemp)"
trap 'rm -f "$block_file"' EXIT
{
    echo '```markdown'
    cat "$WORKFLOW_FILE"
    echo '```'
} > "$block_file"

replace_readme() {
    awk -v start="$START_MARKER" -v end="$END_MARKER" -v blockfile="$block_file" '
    BEGIN { in_block=0 }
    $0 == start {
      in_block=1
      print
      while ((getline line < blockfile) > 0) print line
      close(blockfile)
      next
    }
    in_block && $0 == end { in_block=0; print; next }
    in_block { next }
    { print }
  ' "$README_FILE"
}

if "$CHECK_MODE"; then
    out="$(mktemp)"
    trap 'rm -f "$block_file" "$out"' EXIT
    replace_readme > "$out"
    if ! diff -q "$README_FILE" "$out" >/dev/null 2>&1; then
        echo "README.md is out of sync with WORKFLOW.md. Run: devenv shell -- moon run :inline-workflow" >&2
        exit 1
    fi
    exit 0
fi

replace_readme > "${README_FILE}.tmp" && mv "${README_FILE}.tmp" "$README_FILE"

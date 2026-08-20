#!/usr/bin/env bash
set -euo pipefail

fmt="$1"
shift
[[ $# -gt 0 ]] || { echo "no grammar files" >&2; exit 1; }
"$fmt" --check "$@"
tmp="$(mktemp)"
trap 'rm -f "${tmp}"' EXIT
cat "$1" > "${tmp}"
printf '\n// dirty-format-check\n' >> "${tmp}"
if "$fmt" --check "${tmp}"; then
  echo "expected --check to fail on dirty input" >&2
  exit 1
fi

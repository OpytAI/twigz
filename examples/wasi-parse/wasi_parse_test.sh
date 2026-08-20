#!/usr/bin/env bash
set -euo pipefail
wasmtime="$1"
wasm="$2"
out="$("${wasmtime}" run "${wasm}")"
echo "${out}"
echo "${out}" | grep -q 'root=source_file'
echo "${out}" | grep -q 'function=local_function_declaration'

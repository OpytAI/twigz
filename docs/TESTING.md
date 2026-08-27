# Testing conventions

twigz tests are Bazel `rust_test` and `sh_test` targets. Keep tests hermetic
and deterministic. Do not use the network or the host C compiler.

## Layout

| Test type | Location | Bazel shape |
|-----------|----------|-------------|
| DSL / elaborate / format | `crates/dsl/dsl_test.rs` | `//crates/dsl:dsl_test` |
| Vocabulary | `crates/vocab` | `//crates/vocab:vocab_test` |
| IR / semantics goldens | `data/goldens/{ir,semantics,snapshot}` | `//crates/generate:golden_test` |
| Scanner contract | `crates/scan` | `//crates/scan:scan_test` |
| Structured / S-expr queries | `crates/query` | `//crates/query:query_test` |
| Packed parse | `crates/runtime` | `//crates/runtime:runtime_test` |
| cdylib load | `crates/runtime/load_test.rs` | `//crates/runtime:load_test` |
| Formatter | `grammars/format_test.sh` | `//grammars:format_test` |
| wasm32-wasi C parse | `examples/wasi-parse` | `//examples/wasi-parse:wasi_parse_test` (x86_64-linux wasmtime) |

Production `rust_library` targets must not depend on test-only packages.

## Behavioral goldens

Snapshot goldens freeze compiler output of the copied lua/luau sources.
Product goldens include product lua maps, `scan` rules, and javascript /
python / twiglet.

A lockstep test lists the exact mapping and scan differences between
snapshot and product lua/luau. Extra or omitted differences fail the test.

Do not discover tests through a handwritten import allowlist. Walk
`data/goldens` and `data/fixtures/source` directories.

`ABI_VERSION_MAX` is checked against `TREE_SITTER_PIN.md` in
`//crates/generate:golden_test`.

## Running tests

```bash
bazel test //...
```

Use Bazel with the repository toolchains. Do not use `cargo test` as a
substitute for these tests.

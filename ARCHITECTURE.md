# Architecture

twigz is a grammar compiler, scanner generator, parse runtime, and
language-neutral query library. Callers author `.grammar` files and ask
kinds, not concrete node names.

## Design principles

- Keep `.grammar` as the only authoring surface. The compiler emits C.
- Keep scanners in the same file (`scan`, `keep`, named machines).
- Keep the vocabulary in this repository (kinds 1–22).
- Keep pack outputs as `registry.json` and `registry.rs`. Do not emit Zig.
- Keep production libraries independent of test-only packages.
- Native `Parser` embeds. `wasm32-wasip1` is the C target
  `//examples/wasi-parse:wasi_parse_wasm`.

## Dependency direction

```text
ast, dsl, ir, vocab
          |
          v
elaborate, format, scan-lower
          |
          v
backend, generate, pack
          |
          v
runtime (Tree-sitter C + generated parser + generated scanner)
          |
          v
query (find / binding_at / S-expr)
          |
          v
twigz facade
```

Higher layers can depend on lower layers. The compile graph does not depend
on the C runtime.

## Major boundaries

| Area | Location | Responsibility |
|------|----------|----------------|
| Public facade | `crates/twigz` | Re-exports and `compile_grammar` |
| Compile | `crates/{ast,dsl,ir,elaborate,format,vocab,backend,generate,pack,scan}` | Author and compile grammars |
| Runtime | `crates/runtime` | Parse a buffer |
| Query | `crates/query` | Language-neutral questions |
| Grammars | `grammars/` | lua, luau, javascript, python, twiglet |
| Test data | `data/fixtures/`, `data/goldens/` | Committed inputs and expected results |

## Vocabulary

Kinds 1–19 match the copied concept list. This library appends `class` 20,
`namespace` 21, and `string` 22. `VOCABULARY_VERSION` is 2.
`GRAMMAR_IR_VERSION` is 2.

`binding_at` walks ancestors of the smallest named node covering a span and
returns the innermost node whose mapping derives `declaration`.

## Verification

Bazel is the only supported build and test entry point. See
[`docs/TESTING.md`](docs/TESTING.md).

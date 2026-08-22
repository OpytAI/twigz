<div align="center">
  <h1>twigz</h1>

  <p><strong>Author a grammar. Query any language the same way.</strong></p>

  <p>
    A Rust and C library for authoring <code>.grammar</code> files,<br>
    generating scanners from the same file, parsing buffers, and asking<br>
    language-neutral questions of the tree.
  </p>

  <p>
    <img alt="Rust" src="https://img.shields.io/badge/Rust-2021-dea584">
    <img alt="Native and WASI" src="https://img.shields.io/badge/targets-Native%20%7C%20wasm32--wasip1-654ff0">
    <img alt="Built with Bazel" src="https://img.shields.io/badge/build-Bazel-43a047">
  </p>

  <p>
    <a href="#why-twigz">Why twigz</a> ·
    <a href="#capabilities">Capabilities</a> ·
    <a href="#build">Build</a> ·
    <a href="./ARCHITECTURE.md">Architecture</a>
  </p>
</div>

---

## Why twigz

Editors, indexers, and linters need an owned grammar language and queries that
do not leak language-specific node names. twigz compiles `.grammar` files to a
Tree-sitter parser, emits scanners from `scan` rules in that same file, and
answers the same questions for every first-party language.

You do not write `grammar.js`. You do not write a scanner in C. The compiler
emits Tree-sitter’s five C symbols.

## Capabilities

- **Author** — a small EBNF language: sequences, `|`, `?*+`, literals,
  `/regex/`, fields, families, `=>` maps, and `scan` / named machines.
- **Compile and pack** — Grammar IR, `grammar.json`, `semantics.json`,
  `parser.c`, shared `tables.c`, `registry.json`, and `registry.rs`.
- **Parse** — `Parser` / `Tree` / `Node` / `edit` on packed languages (native).
  `wasm32-wasip1` is `//examples/wasi-parse:wasi_parse_wasm` (C). The Rust
  `Parser` is native until rust-lld can link the C runtime for WASI.
- **Query** — `find`, `binding_at`, and S-expr over vocabulary kinds
  (`function`, `class`, `import`, `string`, …).
- **First-party languages** — lua, luau, javascript, python, plus the twiglet
  contract fixture.

Honest limits: JavaScript has no ASI, JSX, TypeScript, or Annex B. Python has
no implicit line joining and is not CPython.

## Build

twigz uses Bazel, rules_rust, and a hermetic C toolchain. Do not use the
system `CC` or `cargo test` as the gate.

```sh
bazel build //...
bazel test //...
```

Useful targets:

| Target | Purpose |
| --- | --- |
| `//:twigz` | Public compile library (re-exports) |
| `//:twigz-runtime` | Parse buffers |
| `//:twigz-query` | Language-neutral queries |
| `//tools/grammar-gen:twigz-grammar-gen` | Compile a `.grammar` |
| `//tools/pack:twigz-pack` | Pack parsers |
| `//tools/fmt:twigz-fmt` | Format `.grammar` files |
| `//tools/query:twigz-query` | Run a query |

The repository `.bazelrc` selects hermetic build settings. Set the Bazel
output root in the ignored `user.bazelrc` file.

## Project status

twigz is a standalone grammar, parse, and query library.

| Document | What it covers |
| --- | --- |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Layers and dependency direction |
| [`docs/LANGUAGE.md`](docs/LANGUAGE.md) | `.grammar` language and lua maps |
| [`docs/TWIGLET.md`](docs/TWIGLET.md) | Contract fixture |
| [`docs/TESTING.md`](docs/TESTING.md) | Test layout |
| [`TREE_SITTER_PIN.md`](TREE_SITTER_PIN.md) | Tree-sitter pin and ABI |

## Integration

Depend on `@twigz//:twigz`. Use `//:twigz-runtime` to parse and
`//:twigz-query` to ask kinds, not concrete node names.

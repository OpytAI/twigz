<div align="center">
  <h1>twigz</h1>

  <p><strong>Author a grammar. Query any language the same way.</strong></p>

  <p>
    Write one <code>.grammar</code> file. twigz gives you a parser and the same<br>
    questions — function, class, import — on every language. You do not write<br>
    <code>grammar.js</code> or a C scanner.
  </p>

  <p>
    <img alt="Rust" src="https://img.shields.io/badge/Rust-2021-dea584">
    <img alt="Native and WASI" src="https://img.shields.io/badge/targets-Native%20%7C%20wasm32--wasip1-654ff0">
    <img alt="Built with Bazel" src="https://img.shields.io/badge/build-Bazel-43a047">
  </p>

  <p>
    <a href="#why-twigz">Why twigz</a> ·
    <a href="#first-program">First program</a> ·
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

The compiler emits Tree-sitter’s five C symbols. You do not maintain a
hand-written scanner next to the grammar.

## First program

A `.grammar` file is small EBNF plus a map onto a shared vocabulary:

```grammar
grammar example "1.0.0"
start document

skip whitespace | comment
token whitespace = /\s+/
token comment = "--" /[^\n]*/
  => comment
token identifier = /[A-Za-z_][A-Za-z0-9_]*/

document = body:statement*
  => module(body)
     derives scope
```

Ask for kinds, not concrete node names. Against the Lua fixture
`data/fixtures/source/lua/locals.lua`:

```sh
bazel run //tools/query:twigz-query -- query \
  --lang lua --view semantic \
  --source "$PWD/data/fixtures/source/lua/locals.lua" \
  function
```

That prints each `function` with a source location.

## Capabilities

- **Author** — a small EBNF language: sequences, `|`, `?*+`, literals,
  `/regex/`, fields, families, `=>` maps, and `scan` / named machines.
- **Parse** — `Parser` / `Tree` / `Node` / `edit` on packed languages.
- **Query** — the same kinds (`function`, `class`, `import`, `string`, …) on
  every first-party language.
- **Languages** — lua, luau, javascript, python, plus the twiglet contract
  fixture.

Honest limits: JavaScript has no ASI, JSX, TypeScript, or Annex B. Python has
no implicit line joining and is not CPython. Native parse is the supported
path; `wasm32-wasip1` ships a C parse example
(`//examples/wasi-parse:wasi_parse_wasm`). The Rust `Parser` stays native until
rust-lld can link the C runtime for WASI.

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

Generated compiler artifacts (`grammar.json`, `parser.c`, pack registries) are
described in [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Project status

twigz is a standalone grammar, parse, and query library.

| Document | What it covers |
| --- | --- |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Layers and dependency direction |
| [`docs/LANGUAGE.md`](docs/LANGUAGE.md) | `.grammar` language and lua maps |
| [`docs/TWIGLET.md`](docs/TWIGLET.md) | Contract fixture |
| [`docs/TESTING.md`](docs/TESTING.md) | Test layout |
| [`TREE_SITTER_PIN.md`](TREE_SITTER_PIN.md) | Tree-sitter pin and ABI |

Depend on `@twigz//:twigz`. Use `//:twigz-runtime` to parse and
`//:twigz-query` to ask kinds, not concrete node names.

Project-owned code is Apache-2.0. Tree-sitter remains MIT; see
[NOTICE](NOTICE).

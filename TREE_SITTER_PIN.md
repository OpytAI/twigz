# Tree-sitter pin

twigz owns this pin. The archive contributes the Rust generator core and the
generic C runtime. This repository does not execute Tree-sitter’s JavaScript
DSL, load community grammars, or use its CLI.

| Field | Value |
| --- | --- |
| Commit | `d11d18f746fdfd1826362c2531ce06808f386b02` |
| Archive sha256 | `50546072d031b0cc1bf2075f3c2a22cdc94f95845059f3c68060389eab560a40` |
| URL | `https://github.com/tree-sitter/tree-sitter/archive/d11d18f746fdfd1826362c2531ce06808f386b02.tar.gz` |
| Patches | `third_party/tree-sitter/patches/0001-json-only-generator.patch`, `0002-structured-parser-pack.patch` |
| ABI_VERSION_MAX | 15 |

`ABI_VERSION_MAX` is the `tree_sitter_generate::ABI_VERSION_MAX` constant
from this pin. A unit test reads the crate constant and this file together.
Do not assume the number if the pin changes.

Scanner ABI is the same pin. Serialization writes into a 1024-byte slice.

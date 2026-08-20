# Provenance

This file records where algorithms were copied from so the copy is
reproducible. It is not legal attribution.

Snapshot path: `/mnt/workspace/opytai/references/agent-os/`

| Snapshot path | Role in twigz |
| --- | --- |
| `bazel/tools/mc-grammar-gen/` | Compiler frontend, generate, pack |
| `memcontainers/programs/syntax/grammars/` | lua / luau / lua.core sources |
| `memcontainers/programs/syntax/glue/scanner.zig` | Long-bracket serialize/recovery behavior |
| `memcontainers/contracts/syntax.kdl` lines 12–47 | Vocabulary kinds 1–19 |
| `third_party/tree-sitter/` | Pin, patches, `BUILD` rewrite |

Tree-sitter commit and checksum live in [`TREE_SITTER_PIN.md`](../TREE_SITTER_PIN.md).

Protocol messages from that snapshot are not copied.

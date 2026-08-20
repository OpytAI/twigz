# Twiglet

Twiglet is a contract fixture. It exists so scanners and maps cannot stay
Lua-shaped. It is not a product language.

## Concrete syntax

- UTF-8. Newlines `\n` or `\r\n`.
- Keywords: `fn`, `import`.
- Comments: `#` to EOL → `comment` and `skip`.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`.
- Numbers: `[0-9]+` → `literal`.
- Quoted strings: `"([^"\\]|\\.)*"` → `string`.
- `fn` is a declaration statement, not an expression.
- A module is a sequence of statements.

## External tokens (`valid[]` order)

| Index | Token | Meaning |
| --- | --- | --- |
| 0 | `newline` | End of a non-blank line |
| 1 | `indent` | Indent increased |
| 2 | `dedent` | Indent decreased (one token per pop) |
| 3 | `interp_open` | `` ` `` + raw + `${` |
| 4 | `interp_close` | `}` + raw + `` ` `` |

```text
external newline | indent | dedent | interp_open | interp_close
scan indent newline, indent, dedent
scan template open interp_open, close interp_close
```

## Indent rules

- Spaces only. A tab in leading whitespace is a scan failure.
- Stack of absolute space counts, start `[0]`.
- `indent` / `dedent` / `newline` consume the triggering `\n`/`\r\n` and
  following leading spaces. `mark_end` after those bytes.
- Mid-line gaps use `token whitespace = /[ \t]+/` in `skip`.
- A blank line or `#` comment line emits nothing. The stack does not change.
  The scanner still consumes `\n` and spaces.
- Else `n` is the leading space count:
  - `n > top`: emit `indent`, push `n`.
  - `n == top`: emit `newline` only (`newline` is also `skip`).
  - `n < top`: one `dedent` per pop until `top == n`. If `n` is not on the
    stack, fail.
- EOF: `dedent` per stacked level above 0.

## Interpolation

`` `prefix${expr}suffix` ``. At most one `${…}`. No nesting. `expr` is an
identifier or a number. Unterminated input still emits the token that was
started.

## Serialize

```text
buf[0] unit_or_zero
buf[1] stack_len
buf[2] in_interp
buf[3..] stack bytes
```

Max 35 bytes. A short buffer resets to `{unit:0, stack:[0], in_interp:false}`.

## Required maps

| Production | Kind | Roles | Traits |
| --- | --- | --- | --- |
| `module` | `module` | `body` | `scope` |
| `fn_declaration` | `function` | `name`, `parameters`, `body` | `declaration`, `scope` |
| `parameter` | `parameter` | `name` | `declaration` |
| `import_statement` | `import` | `source` | `declaration` |
| `assign_statement` | `assignment` | `left`, `right` | — |
| `block` | `block` | `body` | `scope` |
| `line_comment` | `comment` | — | — |
| `quoted_string` / `interpolated_string` | `string` | — | — |
| `number` | `literal` | — | — |
| `identifier` | `identifier` | — | — |

The grammar source is `grammars/fixtures/twiglet.grammar`.

# Grammar language

`.grammar` is a small EBNF language. The compiler lowers it to Grammar IR,
then to Tree-sitter `grammar.json`. Tokens that are regular stay in EBNF.

## Surface

Adjacent expressions form a sequence. `|` forms a choice. `?`, `*`, and `+`
mean optional, zero-or-more, and one-or-more. Parentheses group an
expression. Quoted strings are literal tokens. `/.../imsu` is a regex token
with optional flags. A field is `name:expression`.

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

Production, field, and fragment names are C identifiers.

## Families

A family owns common productions and slots. The root grammar lists every
family in `use`. Bazel must supply the same module IDs.

Only an `open` production can be extended. A slot can be filled once. An
unfilled slot disappears inside `?`, `*`, or a choice.

## Operators

Operator tables generate `operator`, `left`, `right`, and `argument` fields:

```grammar
prefix unary_expression over expression
  => operator(right=argument)
  right 12: "not" | "-"

infix binary_expression over expression
  => operator(left, right)
  left   1: "or"
  right 13: "^"
```

## Semantics

`=> kind(...)` maps a concrete production onto the vocabulary in
`crates/vocab/vocabulary.kdl`. Arguments name canonical roles.
`role=field` maps a canonical role to a differently named field. `derives`
adds vocabulary traits.

`literal` is numbers, booleans, and null. `string` is quoted and template
text.

## Product lua maps

Product `lua.core` maps locals and for-loop names to `declaration` so
`binding_at` can see them:

```text
local_name = name:identifier type_annotation?
  => declaration(name)
     derives declaration

for_numeric_statement = "for" name:identifier "=" …
  => declaration(name)
     derives declaration

for_generic_statement = "for" name:separated1(identifier, ",") "in" …
  => declaration(name)
     derives declaration
```

`binding_at` on `local x = 1` hits `local_name`. Anonymous
`function() end` is `function` + `scope` only, so `binding_at` returns none.

## Scanners

Tokens that are not regular use `scan` in the same file.

```grammar
scan long_string_start = "[" pad:"="* "["
  keep pad
scan long_string_end = "]" "="{pad} "]"
scan long_string_content = (!long_string_end .)+
```

| Form | Meaning |
| --- | --- |
| `keep name` | Remember a capture in scanner state |
| `"="{pad}` | Repeat a literal as many times as the kept capture’s length |
| `.` | One input byte |
| `!rule` | Negative lookahead |
| `scan indent newline, indent, dedent` | Off-side rule |
| `scan slash regex, division` | `/` is regex or division |
| `scan template open …, close …` | Template interpolation |

`scan slash` lists `regex` and `division` as externals. After `)` `]`
identifier number string `this` `true`/`false`/`null`, the parser marks
`division` valid; otherwise `/` is `regex`. `valid[]` is that prefix table.

Every `external` name must have exactly one `scan` rule or appear in one
named machine. A named-machine token that is not an `external` is an error.

JavaScript maps `return` to `return`. It has no ASI, JSX, TypeScript, or
Annex B.

## Formatting

```bash
bazel run //tools/fmt:twigz-fmt -- path/to/file.grammar
bazel run //tools/fmt:twigz-fmt -- --check path/to/file.grammar
```

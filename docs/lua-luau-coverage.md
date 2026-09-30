# Lua and Luau parser coverage

This document tracks syntax that the packed `lua` and `luau` parsers accept
or reject against the language versions named in the grammars.

Checked on 2026-09-30 with the packed parsers in this worktree.
The check is `ts_node_has_error` on the concrete tree. The query
`//tools/query:twigz-query --view concrete` with capture `(ERROR @e)`
prints ERROR nodes. A MISSING node also makes `ts_node_has_error` true.

`grammars/lua.grammar`, `grammars/luau.grammar`, and
`grammars/families/lua-core.grammar` are ahead of the copies packed by
agent-os at twigz commit `767d8483`. The agent-os syntax service still
packs that commit. A pin bump belongs in a later agent-os change. The
service names the languages Lua 5.4 and Luau. A tree with an ERROR or
MISSING node becomes a `syntax_error` diagnostic in that service.

Both grammars `use lua.core`. Whitespace, comments, long brackets, tables,
and the shared statement list live in that family. Numbers, quoted
strings, and the Luau lexical keywords are external tokens on the
long-bracket scanner (`scan lexical`). Each root lists the externals it
uses.

## Versions

| Grammar | Version string | Spec used for this document |
| --- | --- | --- |
| `grammars/lua.grammar` | `5.4.0` | [Lua 5.4 reference manual](https://www.lua.org/manual/5.4/manual.html). Forms below were also loaded with Lua 5.4.9. |
| `grammars/luau.grammar` | `0.725.0` | Luau tag [0.725](https://github.com/luau-lang/luau/tree/0.725) (`91caa731`), `Ast/src/Lexer.cpp` and `Ast/src/Parser.cpp`. |

`luau-analyze` and `luau-compile` at that tag call `setLuauFlagsDefault()`.
That function enables every fast flag whose name starts with `Luau`, except
analysis experiments. Flags whose names start with `Debug` stay off.
`const`, `export` of values, and the `i` integer suffix are in that default
set (`LuauConst2`, `LuauExportValueSyntax`, `LuauIntegerType2`). The `luau`
REPL does not call `setLuauFlagsDefault()`. This document follows the
analyze and compile default.

The live [luau.org grammar](https://luau.org/grammar) is the current
language. Luau 0.740 was released on 2026-09-25. Compare rows below with
tag 0.725. Later syntax is listed under [Later Luau](#later-luau).

## How a row is classified

| Parser result | Meaning |
| --- | --- |
| Rejects valid syntax | The spec accepts the form. The packed parser has an ERROR or MISSING node. |
| Accepts invalid syntax | The spec rejects the form. The packed parser has no ERROR or MISSING node. |
| Matches | The parser agrees with that language. |

## Shared lexical forms

Lua numbers are the `number` regex in `grammars/lua.grammar`. Luau numbers
are the lexical arm. Quoted strings share one C routine. Lua rejects an
unknown letter escape. Luau keeps that letter. Whitespace in `lua.core`
includes vertical tab and form feed.

| Form | Lua 5.4 | Luau 0.725 | Packed parser | Status |
| --- | --- | --- | --- | --- |
| `1_000`, `0xFF_FF` | Malformed number. [§3.1](https://www.lua.org/manual/5.4/manual.html#3.1) has no digit separators. | One number. The lexer keeps `_`. `parseNumber` deletes `_` before conversion. | Lua has an ERROR node. Luau has one number and no error. | matches |
| `0b1010`, `0b_0101_0101` | Malformed number. | One binary number. | Lua has an ERROR node. Luau has one number and no error. | matches |
| `123i`, `0xFFi` | Malformed number. | One integer when `LuauIntegerType2` is on. The suffix is the last character of the number token. | Lua has an ERROR node. Luau has one number and no error. `0b1010i` is the same Luau integer. | matches |
| `0x1.fp10` | Hex float. Lua 5.4.9 evaluates it to 1984. | The lexer token is `0x1`. `parseDouble` accepts `0x1` through `parseInteger`. The next character is `.`. `parsePrimaryExpr` applies `.` Name to a name or a parenthesized expression. `parseChunk` then requires end of file. The chunk is a syntax error. | Lua has one `number` and no error. Luau has an ERROR node. | matches |
| `0x.1p4` | Hex float. Lua 5.4.9 evaluates it to 1. | Malformed. The lexeme is `0x`. `parseDouble` rejects it. | Lua has one `number` and no error. Luau has an ERROR node. | matches |
| `"a\z\nb"` and `"a\z \n b"` | `\z` skips the following whitespace, including line breaks. | The same skip. `isSpace` includes space, tab, CR, LF, VT, and FF. | Both parsers accept the quoted string. No error. | matches |
| `"a\<newline>b"` | The backslash and the line break become one newline. | The lexer consumes the break inside the string. | Both parsers accept the quoted string. No error. | matches |
| `"a\zb"` and `"a\z   b"` on one line | Valid. | Valid. | One quoted string. No error. | matches |
| `"\q"` | Invalid escape. Lua 5.4.9 rejects it. | The letter is kept (`fixupQuotedString` default). | Lua has an ERROR node. Luau has one quoted string and no error. | matches |
| `"\xZZ"`, `"\u{}"`, `"\999"` | Invalid escape. | `fixupQuotedString` rejects a bad `\x`, a bad `\u{…}`, and a decimal escape above 255. | Both parsers have an ERROR node. | matches |
| Vertical tab or form feed between tokens | Both are spaces. [§3.1](https://www.lua.org/manual/5.4/manual.html#3.1). Lua 5.4.9 loads `1\v+\v2` and `1\f+\f2`. | `isSpace` includes both. | Both parsers accept `return 1` plus either character plus `+` plus the same character plus `2`. No error. | matches |

## Shared statement forms

`return` is a block tail in both languages. Lua also allows `break` as a
statement. Luau allows `break` and `continue` only as a block tail.

| Form | Lua 5.4 | Luau 0.725 | Packed parser | Status |
| --- | --- | --- | --- | --- |
| `function f() return 1 print(2) end` | `return` ends the block. [§9](https://www.lua.org/manual/5.4/manual.html#9) `retstat`. Lua 5.4.9 reports `'end' expected`. | `return` is a last statement. The next token must close the block. | Both parsers have an ERROR node. `function f() print(1) return 1 end` has no error. | matches |
| `{1; 2}`, `{a = 1;}` | `fieldsep` is `,` or `;`, and a trailing separator is allowed. | The same rule in `parseTableConstructor`. | Both parsers accept the table. No error. | matches |
| `while true do break print(1) end` | `break` is an ordinary statement in [§9](https://www.lua.org/manual/5.4/manual.html#9). Lua 5.4.9 loads this chunk. | `break` is a last statement. | Lua has no error. Luau has an ERROR node. `while true do break end` has no error in either parser. | matches |

## Lua 5.4 forms

| Form | Spec | Packed `lua` parser | Status |
| --- | --- | --- | --- |
| `local x <const> = 1`, `local f <close> = io.open(path)` | [§3.3.7](https://www.lua.org/manual/5.4/manual.html#3.3.7). Attribute is `'<' Name '>'`. Lua 5.4.9 loads both `<const>` and `<close>`. | No error. `local_attribute` is `<` Name `>`. | matches |
| A file whose first line is `#!/usr/bin/env lua` | `luaL_loadfile` ignores that first line. The standalone interpreter uses this. Lua 5.4.9 runs such a file. `load` of the same text rejects `#`. | No error when the chunk starts with `#!`. The `shebang` token ends before the newline. A `#!` after a newline is the length operator and the chunk has an error. | matches |

Luau 0.725 has no `<const>` attribute and no shebang skip. The packed
`luau` parser has an ERROR node for `local x <const> = 1` and for a chunk
that starts with `#!`. Luau spells constants as `const name = value`.

## Luau 0.725 statements

| Form | Spec in tag 0.725 | Packed `luau` parser | Status |
| --- | --- | --- | --- |
| `while true do continue print(1) end` | `continue` is a last statement, same as `break`. | ERROR node. | matches |
| `continue()` as a statement | `continue` is a name. A following `(` parses as a call (`parseStat` returns a call before it checks the name `continue`). | No error. The call is an identifier call. | matches |
| `local continue = 1`, `print(continue)`, `while true do continue end` | A name, an expression, and a last statement. | All three parse. No error. A line comment between `continue` and `end` stays a continue statement. | matches |
| `return if a then b elseif c then d else e` | `parseIfElseExpr` allows `elseif` and requires `else`. | No error. `return if a then b` has an ERROR node. | matches |
| `return 1 + if true then 2 else 3` and `return if true then 2 else 3 + 4` | The `if` expression is a simple expression. The `else` branch is a full expression. | The first is a binary `+` whose right side is the `if`. The second keeps `3 + 4` inside the `else` branch. | matches |
| `@native function f() end`, `@[deprecated] function f() end` | Attribute tokens are always parsed. Names in the default set include `native`, `checked`, and `deprecated`. | No error. The same list parses on `local function`, `export function`, and `const function`. `return @native function() end` has no error. `@native local x = 1` has an ERROR node. | matches |
| `const x = 1`, `const function f() end` | `LuauConst2`, on in the analyze/compile default. | No error. `const x` with no `=` has an ERROR node. | matches |
| `export function f() end`, `export local x = 1`, `export const x = 1` | `LuauExportValueSyntax`, on in the analyze/compile default. | No error. `export local function f() end` has an ERROR node. Tag 0.725 reports that form as an error. | matches |
| `export type T = number`, `export type T<U = string> = U` | Type alias, including a default on a type parameter. | No error. | matches |
| `function f<T>(x: T): T return x end`, `local function f<T>(...)`, `function<T>(...)` | `parseFunctionBody` reads a generic list before `(`. | No error. `function f<T, U...>(x)` has no error. `function f<U..., T>(x)` has a MISSING node. | matches |
| `for i: number = 1, 10 do end`, `for k, v: string in t do end` | `parseFor` uses `parseBinding` (`Name [':' Type]`). | No error. | matches |
| `` `hello {name}` `` | `parseInterpString`: begin, expression, mid, expression, end. | No error. The tree is `interp_begin`, an expression, `interp_end`. `` `a{`b`}c` `` has no error. | matches |
| `` `{{oops}}` ``, `` `{1+}` `` | `{{` is a parse error. A hole must be an expression. | Both have an ERROR node. `` `{}` `` has an ERROR node. | matches |
| `` print(`hi`) `` | A parenthesized argument may be any expression. | No error. | matches |
| `` print `hi` `` | Call arguments are `(…)`, a table, or a quoted or long string. | ERROR node. | matches |
| `x += 1`, `x //= 2`, `return 1 // 2`, `return 1 :: number` | Compound assignment includes `//=`. Floor division and `::` are unflagged. | No error. | matches |

## Luau 0.725 types

These aliases parse: `number | string`, `number & string`, `string?`,
`Foo<number, string>`, `typeof(1 + 2)`, `{ a: number, b: string }`,
`{ [number]: string }`, `{ ["a"]: number }`, `{string}`,
`(number) -> string`, `(number, string) -> boolean`,
`() -> number`, `"hi" | true | false | nil`.

| Form | Spec in tag 0.725 | Packed `luau` parser | Status |
| --- | --- | --- | --- |
| `type T = (number)`, `type F = (number) -> (string)` | Parentheses group a type (`AstTypeGroup`) when they are not a function type. | No error. `(number)` is a `grouped_type`. `(number) -> string` is a `function_type`. | matches |
| `type T = Foo.Bar` | `Name ['.' Name]`. | No error. | matches |
| `type T = { read a: number }`, `{ write a: number }` | `read` and `write` set the property access before the name when the next token is a name or `[`. | No error. `{read}` and `{read: number}` keep `read` as a name. | matches |
| `type F = (x: number) -> string` | Function types allow a name on each parameter. | No error. `type T = (x: number)` has an ERROR node. | matches |
| `function f(): (number, string)` | A return annotation may be a parenthesized type list. | No error. `function f(): (number)` is a grouped return. `function f(): ()` is the empty pack. `function f(): (number) -> string` is a function type. | matches |
| `type F = (...number) -> ()` | A parameter list may be a variadic type pack. | No error. | matches |
| `type T<U...> = () -> U...` | A generic list may contain a type pack. The pack form of `...` is a return or a parameter. | No error. `type T<U..., V> = V` has a MISSING node. `local x: U... = 1` has an ERROR node. | matches |
| `type T = \| number \| string` | A union or intersection may start with the operator. | No error. `type T = & number & string` has no error. | matches |
| `type function id(x) return x end` | `parseTypeAlias` dispatches to `parseTypeFunction` when the next token is `function`. The body is a function body. | No error. | matches |

## Luau forms taken from Lua 5.4

Lua 5.4 accepts these forms. Luau 0.725 rejects them. The packed `lua`
parser accepts them. The packed `luau` parser rejects them.

| Form | Luau 0.725 | Status |
| --- | --- | --- |
| `goto L` and `::L::` | No `goto` statement and no label statement. The packed `luau` parser has an ERROR node. The packed `lua` parser accepts `goto L` and `::L::`. | matches |
| `1 & 2`, `1 \| 2`, `1 ~ 2`, `1 << 2`, `1 >> 2` | `parseBinaryOp` has no bitwise operators. The packed `luau` parser has an ERROR node. `1 ~= 2` has no error. | matches |
| `~1` | Unary operators are `not`, `-`, and `#`. The packed `luau` parser has an ERROR node. | matches |
| `0x1.fp10` | See the lexical table. The packed `luau` parser has an ERROR node. | matches |

## Later Luau

These forms are outside the `0.725.0` claim.

| Form | Where it appears |
| --- | --- |
| `if local` expressions | Experimental in Luau 0.739 (2026-09-18). |
| Exact table types | Experimental in Luau 0.740 (2026-09-25). |
| `class` declarations | The 0.725 parser contains `parseClassStat` behind `DebugLuauUserDefinedClasses`. `setLuauFlagsDefault()` leaves that flag off. |
| `declare` global, function, and extern type | Parsed only when `ParseOptions.allowDeclarationSyntax` is set. Ordinary analyze/compile input leaves it off. |

## Outside this tracker

The parsers report syntax shape. These are outside the rows above:

- Type checking, name resolution, and lint results.
- Whether `goto` enters the scope of a local.
- Whether `continue` inside `repeat` / `until` is rejected after parsing. Tag 0.725 parses it when the statement is inside a loop. `repeat continue until false` has no error here.
- Which attribute arguments are legal after the attribute list parses.
- The value of a long string. The long-bracket scanner is present. Skipping the first newline is a string value, and this tracker counts syntax errors.
- `load` of a shebang string. The Lua manual rejects `#` inside `load`. The packed `lua` parser accepts a `#!` line at the start of a chunk, which is the file path used by `luaL_loadfile`.

## Semantic queries

twigz implements the semantic query API.

`crates/query` compiles a query in `QueryView::Semantic` or
`QueryView::Concrete`. The semantic view rewrites vocabulary kinds and
roles onto the concrete node names of one language. Unknown kinds fail
with `unknown_semantic_symbol`. An empty rewrite becomes
`CompiledQuery::Never`. Allowed predicates are `#eq?`, `#not-eq?`,
`#any-of?`, and `#match?`. The limits are 64 patterns and 8192 rendered
bytes. Tests are `//crates/query:query_test`. The CLI is
`//tools/query:twigz-query`. Its default view is semantic.

A Luau query `(function name: (identifier) @n)` rewrites to
`function_declaration`, `local_function_declaration`, and
`const_function_declaration`.

The agent-os syntax service does not call that compiler. `compile_query`
with `view = "semantic"` returns `semantic_query_unavailable`. Concrete
queries in that service go to Tree-sitter directly.

use twigz_generate::compile_sources;
use twigz_ir::{GrammarIr, Rule, ScanRule};
use twigz_scan::{emit_c, GeneratedScanner, MachineKind};

fn compile(root: &str, name: &str, modules: Vec<(String, String, String)>) -> GrammarIr {
    compile_sources(root, name, modules).unwrap().ir
}

fn compile_lua_core() -> GrammarIr {
    let family = include_str!("../../grammars/families/lua-core.grammar");
    let root = include_str!("../../grammars/lua.grammar");
    compile(
        root,
        "lua.grammar",
        vec![("lua.core".into(), family.into(), "lua-core.grammar".into())],
    )
}

fn compile_js() -> GrammarIr {
    compile(
        include_str!("../../grammars/javascript.grammar"),
        "javascript.grammar",
        Vec::new(),
    )
}

fn compile_python() -> GrammarIr {
    compile(
        include_str!("../../grammars/python.grammar"),
        "python.grammar",
        Vec::new(),
    )
}

#[test]
fn lua_externals_follow_declaration_order() {
    let ir = compile_lua_core();
    let scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    assert_eq!(
        scanner.externals,
        [
            "long_string_start",
            "long_string_content",
            "long_string_end",
            "long_comment",
            "quoted_string",
            "shebang",
        ]
    );
    match scanner.kind {
        MachineKind::LongBracket { lexical, .. } => {
            assert_eq!(lexical.as_deref(), Some("lua"));
        }
        other => panic!("lua scanner {other:?}"),
    }
}

#[test]
fn emit_c_contains_five_symbols() {
    let ir = compile_lua_core();
    let c = emit_c(&ir).unwrap();
    for name in [
        "tree_sitter_lua_external_scanner_create",
        "tree_sitter_lua_external_scanner_destroy",
        "tree_sitter_lua_external_scanner_scan",
        "tree_sitter_lua_external_scanner_serialize",
        "tree_sitter_lua_external_scanner_deserialize",
    ] {
        assert!(c.contains(name), "{name}");
    }
    assert!(c.contains("long_string_start"));
    assert!(
        !c.contains("lookahead != before"),
        "C scanner must not treat repeated bytes as a stalled advance:\n{c}"
    );
}

#[test]
fn non_long_bracket_pattern_is_rejected() {
    let ir = compile(
        r#"
grammar demo "1"
start root
external foo
scan foo = "abc"
root = foo
"#,
        "demo.grammar",
        Vec::new(),
    );
    let err = GeneratedScanner::from_grammar(&ir).unwrap_err();
    assert!(err.contains("long-bracket"), "{err}");
}

#[test]
fn slash_defers_line_and_block_comments_in_emitted_c() {
    let ir = compile_js();
    let c = emit_c(&ir).unwrap();
    assert!(
        c.contains("lookahead == '/' || lexer->lookahead == '*'"),
        "{c}"
    );
}

#[test]
fn js_machine_is_slash_template() {
    let ir = compile_js();
    let scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    assert!(matches!(scanner.kind, MachineKind::SlashTemplate { .. }));
    assert_eq!(
        scanner.externals,
        [
            "template_head",
            "template_middle",
            "template_tail",
            "regex",
            "division"
        ]
    );
}

#[test]
fn python_machine_is_offside() {
    let ir = compile_python();
    let scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    assert!(matches!(scanner.kind, MachineKind::Offside { .. }));
}

const LONG_BRACKET: &str = r#"
external long_string_start | long_string_content | long_string_end | long_comment
scan long_string_start = "[" pad:"="* "["
  keep pad
scan long_string_end = "]" "="{pad} "]"
scan long_string_content = (!long_string_end .)+
scan long_comment = "--" "[" pad:"="* "[" (!("]" "="{pad} "]") .)* ("]" "="{pad} "]")?
"#;

#[test]
fn lexical_without_long_bracket_is_rejected() {
    let err = match compile_sources(
        r#"
grammar demo "1"
start root
external quoted_string | shebang
scan lexical lua
root = quoted_string
"#,
        "demo.grammar",
        Vec::new(),
    ) {
        Err(err) => err,
        Ok(_) => panic!("scan lexical without the long-bracket machine must fail"),
    };
    assert!(err.contains("long-bracket"), "{err}");

    let mut ir = GrammarIr::default();
    ir.name = "demo".into();
    ir.scans
        .push(ScanRule::Lexical { language: "lua".into() });
    ir.externals.push(Rule::Symbol("quoted_string".into()));
    ir.externals.push(Rule::Symbol("shebang".into()));
    let err = GeneratedScanner::from_grammar(&ir).unwrap_err();
    assert!(err.contains("long-bracket"), "{err}");
}

fn lexical_source(tail: &str) -> String {
    let mut source = String::from("grammar demo \"1\"\nstart root\n");
    source.push_str(LONG_BRACKET);
    source.push_str(tail);
    source
}

#[test]
fn lexical_long_bracket_is_accepted_for_lua_and_luau() {
    let lua = lexical_source(
        "external quoted_string | shebang\nscan lexical lua\nmap quoted_string => string\nroot = quoted_string\n",
    );
    let lua_ir = compile(&lua, "demo.grammar", Vec::new());
    let lua_c = emit_c(&lua_ir).unwrap();
    assert!(lua_c.contains("is_at_included_range_start"), "{lua_c}");
    assert!(lua_c.contains("lookahead == '['"), "{lua_c}");
    assert!(!lua_c.contains("strtoll"), "{lua_c}");

    let luau = lexical_source(
        "external quoted_string | number | interp_simple | interp_begin | interp_mid | interp_end | continue_keyword | type_keyword | export_keyword | const_keyword | read_keyword | write_keyword\nscan lexical luau\nmap quoted_string => string\nmap number => literal\nroot = quoted_string\n",
    );
    let luau_ir = compile(&luau, "demo.grammar", Vec::new());
    let luau_c = emit_c(&luau_ir).unwrap();
    assert!(luau_c.contains("strtoll"), "{luau_c}");
    assert!(luau_c.contains("depth"), "{luau_c}");
    assert!(!luau_c.contains("shebang"), "{luau_c}");
}

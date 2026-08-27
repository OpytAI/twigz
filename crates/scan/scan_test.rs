use twigz_generate::compile_sources;
use twigz_ir::GrammarIr;
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
            "long_comment"
        ]
    );
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

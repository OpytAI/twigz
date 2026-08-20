use twigz_generate::compile_sources;
use twigz_ir::GrammarIr;
use twigz_scan::{emit_c, GeneratedScanner, MachineKind, MockLexer, Scanner, SERIALIZE_CAP};

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

fn compile_twiglet() -> GrammarIr {
    compile(
        include_str!("../../grammars/fixtures/twiglet.grammar"),
        "twiglet.grammar",
        Vec::new(),
    )
}

#[test]
fn serialize_cap_is_1024() {
    assert_eq!(SERIALIZE_CAP, 1024);
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
fn lua_long_bracket_serialize_is_two_bytes() {
    let ir = compile_lua_core();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut lexer = MockLexer::new("[=[hi]=]");
    let valid = [true, false, false, false];
    assert!(scanner.scan(&mut lexer, &valid));
    let mut buf = [0_u8; SERIALIZE_CAP];
    let n = scanner.serialize_into(&mut buf);
    assert_eq!(n, 2);
    assert_eq!(buf[0], 1);
    assert_eq!(buf[1], 1);
    let mut restored = GeneratedScanner::from_grammar(&ir).unwrap();
    restored.deserialize_from(&buf[..n]);
    let mut buf2 = [0_u8; 2];
    assert_eq!(restored.serialize_into(&mut buf2), 2);
    assert_eq!(buf2, [1, 1]);
}

#[test]
fn unterminated_long_comment_still_emits() {
    let ir = compile_lua_core();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut lexer = MockLexer::new("--[[ unfinished");
    let valid = [false, false, false, true];
    assert!(scanner.scan(&mut lexer, &valid));
}

#[test]
fn all_false_valid_returns_false() {
    let ir = compile_lua_core();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut lexer = MockLexer::new("[=[hi]=]");
    assert!(!scanner.scan(&mut lexer, &[false, false, false, false]));
}

#[test]
fn valid_false_at_start_skips_long_bracket() {
    let ir = compile_lua_core();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut lexer = MockLexer::new("[=[hi]=]");
    assert!(!scanner.scan(&mut lexer, &[false, true, true, true]));
}

#[test]
fn serialize_respects_cap() {
    let ir = compile_lua_core();
    let scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut buf = [0_u8; 1];
    assert_eq!(scanner.serialize_into(&mut buf), 0);
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
fn twiglet_short_buffer_resets() {
    let ir = compile_twiglet();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    scanner.deserialize_from(&[]);
    let mut buf = [0_u8; 35];
    let n = scanner.serialize_into(&mut buf);
    assert!(n <= SERIALIZE_CAP);
    assert_eq!(buf[0], 0);
    assert_eq!(buf[1], 1);
}

#[test]
fn offside_deserialize_caps_stack() {
    let ir = compile_python();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut buf = vec![0_u8, 200];
    buf.extend(std::iter::repeat(1).take(200));
    scanner.deserialize_from(&buf);
    let mut out = [0_u8; SERIALIZE_CAP];
    let n = scanner.serialize_into(&mut out);
    assert!(n <= 2 + 32, "{n}");
}

#[test]
fn indent_emits_on_deeper_line() {
    let ir = compile_python();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut lexer = MockLexer::new("\n    x");
    let valid = [false, true, false];
    assert!(scanner.scan(&mut lexer, &valid));
}

#[test]
fn indent_skipped_when_valid_false() {
    let ir = compile_python();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut lexer = MockLexer::new("\n    x");
    assert!(!scanner.scan(&mut lexer, &[true, false, false]));
}

#[test]
fn slash_division_after_value_prefix() {
    let ir = compile_js();
    let scanner = GeneratedScanner::from_grammar(&ir).unwrap();
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
    let mut scanner = scanner;
    let mut lexer = MockLexer::new("/ 2");
    let mut valid = vec![false; 5];
    valid[4] = true;
    assert!(scanner.scan(&mut lexer, &valid));
    assert_eq!(lexer.at, 1);
}

#[test]
fn slash_regex_when_division_not_valid() {
    let ir = compile_js();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut lexer = MockLexer::new("/ab+/");
    let mut valid = vec![false; 5];
    valid[3] = true;
    assert!(scanner.scan(&mut lexer, &valid));
}

#[test]
fn slash_defers_line_and_block_comments() {
    let ir = compile_js();
    for source in ["// comment", "/* comment */"] {
        let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
        let mut lexer = MockLexer::new(source);
        let mut valid = vec![false; 5];
        valid[3] = true;
        valid[4] = true;
        assert!(
            !scanner.scan(&mut lexer, &valid),
            "scanner ate comment {source}"
        );
    }
    let c = emit_c(&ir).unwrap();
    assert!(
        c.contains("lookahead == '/' || lexer->lookahead == '*'"),
        "{c}"
    );
}

#[test]
fn template_head_scans_backtick() {
    let ir = compile_js();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let mut lexer = MockLexer::new("`hi`");
    let mut valid = vec![false; 5];
    valid[0] = true;
    assert!(scanner.scan(&mut lexer, &valid));
}

#[test]
fn termination_advances_at_most_len_plus_one() {
    let ir = compile_lua_core();
    let mut scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    let source = "[=[hello]=]";
    let mut lexer = MockLexer::new(source);
    assert!(scanner.scan(&mut lexer, &[true, true, true, true]));
    assert!(lexer.at <= source.len() + 1);
}

#[test]
fn js_machine_is_slash_template() {
    let ir = compile_js();
    let scanner = GeneratedScanner::from_grammar(&ir).unwrap();
    assert!(matches!(scanner.kind, MachineKind::SlashTemplate { .. }));
}

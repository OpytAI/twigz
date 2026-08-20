use std::path::{Path, PathBuf};
use twigz_runtime::{
    javascript_lang, lua_lang, luau_lang, python_lang, twiglet_lang, Language, Node, Parser,
    TSInputEdit, TSPoint, Tree,
};
use twigz_vocab::Kind;

fn byte_of(source: &str, needle: &str) -> u32 {
    source.find(needle).expect(needle) as u32
}

fn collect_concrete(tree: &Tree, node: Node, out: &mut Vec<(String, String)>) {
    out.push((
        tree.concrete_kind(node).to_string(),
        tree.text(node).to_string(),
    ));
    for child in tree.raw_children(node) {
        collect_concrete(tree, child.node, out);
    }
}

fn parse(lang: Language, source: &str) -> Tree {
    Parser::new(lang).unwrap().parse_str(source).unwrap()
}

fn find_root() -> PathBuf {
    let mut roots = Vec::new();
    if let Ok(dir) = std::env::var("RUNFILES_DIR") {
        roots.push(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("TEST_SRCDIR") {
        roots.push(PathBuf::from(&dir));
        roots.push(PathBuf::from(dir).join("_main"));
    }
    if let Ok(dir) = std::env::current_dir() {
        roots.push(dir);
    }
    if let Ok(dir) = std::env::var("BUILD_WORKSPACE_DIRECTORY") {
        roots.push(PathBuf::from(dir));
    }
    for root in roots {
        for candidate in [
            root.clone(),
            root.join("data/fixtures/source"),
            root.join("twigz-extract/data/fixtures/source"),
        ] {
            if candidate.join("lua").exists() || candidate.join("twiglet").exists() {
                if candidate.ends_with("source") {
                    return candidate.parent().unwrap().parent().unwrap().to_path_buf();
                }
            }
            if candidate.join("data/fixtures/source/lua").exists() {
                return candidate;
            }
        }
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join("data/fixtures/source/lua").exists() {
                    return path;
                }
            }
        }
    }
    panic!("cannot locate fixtures")
}

fn read_dir_files(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    out
}

#[test]
fn lua_finds_functions_and_strings() {
    let source = include_str!("../../data/fixtures/source/lua/locals.lua");
    let tree = parse(lua_lang(), source);
    let functions: Vec<_> = tree.find(Kind::FUNCTION).collect();
    assert!(functions.iter().any(|n| tree.text(*n).contains("greet")));
    assert!(tree.find(Kind::STRING).next().is_some());
    assert!(tree.find(Kind::COMMENT).next().is_some());
    assert!(tree.find(Kind::IMPORT).next().is_none());
    let x = byte_of(source, "x");
    let binding = tree.binding_at(x).expect("local x");
    assert_eq!(tree.kind(binding), Some(Kind::DECLARATION));
    assert_eq!(tree.concrete_kind(binding), "local_name");
    let greet = byte_of(source, "greet");
    let greet_binding = tree.binding_at(greet).expect("local function greet");
    assert_eq!(tree.kind(greet_binding), Some(Kind::FUNCTION));
    let i = byte_of(source, "for i") + 4;
    let loop_binding = tree.binding_at(i).expect("for i");
    assert_eq!(tree.kind(loop_binding), Some(Kind::DECLARATION));
    assert_eq!(tree.concrete_kind(loop_binding), "for_numeric_statement");
    let anon = byte_of(source, "function()");
    assert!(tree.binding_at(anon).is_none());
}

#[test]
fn lua_parses_long_brackets_through_packed_scanner() {
    let source = include_str!("../../data/fixtures/source/lua/long_brackets.lua");
    let tree = parse(lua_lang(), source);
    let strings: Vec<_> = tree
        .find(Kind::STRING)
        .map(|node| tree.text(node).to_string())
        .collect();
    assert!(
        strings.iter().any(|text| text.contains("hello")),
        "{strings:?}"
    );
    assert!(
        strings.iter().any(|text| text.contains("more")),
        "{strings:?}"
    );
    let comments: Vec<_> = tree
        .find(Kind::COMMENT)
        .map(|node| tree.text(node).to_string())
        .collect();
    assert!(
        comments
            .iter()
            .any(|text| text.contains("[[") || text.contains("comment")),
        "{comments:?}"
    );
    assert!(
        strings.iter().any(|text| text.contains("unfinished")),
        "unfinished long string missing: {strings:?}"
    );
}

#[test]
fn lua_unfinished_long_comment_is_comment() {
    let source = include_str!("../../data/fixtures/source/lua/unfinished_comment.lua");
    let tree = parse(lua_lang(), source);
    let comments: Vec<_> = tree
        .find(Kind::COMMENT)
        .map(|node| tree.text(node).to_string())
        .collect();
    assert!(
        comments.iter().any(|text| text.contains("unfinished")),
        "{comments:?}"
    );
}

#[test]
fn luau_type_is_declaration() {
    let source = include_str!("../../data/fixtures/source/luau/types.luau");
    let tree = parse(luau_lang(), source);
    let t = byte_of(source, "T");
    assert!(tree.binding_at(t).is_some());
    assert!(tree
        .find(Kind::FUNCTION)
        .any(|n| tree.text(n).contains("greet")));
    let i = byte_of(source, "for i") + 4;
    let loop_binding = tree.binding_at(i).expect("for i");
    assert_eq!(tree.kind(loop_binding), Some(Kind::DECLARATION));
    assert!(tree.find(Kind::STRING).any(|n| tree.text(n).contains("hi")));
    assert!(tree
        .find(Kind::COMMENT)
        .any(|n| tree.text(n).contains("comment")));
}

#[test]
fn twiglet_walks_all_fixtures() {
    let root = find_root().join("data/fixtures/source/twiglet");
    let files = read_dir_files(&root);
    assert_eq!(
        files
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [
            "fn_greet.twiglet",
            "import.twiglet",
            "interp.twiglet",
            "two_defs.twiglet"
        ]
    );
    for (name, source) in &files {
        let tree = parse(twiglet_lang(), source);
        match name.as_str() {
            "fn_greet.twiglet" => {
                assert_eq!(tree.find(Kind::FUNCTION).count(), 1);
                let greet = byte_of(source, "greet");
                let binding = tree.binding_at(greet).expect("fn greet");
                assert_eq!(tree.kind(binding), Some(Kind::FUNCTION));
                assert!(tree.find(Kind::CLASS).next().is_none());
                assert!(tree.find(Kind::STRING).next().is_none());
                assert!(tree
                    .find(Kind::COMMENT)
                    .any(|n| tree.text(n).contains("comment")));
            }
            "two_defs.twiglet" => {
                let names: Vec<_> = tree
                    .find(Kind::FUNCTION)
                    .map(|n| tree.text(n).to_string())
                    .collect();
                assert!(names.iter().any(|n| n.contains("greet")));
                assert!(names.iter().any(|n| n.contains("other")));
                let greet = tree
                    .find(Kind::FUNCTION)
                    .find(|n| tree.text(*n).contains("greet"))
                    .unwrap();
                let assigns = tree.find_in(greet, Kind::ASSIGNMENT).count();
                assert_eq!(assigns, 2, "two assign children");
            }
            "import.twiglet" => {
                let imports: Vec<_> = tree
                    .find(Kind::IMPORT)
                    .map(|n| tree.text(n).to_string())
                    .collect();
                assert_eq!(imports.len(), 1);
                assert!(
                    imports.iter().any(|text| text.contains("mod")),
                    "{imports:?}"
                );
                assert!(tree.find(Kind::STRING).next().is_some());
            }
            "interp.twiglet" => {
                let names: Vec<_> = tree
                    .find(Kind::FUNCTION)
                    .map(|n| tree.text(n).to_string())
                    .collect();
                assert!(
                    names.iter().any(|n| n.contains("main")),
                    "interp functions={names:?}"
                );
                assert!(tree.find(Kind::STRING).next().is_some());
            }
            other => panic!("unexpected twiglet fixture {other}"),
        }
    }
}

#[test]
fn javascript_class_import_function() {
    let source = include_str!("../../data/fixtures/source/javascript/module.js");
    let tree = parse(javascript_lang(), source);
    assert!(tree.find(Kind::FUNCTION).next().is_some());
    assert!(tree.find(Kind::CLASS).next().is_some());
    let imports: Vec<_> = tree
        .find(Kind::IMPORT)
        .map(|n| tree.text(n).to_string())
        .collect();
    assert_eq!(imports.len(), 2, "{imports:?}");
    assert!(
        imports.iter().any(|text| text.contains("mod")),
        "{imports:?}"
    );
    assert!(
        imports.iter().any(|text| text.contains("other")),
        "{imports:?}"
    );
    assert!(tree.find(Kind::STRING).any(|n| tree.text(n) == "`hi`"));
    assert!(tree
        .find(Kind::STRING)
        .any(|n| tree.text(n).contains("${name}")));
    assert!(tree.find(Kind::LITERAL).next().is_some());
    assert!(tree
        .find(Kind::COMMENT)
        .any(|n| tree.text(n).contains("comment")));
    let greet = byte_of(source, "greet");
    assert_eq!(
        tree.kind(tree.binding_at(greet).expect("function greet")),
        Some(Kind::FUNCTION)
    );
    let class_name = byte_of(source, "class C") + 6;
    assert_eq!(
        tree.kind(tree.binding_at(class_name).expect("class C")),
        Some(Kind::CLASS)
    );
    let import_name = byte_of(source, "import x") + 7;
    assert_eq!(
        tree.kind(tree.binding_at(import_name).expect("import x")),
        Some(Kind::IMPORT)
    );
    assert!(tree
        .find(Kind::FUNCTION)
        .any(|n| tree.text(n).contains("f = ()")));
    assert!(tree
        .find(Kind::FUNCTION)
        .any(|n| tree.text(n).contains("method")));
    let mut nodes = Vec::new();
    collect_concrete(&tree, tree.root(), &mut nodes);
    assert!(
        nodes
            .iter()
            .any(|(kind, text)| kind == "division" && text.contains('/')),
        "missing division token: {nodes:?}"
    );
    assert!(
        nodes
            .iter()
            .any(|(kind, text)| kind == "regex" && text.contains("ab+")),
        "missing regex: {nodes:?}"
    );
    assert!(
        !nodes
            .iter()
            .any(|(kind, text)| kind == "regex" && text.contains("10")),
        "10 / 2 scanned as regex: {nodes:?}"
    );
    assert!(tree
        .find(Kind::OPERATOR)
        .any(|n| tree.text(n).contains("10") && tree.text(n).contains('/')));
}

#[test]
fn python_def_class_import() {
    let source = include_str!("../../data/fixtures/source/python/module.py");
    let tree = parse(python_lang(), source);
    assert!(
        tree.find(Kind::FUNCTION).count() >= 2,
        "functions={}",
        tree.find(Kind::FUNCTION).count()
    );
    if tree.find(Kind::CLASS).next().is_none() {
        panic!("no class nodes; text={:?}", tree.text(tree.root()));
    }
    assert!(tree.find(Kind::IMPORT).next().is_some());
    assert!(tree.find(Kind::STRING).next().is_some());
    let greet = tree
        .find(Kind::FUNCTION)
        .find(|n| tree.text(*n).contains("greet"))
        .expect("def greet");
    assert_eq!(tree.find_in(greet, Kind::ASSIGNMENT).count(), 2);
    assert!(tree
        .find(Kind::FUNCTION)
        .any(|n| tree.text(n).contains("method")));
    let greet_name = byte_of(source, "greet");
    assert_eq!(
        tree.kind(tree.binding_at(greet_name).expect("def greet")),
        Some(Kind::FUNCTION)
    );
    let class_name = byte_of(source, "class C") + 6;
    assert_eq!(
        tree.kind(tree.binding_at(class_name).expect("class C")),
        Some(Kind::CLASS)
    );
    let import_name = byte_of(source, "import os") + 7;
    assert_eq!(
        tree.kind(tree.binding_at(import_name).expect("import os")),
        Some(Kind::IMPORT)
    );
}

#[test]
fn incremental_edit_keeps_names() {
    let source = "local x = 1\n";
    let mut parser = Parser::new(lua_lang()).unwrap();
    let mut tree = parser.parse_str(source).unwrap();
    tree.edit(TSInputEdit {
        start_byte: 6,
        old_end_byte: 7,
        new_end_byte: 8,
        start_point: TSPoint { row: 0, column: 6 },
        old_end_point: TSPoint { row: 0, column: 7 },
        new_end_point: TSPoint { row: 0, column: 8 },
    });
    let tree = parser.parse("local xy = 1\n", Some(&tree)).unwrap();
    let names: Vec<_> = tree
        .declarations()
        .map(|n| tree.text(n).to_string())
        .collect();
    assert!(names.iter().any(|name| name.contains("xy")), "{names:?}");
}

#[test]
fn incremental_edit_inside_long_string() {
    let source = "local s = [=[hello]=]\n";
    let edited = "local s = [=[hallo]=]\n";
    let mut parser = Parser::new(lua_lang()).unwrap();
    let mut tree = parser.parse_str(source).unwrap();
    tree.edit(TSInputEdit {
        start_byte: 14,
        old_end_byte: 15,
        new_end_byte: 15,
        start_point: TSPoint { row: 0, column: 14 },
        old_end_point: TSPoint { row: 0, column: 15 },
        new_end_point: TSPoint { row: 0, column: 15 },
    });
    let tree = parser.parse(edited, Some(&tree)).unwrap();
    let strings: Vec<_> = tree
        .find(Kind::STRING)
        .map(|n| tree.text(n).to_string())
        .collect();
    assert!(
        strings.iter().any(|text| text.contains("hallo")),
        "{strings:?}"
    );
}

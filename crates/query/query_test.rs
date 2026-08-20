use std::path::{Path, PathBuf};
use twigz_query::{compile_query, matches, pattern_count, rendered_source, QueryView};
use twigz_runtime::{
    javascript_lang, lua_lang, luau_lang, python_lang, twiglet_lang, Language, Parser,
};
use twigz_vocab::Kind;

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
    for root in roots {
        if root.join("data/goldens/queries").exists() {
            return root;
        }
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join("data/goldens/queries").exists() {
                    return path;
                }
            }
        }
    }
    panic!("cannot locate data/goldens")
}

fn json_stems(dir: &Path) -> Vec<String> {
    let mut stems = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            stems.push(path.file_stem().unwrap().to_string_lossy().into_owned());
        }
    }
    stems.sort();
    stems
}

fn languages() -> [(Language, &'static str); 5] {
    [
        (lua_lang(), "lua"),
        (luau_lang(), "luau"),
        (javascript_lang(), "javascript"),
        (python_lang(), "python"),
        (twiglet_lang(), "twiglet"),
    ]
}

#[test]
fn lua_function_name_rewrites_to_two_patterns() {
    let lang = lua_lang();
    let compiled = compile_query(
        &lang,
        "(function name: (identifier) @n)",
        QueryView::Semantic,
    )
    .expect("named function query");
    let src = rendered_source(&compiled).unwrap();
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("../../data/goldens/queries/lua_function.json")).unwrap();
    assert_eq!(
        pattern_count(&compiled) as u64,
        golden["patterns"].as_u64().unwrap()
    );
    assert_eq!(src, golden["rendered"].as_str().unwrap());
    assert!(!src.contains("function_expression"), "{src}");
    let heads: Vec<&str> = src
        .lines()
        .filter_map(|line| line.trim().strip_prefix('(')?.split_whitespace().next())
        .collect();
    let expected_heads: Vec<&str> = golden["heads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(heads, expected_heads);
}

#[test]
fn query_goldens_stems_are_exact() {
    let dir = find_root().join("data/goldens/queries");
    assert_eq!(json_stems(&dir), ["lua_function"]);
}

#[test]
fn lua_import_is_never() {
    let compiled =
        compile_query(&lua_lang(), "(import source: (_) @s)", QueryView::Semantic).unwrap();
    assert!(matches!(compiled, twigz_query::CompiledQuery::Never));
}

#[test]
fn luau_import_is_never() {
    let compiled =
        compile_query(&luau_lang(), "(import source: (_) @s)", QueryView::Semantic).unwrap();
    assert!(matches!(compiled, twigz_query::CompiledQuery::Never));
}

#[test]
fn twiglet_js_python_rewrite_import() {
    for lang in [twiglet_lang(), python_lang(), javascript_lang()] {
        let compiled =
            compile_query(&lang, "(import source: (_) @s)", QueryView::Semantic).unwrap();
        assert!(
            !matches!(compiled, twigz_query::CompiledQuery::Never),
            "{}",
            lang.name
        );
    }
}

#[test]
fn concrete_lua_name_fails_in_semantic_view() {
    let err = compile_query(
        &lua_lang(),
        "(local_function_declaration)",
        QueryView::Semantic,
    )
    .unwrap_err();
    assert_eq!(err.code, "unknown_semantic_symbol");
}

#[test]
fn unknown_predicate_is_rejected() {
    let err =
        compile_query(&lua_lang(), "(#lua-match? @n \"x\")", QueryView::Semantic).unwrap_err();
    assert_eq!(err.code, "unknown_semantic_symbol");
}

#[test]
fn listed_predicates_compile() {
    let lang = lua_lang();
    for source in [
        "(function name: (identifier) @n (#eq? @n \"greet\"))",
        "(function name: (identifier) @n (#not-eq? @n \"nope\"))",
        "(function name: (identifier) @n (#any-of? @n \"greet\" \"other\"))",
        "(function name: (identifier) @n (#match? @n \"^g\"))",
    ] {
        let compiled = compile_query(&lang, source, QueryView::Semantic).unwrap_or_else(|err| {
            panic!("{source}: {err}");
        });
        let rendered = rendered_source(&compiled).unwrap();
        if source.contains("#eq?") {
            assert!(rendered.contains("#eq?"), "{rendered}");
        }
        if source.contains("#not-eq?") {
            assert!(rendered.contains("#not-eq?"), "{rendered}");
        }
        if source.contains("#any-of?") {
            assert!(rendered.contains("#any-of?"), "{rendered}");
        }
        if source.contains("#match?") {
            assert!(rendered.contains("#match?"), "{rendered}");
        }
    }
}

fn lua_named_functions() -> (twigz_runtime::Language, twigz_runtime::Tree) {
    let lang = lua_lang();
    let source = include_str!("../../data/fixtures/source/lua/locals.lua");
    let tree = Parser::new(lang.clone())
        .unwrap()
        .parse_str(source)
        .unwrap();
    (lang, tree)
}

fn hit_texts(tree: &twigz_runtime::Tree, source: &str) -> Vec<String> {
    let compiled = compile_query(&lua_lang(), source, QueryView::Semantic).unwrap();
    matches(tree, &compiled, tree.root())
        .iter()
        .map(|node| tree.text(*node).to_string())
        .collect()
}

#[test]
fn eq_predicate_filters_function_names() {
    let (lang, tree) = lua_named_functions();
    let greet = compile_query(
        &lang,
        "(function name: (identifier) @n (#eq? @n \"greet\"))",
        QueryView::Semantic,
    )
    .unwrap();
    let hits = matches(&tree, &greet, tree.root());
    assert_eq!(
        hits.len(),
        1,
        "{:?}",
        hits.iter().map(|node| tree.text(*node)).collect::<Vec<_>>()
    );
    assert_eq!(tree.text(hits[0]), "greet");
    let nope = compile_query(
        &lang,
        "(function name: (identifier) @n (#eq? @n \"nope\"))",
        QueryView::Semantic,
    )
    .unwrap();
    assert!(matches(&tree, &nope, tree.root()).is_empty());
    assert_eq!(
        hit_texts(
            &tree,
            "(function name: (identifier) @n (#not-eq? @n \"nope\"))"
        ),
        ["greet"]
    );
    assert_eq!(
        hit_texts(
            &tree,
            "(function name: (identifier) @n (#any-of? @n \"greet\" \"other\"))"
        ),
        ["greet"]
    );
    assert_eq!(
        hit_texts(
            &tree,
            "(function name: (identifier) @n (#match? @n \"^g\"))"
        ),
        ["greet"]
    );
    assert!(hit_texts(
        &tree,
        "(function name: (identifier) @n (#match? @n \"^z\"))"
    )
    .is_empty());
}

#[test]
fn luau_named_function_rewrites_to_two_patterns() {
    let compiled = compile_query(
        &luau_lang(),
        "(function name: (identifier) @n)",
        QueryView::Semantic,
    )
    .unwrap();
    assert_eq!(pattern_count(&compiled), 2);
}

#[test]
fn forbidden_query_constructs_are_rejected() {
    let lang = lua_lang();
    let counted = compile_query(&lang, "(function){1,2}", QueryView::Semantic).unwrap_err();
    assert_eq!(counted.code, "query_syntax");
    let set = compile_query(
        &lang,
        "(function name: (identifier) @n (#set! foo bar))",
        QueryView::Semantic,
    )
    .unwrap_err();
    assert_eq!(set.code, "unknown_semantic_symbol");
    let is = compile_query(
        &lang,
        "(function name: (identifier) @n (#is? @n local))",
        QueryView::Semantic,
    )
    .unwrap_err();
    assert_eq!(is.code, "unknown_semantic_symbol");
    let derives = compile_query(
        &lang,
        "(function derives: (identifier))",
        QueryView::Semantic,
    )
    .unwrap_err();
    assert_eq!(derives.code, "unknown_semantic_symbol");
}

#[test]
fn adjacency_compiles() {
    let compiled = compile_query(
        &lua_lang(),
        "(function name: (identifier) . (_))",
        QueryView::Semantic,
    )
    .unwrap();
    assert!(!matches!(compiled, twigz_query::CompiledQuery::Never));
    let rendered = rendered_source(&compiled).unwrap();
    assert!(
        rendered.contains(". (") || rendered.contains(".(_"),
        "{rendered}"
    );
}

#[test]
fn wildcard_head_compiles() {
    compile_query(&lua_lang(), "(_)", QueryView::Semantic).unwrap();
}

#[test]
fn over_cap_errors() {
    let too_many = "(module)\n".repeat(65);
    let err = compile_query(&lua_lang(), &too_many, QueryView::Semantic).unwrap_err();
    assert_eq!(err.code, "query_too_complex");
    let too_big = format!("(module {})", "(_) ".repeat(2500));
    let err = compile_query(&lua_lang(), &too_big, QueryView::Semantic).unwrap_err();
    assert_eq!(err.code, "query_too_complex");
    let mut limited = lua_lang();
    limited.max_query = Some(4);
    let err = compile_query(&limited, "(function)", QueryView::Semantic).unwrap_err();
    assert_eq!(err.code, "query_too_complex");
}

#[test]
fn function_and_import_queries_on_all_languages() {
    let sources = [
        (
            "lua",
            include_str!("../../data/fixtures/source/lua/locals.lua"),
        ),
        (
            "luau",
            include_str!("../../data/fixtures/source/luau/types.luau"),
        ),
        (
            "javascript",
            include_str!("../../data/fixtures/source/javascript/module.js"),
        ),
        (
            "python",
            include_str!("../../data/fixtures/source/python/module.py"),
        ),
        (
            "twiglet",
            include_str!("../../data/fixtures/source/twiglet/fn_greet.twiglet"),
        ),
    ];
    for (lang, name) in languages() {
        let source = sources.iter().find(|(n, _)| *n == name).unwrap().1;
        let tree = Parser::new(lang.clone())
            .unwrap()
            .parse_str(source)
            .unwrap();
        let function = compile_query(
            &lang,
            "(function name: (identifier) @n)",
            QueryView::Semantic,
        )
        .unwrap();
        let hits = matches(&tree, &function, tree.root());
        assert!(!hits.is_empty(), "{name} function query");
        assert!(
            hits.iter().any(|node| tree.text(*node).contains("greet")),
            "{name} function hits missing greet: {:?}",
            hits.iter().map(|node| tree.text(*node)).collect::<Vec<_>>()
        );
        let import = compile_query(&lang, "(import source: (_) @s)", QueryView::Semantic).unwrap();
        if name == "lua" || name == "luau" {
            assert!(matches!(import, twigz_query::CompiledQuery::Never));
        } else {
            assert!(
                !matches!(import, twigz_query::CompiledQuery::Never),
                "{name}"
            );
        }
    }
    let import_cases = [
        (
            twiglet_lang(),
            include_str!("../../data/fixtures/source/twiglet/import.twiglet"),
            "mod",
        ),
        (
            javascript_lang(),
            include_str!("../../data/fixtures/source/javascript/module.js"),
            "mod",
        ),
        (
            python_lang(),
            include_str!("../../data/fixtures/source/python/module.py"),
            "os",
        ),
    ];
    for (lang, source, needle) in import_cases {
        let tree = Parser::new(lang.clone())
            .unwrap()
            .parse_str(source)
            .unwrap();
        let import = compile_query(&lang, "(import source: (_) @s)", QueryView::Semantic).unwrap();
        let hits = matches(&tree, &import, tree.root());
        assert!(
            hits.iter().any(|node| tree.text(*node).contains(needle)),
            "{} import hits missing {needle}: {:?}",
            lang.name,
            hits.iter().map(|node| tree.text(*node)).collect::<Vec<_>>()
        );
    }
}

#[test]
fn structured_queries_cover_five_languages() {
    let root = find_root().join("data/fixtures/source");
    let mut langs = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_dir() {
            continue;
        }
        let lang = path.file_name().unwrap().to_string_lossy().into_owned();
        langs.insert(lang.clone());
        let language = match lang.as_str() {
            "lua" => lua_lang(),
            "luau" => luau_lang(),
            "javascript" => javascript_lang(),
            "python" => python_lang(),
            "twiglet" => twiglet_lang(),
            other => panic!("unexpected fixture language {other}"),
        };
        for file in std::fs::read_dir(&path).unwrap() {
            let file = file.unwrap().path();
            let source = std::fs::read_to_string(&file).unwrap();
            let tree = Parser::new(language.clone())
                .unwrap()
                .parse_str(&source)
                .unwrap();
            let functions = tree.find(Kind::FUNCTION).count();
            let classes = tree.find(Kind::CLASS).count();
            let imports = tree.find(Kind::IMPORT).count();
            let strings = tree.find(Kind::STRING).count();
            let comments = tree.find(Kind::COMMENT).count();
            let declarations = tree.declarations().count();
            let scopes = tree.scopes_containing(0).count();
            let found_in = tree.find_in(tree.root(), Kind::FUNCTION).count();
            assert_eq!(found_in, functions, "find_in root {}", file.display());
            match lang.as_str() {
                "lua" => {
                    assert_eq!(classes, 0, "{}", file.display());
                    assert_eq!(imports, 0, "{}", file.display());
                    if file.file_name().unwrap() == "locals.lua" {
                        assert!(functions > 0);
                        assert!(strings > 0);
                        assert!(comments > 0);
                        assert!(declarations > 0, "{}", file.display());
                        assert!(scopes > 0, "{}", file.display());
                    }
                }
                "luau" => {
                    assert_eq!(classes, 0, "{}", file.display());
                    assert_eq!(imports, 0, "{}", file.display());
                    if file.file_name().unwrap() == "types.luau" {
                        assert!(functions > 0);
                        assert!(strings > 0, "{}", file.display());
                        assert!(comments > 0, "{}", file.display());
                        assert!(tree
                            .find(Kind::FUNCTION)
                            .any(|n| tree.text(n).contains("greet")));
                        assert!(tree.declarations().any(|n| tree.text(n).contains("T")));
                        assert!(scopes > 0, "{}", file.display());
                    }
                }
                "javascript" => {
                    assert!(functions > 0, "{}", file.display());
                    assert!(classes > 0, "{}", file.display());
                    assert!(imports > 0, "{}", file.display());
                    assert!(comments > 0, "{}", file.display());
                    assert!(
                        tree.find(Kind::COMMENT)
                            .any(|n| tree.text(n).contains("comment")),
                        "{}",
                        file.display()
                    );
                    assert!(declarations > 0, "{}", file.display());
                    assert!(scopes > 0, "{}", file.display());
                    assert!(
                        tree.find(Kind::LITERAL).next().is_some(),
                        "{}",
                        file.display()
                    );
                }
                "python" => {
                    assert!(functions > 0, "{}", file.display());
                    assert!(classes > 0, "{}", file.display());
                    assert!(imports > 0, "{}", file.display());
                    assert!(comments > 0, "{}", file.display());
                    assert!(declarations > 0, "{}", file.display());
                    assert!(scopes > 0, "{}", file.display());
                }
                "twiglet" => {
                    assert_eq!(classes, 0, "{}", file.display());
                    match file.file_name().unwrap().to_string_lossy().as_ref() {
                        "fn_greet.twiglet" => {
                            assert_eq!(functions, 1, "{}", file.display());
                            assert!(comments > 0, "{}", file.display());
                            assert!(tree
                                .find(Kind::COMMENT)
                                .any(|n| tree.text(n).contains("comment")));
                        }
                        "two_defs.twiglet" => {
                            assert_eq!(functions, 2, "{}", file.display());
                            assert!(tree
                                .find(Kind::FUNCTION)
                                .any(|n| tree.text(n).contains("greet")));
                            assert!(tree
                                .find(Kind::FUNCTION)
                                .any(|n| tree.text(n).contains("other")));
                        }
                        "import.twiglet" => {
                            assert_eq!(imports, 1, "{}", file.display());
                            assert!(strings > 0, "{}", file.display());
                            assert!(tree
                                .find(Kind::IMPORT)
                                .any(|n| tree.text(n).contains("mod")));
                        }
                        "interp.twiglet" => {
                            assert!(functions > 0, "{}", file.display());
                            assert!(strings > 0, "{}", file.display());
                        }
                        other => panic!("unexpected twiglet fixture {other}"),
                    }
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        langs,
        ["javascript", "lua", "luau", "python", "twiglet"]
            .into_iter()
            .map(str::to_string)
            .collect()
    );
}

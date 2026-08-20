use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter_generate::ABI_VERSION_MAX;
use twigz_generate::compile_sources;

fn find_root() -> PathBuf {
    let candidates = [
        std::env::var_os("BUILD_WORKSPACE_DIRECTORY").map(PathBuf::from),
        std::env::current_dir().ok(),
        std::env::var_os("TEST_SRCDIR").map(PathBuf::from),
        std::env::var_os("RUNFILES_DIR").map(PathBuf::from),
    ];
    for candidate in candidates.into_iter().flatten() {
        if candidate.join("grammars/lua.grammar").exists() {
            return candidate;
        }
        if candidate.join("_main/grammars/lua.grammar").exists() {
            return candidate.join("_main");
        }
        if let Ok(entries) = fs::read_dir(&candidate) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join("grammars/lua.grammar").exists() {
                    return path;
                }
                if let Ok(nested) = fs::read_dir(&path) {
                    for child in nested.flatten() {
                        if child.path().join("grammars/lua.grammar").exists() {
                            return child.path();
                        }
                    }
                }
            }
        }
    }
    panic!("cannot locate workspace grammars")
}

fn compile_pair(_root: &Path, grammar: &Path, modules: &[(&str, &Path)]) -> Value {
    let source = fs::read_to_string(grammar).unwrap();
    let loaded = modules
        .iter()
        .map(|(name, path)| {
            (
                (*name).to_string(),
                fs::read_to_string(path).unwrap(),
                path.to_string_lossy().into_owned(),
            )
        })
        .collect();
    let compiled = compile_sources(&source, &grammar.to_string_lossy(), loaded).unwrap();
    json!({
        "ir": compiled.ir,
        "semantics": compiled.semantics,
    })
}

fn strip_span_sources(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("span");
            if let Some(source) = map.get_mut("source") {
                if let Some(text) = source.as_str() {
                    if let Some(name) = Path::new(text).file_name() {
                        *source = json!(name.to_string_lossy().into_owned());
                    }
                }
            }
            for child in map.values_mut() {
                strip_span_sources(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                strip_span_sources(child);
            }
        }
        _ => {}
    }
}

fn normalize(mut value: Value) -> Value {
    strip_span_sources(&mut value);
    value
}

fn compare_golden(path: &Path, value: &Value) {
    let value = normalize(value.clone());
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let pretty = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
        fs::write(path, pretty).unwrap();
        return;
    }
    let existing = fs::read_to_string(path).unwrap_or_else(|error| {
        panic!("missing golden {}: {error}", path.display());
    });
    let existing_value: Value = serde_json::from_str(&existing).unwrap_or_else(|error| {
        panic!("unparsable golden {}: {error}", path.display());
    });
    assert_eq!(existing_value, value, "golden mismatch {}", path.display());
}

fn language_stems(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "json")
                .then(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .flatten()
        })
        .collect()
}

fn mapping_names(semantics: &Value) -> BTreeSet<String> {
    semantics["mappings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["concrete"].as_str().map(str::to_string))
        .collect()
}

fn scan_keys(ir: &Value) -> BTreeSet<String> {
    ir["scans"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|row| {
            row.get("name")
                .and_then(Value::as_str)
                .or_else(|| row.get("machine").and_then(Value::as_str))
                .unwrap_or("?")
                .to_string()
        })
        .collect()
}

#[test]
fn abi_version_matches_pin() {
    assert_eq!(ABI_VERSION_MAX, 15);
    let pin =
        fs::read_to_string(find_root().join("TREE_SITTER_PIN.md")).expect("TREE_SITTER_PIN.md");
    assert!(
        pin.contains("ABI_VERSION_MAX") && pin.contains("15"),
        "{pin}"
    );
}

#[test]
fn product_and_snapshot_goldens() {
    let root = find_root();
    let family = root.join("grammars/families/lua-core.grammar");
    let snapshot_family = root.join("data/goldens/snapshot/sources/lua-core.grammar");
    let product = [
        (
            "lua",
            root.join("grammars/lua.grammar"),
            vec![("lua.core", family.as_path())],
        ),
        (
            "luau",
            root.join("grammars/luau.grammar"),
            vec![("lua.core", family.as_path())],
        ),
        (
            "javascript",
            root.join("grammars/javascript.grammar"),
            vec![],
        ),
        ("python", root.join("grammars/python.grammar"), vec![]),
        (
            "twiglet",
            root.join("grammars/fixtures/twiglet.grammar"),
            vec![],
        ),
    ];
    let mut compiled_names = BTreeSet::new();
    for (name, grammar, modules) in &product {
        compiled_names.insert((*name).to_string());
        let bundle = compile_pair(&root, grammar, modules);
        compare_golden(
            &root.join(format!("data/goldens/ir/{name}.json")),
            &bundle["ir"],
        );
        compare_golden(
            &root.join(format!("data/goldens/semantics/{name}.json")),
            &bundle["semantics"],
        );
    }
    let ir_stems = language_stems(&root.join("data/goldens/ir"));
    let sem_stems = language_stems(&root.join("data/goldens/semantics"));
    assert_eq!(ir_stems, compiled_names, "ir goldens extra/omitted");
    assert_eq!(sem_stems, compiled_names, "semantics goldens extra/omitted");
    assert_eq!(ir_stems, sem_stems);

    let snapshot = [
        (
            "lua",
            root.join("data/goldens/snapshot/sources/lua.grammar"),
            vec![("lua.core", snapshot_family.as_path())],
        ),
        (
            "luau",
            root.join("data/goldens/snapshot/sources/luau.grammar"),
            vec![("lua.core", snapshot_family.as_path())],
        ),
    ];
    let expected_snapshot: BTreeSet<String> =
        ["lua", "luau"].into_iter().map(str::to_string).collect();
    assert_eq!(
        language_stems(&root.join("data/goldens/snapshot/ir")),
        expected_snapshot
    );
    assert_eq!(
        language_stems(&root.join("data/goldens/snapshot/semantics")),
        expected_snapshot
    );
    let expected_extra: BTreeSet<_> = [
        "local_name",
        "for_numeric_statement",
        "for_generic_statement",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    let expected_scan_extra: BTreeSet<_> = [
        "long_string_start",
        "long_string_content",
        "long_string_end",
        "long_comment",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    for (name, grammar, modules) in &snapshot {
        let bundle = compile_pair(&root, grammar, modules);
        compare_golden(
            &root.join(format!("data/goldens/snapshot/ir/{name}.json")),
            &bundle["ir"],
        );
        compare_golden(
            &root.join(format!("data/goldens/snapshot/semantics/{name}.json")),
            &bundle["semantics"],
        );
        let product = compile_pair(
            &root,
            &root.join(format!(
                "grammars/{}.grammar",
                if *name == "luau" { "luau" } else { "lua" }
            )),
            &[("lua.core", family.as_path())],
        );
        let extra: BTreeSet<_> = mapping_names(&product["semantics"])
            .difference(&mapping_names(&bundle["semantics"]))
            .cloned()
            .collect();
        let missing: BTreeSet<_> = mapping_names(&bundle["semantics"])
            .difference(&mapping_names(&product["semantics"]))
            .cloned()
            .collect();
        assert!(
            missing.is_empty(),
            "{name} product omitted snapshot mappings {missing:?}"
        );
        assert_eq!(extra, expected_extra, "{name} mapping extras");
        let scan_extra: BTreeSet<_> = scan_keys(&product["ir"])
            .difference(&scan_keys(&bundle["ir"]))
            .cloned()
            .collect();
        assert_eq!(scan_extra, expected_scan_extra, "{name} scan extras");
    }
}

#[test]
fn compile_twice_agrees() {
    let root = find_root();
    let family = root.join("grammars/families/lua-core.grammar");
    let first = compile_pair(
        &root,
        &root.join("grammars/lua.grammar"),
        &[("lua.core", family.as_path())],
    );
    let second = compile_pair(
        &root,
        &root.join("grammars/lua.grammar"),
        &[("lua.core", family.as_path())],
    );
    assert_eq!(first, second);
}

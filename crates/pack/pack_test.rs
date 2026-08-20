use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use twigz_generate::compile_sources;
use twigz_pack::{run, LanguageInput, PackOptions};

fn tmp() -> PathBuf {
    let dir = std::env::var("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let path = dir.join(format!("twigz-pack-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn find_named(root: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                return Some(path);
            }
        }
    }
    None
}

fn pack_bin() -> PathBuf {
    let mut roots = Vec::new();
    if let Ok(dir) = std::env::var("RUNFILES_DIR") {
        roots.push(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("TEST_SRCDIR") {
        roots.push(PathBuf::from(dir));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    for root in roots {
        if let Some(path) = find_named(&root, "twigz-pack") {
            return path;
        }
    }
    panic!("twigz-pack binary not found")
}

fn compile_tiny() -> (serde_json::Value, serde_json::Value) {
    let compiled = compile_sources(
        r#"
grammar tiny "1.0.0"
start source_file
source_file = "x"
"#,
        "tiny.grammar",
        Vec::new(),
    )
    .unwrap();
    (compiled.grammar_json, compiled.semantics)
}

fn pack_into(dir: &Path) -> PathBuf {
    let (grammar, semantics) = compile_tiny();
    fs::create_dir_all(dir).unwrap();
    let grammar_path = dir.join("grammar.json");
    let semantics_path = dir.join("semantics.json");
    fs::write(&grammar_path, serde_json::to_vec_pretty(&grammar).unwrap()).unwrap();
    fs::write(
        &semantics_path,
        serde_json::to_vec_pretty(&semantics).unwrap(),
    )
    .unwrap();
    run(PackOptions {
        languages: vec![LanguageInput {
            name: "tiny".into(),
            version: String::new(),
            grammar: grammar_path,
            semantics: semantics_path,
            parser_out: dir.join("parser.c"),
            node_types_out: dir.join("node-types.json"),
            manifest_out: dir.join("manifest.json"),
        }],
        tables_c: dir.join("tables.c"),
        registry_json: dir.join("registry.json"),
        report: dir.join("report.json"),
    })
    .unwrap();
    dir.to_path_buf()
}

fn hash_file(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

#[test]
fn rejects_empty_language_list() {
    let err = run(PackOptions {
        languages: Vec::new(),
        tables_c: PathBuf::from("/tmp/tables.c"),
        registry_json: PathBuf::from("/tmp/registry.json"),
        report: PathBuf::from("/tmp/report.json"),
    })
    .unwrap_err();
    assert!(err.contains("at least one language"), "{err}");
}

#[test]
fn pack_binary_has_no_registry_zig_flag() {
    let output = Command::new(pack_bin()).output().expect("spawn twigz-pack");
    let err = String::from_utf8_lossy(&output.stderr);
    for flag in ["--language", "--tables-c", "--registry-json", "--report"] {
        assert!(err.contains(flag), "{err} missing {flag}");
    }
    assert!(!err.contains("--registry-zig"), "{err}");
}

#[test]
fn pack_twice_is_deterministic() {
    let base = tmp();
    let first = pack_into(&base.join("a"));
    let second = pack_into(&base.join("b"));
    for name in ["parser.c", "tables.c", "registry.json", "registry.rs"] {
        assert_eq!(
            hash_file(&first.join(name)),
            hash_file(&second.join(name)),
            "{name}"
        );
    }
    assert!(first.join("registry.rs").exists());
    assert!(!first.join("registry.zig").exists());
    assert!(!second.join("registry.zig").exists());
}

#[test]
fn aliased_grammar_is_rejected() {
    let dir = tmp().join("alias");
    fs::create_dir_all(&dir).unwrap();
    let grammar = json!({
        "name": "aliasdemo",
        "rules": {
            "source_file": {
                "type": "ALIAS",
                "named": true,
                "value": "renamed",
                "content": { "type": "STRING", "value": "x" }
            }
        },
        "extras": [],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    });
    let semantics = json!({
        "language": "aliasdemo",
        "language_version": "1.0.0",
        "grammar_ir_version": 2,
        "vocabulary_version": 2,
        "tree_sitter_abi": 15,
        "mappings": []
    });
    fs::write(
        dir.join("grammar.json"),
        serde_json::to_vec_pretty(&grammar).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("semantics.json"),
        serde_json::to_vec_pretty(&semantics).unwrap(),
    )
    .unwrap();
    let err = run(PackOptions {
        languages: vec![LanguageInput {
            name: "aliasdemo".into(),
            version: String::new(),
            grammar: dir.join("grammar.json"),
            semantics: dir.join("semantics.json"),
            parser_out: dir.join("parser.c"),
            node_types_out: dir.join("node-types.json"),
            manifest_out: dir.join("manifest.json"),
        }],
        tables_c: dir.join("tables.c"),
        registry_json: dir.join("registry.json"),
        report: dir.join("report.json"),
    })
    .unwrap_err();
    assert!(err.to_lowercase().contains("alias"), "{err}");
}

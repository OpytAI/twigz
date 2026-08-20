use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use twigz_generate::compile_sources;

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

fn write_json(path: &Path, mut value: Value) {
    strip_span_sources(&mut value);
    let pretty = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, pretty).unwrap();
}

fn compile(root: &Path, grammar: &Path, modules: &[(&str, PathBuf)]) -> (Value, Value) {
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
    let compiled =
        compile_sources(&source, &grammar.to_string_lossy(), loaded).unwrap_or_else(|e| {
            panic!("{}: {e}", grammar.display());
        });
    let _ = root;
    (
        serde_json::to_value(compiled.ir).unwrap(),
        compiled.semantics,
    )
}

fn main() {
    let root =
        PathBuf::from(std::env::var("BUILD_WORKSPACE_DIRECTORY").expect("run via bazel run"));
    let family = root.join("grammars/families/lua-core.grammar");
    let snapshot_family = root.join("data/goldens/snapshot/sources/lua-core.grammar");
    let product = [
        (
            "lua",
            root.join("grammars/lua.grammar"),
            vec![("lua.core", family.clone())],
        ),
        (
            "luau",
            root.join("grammars/luau.grammar"),
            vec![("lua.core", family.clone())],
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
    for (name, grammar, modules) in &product {
        let (ir, sem) = compile(&root, grammar, modules);
        write_json(&root.join(format!("data/goldens/ir/{name}.json")), ir);
        write_json(
            &root.join(format!("data/goldens/semantics/{name}.json")),
            sem,
        );
    }
    for (name, grammar) in [
        (
            "lua",
            root.join("data/goldens/snapshot/sources/lua.grammar"),
        ),
        (
            "luau",
            root.join("data/goldens/snapshot/sources/luau.grammar"),
        ),
    ] {
        let (ir, sem) = compile(&root, &grammar, &[("lua.core", snapshot_family.clone())]);
        write_json(
            &root.join(format!("data/goldens/snapshot/ir/{name}.json")),
            ir,
        );
        write_json(
            &root.join(format!("data/goldens/snapshot/semantics/{name}.json")),
            sem,
        );
    }
    eprintln!("wrote goldens under {}", root.display());
}

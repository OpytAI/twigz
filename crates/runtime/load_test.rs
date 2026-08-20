use std::path::{Path, PathBuf};
use twigz_runtime::{LanguageSet, Parser};
use twigz_vocab::Kind;

fn find_named(root: &Path, needle: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name.contains(needle) && (name.ends_with(".so") || name.ends_with(".dylib")) {
                return Some(path);
            }
        }
    }
    None
}

fn cdylib_path() -> PathBuf {
    let mut roots = Vec::new();
    if let Ok(dir) = std::env::var("RUNFILES_DIR") {
        roots.push(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("TEST_SRCDIR") {
        roots.push(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::current_dir() {
        roots.push(dir);
    }
    for root in roots {
        if let Some(path) = find_named(&root, "twiglet_cdylib") {
            return path;
        }
        if let Some(path) = find_named(&root, "libtwiglet") {
            return path;
        }
    }
    panic!("twiglet cdylib not in runfiles")
}

#[test]
fn load_twiglet_and_parse_function() {
    let mut set = LanguageSet::new();
    let loaded = set.load(&cdylib_path()).expect("load twiglet cdylib");
    assert_eq!(loaded.name, "twiglet");
    let language = loaded.clone();
    let mut parser = Parser::new(language).expect("parser from loaded language");
    let tree = parser
        .parse_str("fn greet(name):\n    x = name\n")
        .expect("parse via load");
    let names: Vec<_> = tree
        .find(Kind::FUNCTION)
        .map(|node| tree.text(node).to_string())
        .collect();
    assert!(names.iter().any(|name| name.contains("greet")), "{names:?}");
}

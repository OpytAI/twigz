use std::path::{Path, PathBuf};
use std::process::Command;

fn find_named(root: &Path, name: &str) -> Option<PathBuf> {
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
            if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                return Some(path);
            }
        }
    }
    None
}

fn grammar_gen_bin() -> PathBuf {
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
        if let Some(path) = find_named(&root, "twigz-grammar-gen") {
            return path;
        }
    }
    panic!("twigz-grammar-gen binary not found")
}

#[test]
fn grammar_gen_lists_required_flags() {
    let output = Command::new(grammar_gen_bin())
        .output()
        .expect("spawn twigz-grammar-gen");
    let err = String::from_utf8_lossy(&output.stderr);
    for flag in [
        "--root",
        "--module",
        "--ir",
        "--grammar-json",
        "--semantics",
        "--diagnostics",
    ] {
        assert!(err.contains(flag), "{err} missing {flag}");
    }
    assert!(!err.contains("--registry-zig"), "{err}");
}

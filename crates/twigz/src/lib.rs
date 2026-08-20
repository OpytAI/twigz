//! Compile-only facade. Parse and query live behind `//:twigz-runtime` and `//:twigz-query`.

pub use twigz_ast as ast;
pub use twigz_backend as backend;
pub use twigz_dsl as dsl;
pub use twigz_elaborate as elaborate;
pub use twigz_format as format;
pub use twigz_generate as generate;
pub use twigz_ir as ir;
pub use twigz_pack as pack;
pub use twigz_scan as scan;
pub use twigz_vocab as vocab;

pub use twigz_ast::{Error, Span};
pub use twigz_generate::{compile_sources, CompiledGrammar};
pub use twigz_vocab::{Kind, Role, Trait, TraitSet};

use std::fs;
use std::path::Path;

pub fn compile_grammar(root: &Path, modules: &[(&str, &Path)]) -> Result<CompiledGrammar, Error> {
    let root_source = fs::read_to_string(root)
        .map_err(|error| Error::new(root.display().to_string(), 0, 0, error.to_string()))?;
    let mut loaded = Vec::new();
    for (name, path) in modules {
        let source = fs::read_to_string(path)
            .map_err(|error| Error::new(path.display().to_string(), 0, 0, error.to_string()))?;
        loaded.push((
            (*name).to_string(),
            source,
            path.to_string_lossy().into_owned(),
        ));
    }
    compile_sources(&root_source, &root.to_string_lossy(), loaded).map_err(Error::from_message)
}

//! Hermetic adapter from grammar sources to the pinned Tree-sitter generator core.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter_generate::ABI_VERSION_MAX;
use twigz_backend::grammar_json;
use twigz_dsl::parse;
use twigz_elaborate::elaborate;
use twigz_ir::{GrammarIr, SemanticMapping};
use twigz_vocab::{GRAMMAR_IR_VERSION, SEMANTIC_KINDS, VOCABULARY_VERSION};

pub struct Outputs {
    pub ir: PathBuf,
    pub grammar_json: PathBuf,
    pub semantics: PathBuf,
    pub diagnostics: PathBuf,
    pub scanner_c: Option<PathBuf>,
}

pub struct Options {
    pub root: PathBuf,
    pub modules: Vec<(String, PathBuf)>,
    pub outputs: Outputs,
}

pub struct CompiledGrammar {
    pub ir: GrammarIr,
    pub grammar_json: Value,
    pub semantics: Value,
}

#[derive(Default)]
struct SemanticKind {
    id: u32,
    roles: BTreeMap<String, bool>,
    traits: std::collections::BTreeSet<String>,
}

#[derive(Default)]
struct Vocabulary {
    version: u32,
    grammar_ir_version: u32,
    kinds: BTreeMap<String, SemanticKind>,
    roles: BTreeMap<String, u32>,
    traits: BTreeMap<String, u32>,
}

fn projected_vocabulary() -> Vocabulary {
    let mut out = Vocabulary {
        version: VOCABULARY_VERSION,
        grammar_ir_version: GRAMMAR_IR_VERSION,
        ..Vocabulary::default()
    };
    for spec in SEMANTIC_KINDS {
        let mut kind = SemanticKind {
            id: spec.id,
            ..SemanticKind::default()
        };
        for role in spec.roles {
            kind.roles.insert(role.name.into(), role.required);
            out.roles.insert(role.name.into(), role.id);
        }
        for semantic_trait in spec.traits {
            kind.traits.insert(semantic_trait.name.into());
            out.traits
                .insert(semantic_trait.name.into(), semantic_trait.id);
        }
        out.kinds.insert(spec.name.into(), kind);
    }
    out
}

fn semantic_json(
    mappings: &[SemanticMapping],
    vocabulary: &Vocabulary,
    grammar: &GrammarIr,
) -> Result<Value, String> {
    let mut rows = Vec::new();
    for mapping in mappings {
        let kind = vocabulary.kinds.get(&mapping.semantic).ok_or_else(|| {
            format!(
                "{}:{}:{}: unknown semantic kind {}",
                mapping.span.source, mapping.span.line, mapping.span.column, mapping.semantic
            )
        })?;
        let mut roles = serde_json::Map::new();
        for (canonical, concrete) in &mapping.roles {
            if !kind.roles.contains_key(canonical) {
                return Err(format!(
                    "{}:{}:{}: semantic kind {} does not define role {canonical}",
                    mapping.span.source, mapping.span.line, mapping.span.column, mapping.semantic
                ));
            }
            let id = vocabulary.roles.get(canonical).ok_or_else(|| {
                format!(
                    "{}:{}:{}: unknown semantic role {canonical}",
                    mapping.span.source, mapping.span.line, mapping.span.column
                )
            })?;
            let types = field_child_types(grammar, &mapping.concrete, concrete);
            roles.insert(
                canonical.clone(),
                json!({"id": id, "concrete": concrete, "types": types}),
            );
        }
        for (role, required) in &kind.roles {
            if *required && !mapping.roles.contains_key(role) {
                return Err(format!(
                    "{}:{}:{}: semantic kind {} requires role {role}",
                    mapping.span.source, mapping.span.line, mapping.span.column, mapping.semantic
                ));
            }
        }
        let mut traits = Vec::new();
        for name in &mapping.traits {
            if !kind.traits.contains(name) {
                return Err(format!(
                    "{}:{}:{}: semantic kind {} does not define trait {name}",
                    mapping.span.source, mapping.span.line, mapping.span.column, mapping.semantic
                ));
            }
            let id = vocabulary.traits.get(name).ok_or_else(|| {
                format!(
                    "{}:{}:{}: unknown semantic trait {name}",
                    mapping.span.source, mapping.span.line, mapping.span.column
                )
            })?;
            traits.push(json!({"id": id, "name": name}));
        }
        rows.push(json!({"concrete": mapping.concrete, "semantic": mapping.semantic, "semantic_id": kind.id, "roles": roles, "traits": traits}));
    }
    rows.sort_by(|a, b| a["concrete"].as_str().cmp(&b["concrete"].as_str()));
    Ok(json!({
        "language": grammar.name,
        "language_version": grammar.version,
        "grammar_version": grammar.version,
        "grammar_ir_version": grammar.ir_version,
        "vocabulary_version": vocabulary.version,
        "tree_sitter_abi": ABI_VERSION_MAX,
        "mappings": rows
    }))
}

fn field_child_types(grammar: &GrammarIr, production: &str, field: &str) -> Vec<String> {
    let Some(rule) = grammar.rules.get(production) else {
        return Vec::new();
    };
    let mut names = std::collections::BTreeSet::new();
    collect_field_types(rule, field, &mut names);
    names.into_iter().collect()
}

fn collect_field_types(
    rule: &twigz_ir::Rule,
    field: &str,
    out: &mut std::collections::BTreeSet<String>,
) {
    match rule {
        twigz_ir::Rule::Field { name, content } if name == field => {
            collect_named_types(content, out);
        }
        twigz_ir::Rule::Field { content, .. } => collect_field_types(content, field, out),
        twigz_ir::Rule::Choice(members) | twigz_ir::Rule::Sequence(members) => {
            for member in members {
                collect_field_types(member, field, out);
            }
        }
        twigz_ir::Rule::Repeat(content)
        | twigz_ir::Rule::Repeat1(content)
        | twigz_ir::Rule::Token(content)
        | twigz_ir::Rule::ImmediateToken(content)
        | twigz_ir::Rule::Alias { content, .. }
        | twigz_ir::Rule::Precedence { content, .. }
        | twigz_ir::Rule::DynamicPrecedence { content, .. }
        | twigz_ir::Rule::Reserved { content, .. } => collect_field_types(content, field, out),
        _ => {}
    }
}

fn collect_named_types(rule: &twigz_ir::Rule, out: &mut std::collections::BTreeSet<String>) {
    match rule {
        twigz_ir::Rule::Symbol(name) => {
            out.insert(name.clone());
        }
        twigz_ir::Rule::Choice(members) | twigz_ir::Rule::Sequence(members) => {
            for member in members {
                collect_named_types(member, out);
            }
        }
        twigz_ir::Rule::Repeat(content)
        | twigz_ir::Rule::Repeat1(content)
        | twigz_ir::Rule::Token(content)
        | twigz_ir::Rule::ImmediateToken(content)
        | twigz_ir::Rule::Field { content, .. }
        | twigz_ir::Rule::Alias { content, .. }
        | twigz_ir::Rule::Precedence { content, .. }
        | twigz_ir::Rule::DynamicPrecedence { content, .. }
        | twigz_ir::Rule::Reserved { content, .. } => collect_named_types(content, out),
        _ => {}
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn compile_sources(
    root_source: &str,
    root_name: &str,
    modules: Vec<(String, String, String)>,
) -> Result<CompiledGrammar, String> {
    let root = parse(root_source, root_name).map_err(|e| e.to_string())?;
    let mut parsed = Vec::new();
    for (name, source, path) in modules {
        parsed.push((name, parse(&source, &path).map_err(|e| e.to_string())?));
    }
    let vocabulary = projected_vocabulary();
    let mut grammar = elaborate(root, parsed)?;
    grammar.ir_version = vocabulary.grammar_ir_version;
    let semantic = semantic_json(&grammar.semantic, &vocabulary, &grammar)?;
    let grammar_value = grammar_json(&grammar);
    Ok(CompiledGrammar {
        ir: grammar,
        grammar_json: grammar_value,
        semantics: semantic,
    })
}

pub fn run(options: Options) -> Result<(), String> {
    let root_source = fs::read_to_string(&options.root)
        .map_err(|e| format!("{}: {e}", options.root.display()))?;
    let mut modules = Vec::new();
    for (name, path) in &options.modules {
        let source = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        modules.push((name.clone(), source, path.to_string_lossy().into_owned()));
    }
    let compiled = compile_sources(&root_source, &options.root.to_string_lossy(), modules)?;
    let ir_value = serde_json::to_value(&compiled.ir).map_err(|e| e.to_string())?;
    write_json(&options.outputs.ir, &ir_value)?;
    write_json(&options.outputs.grammar_json, &compiled.grammar_json)?;
    write_json(&options.outputs.semantics, &compiled.semantics)?;
    write_json(&options.outputs.diagnostics, &json!({"diagnostics": []}))?;
    if let Some(path) = &options.outputs.scanner_c {
        let scanner = twigz_scan::emit_c(&compiled.ir)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(path, scanner).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

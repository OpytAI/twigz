//! Parse a buffer with a packed language.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
use twigz_vocab::{Kind, Role, Trait, TraitSet};

#[repr(C)]
pub struct TSLanguage {
    _private: [u8; 0],
}

#[repr(C)]
pub struct TSParser {
    _private: [u8; 0],
}

#[repr(C)]
pub struct TSTree {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TSPoint {
    pub row: u32,
    pub column: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TSNode {
    context: [u32; 4],
    id: *const std::ffi::c_void,
    tree: *const TSTree,
}

#[repr(C)]
pub struct TSInputEdit {
    pub start_byte: u32,
    pub old_end_byte: u32,
    pub new_end_byte: u32,
    pub start_point: TSPoint,
    pub old_end_point: TSPoint,
    pub new_end_point: TSPoint,
}

#[repr(C)]
pub struct TSQuery {
    _private: [u8; 0],
}

#[repr(C)]
pub struct TSQueryCursor {
    _private: [u8; 0],
}

#[repr(C)]
pub struct TSQueryCapture {
    pub node: TSNode,
    pub index: u32,
}

#[repr(C)]
pub struct TSQueryMatch {
    pub id: u32,
    pub pattern_index: u16,
    pub capture_count: u16,
    pub captures: *const TSQueryCapture,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TSQueryPredicateStep {
    pub type_: u32,
    pub value_id: u32,
}

pub const TS_QUERY_PREDICATE_STEP_DONE: u32 = 0;
pub const TS_QUERY_PREDICATE_STEP_CAPTURE: u32 = 1;
pub const TS_QUERY_PREDICATE_STEP_STRING: u32 = 2;

pub type TSSymbol = u16;
pub type TSQueryError = i32;

extern "C" {
    pub fn ts_parser_new() -> *mut TSParser;
    pub fn ts_parser_delete(parser: *mut TSParser);
    pub fn ts_parser_set_language(parser: *mut TSParser, language: *const TSLanguage) -> bool;
    pub fn ts_parser_parse_string(
        parser: *mut TSParser,
        old_tree: *const TSTree,
        string: *const std::os::raw::c_char,
        length: u32,
    ) -> *mut TSTree;
    pub fn ts_tree_delete(tree: *mut TSTree);
    pub fn ts_tree_root_node(tree: *const TSTree) -> TSNode;
    pub fn ts_tree_edit(tree: *mut TSTree, edit: *const TSInputEdit);
    pub fn ts_node_type(node: TSNode) -> *const std::os::raw::c_char;
    pub fn ts_node_symbol(node: TSNode) -> TSSymbol;
    pub fn ts_node_start_byte(node: TSNode) -> u32;
    pub fn ts_node_end_byte(node: TSNode) -> u32;
    pub fn ts_node_start_point(node: TSNode) -> TSPoint;
    pub fn ts_node_end_point(node: TSNode) -> TSPoint;
    pub fn ts_node_child_count(node: TSNode) -> u32;
    pub fn ts_node_child(node: TSNode, index: u32) -> TSNode;
    pub fn ts_node_field_name_for_child(node: TSNode, index: u32) -> *const std::os::raw::c_char;
    pub fn ts_node_is_named(node: TSNode) -> bool;
    pub fn ts_node_is_null(node: TSNode) -> bool;
    pub fn ts_node_has_error(node: TSNode) -> bool;
    pub fn ts_node_parent(node: TSNode) -> TSNode;
    pub fn ts_node_descendant_for_byte_range(node: TSNode, start: u32, end: u32) -> TSNode;
    pub fn ts_query_new(
        language: *const TSLanguage,
        source: *const std::os::raw::c_char,
        source_len: u32,
        error_offset: *mut u32,
        error_type: *mut TSQueryError,
    ) -> *mut TSQuery;
    pub fn ts_query_delete(query: *mut TSQuery);
    pub fn ts_query_pattern_count(query: *const TSQuery) -> u32;
    pub fn ts_query_cursor_new() -> *mut TSQueryCursor;
    pub fn ts_query_cursor_delete(cursor: *mut TSQueryCursor);
    pub fn ts_query_cursor_exec(cursor: *mut TSQueryCursor, query: *const TSQuery, node: TSNode);
    pub fn ts_query_cursor_next_match(
        cursor: *mut TSQueryCursor,
        match_: *mut TSQueryMatch,
    ) -> bool;
    pub fn ts_query_predicates_for_pattern(
        query: *const TSQuery,
        pattern_index: u32,
        step_count: *mut u32,
    ) -> *const TSQueryPredicateStep;
    pub fn ts_query_string_value_for_id(
        query: *const TSQuery,
        index: u32,
        length: *mut u32,
    ) -> *const std::os::raw::c_char;
    pub fn ts_query_capture_name_for_id(
        query: *const TSQuery,
        index: u32,
        length: *mut u32,
    ) -> *const std::os::raw::c_char;
}

#[cfg(feature = "langs")]
extern "C" {
    fn tree_sitter_lua() -> *const TSLanguage;
    fn tree_sitter_luau() -> *const TSLanguage;
    fn tree_sitter_javascript() -> *const TSLanguage;
    fn tree_sitter_python() -> *const TSLanguage;
    fn tree_sitter_twiglet() -> *const TSLanguage;
}

#[derive(Clone, Debug)]
pub struct Mapping {
    pub concrete: String,
    pub semantic: String,
    pub semantic_id: u32,
    pub roles: BTreeMap<String, RoleField>,
    pub traits: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct RoleField {
    pub id: u32,
    pub concrete: String,
    pub types: Vec<String>,
}

#[derive(Clone)]
pub struct Language {
    pub name: String,
    pub ts: *const TSLanguage,
    pub mappings: Vec<Mapping>,
    pub by_concrete: BTreeMap<String, usize>,
    pub max_query: Option<usize>,
    #[cfg(not(target_arch = "wasm32"))]
    keep: Option<Arc<libloading::Library>>,
}

impl std::fmt::Debug for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Language")
            .field("name", &self.name)
            .field("max_query", &self.max_query)
            .finish()
    }
}

unsafe impl Send for Language {}
unsafe impl Sync for Language {}

impl Language {
    pub fn from_semantics(
        name: &str,
        ts: *const TSLanguage,
        semantics: &str,
    ) -> Result<Self, String> {
        let value: Value = serde_json::from_str(semantics).map_err(|e| e.to_string())?;
        let mut mappings = Vec::new();
        for row in value["mappings"]
            .as_array()
            .ok_or("semantics.mappings must be an array")?
        {
            let mut roles = BTreeMap::new();
            if let Some(object) = row["roles"].as_object() {
                for (name, spec) in object {
                    roles.insert(
                        name.clone(),
                        RoleField {
                            id: spec["id"].as_u64().unwrap_or(0) as u32,
                            concrete: spec["concrete"].as_str().unwrap_or(name).into(),
                            types: spec["types"]
                                .as_array()
                                .map(|values| {
                                    values
                                        .iter()
                                        .filter_map(|value| value.as_str().map(str::to_string))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        },
                    );
                }
            }
            let traits = row["traits"]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value["id"].as_u64().map(|id| id as u32))
                        .collect()
                })
                .unwrap_or_default();
            mappings.push(Mapping {
                concrete: row["concrete"].as_str().unwrap_or("").into(),
                semantic: row["semantic"].as_str().unwrap_or("").into(),
                semantic_id: row["semantic_id"].as_u64().unwrap_or(0) as u32,
                roles,
                traits,
            });
        }
        let mut by_concrete = BTreeMap::new();
        for (index, mapping) in mappings.iter().enumerate() {
            by_concrete.insert(mapping.concrete.clone(), index);
        }
        Ok(Self {
            name: name.into(),
            ts,
            mappings,
            by_concrete,
            max_query: None,
            #[cfg(not(target_arch = "wasm32"))]
            keep: None,
        })
    }

    pub fn concretes_for(&self, kind: Kind) -> Vec<&Mapping> {
        self.mappings
            .iter()
            .filter(|mapping| mapping.semantic_id == kind.0)
            .collect()
    }
}

#[cfg(feature = "langs")]
fn load_lang(name: &str, ts: *const TSLanguage, json: &str) -> Language {
    Language::from_semantics(name, ts, json).unwrap_or_else(|_| Language {
        name: name.into(),
        ts,
        mappings: Vec::new(),
        by_concrete: BTreeMap::new(),
        max_query: None,
        #[cfg(not(target_arch = "wasm32"))]
        keep: None,
    })
}

#[cfg(feature = "langs")]
pub fn lua_lang() -> Language {
    load_lang(
        "lua",
        unsafe { tree_sitter_lua() },
        include_str!("../../../data/goldens/semantics/lua.json"),
    )
}

#[cfg(feature = "langs")]
pub fn luau_lang() -> Language {
    load_lang(
        "luau",
        unsafe { tree_sitter_luau() },
        include_str!("../../../data/goldens/semantics/luau.json"),
    )
}

#[cfg(feature = "langs")]
pub fn javascript_lang() -> Language {
    load_lang(
        "javascript",
        unsafe { tree_sitter_javascript() },
        include_str!("../../../data/goldens/semantics/javascript.json"),
    )
}

#[cfg(feature = "langs")]
pub fn python_lang() -> Language {
    load_lang(
        "python",
        unsafe { tree_sitter_python() },
        include_str!("../../../data/goldens/semantics/python.json"),
    )
}

#[cfg(feature = "langs")]
pub fn twiglet_lang() -> Language {
    load_lang(
        "twiglet",
        unsafe { tree_sitter_twiglet() },
        include_str!("../../../data/goldens/semantics/twiglet.json"),
    )
}

pub struct Limits {
    pub max_source: Option<usize>,
    pub max_query: Option<usize>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_source: None,
            max_query: None,
        }
    }
}

pub struct Parser {
    raw: *mut TSParser,
    language: Language,
    limits: Limits,
}

impl Parser {
    pub fn new(language: Language) -> Result<Self, String> {
        let raw = unsafe { ts_parser_new() };
        if raw.is_null() {
            return Err("failed to create parser".into());
        }
        if !unsafe { ts_parser_set_language(raw, language.ts) } {
            unsafe { ts_parser_delete(raw) };
            return Err("language ABI mismatch".into());
        }
        Ok(Self {
            raw,
            language,
            limits: Limits::default(),
        })
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.language.max_query = limits.max_query;
        self.limits = limits;
        self
    }

    pub fn parse_str(&mut self, source: &str) -> Result<Tree, String> {
        self.parse(source, None)
    }

    pub fn parse(&mut self, source: &str, old: Option<&Tree>) -> Result<Tree, String> {
        if let Some(max) = self.limits.max_source {
            if source.len() > max {
                return Err("source exceeds max_source".into());
            }
        }
        let old_ptr = old
            .map(|tree| tree.raw as *const TSTree)
            .unwrap_or(std::ptr::null());
        let raw = unsafe {
            ts_parser_parse_string(
                self.raw,
                old_ptr,
                source.as_ptr() as *const std::os::raw::c_char,
                source.len() as u32,
            )
        };
        if raw.is_null() {
            return Err("parse failed".into());
        }
        Ok(Tree {
            raw,
            source: source.to_string(),
            language: self.language.clone(),
        })
    }
}

impl Drop for Parser {
    fn drop(&mut self) {
        unsafe { ts_parser_delete(self.raw) }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub raw: TSNode,
}

#[derive(Clone, Copy, Debug)]
pub struct Range {
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_point: TSPoint,
    pub end_point: TSPoint,
}

#[derive(Clone, Copy, Debug)]
pub struct Child {
    pub node: Node,
    pub role: Option<Role>,
    pub field: Option<&'static str>,
}

pub struct Tree {
    raw: *mut TSTree,
    pub source: String,
    pub language: Language,
}

impl Tree {
    pub fn root(&self) -> Node {
        Node {
            raw: unsafe { ts_tree_root_node(self.raw) },
        }
    }

    pub fn has_error(&self) -> bool {
        unsafe { ts_node_has_error(self.root().raw) }
    }

    pub fn node_at(&self, byte: u32) -> Node {
        let root = self.root();
        Node {
            raw: unsafe { ts_node_descendant_for_byte_range(root.raw, byte, byte) },
        }
    }

    pub fn concrete_kind(&self, n: Node) -> &str {
        unsafe {
            let ptr = ts_node_type(n.raw);
            if ptr.is_null() {
                ""
            } else {
                std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("")
            }
        }
    }

    pub fn mapping(&self, n: Node) -> Option<&Mapping> {
        let name = self.concrete_kind(n);
        let index = *self.language.by_concrete.get(name)?;
        Some(&self.language.mappings[index])
    }

    pub fn kind(&self, n: Node) -> Option<Kind> {
        self.mapping(n).map(|mapping| Kind(mapping.semantic_id))
    }

    pub fn traits(&self, n: Node) -> TraitSet {
        let mut set = TraitSet::empty();
        if let Some(mapping) = self.mapping(n) {
            for id in &mapping.traits {
                set.insert(Trait(*id));
            }
        }
        set
    }

    pub fn range(&self, n: Node) -> Range {
        unsafe {
            Range {
                start_byte: ts_node_start_byte(n.raw),
                end_byte: ts_node_end_byte(n.raw),
                start_point: ts_node_start_point(n.raw),
                end_point: ts_node_end_point(n.raw),
            }
        }
    }

    pub fn raw_children(&self, n: Node) -> impl Iterator<Item = Child> + '_ {
        self.all_children(n).into_iter()
    }

    fn all_children(&self, n: Node) -> Vec<Child> {
        let count = unsafe { ts_node_child_count(n.raw) };
        let mapping = self.mapping(n);
        let mut out = Vec::new();
        for index in 0..count {
            let child = Node {
                raw: unsafe { ts_node_child(n.raw, index) },
            };
            if unsafe { ts_node_is_null(child.raw) } {
                continue;
            }
            let field = unsafe {
                let ptr = ts_node_field_name_for_child(n.raw, index);
                if ptr.is_null() {
                    None
                } else {
                    Some(std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or(""))
                }
            };
            let role = mapping.and_then(|mapping| {
                field.and_then(|field| {
                    mapping
                        .roles
                        .iter()
                        .find(|(_, spec)| spec.concrete == field)
                        .map(|(_, spec)| Role(spec.id))
                })
            });
            out.push(Child {
                node: child,
                role,
                field,
            });
        }
        out
    }

    pub fn children(&self, n: Node) -> impl Iterator<Item = Child> + '_ {
        let all = self.all_children(n);
        let role_tagged: Vec<Child> = all
            .iter()
            .copied()
            .filter(|child| child.role.is_some())
            .collect();
        let filtered = if role_tagged.is_empty() {
            all.into_iter()
                .filter(|child| self.kind(child.node).is_some())
                .collect()
        } else {
            let mut out = role_tagged;
            for child in all {
                if child.role.is_none() && self.kind(child.node).is_some() {
                    out.push(child);
                }
            }
            out
        };
        filtered.into_iter()
    }

    pub fn ancestors(&self, n: Node) -> impl Iterator<Item = Node> + '_ {
        let mut out = Vec::new();
        let mut current = n;
        loop {
            let parent = Node {
                raw: unsafe { ts_node_parent(current.raw) },
            };
            if unsafe { ts_node_is_null(parent.raw) } {
                break;
            }
            out.push(parent);
            current = parent;
        }
        out.into_iter()
    }

    pub fn text(&self, n: Node) -> &str {
        let range = self.range(n);
        let start = range.start_byte as usize;
        let end = (range.end_byte as usize).min(self.source.len());
        self.source.get(start..end).unwrap_or("")
    }

    pub fn edit(&mut self, edit: TSInputEdit) {
        unsafe { ts_tree_edit(self.raw, &edit) }
    }

    pub fn find(&self, kind: Kind) -> impl Iterator<Item = Node> + '_ {
        self.find_in(self.root(), kind)
    }

    pub fn find_in(&self, n: Node, kind: Kind) -> impl Iterator<Item = Node> + '_ {
        let mut out = Vec::new();
        self.collect(n, kind, &mut out);
        out.into_iter()
    }

    fn collect(&self, n: Node, kind: Kind, out: &mut Vec<Node>) {
        if self.kind(n) == Some(kind) {
            out.push(n);
        }
        for child in self.all_children(n) {
            self.collect(child.node, kind, out);
        }
    }

    pub fn declarations(&self) -> impl Iterator<Item = Node> + '_ {
        let mut out = Vec::new();
        self.collect_trait(self.root(), Trait::DECLARATION, &mut out);
        out.into_iter()
    }

    fn collect_trait(&self, n: Node, marker: Trait, out: &mut Vec<Node>) {
        if self.traits(n).contains(marker) {
            out.push(n);
        }
        for child in self.all_children(n) {
            self.collect_trait(child.node, marker, out);
        }
    }

    pub fn scopes_containing(&self, byte: u32) -> impl Iterator<Item = Node> + '_ {
        let mut node = self.node_at(byte);
        if !unsafe { ts_node_is_named(node.raw) } {
            let parent = Node {
                raw: unsafe { ts_node_parent(node.raw) },
            };
            if !unsafe { ts_node_is_null(parent.raw) } {
                node = parent;
            }
        }
        let mut out = Vec::new();
        let mut current = Some(node);
        while let Some(n) = current {
            if self.traits(n).contains(Trait::SCOPE) {
                out.push(n);
            }
            let parent = Node {
                raw: unsafe { ts_node_parent(n.raw) },
            };
            current = if unsafe { ts_node_is_null(parent.raw) } {
                None
            } else {
                Some(parent)
            };
        }
        out.into_iter()
    }

    pub fn binding_at(&self, byte: u32) -> Option<Node> {
        let mut node = self.node_at(byte);
        if !unsafe { ts_node_is_named(node.raw) } {
            let parent = Node {
                raw: unsafe { ts_node_parent(node.raw) },
            };
            if !unsafe { ts_node_is_null(parent.raw) } {
                node = parent;
            }
        }
        let mut current = Some(node);
        while let Some(n) = current {
            if self.traits(n).contains(Trait::DECLARATION) {
                return Some(n);
            }
            let parent = Node {
                raw: unsafe { ts_node_parent(n.raw) },
            };
            current = if unsafe { ts_node_is_null(parent.raw) } {
                None
            } else {
                Some(parent)
            };
        }
        None
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        unsafe { ts_tree_delete(self.raw) }
    }
}

pub unsafe fn ts_language(lang: &Language) -> *const TSLanguage {
    lang.ts
}

pub struct LanguageSet {
    languages: Vec<Language>,
}

impl LanguageSet {
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
        }
    }

    pub fn add(&mut self, lang: Language) {
        self.languages.push(lang);
    }

    pub fn get(&self, name: &str) -> Option<&Language> {
        self.languages.iter().find(|lang| lang.name == name)
    }

    pub fn load(&mut self, cdylib: &Path) -> Result<&Language, String> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = cdylib;
            return Err("LanguageSet::load is native only".into());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            unsafe {
                let lib = libloading::Library::new(cdylib)
                    .map_err(|error| format!("{}: {error}", cdylib.display()))?;
                let name_fn: libloading::Symbol<
                    unsafe extern "C" fn() -> *const std::os::raw::c_char,
                > = lib
                    .get(b"twigz_language_name\0")
                    .map_err(|error| error.to_string())?;
                let sem_fn: libloading::Symbol<
                    unsafe extern "C" fn() -> *const std::os::raw::c_char,
                > = lib
                    .get(b"twigz_semantics_json\0")
                    .map_err(|error| error.to_string())?;
                let name_ptr = name_fn();
                let sem_ptr = sem_fn();
                if name_ptr.is_null() || sem_ptr.is_null() {
                    return Err(format!(
                        "{}: twigz_language_name or twigz_semantics_json returned null",
                        cdylib.display()
                    ));
                }
                let name = std::ffi::CStr::from_ptr(name_ptr)
                    .to_string_lossy()
                    .into_owned();
                let semantics = std::ffi::CStr::from_ptr(sem_ptr)
                    .to_string_lossy()
                    .into_owned();
                let symbol = format!("tree_sitter_{name}\0");
                let ts_fn: libloading::Symbol<unsafe extern "C" fn() -> *const TSLanguage> = lib
                    .get(symbol.as_bytes())
                    .map_err(|error| error.to_string())?;
                let ts = ts_fn();
                if ts.is_null() {
                    return Err(format!(
                        "{}: tree_sitter_{name} returned null",
                        cdylib.display()
                    ));
                }
                let mut language = Language::from_semantics(&name, ts, &semantics)?;
                language.keep = Some(Arc::new(lib));
                self.languages.push(language);
            }
            Ok(self.languages.last().unwrap())
        }
    }
}

impl Default for LanguageSet {
    fn default() -> Self {
        Self::new()
    }
}

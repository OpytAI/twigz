//! Language-neutral queries over vocabulary names.

use std::fmt;
use twigz_runtime::{
    ts_query_cursor_delete, ts_query_cursor_exec, ts_query_cursor_new, ts_query_cursor_next_match,
    ts_query_delete, ts_query_new, ts_query_pattern_count, ts_query_predicates_for_pattern,
    ts_query_string_value_for_id, Language, Node, TSQuery, TSQueryCapture, TSQueryMatch,
    TSQueryPredicateStep, Tree, TS_QUERY_PREDICATE_STEP_CAPTURE, TS_QUERY_PREDICATE_STEP_DONE,
    TS_QUERY_PREDICATE_STEP_STRING,
};
use twigz_vocab::{kind_by_name, Kind, Role};

const MAX_PATTERNS: usize = 64;
const MAX_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryView {
    Semantic,
    Concrete,
}

pub enum CompiledQuery {
    Ts(TsQuery),
    Never,
}

impl std::fmt::Debug for CompiledQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompiledQuery::Never => write!(f, "Never"),
            CompiledQuery::Ts(query) => write!(f, "Ts({})", query.source),
        }
    }
}

pub struct TsQuery {
    pub raw: *mut TSQuery,
    pub source: String,
}

impl Drop for TsQuery {
    fn drop(&mut self) {
        unsafe { ts_query_delete(self.raw) }
    }
}

unsafe impl Send for TsQuery {}

#[derive(Clone, Debug)]
pub struct QueryError {
    pub code: String,
    pub message: String,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for QueryError {}

#[derive(Clone, Debug)]
enum Atom {
    Ident(String),
    String(String),
    Capture(String),
}

#[derive(Clone, Debug)]
enum Sexp {
    List {
        head: Box<Sexp>,
        children: Vec<FieldOrChild>,
        capture: Option<String>,
        quantifier: Option<char>,
    },
    Alternation(Vec<Sexp>),
    Anonymous(String, Option<String>),
    Wildcard(Option<String>),
    Predicate {
        name: String,
        args: Vec<Atom>,
    },
}

#[derive(Clone, Debug)]
enum FieldOrChild {
    Field { role: String, value: Sexp },
    Child(Sexp),
    Adjacent(Sexp),
}

struct Parser<'a> {
    src: &'a str,
    at: usize,
}

impl<'a> Parser<'a> {
    fn skip(&mut self) {
        while let Some(c) = self.src[self.at..].chars().next() {
            if c.is_whitespace() {
                self.at += c.len_utf8();
            } else {
                break;
            }
        }
    }
    fn peek(&self) -> Option<char> {
        self.src[self.at..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        Some(c)
    }
    fn ident(&mut self) -> Result<String, QueryError> {
        let start = self.at;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '?' || c == '!' {
                self.bump();
            } else {
                break;
            }
        }
        if start == self.at {
            return Err(error("query_syntax", "expected identifier"));
        }
        Ok(self.src[start..self.at].into())
    }
    fn string(&mut self) -> Result<String, QueryError> {
        if self.bump() != Some('"') {
            return Err(error("query_syntax", "expected string"));
        }
        let mut out = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some(c) => out.push(c),
                    None => return Err(error("query_syntax", "unterminated string")),
                },
                Some(c) => out.push(c),
                None => return Err(error("query_syntax", "unterminated string")),
            }
        }
    }
    fn sexp(&mut self) -> Result<Sexp, QueryError> {
        self.skip();
        match self.peek() {
            Some('(') => {
                self.bump();
                self.skip();
                if self.peek() == Some('#') {
                    self.bump();
                    let name = self.ident()?;
                    let mut args = Vec::new();
                    loop {
                        self.skip();
                        match self.peek() {
                            Some(')') => {
                                self.bump();
                                break;
                            }
                            Some('@') => {
                                self.bump();
                                args.push(Atom::Capture(self.ident()?));
                            }
                            Some('"') => args.push(Atom::String(self.string()?)),
                            Some(_) => args.push(Atom::Ident(self.ident()?)),
                            None => return Err(error("query_syntax", "unterminated predicate")),
                        }
                    }
                    return Ok(Sexp::Predicate { name, args });
                }
                let head = if self.peek() == Some('_') {
                    self.bump();
                    Sexp::Wildcard(None)
                } else {
                    let name = self.ident()?;
                    Sexp::Anonymous(name, None)
                };
                let head_name = match &head {
                    Sexp::Anonymous(name, _) => Some(name.clone()),
                    Sexp::Wildcard(_) => None,
                    _ => None,
                };
                let mut children = Vec::new();
                let mut capture = None;
                loop {
                    self.skip();
                    match self.peek() {
                        Some(')') => {
                            self.bump();
                            break;
                        }
                        Some('@') => {
                            self.bump();
                            let name = self.ident()?;
                            if let Some(last) = children.last_mut() {
                                attach_capture(last, name);
                            } else {
                                capture = Some(name);
                            }
                        }
                        Some('.') => {
                            self.bump();
                            children.push(FieldOrChild::Adjacent(self.sexp()?));
                        }
                        Some('[') | Some('(') | Some('"') | Some('_') => {
                            children.push(FieldOrChild::Child(self.sexp()?));
                        }
                        Some(_) => {
                            let name = self.ident()?;
                            self.skip();
                            if self.peek() == Some(':') {
                                self.bump();
                                children.push(FieldOrChild::Field {
                                    role: name,
                                    value: self.sexp()?,
                                });
                            } else {
                                return Err(error(
                                    "query_syntax",
                                    "expected field `name:` or a child sexp",
                                ));
                            }
                        }
                        None => return Err(error("query_syntax", "unterminated list")),
                    }
                }
                self.skip();
                let quantifier = match self.peek() {
                    Some(c @ ('?' | '*' | '+')) => {
                        self.bump();
                        Some(c)
                    }
                    _ => None,
                };
                Ok(Sexp::List {
                    head: Box::new(match head_name {
                        Some(name) => Sexp::Anonymous(name, None),
                        None => Sexp::Wildcard(None),
                    }),
                    children,
                    capture,
                    quantifier,
                })
            }
            Some('[') => {
                self.bump();
                let mut items = Vec::new();
                loop {
                    self.skip();
                    if self.peek() == Some(']') {
                        self.bump();
                        break;
                    }
                    items.push(self.sexp()?);
                }
                Ok(Sexp::Alternation(items))
            }
            Some('"') => {
                let value = self.string()?;
                self.skip();
                let capture = if self.peek() == Some('@') {
                    self.bump();
                    Some(self.ident()?)
                } else {
                    None
                };
                Ok(Sexp::Anonymous(value, capture))
            }
            Some('_') => {
                self.bump();
                let capture = if self.peek() == Some('@') {
                    self.bump();
                    Some(self.ident()?)
                } else {
                    None
                };
                Ok(Sexp::Wildcard(capture))
            }
            _ => Err(error("query_syntax", "expected sexp")),
        }
    }
}

fn attach_capture(child: &mut FieldOrChild, name: String) {
    let sexp = match child {
        FieldOrChild::Field { value, .. }
        | FieldOrChild::Child(value)
        | FieldOrChild::Adjacent(value) => value,
    };
    match sexp {
        Sexp::List { capture, .. } | Sexp::Wildcard(capture) | Sexp::Anonymous(_, capture)
            if capture.is_none() =>
        {
            *capture = Some(name);
        }
        _ => {}
    }
}

fn error(code: &str, message: impl Into<String>) -> QueryError {
    QueryError {
        code: code.into(),
        message: message.into(),
    }
}

fn parse_query(source: &str) -> Result<Vec<Sexp>, QueryError> {
    let mut parser = Parser { src: source, at: 0 };
    let mut patterns = Vec::new();
    loop {
        parser.skip();
        if parser.peek().is_none() {
            break;
        }
        patterns.push(parser.sexp()?);
    }
    if patterns.is_empty() {
        return Err(error("query_syntax", "empty query"));
    }
    Ok(patterns)
}

fn reject_forbidden(sexp: &Sexp) -> Result<(), QueryError> {
    match sexp {
        Sexp::Predicate { name, .. } => match name.as_str() {
            "eq?" | "match?" | "any-of?" | "not-eq?" => Ok(()),
            "lua-match?" | "set!" | "is?" => Err(error(
                "unknown_semantic_symbol",
                format!("#{name} is not allowed"),
            )),
            other => Err(error(
                "unknown_semantic_symbol",
                format!("#{other} is not allowed"),
            )),
        },
        Sexp::List { children, .. } => {
            for child in children {
                let inner = match child {
                    FieldOrChild::Field { role, value } => {
                        if role == "derives" {
                            return Err(error("unknown_semantic_symbol", "derives is not allowed"));
                        }
                        value
                    }
                    FieldOrChild::Child(value) | FieldOrChild::Adjacent(value) => value,
                };
                reject_forbidden(inner)?;
            }
            Ok(())
        }
        Sexp::Alternation(items) => {
            for item in items {
                reject_forbidden(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

struct Rewrite<'a> {
    language: &'a Language,
    view: QueryView,
}

impl Rewrite<'_> {
    fn resolve_head(&self, name: &str) -> Result<Vec<String>, QueryError> {
        if self.view == QueryView::Concrete {
            return Ok(vec![name.into()]);
        }
        let Some(kind) = kind_by_name(name) else {
            return Err(error(
                "unknown_semantic_symbol",
                format!("{name} is not a vocabulary kind"),
            ));
        };
        let concretes = self
            .language
            .concretes_for(Kind(kind.id))
            .into_iter()
            .map(|mapping| mapping.concrete.clone())
            .collect::<Vec<_>>();
        Ok(concretes)
    }

    fn rewrite(&self, sexp: &Sexp) -> Result<Vec<String>, QueryError> {
        match sexp {
            Sexp::Predicate { name, args } => {
                let mut out = format!("(#{name}");
                for arg in args {
                    match arg {
                        Atom::Capture(name) => out.push_str(&format!(" @{name}")),
                        Atom::String(value) => out.push_str(&format!(" \"{value}\"")),
                        Atom::Ident(value) => out.push_str(&format!(" {value}")),
                    }
                }
                out.push(')');
                Ok(vec![out])
            }
            Sexp::Wildcard(capture) => {
                let mut out = "(_)".to_string();
                if let Some(capture) = capture {
                    out.push_str(&format!(" @{capture}"));
                }
                Ok(vec![out])
            }
            Sexp::Anonymous(value, capture) => {
                let mut out = format!("\"{value}\"");
                if let Some(capture) = capture {
                    out.push_str(&format!(" @{capture}"));
                }
                Ok(vec![out])
            }
            Sexp::Alternation(items) => {
                let mut parts = Vec::new();
                for item in items {
                    parts.extend(self.rewrite(item)?);
                }
                Ok(vec![format!("[{}]", parts.join(" "))])
            }
            Sexp::List {
                head,
                children,
                capture,
                quantifier,
            } => {
                let head_name = match head.as_ref() {
                    Sexp::Anonymous(name, _) => Some(name.as_str()),
                    Sexp::Wildcard(_) => None,
                    _ => None,
                };
                let concretes = if let Some(name) = head_name {
                    let mut names = self.resolve_head(name)?;
                    let used_roles: Vec<&str> = children
                        .iter()
                        .filter_map(|child| match child {
                            FieldOrChild::Field { role, .. } => Some(role.as_str()),
                            _ => None,
                        })
                        .collect();
                    if self.view == QueryView::Semantic {
                        names.retain(|concrete| {
                            self.language
                                .by_concrete
                                .get(concrete)
                                .map(|index| {
                                    let mapping = &self.language.mappings[*index];
                                    used_roles
                                        .iter()
                                        .all(|role| mapping.roles.contains_key(*role))
                                })
                                .unwrap_or(false)
                        });
                    }
                    if names.is_empty() {
                        return Ok(Vec::new());
                    }
                    names
                } else {
                    vec!["_".into()]
                };
                let mut child_renders: Vec<Vec<String>> = Vec::new();
                for child in children {
                    let rendered = match child {
                        FieldOrChild::Child(value) | FieldOrChild::Adjacent(value) => {
                            self.rewrite(value)?
                        }
                        FieldOrChild::Field { role, value } => {
                            if self.view == QueryView::Semantic && Role::from_name(role).is_none() {
                                return Err(error(
                                    "unknown_semantic_symbol",
                                    format!("{role} is not a vocabulary role"),
                                ));
                            }
                            self.rewrite(value)?
                        }
                    };
                    if rendered.is_empty() {
                        return Ok(Vec::new());
                    }
                    child_renders.push(rendered);
                }
                let mut patterns = Vec::new();
                for concrete in &concretes {
                    let mapping = self
                        .language
                        .by_concrete
                        .get(concrete)
                        .map(|index| &self.language.mappings[*index]);
                    let field_names: Vec<String> = children
                        .iter()
                        .map(|child| match child {
                            FieldOrChild::Field { role, .. } => {
                                if self.view == QueryView::Semantic {
                                    mapping
                                        .and_then(|mapping| {
                                            mapping
                                                .roles
                                                .get(role)
                                                .map(|spec| spec.concrete.clone())
                                        })
                                        .unwrap_or_else(|| role.clone())
                                } else {
                                    role.clone()
                                }
                            }
                            _ => String::new(),
                        })
                        .collect();
                    let mut pieces = vec![vec![format!("({concrete}")]];
                    for (index, child) in children.iter().enumerate() {
                        let rendered = match child {
                            FieldOrChild::Field { role, .. } => {
                                let types = mapping
                                    .and_then(|mapping| mapping.roles.get(role))
                                    .map(|spec| spec.types.as_slice())
                                    .unwrap_or(&[]);
                                retarget_field_children(&child_renders[index], types)
                            }
                            _ => child_renders[index].clone(),
                        };
                        let prefix = match child {
                            FieldOrChild::Field { .. } => format!("{}:", field_names[index]),
                            FieldOrChild::Adjacent(_) => ".".into(),
                            FieldOrChild::Child(_) => String::new(),
                        };
                        let next = rendered
                            .iter()
                            .map(|item| {
                                if prefix.is_empty() {
                                    item.clone()
                                } else if prefix == "." {
                                    format!(". {item}")
                                } else {
                                    format!("{prefix} {item}")
                                }
                            })
                            .collect::<Vec<_>>();
                        pieces.push(next);
                    }
                    let mut tails = vec![String::new()];
                    for piece in pieces.into_iter().skip(1) {
                        let mut next = Vec::new();
                        for prefix in &tails {
                            for item in &piece {
                                next.push(format!("{prefix} {item}"));
                            }
                        }
                        tails = next;
                    }
                    for tail in tails {
                        let mut pattern = format!("({concrete}{tail})");
                        if let Some(capture) = capture {
                            pattern.push_str(&format!(" @{capture}"));
                        }
                        if let Some(q) = quantifier {
                            pattern.push(*q);
                        }
                        patterns.push(pattern);
                    }
                }
                Ok(patterns)
            }
        }
    }
}

pub fn compile_query(
    language: &Language,
    source: &str,
    view: QueryView,
) -> Result<CompiledQuery, QueryError> {
    if let Some(max) = language.max_query {
        if source.len() > max {
            return Err(error("query_too_complex", "query exceeds max_query"));
        }
    }
    if source.contains("{") {
        return Err(error("query_syntax", "counted repetition is not allowed"));
    }
    let patterns = parse_query(source)?;
    for pattern in &patterns {
        reject_forbidden(pattern)?;
    }
    let rewrite = Rewrite { language, view };
    let mut rendered = Vec::new();
    for pattern in &patterns {
        rendered.extend(rewrite.rewrite(pattern)?);
    }
    if rendered.is_empty() {
        return Ok(CompiledQuery::Never);
    }
    if rendered.len() > MAX_PATTERNS {
        return Err(error("query_too_complex", "more than 64 patterns"));
    }
    let joined = rendered.join("\n");
    if joined.len() > MAX_BYTES {
        return Err(error("query_too_complex", "query exceeds 8192 bytes"));
    }
    let mut error_offset = 0_u32;
    let mut error_type = 0;
    let raw = unsafe {
        ts_query_new(
            language.ts,
            joined.as_ptr() as *const std::os::raw::c_char,
            joined.len() as u32,
            &mut error_offset,
            &mut error_type,
        )
    };
    if raw.is_null() {
        return Err(error(
            "query_syntax",
            format!("tree-sitter rejected query at {error_offset} type={error_type}: {joined}"),
        ));
    }
    Ok(CompiledQuery::Ts(TsQuery {
        raw,
        source: joined,
    }))
}

fn retarget_field_children(rendered: &[String], types: &[String]) -> Vec<String> {
    if types.is_empty() {
        return rendered.to_vec();
    }
    let mut out = Vec::new();
    for item in rendered {
        if field_kind_head(item).is_none() {
            out.push(item.clone());
            continue;
        }
        for ty in types {
            out.push(replace_sexp_head(item, ty));
        }
    }
    out
}

fn field_kind_head(item: &str) -> Option<&str> {
    let item = item.strip_prefix('(')?;
    if item.starts_with('#') || item.starts_with('_') || item.starts_with('"') {
        return None;
    }
    let head = item.split([' ', ')']).next()?;
    if head.is_empty() {
        None
    } else {
        Some(head)
    }
}

fn replace_sexp_head(item: &str, head: &str) -> String {
    let Some(rest_at) = item.find([' ', ')']) else {
        return item.to_string();
    };
    format!("({head}{}", &item[rest_at..])
}

pub fn pattern_count(query: &CompiledQuery) -> usize {
    match query {
        CompiledQuery::Never => 0,
        CompiledQuery::Ts(query) => unsafe { ts_query_pattern_count(query.raw) as usize },
    }
}

pub fn rendered_source(query: &CompiledQuery) -> Option<&str> {
    match query {
        CompiledQuery::Never => None,
        CompiledQuery::Ts(query) => Some(query.source.as_str()),
    }
}

pub fn matches(tree: &Tree, query: &CompiledQuery, node: Node) -> Vec<Node> {
    let CompiledQuery::Ts(query) = query else {
        return Vec::new();
    };
    let cursor = unsafe { ts_query_cursor_new() };
    unsafe { ts_query_cursor_exec(cursor, query.raw, node.raw) };
    let mut out = Vec::new();
    let mut match_ = TSQueryMatch {
        id: 0,
        pattern_index: 0,
        capture_count: 0,
        captures: std::ptr::null(),
    };
    while unsafe { ts_query_cursor_next_match(cursor, &mut match_) } {
        if !predicates_hold(tree, query.raw, &match_) {
            continue;
        }
        if match_.capture_count > 0 && !match_.captures.is_null() {
            let capture = unsafe { &*match_.captures };
            out.push(Node { raw: capture.node });
        }
    }
    unsafe { ts_query_cursor_delete(cursor) };
    out
}

fn query_string(query: *const TSQuery, id: u32) -> String {
    let mut len = 0_u32;
    let ptr = unsafe { ts_query_string_value_for_id(query, id, &mut len) };
    if ptr.is_null() {
        return String::new();
    }
    String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) })
        .into_owned()
}

fn match_captures(match_: &TSQueryMatch) -> &[TSQueryCapture] {
    if match_.captures.is_null() || match_.capture_count == 0 {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(match_.captures, match_.capture_count as usize) }
}

fn capture_text<'a>(tree: &'a Tree, match_: &TSQueryMatch, capture_id: u32) -> Option<&'a str> {
    match_captures(match_)
        .iter()
        .find(|capture| capture.index == capture_id)
        .map(|capture| tree.text(Node { raw: capture.node }))
}

fn step_text(
    tree: &Tree,
    query: *const TSQuery,
    match_: &TSQueryMatch,
    step: &TSQueryPredicateStep,
) -> Option<String> {
    match step.type_ {
        TS_QUERY_PREDICATE_STEP_STRING => Some(query_string(query, step.value_id)),
        TS_QUERY_PREDICATE_STEP_CAPTURE => {
            capture_text(tree, match_, step.value_id).map(str::to_string)
        }
        _ => None,
    }
}

fn predicates_hold(tree: &Tree, query: *const TSQuery, match_: &TSQueryMatch) -> bool {
    let mut step_count = 0_u32;
    let steps = unsafe {
        ts_query_predicates_for_pattern(query, u32::from(match_.pattern_index), &mut step_count)
    };
    if steps.is_null() || step_count == 0 {
        return true;
    }
    let steps = unsafe { std::slice::from_raw_parts(steps, step_count as usize) };
    let mut current = Vec::new();
    for step in steps {
        if step.type_ == TS_QUERY_PREDICATE_STEP_DONE {
            if !eval_predicate(tree, query, match_, &current) {
                return false;
            }
            current.clear();
        } else {
            current.push(*step);
        }
    }
    true
}

fn eval_predicate(
    tree: &Tree,
    query: *const TSQuery,
    match_: &TSQueryMatch,
    steps: &[TSQueryPredicateStep],
) -> bool {
    let Some(name_step) = steps.first() else {
        return true;
    };
    if name_step.type_ != TS_QUERY_PREDICATE_STEP_STRING {
        return true;
    }
    let name = query_string(query, name_step.value_id);
    let args = &steps[1..];
    match name.as_str() {
        "eq?" => predicate_eq(tree, query, match_, args, true),
        "not-eq?" => predicate_eq(tree, query, match_, args, false),
        "any-of?" => predicate_any_of(tree, query, match_, args),
        "match?" => predicate_match(tree, query, match_, args),
        _ => true,
    }
}

fn predicate_eq(
    tree: &Tree,
    query: *const TSQuery,
    match_: &TSQueryMatch,
    args: &[TSQueryPredicateStep],
    positive: bool,
) -> bool {
    if args.len() != 2 {
        return false;
    }
    let left = step_text(tree, query, match_, &args[0]);
    let right = step_text(tree, query, match_, &args[1]);
    match (left, right) {
        (Some(left), Some(right)) => (left == right) == positive,
        _ => false,
    }
}

fn predicate_any_of(
    tree: &Tree,
    query: *const TSQuery,
    match_: &TSQueryMatch,
    args: &[TSQueryPredicateStep],
) -> bool {
    let Some((first, rest)) = args.split_first() else {
        return false;
    };
    let Some(left) = step_text(tree, query, match_, first) else {
        return false;
    };
    rest.iter()
        .filter_map(|step| step_text(tree, query, match_, step))
        .any(|value| value == left)
}

fn predicate_match(
    tree: &Tree,
    query: *const TSQuery,
    match_: &TSQueryMatch,
    args: &[TSQueryPredicateStep],
) -> bool {
    if args.len() != 2 {
        return false;
    }
    let Some(text) = step_text(tree, query, match_, &args[0]) else {
        return false;
    };
    let Some(pattern) = step_text(tree, query, match_, &args[1]) else {
        return false;
    };
    regex::Regex::new(&pattern)
        .map(|re| re.is_match(&text))
        .unwrap_or(false)
}

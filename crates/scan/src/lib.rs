//! Lower `scan` rules to a matcher and to Tree-sitter’s five C symbols.

use twigz_ir::{GrammarIr, ScanExpr, ScanRule};

pub const SERIALIZE_CAP: usize = 1024;
const OFFSIDE_STACK: usize = 32;

pub trait Lexer {
    fn lookahead(&self) -> i32;
    fn advance(&mut self, skip: bool);
    fn mark_end(&mut self);
    fn column(&self) -> u32;
    fn eof(&self) -> bool;
}

pub trait Scanner: Send {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid: &[bool]) -> bool;
    fn serialize(&self, buf: &mut [u8]) -> usize;
    fn deserialize(&mut self, buf: &[u8]);
}

#[derive(Clone, Debug)]
pub struct GeneratedScanner {
    pub language: String,
    pub externals: Vec<String>,
    pub kind: MachineKind,
}

#[derive(Clone, Debug)]
pub enum MachineKind {
    LongBracket {
        start: usize,
        content: usize,
        end: usize,
        comment: Option<usize>,
        equals: u8,
        in_string: bool,
    },
    Offside {
        tab_width: u8,
        mixed_tabs: bool,
        spaces_only: bool,
        has_template: bool,
        newline: usize,
        indent: usize,
        dedent: usize,
        interp_open: Option<usize>,
        interp_close: Option<usize>,
        stack: Vec<u8>,
        pending_dedents: u8,
        in_interp: bool,
    },
    SlashTemplate {
        regex: Option<usize>,
        division: Option<usize>,
        open: Option<usize>,
        close: Option<usize>,
        middle: Option<usize>,
        in_interp: bool,
    },
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Matcher {
    Empty,
    Literal(String),
    AnyByte,
    Capture { name: String, inner: Box<Matcher> },
    RepeatKept { literal: String, capture: String },
    Not(Box<Matcher>),
    Symbol(String),
    Choice(Vec<Matcher>),
    Sequence(Vec<Matcher>),
    Repeat(Box<Matcher>),
    Repeat1(Box<Matcher>),
    Optional(Box<Matcher>),
}

impl Matcher {
    fn from_expr(expr: &ScanExpr) -> Self {
        match expr {
            ScanExpr::Empty => Matcher::Empty,
            ScanExpr::Literal(value) => Matcher::Literal(value.clone()),
            ScanExpr::Pattern { value, .. } => Matcher::Literal(value.clone()),
            ScanExpr::AnyByte => Matcher::AnyByte,
            ScanExpr::Capture { name, content } => Matcher::Capture {
                name: name.clone(),
                inner: Box::new(Self::from_expr(content)),
            },
            ScanExpr::RepeatKept { literal, capture } => Matcher::RepeatKept {
                literal: literal.clone(),
                capture: capture.clone(),
            },
            ScanExpr::Not(inner) => Matcher::Not(Box::new(Self::from_expr(inner))),
            ScanExpr::Symbol(name) => Matcher::Symbol(name.clone()),
            ScanExpr::Choice(values) => {
                Matcher::Choice(values.iter().map(Self::from_expr).collect())
            }
            ScanExpr::Sequence(values) => {
                Matcher::Sequence(values.iter().map(Self::from_expr).collect())
            }
            ScanExpr::Repeat(inner) => Matcher::Repeat(Box::new(Self::from_expr(inner))),
            ScanExpr::Repeat1(inner) => Matcher::Repeat1(Box::new(Self::from_expr(inner))),
            ScanExpr::Optional(inner) => Matcher::Optional(Box::new(Self::from_expr(inner))),
        }
    }

    fn seq(&self) -> Vec<&Matcher> {
        match self {
            Matcher::Sequence(values) => values.iter().collect(),
            other => vec![other],
        }
    }
}

fn is_lit(matcher: &Matcher, value: &str) -> bool {
    matches!(matcher, Matcher::Literal(got) if got == value)
}

fn pad_capture<'a>(matcher: &'a Matcher) -> Option<&'a str> {
    match matcher {
        Matcher::Capture { name, inner } => match inner.as_ref() {
            Matcher::Repeat(inner) if is_lit(inner, "=") => Some(name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn is_repeat_kept(matcher: &Matcher, pad: &str) -> bool {
    matches!(
        matcher,
        Matcher::RepeatKept { literal, capture } if literal == "=" && capture == pad
    )
}

fn is_closer_seq(parts: &[&Matcher], pad: &str) -> bool {
    parts.len() == 3
        && is_lit(parts[0], "]")
        && is_repeat_kept(parts[1], pad)
        && is_lit(parts[2], "]")
}

fn is_opener_seq<'a>(parts: &'a [&'a Matcher]) -> Option<&'a str> {
    if parts.len() == 3 && is_lit(parts[0], "[") && is_lit(parts[2], "[") {
        pad_capture(parts[1])
    } else {
        None
    }
}

fn is_start(matcher: &Matcher) -> Option<String> {
    let parts = matcher.seq();
    is_opener_seq(&parts).map(str::to_string)
}

fn is_end(matcher: &Matcher, pad: &str) -> bool {
    is_closer_seq(&matcher.seq(), pad)
}

fn is_content(matcher: &Matcher, end_name: &str, pad: &str) -> bool {
    let Matcher::Repeat1(inner) = matcher else {
        return false;
    };
    let parts = inner.seq();
    if parts.len() != 2 || !matches!(parts[1], Matcher::AnyByte) {
        return false;
    }
    let Matcher::Not(denied) = parts[0] else {
        return false;
    };
    matches!(denied.as_ref(), Matcher::Symbol(name) if name == end_name) || is_end(denied, pad)
}

fn is_comment(matcher: &Matcher, pad: &str) -> bool {
    let parts = matcher.seq();
    if parts.len() < 4 || !is_lit(parts[0], "--") {
        return false;
    }
    let rest = if is_lit(parts[1], "[") {
        &parts[1..]
    } else {
        &parts[1..]
    };
    rest.iter()
        .any(|part| pad_capture(part).is_some_and(|name| name == pad))
        || is_opener_seq(rest).is_some_and(|name| name == pad)
}

struct LongBracketLayout {
    start: usize,
    content: usize,
    end: usize,
    comment: Option<usize>,
}

fn index_of(externals: &[String], name: &str) -> Option<usize> {
    externals.iter().position(|item| item == name)
}

fn required_index(externals: &[String], name: &str, language: &str) -> Result<usize, String> {
    index_of(externals, name).ok_or_else(|| {
        format!("{language}: scan {name} is not listed in externals (declaration order)")
    })
}

fn recognize_long_bracket(
    grammar: &GrammarIr,
    externals: &[String],
) -> Result<LongBracketLayout, String> {
    let mut patterns: Vec<(&str, &ScanExpr, &[String])> = Vec::new();
    for rule in &grammar.scans {
        match rule {
            ScanRule::Pattern {
                name,
                expression,
                keep,
                ..
            } => patterns.push((name, expression, keep)),
            ScanRule::Indent { .. } | ScanRule::Slash { .. } | ScanRule::Template { .. } => {
                return Err(format!(
                    "{}: pattern scan rules cannot mix with named machines",
                    grammar.name
                ));
            }
        }
    }
    if patterns.is_empty() {
        return Err(format!("{}: no pattern scan rules", grammar.name));
    }
    let lowered: Vec<(&str, Matcher, &[String])> = patterns
        .iter()
        .map(|(name, expr, keep)| (*name, Matcher::from_expr(expr), *keep))
        .collect();
    let start = lowered.iter().find(|(_, matcher, keep)| {
        is_start(matcher).is_some_and(|pad| keep.iter().any(|name| name == &pad))
    });
    let Some((start_name, start_matcher, start_keep)) = start else {
        return Err(format!(
            "{}: pattern scans are not the long-bracket machine (missing `[` pad:\"=\"* `[` keep)",
            grammar.name
        ));
    };
    let pad = is_start(start_matcher).expect("classified as start");
    if start_keep.iter().all(|name| name != &pad) {
        return Err(format!(
            "{}: long-bracket start must keep capture {pad}",
            grammar.name
        ));
    }
    let end = lowered
        .iter()
        .find(|(name, matcher, _)| *name != *start_name && is_end(matcher, &pad));
    let Some((end_name, _, _)) = end else {
        return Err(format!(
            "{}: pattern scans are not the long-bracket machine (missing `]` \"=\"{{{pad}}} `]`)",
            grammar.name
        ));
    };
    let content = lowered.iter().find(|(name, matcher, _)| {
        *name != *start_name && *name != *end_name && is_content(matcher, end_name, &pad)
    });
    let Some((content_name, _, _)) = content else {
        return Err(format!(
            "{}: pattern scans are not the long-bracket machine (missing (!{end_name} .)+)",
            grammar.name
        ));
    };
    let mut comment = None;
    for (name, matcher, _) in &lowered {
        if *name == *start_name || *name == *end_name || *name == *content_name {
            continue;
        }
        if is_comment(matcher, &pad) {
            if comment.is_some() {
                return Err(format!(
                    "{}: extra pattern scan {name} is not part of the long-bracket machine",
                    grammar.name
                ));
            }
            comment = Some(*name);
            continue;
        }
        return Err(format!(
            "{}: extra pattern scan {name} is not the long-bracket machine this work implements",
            grammar.name
        ));
    }
    Ok(LongBracketLayout {
        start: required_index(externals, start_name, &grammar.name)?,
        content: required_index(externals, content_name, &grammar.name)?,
        end: required_index(externals, end_name, &grammar.name)?,
        comment: comment
            .map(|name| required_index(externals, name, &grammar.name))
            .transpose()?,
    })
}

fn external_names(grammar: &GrammarIr) -> Vec<String> {
    grammar
        .externals
        .iter()
        .filter_map(|rule| match rule {
            twigz_ir::Rule::Symbol(name) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

impl GeneratedScanner {
    pub fn from_grammar(grammar: &GrammarIr) -> Result<Self, String> {
        let externals = external_names(grammar);
        let has_pattern = grammar
            .scans
            .iter()
            .any(|rule| matches!(rule, ScanRule::Pattern { .. }));
        let kind = if has_pattern {
            let layout = recognize_long_bracket(grammar, &externals)?;
            MachineKind::LongBracket {
                start: layout.start,
                content: layout.content,
                end: layout.end,
                comment: layout.comment,
                equals: 0,
                in_string: false,
            }
        } else if let Some(ScanRule::Indent {
            tab_width,
            mixed,
            newline,
            indent,
            dedent,
            ..
        }) = grammar
            .scans
            .iter()
            .find(|rule| matches!(rule, ScanRule::Indent { .. }))
        {
            let mut interp_open = None;
            let mut interp_close = None;
            for rule in &grammar.scans {
                if let ScanRule::Template { open, close, .. } = rule {
                    interp_open = Some(required_index(&externals, open, &grammar.name)?);
                    interp_close = Some(required_index(&externals, close, &grammar.name)?);
                }
            }
            MachineKind::Offside {
                tab_width: *tab_width,
                mixed_tabs: mixed == "tab_is_n_spaces",
                spaces_only: mixed == "error",
                has_template: interp_open.is_some(),
                newline: required_index(&externals, newline, &grammar.name)?,
                indent: required_index(&externals, indent, &grammar.name)?,
                dedent: required_index(&externals, dedent, &grammar.name)?,
                interp_open,
                interp_close,
                stack: vec![0],
                pending_dedents: 0,
                in_interp: false,
            }
        } else if grammar
            .scans
            .iter()
            .any(|rule| matches!(rule, ScanRule::Slash { .. } | ScanRule::Template { .. }))
        {
            let mut regex = None;
            let mut division = None;
            let mut open = None;
            let mut close = None;
            let mut middle = None;
            for rule in &grammar.scans {
                match rule {
                    ScanRule::Slash {
                        regex: regex_name,
                        division: division_name,
                    } => {
                        regex = Some(required_index(&externals, regex_name, &grammar.name)?);
                        division = Some(required_index(&externals, division_name, &grammar.name)?);
                    }
                    ScanRule::Template {
                        open: open_name,
                        close: close_name,
                        chunk,
                    } => {
                        open = Some(required_index(&externals, open_name, &grammar.name)?);
                        close = Some(required_index(&externals, close_name, &grammar.name)?);
                        middle = chunk
                            .as_ref()
                            .map(|name| required_index(&externals, name, &grammar.name))
                            .transpose()?;
                    }
                    _ => {}
                }
            }
            MachineKind::SlashTemplate {
                regex,
                division,
                open,
                close,
                middle,
                in_interp: false,
            }
        } else if externals.is_empty() {
            MachineKind::Empty
        } else {
            return Err(format!("{}: externals require scan rules", grammar.name));
        };
        Ok(Self {
            language: grammar.name.clone(),
            externals,
            kind,
        })
    }
}

impl Scanner for GeneratedScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid: &[bool]) -> bool {
        match &mut self.kind {
            MachineKind::LongBracket {
                start,
                content,
                end,
                comment,
                equals,
                in_string,
            } => scan_long_bracket(
                lexer, valid, *start, *content, *end, *comment, equals, in_string,
            ),
            MachineKind::Offside { .. } => scan_offside(&mut self.kind, lexer, valid),
            MachineKind::SlashTemplate { .. } => scan_slash_template(&mut self.kind, lexer, valid),
            MachineKind::Empty => false,
        }
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        if buf.is_empty() {
            return 0;
        }
        let cap = buf.len().min(SERIALIZE_CAP);
        match &self.kind {
            MachineKind::LongBracket {
                equals, in_string, ..
            } => {
                if cap < 2 {
                    0
                } else {
                    buf[0] = *equals;
                    buf[1] = u8::from(*in_string);
                    2
                }
            }
            MachineKind::Offside {
                stack,
                in_interp,
                has_template,
                ..
            } => serialize_offside(buf, cap, stack, *in_interp, *has_template),
            MachineKind::SlashTemplate { in_interp, .. } => {
                buf[0] = 0;
                buf[1] = 0;
                if cap > 2 {
                    buf[2] = u8::from(*in_interp);
                    3
                } else {
                    0
                }
            }
            MachineKind::Empty => 0,
        }
    }

    fn deserialize(&mut self, buf: &[u8]) {
        match &mut self.kind {
            MachineKind::LongBracket {
                equals, in_string, ..
            } => {
                *equals = 0;
                *in_string = false;
                if buf.len() >= 2 {
                    *equals = buf[0];
                    *in_string = buf[1] != 0;
                }
            }
            MachineKind::Offside {
                stack,
                in_interp,
                has_template,
                pending_dedents,
                ..
            } => {
                *pending_dedents = 0;
                if buf.len() < 2 {
                    *stack = vec![0];
                    *in_interp = false;
                    return;
                }
                let mut len = buf[1] as usize;
                if len > OFFSIDE_STACK {
                    len = OFFSIDE_STACK;
                }
                *in_interp = *has_template && buf.get(2).copied().unwrap_or(0) != 0;
                let start = if *has_template { 3 } else { 2 };
                if buf.len() < start + len {
                    *stack = vec![0];
                    *in_interp = false;
                    return;
                }
                *stack = buf[start..start + len].to_vec();
                if stack.is_empty() {
                    *stack = vec![0];
                }
            }
            MachineKind::SlashTemplate { in_interp, .. } => {
                *in_interp = buf.get(2).copied().unwrap_or(0) != 0;
            }
            MachineKind::Empty => {}
        }
    }
}

impl GeneratedScanner {
    pub fn serialize_into(&self, buf: &mut [u8]) -> usize {
        Scanner::serialize(self, buf)
    }

    pub fn deserialize_from(&mut self, buf: &[u8]) {
        Scanner::deserialize(self, buf)
    }
}

fn advance(lexer: &mut dyn Lexer) {
    lexer.advance(false);
}

fn opener(lexer: &mut dyn Lexer) -> Option<u8> {
    if lexer.lookahead() != i32::from(b'[') {
        return None;
    }
    advance(lexer);
    let mut equals = 0_u8;
    while lexer.lookahead() == i32::from(b'=') {
        if equals == 255 {
            return None;
        }
        equals += 1;
        advance(lexer);
    }
    if lexer.lookahead() != i32::from(b'[') {
        return None;
    }
    advance(lexer);
    Some(equals)
}

fn closer(lexer: &mut dyn Lexer, equals: u8) -> bool {
    if lexer.lookahead() != i32::from(b']') {
        return false;
    }
    advance(lexer);
    let mut seen = 0_u8;
    while seen < equals && lexer.lookahead() == i32::from(b'=') {
        seen += 1;
        advance(lexer);
    }
    if seen != equals || lexer.lookahead() != i32::from(b']') {
        return false;
    }
    advance(lexer);
    true
}

fn valid_at(valid: &[bool], index: usize) -> bool {
    valid.get(index).copied().unwrap_or(false)
}

fn scan_long_bracket(
    lexer: &mut dyn Lexer,
    valid: &[bool],
    start: usize,
    content: usize,
    end: usize,
    comment: Option<usize>,
    equals: &mut u8,
    in_string: &mut bool,
) -> bool {
    if let Some(comment) = comment {
        if valid_at(valid, comment) && lexer.lookahead() == i32::from(b'-') {
            advance(lexer);
            if lexer.lookahead() != i32::from(b'-') {
                return false;
            }
            advance(lexer);
            let Some(eq) = opener(lexer) else {
                return false;
            };
            while !lexer.eof() && lexer.lookahead() != 0 {
                if closer(lexer, eq) {
                    break;
                }
                advance(lexer);
            }
            return true;
        }
    }
    if valid_at(valid, start) && !*in_string {
        let Some(eq) = opener(lexer) else {
            return false;
        };
        *equals = eq;
        *in_string = true;
        return true;
    }
    if *in_string && valid_at(valid, end) && closer(lexer, *equals) {
        *in_string = false;
        return true;
    }
    if *in_string && valid_at(valid, content) {
        let mut consumed = false;
        while !lexer.eof() && lexer.lookahead() != 0 {
            lexer.mark_end();
            if closer(lexer, *equals) {
                break;
            }
            advance(lexer);
            consumed = true;
        }
        return consumed;
    }
    false
}

fn serialize_offside(
    buf: &mut [u8],
    cap: usize,
    stack: &[u8],
    in_interp: bool,
    has_template: bool,
) -> usize {
    if cap < 2 {
        return 0;
    }
    buf[0] = 0;
    let stack_len = stack.len().min(OFFSIDE_STACK);
    buf[1] = u8::try_from(stack_len).unwrap_or(0);
    let mut used = 2;
    if has_template {
        if cap < 3 {
            return 0;
        }
        buf[2] = u8::from(in_interp);
        used = 3;
    }
    let copy = stack_len.min(cap.saturating_sub(used));
    buf[used..used + copy].copy_from_slice(&stack[..copy]);
    (used + copy).min(SERIALIZE_CAP)
}

fn is_newline(value: i32) -> bool {
    value == i32::from(b'\n')
}

fn scan_offside(kind: &mut MachineKind, lexer: &mut dyn Lexer, valid: &[bool]) -> bool {
    let MachineKind::Offside {
        tab_width,
        mixed_tabs,
        spaces_only,
        has_template,
        newline,
        indent,
        dedent,
        interp_open,
        interp_close,
        stack,
        pending_dedents,
        in_interp,
    } = kind
    else {
        return false;
    };
    if *has_template
        && scan_template_pair(lexer, valid, in_interp, *interp_open, *interp_close, None)
    {
        return true;
    }
    if *pending_dedents > 0 && valid_at(valid, *dedent) {
        *pending_dedents -= 1;
        if stack.len() > 1 {
            stack.pop();
        }
        if stack.is_empty() {
            stack.push(0);
        }
        return true;
    }
    if lexer.eof() || lexer.lookahead() == 0 {
        if stack.last().copied().unwrap_or(0) > 0 && valid_at(valid, *dedent) {
            stack.pop();
            if stack.is_empty() {
                stack.push(0);
            }
            return true;
        }
        return false;
    }
    if !is_newline(lexer.lookahead()) && lexer.lookahead() != i32::from(b'\r') {
        return false;
    }
    if lexer.lookahead() == i32::from(b'\r') {
        advance(lexer);
        if is_newline(lexer.lookahead()) {
            advance(lexer);
        }
    } else {
        advance(lexer);
    }
    let mut n = 0_u32;
    loop {
        match lexer.lookahead() {
            value if value == i32::from(b' ') => {
                n += 1;
                advance(lexer);
            }
            value if value == i32::from(b'\t') => {
                if *spaces_only {
                    return false;
                }
                if !*mixed_tabs {
                    return false;
                }
                n += u32::from(*tab_width);
                advance(lexer);
            }
            _ => break,
        }
    }
    if lexer.lookahead() == 0
        || is_newline(lexer.lookahead())
        || lexer.lookahead() == i32::from(b'#')
    {
        lexer.mark_end();
        return valid_at(valid, *newline);
    }
    lexer.mark_end();
    let n = u8::try_from(n).unwrap_or(u8::MAX);
    let top = stack.last().copied().unwrap_or(0);
    if n > top {
        if !valid_at(valid, *indent) || stack.len() >= OFFSIDE_STACK {
            return false;
        }
        stack.push(n);
        return true;
    }
    if n == top {
        return valid_at(valid, *newline);
    }
    if !valid_at(valid, *dedent) {
        return false;
    }
    if !stack.contains(&n) {
        return false;
    }
    stack.pop();
    while stack.last().copied().unwrap_or(0) > n {
        *pending_dedents += 1;
        stack.pop();
    }
    if stack.last().copied() != Some(n) {
        return false;
    }
    true
}

fn scan_template_pair(
    lexer: &mut dyn Lexer,
    valid: &[bool],
    in_interp: &mut bool,
    open: Option<usize>,
    close: Option<usize>,
    middle: Option<usize>,
) -> bool {
    if !*in_interp {
        if let Some(open) = open {
            if valid_at(valid, open) && lexer.lookahead() == i32::from(b'`') {
                advance(lexer);
                loop {
                    if lexer.eof() || lexer.lookahead() == 0 {
                        lexer.mark_end();
                        *in_interp = true;
                        return true;
                    }
                    if lexer.lookahead() == i32::from(b'`') {
                        advance(lexer);
                        lexer.mark_end();
                        return true;
                    }
                    if lexer.lookahead() == i32::from(b'$') {
                        advance(lexer);
                        if lexer.lookahead() == i32::from(b'{') {
                            advance(lexer);
                            lexer.mark_end();
                            *in_interp = true;
                            return true;
                        }
                        continue;
                    }
                    if lexer.lookahead() == i32::from(b'\\') {
                        advance(lexer);
                    }
                    advance(lexer);
                }
            }
        }
        return false;
    }
    if lexer.lookahead() != i32::from(b'}') {
        return false;
    }
    advance(lexer);
    loop {
        if lexer.eof() || lexer.lookahead() == 0 {
            lexer.mark_end();
            *in_interp = false;
            return close.is_some_and(|index| valid_at(valid, index));
        }
        if lexer.lookahead() == i32::from(b'`') {
            advance(lexer);
            lexer.mark_end();
            *in_interp = false;
            return close.is_some_and(|index| valid_at(valid, index));
        }
        if lexer.lookahead() == i32::from(b'$') {
            advance(lexer);
            if lexer.lookahead() == i32::from(b'{') {
                advance(lexer);
                lexer.mark_end();
                return middle.is_some_and(|index| valid_at(valid, index));
            }
            continue;
        }
        if lexer.lookahead() == i32::from(b'\\') {
            advance(lexer);
        }
        advance(lexer);
    }
}

fn scan_slash_template(kind: &mut MachineKind, lexer: &mut dyn Lexer, valid: &[bool]) -> bool {
    let MachineKind::SlashTemplate {
        regex,
        division,
        open,
        close,
        middle,
        in_interp,
    } = kind
    else {
        return false;
    };
    if scan_template_pair(lexer, valid, in_interp, *open, *close, *middle) {
        return true;
    }
    if lexer.lookahead() != i32::from(b'/') {
        return false;
    }
    advance(lexer);
    // Recovery may mark division/regex valid; extras must still see comments.
    if lexer.lookahead() == i32::from(b'/') || lexer.lookahead() == i32::from(b'*') {
        return false;
    }
    if let Some(division) = *division {
        if valid_at(valid, division) {
            lexer.mark_end();
            return true;
        }
    }
    if let Some(regex) = *regex {
        if valid_at(valid, regex) && !division.is_some_and(|index| valid_at(valid, index)) {
            let mut in_class = false;
            loop {
                if lexer.eof() || lexer.lookahead() == 0 || is_newline(lexer.lookahead()) {
                    lexer.mark_end();
                    return true;
                }
                let value = lexer.lookahead();
                if value == i32::from(b'\\') {
                    advance(lexer);
                    advance(lexer);
                    continue;
                }
                if value == i32::from(b'[') {
                    in_class = true;
                } else if value == i32::from(b']') {
                    in_class = false;
                } else if value == i32::from(b'/') && !in_class {
                    advance(lexer);
                    while lexer.lookahead() >= i32::from(b'a')
                        && lexer.lookahead() <= i32::from(b'z')
                    {
                        advance(lexer);
                    }
                    lexer.mark_end();
                    return true;
                }
                advance(lexer);
            }
        }
    }
    false
}

pub fn emit_c(grammar: &GrammarIr) -> Result<String, String> {
    let scanner = GeneratedScanner::from_grammar(grammar)?;
    Ok(emit_c_for(&scanner))
}

fn token_enum(externals: &[String]) -> String {
    format!("enum TokenType {{ {} }};", externals.join(", "))
}

fn emit_c_for(scanner: &GeneratedScanner) -> String {
    let lang = &scanner.language;
    let mut out = String::new();
    out.push_str("/* @generated by twigz; do not edit. */\n");
    out.push_str("#include \"parser.h\"\n#include <stdlib.h>\n#include <string.h>\n#include <stdbool.h>\n#include <stdint.h>\n\n");
    match &scanner.kind {
        MachineKind::LongBracket {
            start,
            content,
            end,
            comment,
            ..
        } => out.push_str(&long_bracket_c(
            lang,
            &scanner.externals,
            *start,
            *content,
            *end,
            *comment,
        )),
        MachineKind::Offside {
            tab_width,
            mixed_tabs,
            spaces_only,
            has_template,
            newline,
            indent,
            dedent,
            interp_open,
            interp_close,
            ..
        } => out.push_str(&offside_c(
            lang,
            &scanner.externals,
            *tab_width,
            *mixed_tabs,
            *spaces_only,
            *has_template,
            &scanner.externals[*newline],
            &scanner.externals[*indent],
            &scanner.externals[*dedent],
            interp_open.map(|index| scanner.externals[index].as_str()),
            interp_close.map(|index| scanner.externals[index].as_str()),
        )),
        MachineKind::SlashTemplate {
            regex,
            division,
            open,
            close,
            middle,
            ..
        } => out.push_str(&slash_template_c(
            lang,
            &scanner.externals,
            regex.map(|index| scanner.externals[index].as_str()),
            division.map(|index| scanner.externals[index].as_str()),
            open.map(|index| scanner.externals[index].as_str()),
            close.map(|index| scanner.externals[index].as_str()),
            middle.map(|index| scanner.externals[index].as_str()),
        )),
        MachineKind::Empty => {
            out.push_str(&format!(
                "void *tree_sitter_{lang}_external_scanner_create(void) {{ return NULL; }}\n\
                 void tree_sitter_{lang}_external_scanner_destroy(void *p) {{ (void)p; }}\n\
                 bool tree_sitter_{lang}_external_scanner_scan(void *p, TSLexer *l, const bool *v) {{ (void)p; (void)l; (void)v; return false; }}\n\
                 unsigned tree_sitter_{lang}_external_scanner_serialize(void *p, char *b) {{ (void)p; (void)b; return 0; }}\n\
                 void tree_sitter_{lang}_external_scanner_deserialize(void *p, const char *b, unsigned n) {{ (void)p; (void)b; (void)n; }}\n"
            ));
        }
    }
    out
}

fn long_bracket_c(
    lang: &str,
    externals: &[String],
    start: usize,
    content: usize,
    end: usize,
    comment: Option<usize>,
) -> String {
    let start_tok = &externals[start];
    let content_tok = &externals[content];
    let end_tok = &externals[end];
    let comment_block = if let Some(comment) = comment {
        let comment_tok = &externals[comment];
        format!(
            r#"
  if (valid[{comment_tok}] && lexer->lookahead == '-') {{
    adv(lexer);
    if (lexer->lookahead != '-') return false;
    adv(lexer);
    uint8_t eq = 0;
    if (!opener(lexer, &eq)) return false;
    while (!done(lexer)) {{
      if (closer(lexer, eq)) {{ lexer->result_symbol = {comment_tok}; return true; }}
      if (!bump(lexer)) break;
    }}
    lexer->result_symbol = {comment_tok};
    return true;
  }}"#
        )
    } else {
        String::new()
    };
    format!(
        r#"{enum_body}
typedef struct {{ uint8_t equals; bool in_string; }} Scanner;
static void adv(TSLexer *lexer) {{ lexer->advance(lexer, false); }}
static bool done(TSLexer *lexer) {{ return lexer->lookahead == 0 || lexer->eof(lexer); }}
static bool bump(TSLexer *lexer) {{
  int32_t before = lexer->lookahead;
  lexer->advance(lexer, false);
  return lexer->lookahead != before || lexer->eof(lexer) || lexer->lookahead == 0;
}}
static bool opener(TSLexer *lexer, uint8_t *equals) {{
  if (lexer->lookahead != '[') return false;
  adv(lexer);
  *equals = 0;
  while (lexer->lookahead == '=') {{
    if (*equals == 255) return false;
    (*equals)++;
    adv(lexer);
  }}
  if (lexer->lookahead != '[') return false;
  adv(lexer);
  return true;
}}
static bool closer(TSLexer *lexer, uint8_t equals) {{
  if (lexer->lookahead != ']') return false;
  adv(lexer);
  uint8_t seen = 0;
  while (seen < equals && lexer->lookahead == '=') {{ seen++; adv(lexer); }}
  if (seen != equals || lexer->lookahead != ']') return false;
  adv(lexer);
  return true;
}}
void *tree_sitter_{lang}_external_scanner_create(void) {{
  return calloc(1, sizeof(Scanner));
}}
void tree_sitter_{lang}_external_scanner_destroy(void *p) {{ free(p); }}
unsigned tree_sitter_{lang}_external_scanner_serialize(void *p, char *buf) {{
  Scanner *s = (Scanner *)p;
  if (!s) return 0;
  buf[0] = (char)s->equals;
  buf[1] = s->in_string ? 1 : 0;
  return 2;
}}
void tree_sitter_{lang}_external_scanner_deserialize(void *p, const char *buf, unsigned len) {{
  Scanner *s = (Scanner *)p;
  if (!s) return;
  s->equals = 0;
  s->in_string = false;
  if (len >= 2) {{ s->equals = (uint8_t)buf[0]; s->in_string = buf[1] != 0; }}
}}
bool tree_sitter_{lang}_external_scanner_scan(void *p, TSLexer *lexer, const bool *valid) {{
  Scanner *s = (Scanner *)p;
  if (!s) return false;{comment_block}
  if (valid[{start_tok}] && !s->in_string) {{
    uint8_t eq = 0;
    if (!opener(lexer, &eq)) return false;
    s->equals = eq;
    s->in_string = true;
    lexer->result_symbol = {start_tok};
    return true;
  }}
  if (s->in_string && valid[{end_tok}] && closer(lexer, s->equals)) {{
    s->in_string = false;
    lexer->result_symbol = {end_tok};
    return true;
  }}
  if (s->in_string && valid[{content_tok}]) {{
    bool consumed = false;
    while (!done(lexer)) {{
      lexer->mark_end(lexer);
      if (closer(lexer, s->equals)) break;
      if (!bump(lexer)) break;
      consumed = true;
    }}
    if (consumed) {{ lexer->result_symbol = {content_tok}; return true; }}
  }}
  return false;
}}
"#,
        enum_body = token_enum(externals),
        lang = lang,
        start_tok = start_tok,
        content_tok = content_tok,
        end_tok = end_tok,
        comment_block = comment_block,
    )
}

fn offside_c(
    lang: &str,
    externals: &[String],
    tab_width: u8,
    mixed_tabs: bool,
    spaces_only: bool,
    has_template: bool,
    newline: &str,
    indent: &str,
    dedent: &str,
    interp_open: Option<&str>,
    interp_close: Option<&str>,
) -> String {
    let template_scan = match (has_template, interp_open, interp_close) {
        (true, Some(open), Some(close)) => format!(
            r#"
  if (!s->in_interp && valid[{open}] && lexer->lookahead == '`') {{
    adv(lexer);
    while (!done(lexer)) {{
      if (lexer->lookahead == '`') {{ adv(lexer); lexer->result_symbol = {open}; return true; }}
      if (lexer->lookahead == '$') {{
        adv(lexer);
        if (lexer->lookahead == '{{') {{ adv(lexer); s->in_interp = true; lexer->result_symbol = {open}; return true; }}
        continue;
      }}
      if (lexer->lookahead == 92) adv(lexer);
      adv(lexer);
    }}
    lexer->result_symbol = {open}; return true;
  }}
  if (s->in_interp && valid[{close}] && lexer->lookahead == '}}') {{
    adv(lexer);
    while (!done(lexer) && lexer->lookahead != '`') {{
      if (lexer->lookahead == 92) adv(lexer);
      adv(lexer);
    }}
    if (lexer->lookahead == '`') adv(lexer);
    s->in_interp = false;
    lexer->result_symbol = {close};
    return true;
  }}"#
        ),
        _ => String::new(),
    };
    format!(
        r#"{enum_body}
typedef struct {{ uint8_t stack[32]; uint8_t len; uint8_t pending; bool in_interp; }} Scanner;
static void adv(TSLexer *lexer) {{ lexer->advance(lexer, false); }}
static bool done(TSLexer *lexer) {{ return lexer->lookahead == 0 || lexer->eof(lexer); }}
static bool bump(TSLexer *lexer) {{
  int32_t before = lexer->lookahead;
  lexer->advance(lexer, false);
  return lexer->lookahead != before || lexer->eof(lexer) || lexer->lookahead == 0;
}}
void *tree_sitter_{lang}_external_scanner_create(void) {{
  Scanner *s = (Scanner *)calloc(1, sizeof(Scanner));
  if (!s) return NULL;
  s->stack[0] = 0; s->len = 1; return s;
}}
void tree_sitter_{lang}_external_scanner_destroy(void *p) {{ free(p); }}
unsigned tree_sitter_{lang}_external_scanner_serialize(void *p, char *buf) {{
  Scanner *s = (Scanner *)p;
  if (!s) return 0;
  buf[0] = 0;
  buf[1] = (char)s->len;
  unsigned used = 2;
  if ({has_template}) {{ buf[2] = s->in_interp ? 1 : 0; used = 3; }}
  unsigned copy = s->len;
  if (used + copy > 1024) copy = 1024 - used;
  memcpy(buf + used, s->stack, copy);
  used += copy;
  return used > 1024 ? 1024 : used;
}}
void tree_sitter_{lang}_external_scanner_deserialize(void *p, const char *buf, unsigned len) {{
  Scanner *s = (Scanner *)p;
  if (!s) return;
  s->pending = 0; s->in_interp = false; s->stack[0] = 0; s->len = 1;
  if (len < 2) return;
  uint8_t n = (uint8_t)buf[1];
  if (n > 32) n = 32;
  unsigned start = {has_template} ? 3 : 2;
  if ({has_template} && len >= 3) s->in_interp = buf[2] != 0;
  if (len < start + n) return;
  memcpy(s->stack, buf + start, n);
  s->len = n ? n : 1;
}}
bool tree_sitter_{lang}_external_scanner_scan(void *p, TSLexer *lexer, const bool *valid) {{
  Scanner *s = (Scanner *)p;
  if (!s) return false;
  {template_scan}
  if (s->pending && valid[{dedent}]) {{ s->pending--; if (s->len > 1) s->len--; lexer->result_symbol = {dedent}; return true; }}
  if (lexer->lookahead == 0) {{
    if (s->len > 1 && valid[{dedent}]) {{ s->len--; lexer->result_symbol = {dedent}; return true; }}
    return false;
  }}
  if (lexer->lookahead != 10 && lexer->lookahead != 13) return false;
  if (lexer->lookahead == 13) {{ adv(lexer); if (lexer->lookahead == 10) adv(lexer); }} else adv(lexer);
  unsigned n = 0;
  for (;;) {{
    if (lexer->lookahead == ' ') {{ n++; adv(lexer); }}
    else if (lexer->lookahead == 9) {{
      if ({spaces_only}) return false;
      if (!{mixed}) return false;
      n += {tab_width};
      adv(lexer);
    }} else break;
  }}
  if (lexer->lookahead == 0 || lexer->lookahead == 10 || lexer->lookahead == '#') {{
    lexer->mark_end(lexer);
    if (!valid[{newline}]) return false;
    lexer->result_symbol = {newline};
    return true;
  }}
  lexer->mark_end(lexer);
  uint8_t top = s->stack[s->len - 1];
  if (n > top) {{
    if (!valid[{indent}] || s->len >= 32) return false;
    s->stack[s->len++] = (uint8_t)n;
    lexer->result_symbol = {indent};
    return true;
  }}
  if (n == top) {{
    if (!valid[{newline}]) return false;
    lexer->result_symbol = {newline};
    return true;
  }}
  if (!valid[{dedent}]) return false;
  bool found = false;
  for (uint8_t i = 0; i < s->len; i++) if (s->stack[i] == (uint8_t)n) found = true;
  if (!found) return false;
  s->len--;
  while (s->len > 0 && s->stack[s->len - 1] > (uint8_t)n) {{ s->pending++; s->len--; }}
  lexer->result_symbol = {dedent};
  return true;
}}
"#,
        enum_body = token_enum(externals),
        lang = lang,
        tab_width = tab_width,
        mixed = if mixed_tabs { "true" } else { "false" },
        spaces_only = if spaces_only { "true" } else { "false" },
        has_template = if has_template { "true" } else { "false" },
        template_scan = template_scan,
        newline = newline,
        indent = indent,
        dedent = dedent,
    )
}

fn slash_template_c(
    lang: &str,
    externals: &[String],
    regex: Option<&str>,
    division: Option<&str>,
    open: Option<&str>,
    close: Option<&str>,
    middle: Option<&str>,
) -> String {
    let open = open.unwrap_or("template_head");
    let close = close.unwrap_or("template_tail");
    let middle = middle.unwrap_or("template_middle");
    let regex = regex.unwrap_or("regex");
    let division = division.unwrap_or("division");
    let has_division = externals.iter().any(|name| name == division);
    let has_regex = externals.iter().any(|name| name == regex);
    format!(
        r#"{enum_body}
typedef struct {{ bool in_interp; }} Scanner;
static void adv(TSLexer *lexer) {{ lexer->advance(lexer, false); }}
static bool done(TSLexer *lexer) {{ return lexer->lookahead == 0 || lexer->eof(lexer); }}
static bool bump(TSLexer *lexer) {{
  int32_t before = lexer->lookahead;
  lexer->advance(lexer, false);
  return lexer->lookahead != before || lexer->eof(lexer) || lexer->lookahead == 0;
}}
void *tree_sitter_{lang}_external_scanner_create(void) {{ return calloc(1, sizeof(Scanner)); }}
void tree_sitter_{lang}_external_scanner_destroy(void *p) {{ free(p); }}
unsigned tree_sitter_{lang}_external_scanner_serialize(void *p, char *buf) {{
  Scanner *s = (Scanner *)p;
  if (!s) return 0;
  buf[0] = 0; buf[1] = 0; buf[2] = s->in_interp ? 1 : 0; return 3;
}}
void tree_sitter_{lang}_external_scanner_deserialize(void *p, const char *buf, unsigned len) {{
  Scanner *s = (Scanner *)p;
  if (!s) return;
  s->in_interp = len >= 3 && buf[2] != 0;
}}
bool tree_sitter_{lang}_external_scanner_scan(void *p, TSLexer *lexer, const bool *valid) {{
  Scanner *s = (Scanner *)p;
  if (!s) return false;
  if (!s->in_interp && valid[{open}] && lexer->lookahead == '`') {{
    adv(lexer);
    while (!done(lexer)) {{
      if (lexer->lookahead == '`') {{ adv(lexer); lexer->result_symbol = {open}; return true; }}
      if (lexer->lookahead == '$') {{
        adv(lexer);
        if (lexer->lookahead == '{{') {{ adv(lexer); s->in_interp = true; lexer->result_symbol = {open}; return true; }}
        continue;
      }}
      if (lexer->lookahead == 92) adv(lexer);
      adv(lexer);
    }}
    lexer->result_symbol = {open}; return true;
  }}
  if (s->in_interp && lexer->lookahead == '}}') {{
    adv(lexer);
    while (!done(lexer)) {{
      if (lexer->lookahead == '`') {{
        adv(lexer); s->in_interp = false;
        if (valid[{close}]) {{ lexer->result_symbol = {close}; return true; }}
        return false;
      }}
      if (lexer->lookahead == '$') {{
        adv(lexer);
        if (lexer->lookahead == '{{') {{
          adv(lexer);
          if (valid[{middle}]) {{ lexer->result_symbol = {middle}; return true; }}
          return false;
        }}
        continue;
      }}
      if (lexer->lookahead == 92) adv(lexer);
      adv(lexer);
    }}
    s->in_interp = false;
    if (valid[{close}]) {{ lexer->result_symbol = {close}; return true; }}
  }}
  if (lexer->lookahead != '/') return false;
  adv(lexer);
  /* Recovery may mark division/regex valid; extras must still see comments. */
  if (lexer->lookahead == '/' || lexer->lookahead == '*') return false;
  if ({has_division} && valid[{division}]) {{
    lexer->result_symbol = {division};
    return true;
  }}
  if ({has_regex} && valid[{regex}] && !({has_division} && valid[{division}])) {{
    bool in_class = false;
    while (!done(lexer) && lexer->lookahead != 10) {{
      if (lexer->lookahead == 92) {{ adv(lexer); adv(lexer); continue; }}
      if (lexer->lookahead == '[') in_class = true;
      else if (lexer->lookahead == ']') in_class = false;
      else if (lexer->lookahead == '/' && !in_class) {{
        adv(lexer);
        while (lexer->lookahead >= 'a' && lexer->lookahead <= 'z') adv(lexer);
        lexer->result_symbol = {regex};
        return true;
      }}
      adv(lexer);
    }}
    lexer->result_symbol = {regex};
    return true;
  }}
  return false;
}}
"#,
        enum_body = token_enum(externals),
        lang = lang,
        has_division = if has_division { "true" } else { "false" },
        has_regex = if has_regex { "true" } else { "false" },
        open = open,
        close = close,
        middle = middle,
        regex = regex,
        division = division,
    )
}

pub struct MockLexer {
    pub source: Vec<u8>,
    pub at: usize,
    pub marked: usize,
    pub column: u32,
}

impl MockLexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.as_bytes().to_vec(),
            at: 0,
            marked: 0,
            column: 0,
        }
    }
}

impl Lexer for MockLexer {
    fn lookahead(&self) -> i32 {
        self.source
            .get(self.at)
            .copied()
            .map(i32::from)
            .unwrap_or(0)
    }
    fn advance(&mut self, _skip: bool) {
        if self.at < self.source.len() {
            if self.source[self.at] == b'\n' {
                self.column = 0;
            } else {
                self.column += 1;
            }
            self.at += 1;
        }
    }
    fn mark_end(&mut self) {
        self.marked = self.at;
    }
    fn column(&self) -> u32 {
        self.column
    }
    fn eof(&self) -> bool {
        self.at >= self.source.len()
    }
}

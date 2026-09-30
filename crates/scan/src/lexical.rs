//! C helpers for `scan lexical lua|luau` on the long-bracket scanner.

pub fn helpers(language: &str) -> Option<String> {
    let specific = match language {
        "lua" => LUA,
        "luau" => LUAU,
        _ => return None,
    };
    let mut out = String::with_capacity(SHARED.len() + specific.len());
    out.push_str(SHARED);
    out.push_str(specific);
    Some(out)
}

pub const SKELETON: &str = r#"@@ENUM@@
typedef struct { uint8_t equals; bool in_string; uint8_t depth; } Scanner;
static void adv(TSLexer *lexer) { lexer->advance(lexer, false); }
static bool done(TSLexer *lexer) { return lexer->lookahead == 0 || lexer->eof(lexer); }
static bool opener(TSLexer *lexer, uint8_t *equals) {
  if (lexer->lookahead != '[') return false;
  adv(lexer);
  *equals = 0;
  while (lexer->lookahead == '=') {
    if (*equals == 255) return false;
    (*equals)++;
    adv(lexer);
  }
  if (lexer->lookahead != '[') return false;
  adv(lexer);
  return true;
}
static bool closer(TSLexer *lexer, uint8_t equals) {
  if (lexer->lookahead != ']') return false;
  adv(lexer);
  uint8_t seen = 0;
  while (seen < equals && lexer->lookahead == '=') { seen++; adv(lexer); }
  if (seen != equals || lexer->lookahead != ']') return false;
  adv(lexer);
  return true;
}
@@HELPERS@@
void *tree_sitter_@@LANG@@_external_scanner_create(void) {
  return calloc(1, sizeof(Scanner));
}
void tree_sitter_@@LANG@@_external_scanner_destroy(void *p) { free(p); }
unsigned tree_sitter_@@LANG@@_external_scanner_serialize(void *p, char *buf) {
  Scanner *s = (Scanner *)p;
  if (!s) return 0;
  buf[0] = (char)s->equals;
  buf[1] = s->in_string ? 1 : 0;
  buf[2] = (char)s->depth;
  return 3;
}
void tree_sitter_@@LANG@@_external_scanner_deserialize(void *p, const char *buf, unsigned len) {
  Scanner *s = (Scanner *)p;
  if (!s) return;
  s->equals = 0;
  s->in_string = false;
  s->depth = 0;
  if (len >= 2) { s->equals = (uint8_t)buf[0]; s->in_string = buf[1] != 0; }
  if (len >= 3) s->depth = (uint8_t)buf[2];
}
bool tree_sitter_@@LANG@@_external_scanner_scan(void *p, TSLexer *lexer, const bool *valid) {
  Scanner *s = (Scanner *)p;
  if (!s) return false;@@COMMENT@@
  if (valid[@@START@@] && !s->in_string && lexer->lookahead == '[') {
    uint8_t eq = 0;
    if (!opener(lexer, &eq)) return false;
    s->equals = eq;
    s->in_string = true;
    lexer->result_symbol = @@START@@;
    return true;
  }
  if (s->in_string && valid[@@END@@] && closer(lexer, s->equals)) {
    s->in_string = false;
    lexer->result_symbol = @@END@@;
    return true;
  }
  if (s->in_string && valid[@@CONTENT@@]) {
    bool consumed = false;
    unsigned n = 0;
    while (!done(lexer) && n < 8388608u) {
      n++;
      if (lexer->lookahead == ']') {
        lexer->mark_end(lexer);
        if (closer(lexer, s->equals)) break;
        consumed = true;
        continue;
      }
      int32_t ch = lexer->lookahead;
      uint32_t col = lexer->get_column(lexer);
      adv(lexer);
      consumed = true;
      lexer->mark_end(lexer);
      if (!done(lexer) && lexer->lookahead == ch && lexer->get_column(lexer) == col) break;
    }
    if (consumed) { lexer->result_symbol = @@CONTENT@@; return true; }
  }
  if (!s->in_string) {
    int lex = lex_arm(s, lexer, valid);
    if (lex > 0) return true;
    if (lex < 0) return false;
  }
  return false;
}
"#;

const SHARED: &str = r#"
static bool is_digit(int32_t ch) { return ch >= '0' && ch <= '9'; }
static bool is_hex(int32_t ch) {
  return is_digit(ch) || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F');
}
static unsigned hex_value(int32_t ch) {
  if (ch >= '0' && ch <= '9') return (unsigned)(ch - '0');
  if (ch >= 'a' && ch <= 'f') return (unsigned)(ch - 'a' + 10);
  return (unsigned)(ch - 'A' + 10);
}
static bool is_space(int32_t ch) {
  return ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' || ch == '\v' || ch == '\f';
}
static int consume_escape(TSLexer *lexer, bool lua_strict) {
  if (done(lexer)) return -1;
  int32_t ch = lexer->lookahead;
  if (ch == '\n') { adv(lexer); return 1; }
  if (ch == '\r') {
    adv(lexer);
    if (lexer->lookahead == '\n') adv(lexer);
    return 1;
  }
  if (ch == 'z') {
    adv(lexer);
    while (is_space(lexer->lookahead)) adv(lexer);
    return 1;
  }
  if (ch == 'x') {
    adv(lexer);
    if (!is_hex(lexer->lookahead)) return -1;
    adv(lexer);
    if (!is_hex(lexer->lookahead)) return -1;
    adv(lexer);
    return 1;
  }
  if (ch == 'u') {
    adv(lexer);
    if (lexer->lookahead != '{') return -1;
    adv(lexer);
    unsigned code = 0;
    int digits = 0;
    while (digits < 16 && is_hex(lexer->lookahead)) {
      unsigned digit = hex_value(lexer->lookahead);
      if (code > (0x10FFFFu / 16u)) return -1;
      code = code * 16u + digit;
      adv(lexer);
      digits++;
    }
    if (digits == 0 || lexer->lookahead != '}') return -1;
    adv(lexer);
    if (code >= 0x110000u) return -1;
    return 1;
  }
  if (is_digit(ch)) {
    unsigned code = (unsigned)(ch - '0');
    adv(lexer);
    for (int j = 0; j < 2 && is_digit(lexer->lookahead); j++) {
      code = 10u * code + (unsigned)(lexer->lookahead - '0');
      adv(lexer);
    }
    if (code > UCHAR_MAX) return -1;
    return 1;
  }
  if (lua_strict) {
    switch (ch) {
      case 'a': case 'b': case 'f': case 'n': case 'r': case 't': case 'v':
      case '\\': case '\'': case '"':
        adv(lexer);
        return 1;
      default:
        return -1;
    }
  }
  if (ch == 0) return -1;
  adv(lexer);
  return 1;
}
static int read_quoted(TSLexer *lexer, bool lua_strict) {
  int32_t quote = lexer->lookahead;
  if (quote != '"' && quote != '\'') return 0;
  adv(lexer);
  while (!done(lexer) && lexer->lookahead != quote) {
    if (lexer->lookahead == '\n' || lexer->lookahead == '\r') return -1;
    if (lexer->lookahead == '\\') {
      adv(lexer);
      if (consume_escape(lexer, lua_strict) < 0) return -1;
      continue;
    }
    adv(lexer);
  }
  if (lexer->lookahead != quote) return -1;
  adv(lexer);
  lexer->result_symbol = quoted_string;
  return 1;
}
"#;

const LUA: &str = r#"
static int read_shebang(TSLexer *lexer, const bool *valid) {
  if (!valid[shebang]) return 0;
  if (!lexer->is_at_included_range_start(lexer)) return 0;
  if (lexer->lookahead != '#') return 0;
  adv(lexer);
  if (lexer->lookahead != '!') return -1;
  adv(lexer);
  while (!done(lexer) && lexer->lookahead != '\n' && lexer->lookahead != '\r') adv(lexer);
  lexer->result_symbol = shebang;
  return 1;
}
static int lex_arm(Scanner *s, TSLexer *lexer, const bool *valid) {
  (void)s;
  if (valid[quoted_string] && (lexer->lookahead == '"' || lexer->lookahead == '\'')) {
    return read_quoted(lexer, true);
  }
  return read_shebang(lexer, valid);
}
"#;

const LUAU: &str = r#"
#include <errno.h>
static bool is_alpha(int32_t ch) {
  return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z');
}
static bool is_ident(int32_t ch) { return is_alpha(ch) || is_digit(ch) || ch == '_'; }
static bool integer_ok(char *data) {
  const char *payload = data;
  int base = 10;
  if (data[0] == '0' && data[1] != '\0' && (data[1] == 'x' || data[1] == 'X')) {
    base = 16;
  } else if (data[0] == '0' && data[1] != '\0' && (data[1] == 'b' || data[1] == 'B')) {
    base = 2;
    payload = data + 2;
  }
  char *stop = NULL;
  if (base == 10) {
    long long result = strtoll(payload, &stop, 10);
    if (stop == payload || *stop != 'i' || stop[1] != '\0') return false;
    if ((result == LLONG_MIN || result == LLONG_MAX) && errno == ERANGE) {
      errno = 0;
      result = strtoll(payload, &stop, 10);
      if (errno == ERANGE) return false;
    }
    return true;
  }
  unsigned long long value = strtoull(payload, &stop, base);
  if (stop == payload || *stop != 'i' || stop[1] != '\0') return false;
  if (value == ULLONG_MAX && errno == ERANGE) {
    errno = 0;
    value = strtoull(payload, &stop, base);
    if (errno == ERANGE) return false;
  }
  return true;
}
static bool double_ok(const char *data) {
  char *stop = NULL;
  if (data[0] == '0' && data[1] != '\0' && (data[1] == 'b' || data[1] == 'B')) {
    if (data[2] == '\0') return false;
    (void)strtoull(data + 2, &stop, 2);
    return *stop == '\0';
  }
  if (data[0] == '0' && data[1] != '\0' && (data[1] == 'x' || data[1] == 'X')) {
    if (data[2] == '\0') return false;
    (void)strtoull(data, &stop, 16);
    return *stop == '\0';
  }
  (void)strtod(data, &stop);
  return *stop == '\0';
}
static bool luau_number_ok(const char *raw, int raw_len) {
  char data[4096];
  int n = 0;
  for (int i = 0; i < raw_len; i++) {
    if (raw[i] == '_') continue;
    if (n >= 4095) return false;
    data[n++] = raw[i];
  }
  if (n == 0) return false;
  data[n] = 0;
  if (data[n - 1] == 'i') return integer_ok(data);
  return double_ok(data);
}
static int read_luau_number(TSLexer *lexer, const bool *valid) {
  if (!valid[number]) return 0;
  char buf[4096];
  int n = 0;
  if (lexer->lookahead == '.') {
    adv(lexer);
    if (!is_digit(lexer->lookahead)) return -1;
    buf[n++] = '.';
  } else if (!is_digit(lexer->lookahead)) {
    return 0;
  }
  do {
    if (n >= 4095) return -1;
    buf[n++] = (char)lexer->lookahead;
    adv(lexer);
  } while (is_digit(lexer->lookahead) || lexer->lookahead == '.' || lexer->lookahead == '_');
  if (lexer->lookahead == 'e' || lexer->lookahead == 'E') {
    if (n >= 4095) return -1;
    buf[n++] = (char)lexer->lookahead;
    adv(lexer);
    if (lexer->lookahead == '+' || lexer->lookahead == '-') {
      if (n >= 4095) return -1;
      buf[n++] = (char)lexer->lookahead;
      adv(lexer);
    }
  }
  while (is_alpha(lexer->lookahead) || is_digit(lexer->lookahead) || lexer->lookahead == '_') {
    if (n >= 4095) return -1;
    buf[n++] = (char)lexer->lookahead;
    adv(lexer);
  }
  buf[n] = 0;
  if (!luau_number_ok(buf, n)) return -1;
  lexer->result_symbol = number;
  return 1;
}
static int read_section(TSLexer *lexer) {
  while (!done(lexer) && lexer->lookahead != '`') {
    if (lexer->lookahead == '\n' || lexer->lookahead == '\r') return 0;
    if (lexer->lookahead == '\\') {
      adv(lexer);
      if (consume_escape(lexer, false) < 0) return 0;
      continue;
    }
    if (lexer->lookahead == '{') {
      adv(lexer);
      if (lexer->lookahead == '{') return 0;
      return 2;
    }
    adv(lexer);
  }
  if (lexer->lookahead != '`') return 0;
  adv(lexer);
  return 1;
}
static int read_interp_start(Scanner *s, TSLexer *lexer, const bool *valid) {
  if (lexer->lookahead != '`') return 0;
  if (!valid[interp_simple] && !valid[interp_begin]) return 0;
  adv(lexer);
  int section = read_section(lexer);
  if (section <= 0) return -1;
  if (section == 1) {
    if (!valid[interp_simple]) return -1;
    lexer->result_symbol = interp_simple;
    return 1;
  }
  if (!valid[interp_begin] || s->depth == 255) return -1;
  s->depth++;
  lexer->result_symbol = interp_begin;
  return 1;
}
static int read_interp_cont(Scanner *s, TSLexer *lexer, const bool *valid) {
  if (s->depth == 0 || lexer->lookahead != '}') return 0;
  if (!valid[interp_mid] && !valid[interp_end]) return 0;
  adv(lexer);
  int section = read_section(lexer);
  if (section <= 0) return -1;
  if (section == 2) {
    if (!valid[interp_mid]) return -1;
    lexer->result_symbol = interp_mid;
    return 1;
  }
  if (!valid[interp_end] || s->depth == 0) return -1;
  s->depth--;
  lexer->result_symbol = interp_end;
  return 1;
}
enum {
  FOL_LPAREN = 1,
  FOL_LBRACK,
  FOL_COLON,
  FOL_COMMA,
  FOL_EQ,
  FOL_DOT,
  FOL_COMPOUND,
  FOL_FUNCTION,
  FOL_NAME,
  FOL_OTHER
};
static void skip_line_remainder(TSLexer *lexer) {
  while (!done(lexer) && lexer->lookahead != '\n' && lexer->lookahead != '\r') adv(lexer);
}
static void skip_long_comment(TSLexer *lexer) {
  if (lexer->lookahead != '[') {
    skip_line_remainder(lexer);
    return;
  }
  adv(lexer);
  uint8_t eq = 0;
  while (lexer->lookahead == '=') {
    if (eq == 255) {
      skip_line_remainder(lexer);
      return;
    }
    eq++;
    adv(lexer);
  }
  if (lexer->lookahead != '[') {
    skip_line_remainder(lexer);
    return;
  }
  adv(lexer);
  while (!done(lexer)) {
    if (lexer->lookahead == ']') {
      if (closer(lexer, eq)) return;
      continue;
    }
    adv(lexer);
  }
}
static int read_follow(TSLexer *lexer, char *next, int *next_len) {
  *next_len = 0;
  next[0] = 0;
  for (;;) {
    while (is_space(lexer->lookahead)) adv(lexer);
    if (done(lexer)) return FOL_OTHER;
    if (lexer->lookahead != '-') break;
    adv(lexer);
    if (lexer->lookahead != '-') {
      if (lexer->lookahead == '=') return FOL_COMPOUND;
      return FOL_OTHER;
    }
    adv(lexer);
    skip_long_comment(lexer);
  }
  int32_t ch = lexer->lookahead;
  if (ch == '(') return FOL_LPAREN;
  if (ch == '[') return FOL_LBRACK;
  if (ch == ':') return FOL_COLON;
  if (ch == ',') return FOL_COMMA;
  if (ch == '=') {
    adv(lexer);
    if (lexer->lookahead == '=') return FOL_OTHER;
    return FOL_EQ;
  }
  if (ch == '.') {
    adv(lexer);
    if (lexer->lookahead == '.') {
      adv(lexer);
      if (lexer->lookahead == '=') return FOL_COMPOUND;
      return FOL_OTHER;
    }
    return FOL_DOT;
  }
  if (ch == '+' || ch == '*' || ch == '%' || ch == '^') {
    adv(lexer);
    if (lexer->lookahead == '=') return FOL_COMPOUND;
    return FOL_OTHER;
  }
  if (ch == '/') {
    adv(lexer);
    if (lexer->lookahead == '/') {
      adv(lexer);
      if (lexer->lookahead == '=') return FOL_COMPOUND;
      return FOL_OTHER;
    }
    if (lexer->lookahead == '=') return FOL_COMPOUND;
    return FOL_OTHER;
  }
  if (is_alpha(ch) || ch == '_') {
    int n = 0;
    int overflow = 0;
    while (is_ident(lexer->lookahead)) {
      if (n < 15) next[n++] = (char)lexer->lookahead;
      else overflow = 1;
      adv(lexer);
    }
    next[n] = 0;
    *next_len = overflow ? 0 : n;
    if (overflow) {
      next[0] = 0;
      return FOL_NAME;
    }
    if (strcmp(next, "function") == 0) return FOL_FUNCTION;
    return FOL_NAME;
  }
  return FOL_OTHER;
}
static int lex_keyword(TSLexer *lexer, const bool *valid) {
  if (!(valid[continue_keyword] || valid[type_keyword] || valid[export_keyword] ||
        valid[const_keyword] || valid[read_keyword] || valid[write_keyword])) {
    return 0;
  }
  if (!is_alpha(lexer->lookahead) && lexer->lookahead != '_') return 0;
  char word[16];
  int n = 0;
  while (is_ident(lexer->lookahead)) {
    if (n >= 15) return -1;
    word[n++] = (char)lexer->lookahead;
    adv(lexer);
  }
  word[n] = 0;
  lexer->mark_end(lexer);
  char next[16];
  int next_len = 0;
  int follow = read_follow(lexer, next, &next_len);
  if (strcmp(word, "continue") == 0) {
    if (!valid[continue_keyword]) return -1;
    if (follow == FOL_LPAREN || follow == FOL_DOT || follow == FOL_LBRACK || follow == FOL_COLON ||
        follow == FOL_COMMA || follow == FOL_EQ || follow == FOL_COMPOUND) {
      return -1;
    }
    lexer->result_symbol = continue_keyword;
    return 1;
  }
  if (strcmp(word, "type") == 0) {
    if (!valid[type_keyword]) return -1;
    if (follow == FOL_NAME || follow == FOL_FUNCTION) {
      lexer->result_symbol = type_keyword;
      return 1;
    }
    return -1;
  }
  if (strcmp(word, "export") == 0) {
    if (!valid[export_keyword]) return -1;
    if (follow == FOL_FUNCTION ||
        (follow == FOL_NAME &&
         (strcmp(next, "type") == 0 || strcmp(next, "local") == 0 || strcmp(next, "const") == 0))) {
      lexer->result_symbol = export_keyword;
      return 1;
    }
    return -1;
  }
  if (strcmp(word, "const") == 0) {
    if (!valid[const_keyword]) return -1;
    if (follow == FOL_NAME || follow == FOL_FUNCTION) {
      lexer->result_symbol = const_keyword;
      return 1;
    }
    return -1;
  }
  if (strcmp(word, "read") == 0 || strcmp(word, "write") == 0) {
    bool is_read = word[0] == 'r';
    if (is_read && !valid[read_keyword]) return -1;
    if (!is_read && !valid[write_keyword]) return -1;
    if (follow == FOL_NAME || follow == FOL_LBRACK) {
      lexer->result_symbol = is_read ? read_keyword : write_keyword;
      return 1;
    }
    return -1;
  }
  return -1;
}
static int lex_arm(Scanner *s, TSLexer *lexer, const bool *valid) {
  int step = read_interp_cont(s, lexer, valid);
  if (step != 0) return step;
  step = read_interp_start(s, lexer, valid);
  if (step != 0) return step;
  if (valid[quoted_string] && (lexer->lookahead == '"' || lexer->lookahead == '\'')) {
    step = read_quoted(lexer, false);
    if (step != 0) return step;
  }
  step = read_luau_number(lexer, valid);
  if (step != 0) return step;
  return lex_keyword(lexer, valid);
}
"#;

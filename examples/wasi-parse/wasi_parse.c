#include "tree_sitter/api.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>

const TSLanguage *tree_sitter_lua(void);

int main(void) {
  const char *src =
      "local x = 1\n"
      "local function greet(name)\n"
      "  return name\n"
      "end\n"
      "local f = function()\n"
      "  return 1\n"
      "end\n"
      "local s = \"hi\"\n"
      "-- a comment\n"
      "print(x)\n"
      "for i = 1, 2 do\n"
      "end\n";
  TSParser *parser = ts_parser_new();
  if (!parser) {
    return 2;
  }
  if (!ts_parser_set_language(parser, tree_sitter_lua())) {
    ts_parser_delete(parser);
    return 3;
  }
  TSTree *tree = ts_parser_parse_string(parser, NULL, src, (uint32_t)strlen(src));
  if (!tree) {
    ts_parser_delete(parser);
    return 4;
  }
  TSNode root = ts_tree_root_node(tree);
  const char *ty = ts_node_type(root);
  unsigned n = ts_node_child_count(root);
  const char *fn_ty = NULL;
  for (unsigned i = 0; i < n; i++) {
    TSNode child = ts_node_child(root, i);
    const char *ct = ts_node_type(child);
    if (ct && (strcmp(ct, "local_function_declaration") == 0 ||
               strcmp(ct, "function_declaration") == 0)) {
      fn_ty = ct;
      break;
    }
  }
  printf("root=%s children=%u function=%s\n", ty ? ty : "?", n, fn_ty ? fn_ty : "?");
  int ok = ty && strcmp(ty, "source_file") == 0 && fn_ty != NULL;
  ts_tree_delete(tree);
  ts_parser_delete(parser);
  return ok ? 0 : 1;
}

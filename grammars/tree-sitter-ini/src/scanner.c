#include "tree_sitter/parser.h"

enum { CONTINUATION_TEXT, INDENT, COMMENT_END };

void *tree_sitter_ini_external_scanner_create(void) { return NULL; }
void tree_sitter_ini_external_scanner_destroy(void *p) { (void)p; }
unsigned tree_sitter_ini_external_scanner_serialize(void *p, char *b) { (void)p; (void)b; return 0; }
void tree_sitter_ini_external_scanner_deserialize(void *p, const char *b, unsigned n) { (void)p; (void)b; (void)n; }

// At the start of a line: blanks then text (not a comment) is a continuation line when the grammar
// allows one (the rest of the line is its token), else the indentation of a setting.
bool tree_sitter_ini_external_scanner_scan(void *p, TSLexer *lexer, const bool *valid) {
  (void)p;
  // The end of a comment: its line break, or the end of the file when there is none.
  if (valid[COMMENT_END]) {
    if (lexer->lookahead == '\r') lexer->advance(lexer, false);
    if (lexer->lookahead == '\n') lexer->advance(lexer, false);
    else if (lexer->lookahead != 0) return false;
    lexer->result_symbol = COMMENT_END;
    return true;
  }
  if (!(valid[CONTINUATION_TEXT] || valid[INDENT]) || lexer->get_column(lexer) != 0) return false;
  bool blank = false;
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
    lexer->advance(lexer, false);
    blank = true;
  }
  int32_t c = lexer->lookahead;
  if (!blank || c == 0 || c == '\n' || c == '\r' || c == ';' || c == '#') return false;
  if (valid[CONTINUATION_TEXT]) {
    // The token ends at the last non-blank character.
    while (lexer->lookahead != 0 && lexer->lookahead != '\n' && lexer->lookahead != '\r') {
      bool text = lexer->lookahead != ' ' && lexer->lookahead != '\t';
      lexer->advance(lexer, false);
      if (text) lexer->mark_end(lexer);
    }
    lexer->result_symbol = CONTINUATION_TEXT;
  } else {
    lexer->mark_end(lexer);
    lexer->result_symbol = INDENT;
  }
  return true;
}

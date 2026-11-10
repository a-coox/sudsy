/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
  name: 'ini',

  extras: $ => [
    $.comment,
    $._blank,
    /[\t ]/
  ],

  // src/scanner.c: an indented line after `name =` (a continuation) or before a setting (`_indent`).
  externals: $ => [$._continuation_text, $._indent],

  rules: {
    document: $ => seq(
      repeat($._blank),  // Eat blank lines at top of file.
      optional(repeat(seq($.setting))),
      repeat($.section),
    ),

    // Section has:
    // - a title
    // - zero or more settings (name=value pairs)
    // - comments (optional)
    section: $ => prec.left(seq(
      $.section_name,
      repeat(seq(
        $.setting,
      )),
    )),

    section_name: $ => seq(
      '[',
      alias(/[^\[\]]+/, $.text),
      ']',
      /\r?\n/,
    ),

    // A flush-left `name = value` owns the indented lines under it; an indented setting (gitconfig) does not.
    // A setting without `=` is a bare key.
    setting: $ => choice(
      seq($._setting_head, choice(
        seq('=', optional(alias(/.+/, $.setting_value)), /\r?\n/, repeat($._continuation)),
        /\r?\n/,
      )),
      seq($._indent, $._setting_head, choice(
        seq('=', optional(alias(/.+/, $.setting_value)), /\r?\n/),
        /\r?\n/,
      )),
    ),

    _setting_head: $ => alias(/[^;#=\s\[]+( *[^;#=\s\[])*/, $.setting_name),

    _continuation: $ => seq(alias($._continuation_text, $.continuation), /\r?\n/),

    comment: $ => seq(/[;#]/, alias(/[^\r\n]*/, $.text), /\r?\n/),

    _blank: () => field('blank', /\r?\n/),
  }
});

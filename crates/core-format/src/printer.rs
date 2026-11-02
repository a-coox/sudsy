//! The greedy, language-unaware printer. It follows Prettier's algorithm: a group is printed flat
//! if the rest of its line (up to the next possible break) fits in the line width, else broken.

use crate::doc::Doc;
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintOptions {
    pub line_width: usize,
    /// Columns per indentation level.
    pub indent_width: usize,
    pub use_tabs: bool,
}

impl Default for PrintOptions {
    fn default() -> Self {
        PrintOptions {
            line_width: 80,
            indent_width: 2,
            use_tabs: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Flat,
    Break,
}

/// What is left to print: one document or a run of them (a concat, or the rest of a fill).
#[derive(Clone, Copy)]
enum Item<'a> {
    Doc(&'a Doc),
    Seq(&'a [Doc]),
    Fill(&'a [Doc]),
}

#[derive(Clone, Copy)]
struct Cmd<'a> {
    indent: usize,
    mode: Mode,
    item: Item<'a>,
}

/// One interned indentation level.
struct Level {
    text: String,
    width: usize,
    /// The level one indent deeper, once it exists.
    indented: Option<usize>,
    /// The levels `align` made from this one, as `(spaces, level)`.
    aligned: Vec<(usize, usize)>,
}

/// Indentation levels are interned: a command carries an index into this arena, and a level is
/// built once however often the document indents by it.
struct Indents {
    levels: Vec<Level>,
    opts: PrintOptions,
}

impl Indents {
    fn new(opts: PrintOptions) -> Self {
        let mut indents = Indents {
            levels: Vec::new(),
            opts,
        };
        indents.push(String::new(), 0);
        indents
    }

    fn push(&mut self, text: String, width: usize) -> usize {
        self.levels.push(Level {
            text,
            width,
            indented: None,
            aligned: Vec::new(),
        });
        self.levels.len() - 1
    }

    fn indent(&mut self, parent: usize) -> usize {
        if let Some(i) = self.levels[parent].indented {
            return i;
        }
        let Level { text, width, .. } = &self.levels[parent];
        let w = self.opts.indent_width;
        let (text, width) = if self.opts.use_tabs {
            (format!("{text}\t"), width + w)
        } else {
            (format!("{text}{}", " ".repeat(w)), width + w)
        };
        let i = self.push(text, width);
        self.levels[parent].indented = Some(i);
        i
    }

    fn align(&mut self, parent: usize, spaces: usize) -> usize {
        if let Some((_, i)) = self.levels[parent]
            .aligned
            .iter()
            .find(|(n, _)| *n == spaces)
        {
            return *i;
        }
        let Level { text, width, .. } = &self.levels[parent];
        let (text, width) = (format!("{text}{}", " ".repeat(spaces)), width + spaces);
        let i = self.push(text, width);
        self.levels[parent].aligned.push((spaces, i));
        i
    }
}

fn str_width(s: &str, tab: usize) -> usize {
    // Printable ASCII is one column per byte.
    if s.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return s.len();
    }
    s.chars()
        .map(|c| {
            if c == '\t' {
                tab
            } else {
                c.width().unwrap_or(0)
            }
        })
        .sum()
}

/// Prints `doc`. Trailing whitespace before a newline is trimmed.
pub fn print(doc: &mut Doc, opts: &PrintOptions) -> String {
    print_protected(doc, opts).0
}

/// Like [`print`]; also returns the length of the output up to which trailing whitespace is
/// significant because it was written by [`Doc::Verbatim`].
pub fn print_protected(doc: &mut Doc, opts: &PrintOptions) -> (String, usize) {
    doc.propagate_breaks();
    let mut p = Printer {
        opts: *opts,
        indents: Indents::new(*opts),
        out: String::new(),
        pos: 0,
        suffixes: Vec::new(),
        remeasure: false,
        keep: 0,
        scratch: Vec::new(),
    };
    p.run(doc);
    (p.out, p.keep)
}

struct Printer<'a> {
    opts: PrintOptions,
    indents: Indents,
    out: String,
    pos: usize,
    suffixes: Vec<Cmd<'a>>,
    /// Set after a hard line in flat mode: the next group must be measured again.
    remeasure: bool,
    /// Output before this length ends with significant blanks (see [`Doc::Verbatim`]).
    keep: usize,
    /// Work list of [`fits`], kept between calls.
    scratch: Vec<(Mode, Item<'a>)>,
}

impl<'a> Printer<'a> {
    fn run(&mut self, root: &'a Doc) {
        let mut cmds = vec![Cmd {
            indent: 0,
            mode: Mode::Break,
            item: Item::Doc(root),
        }];
        loop {
            let Some(cmd) = cmds.pop() else {
                if self.suffixes.is_empty() {
                    break;
                }
                cmds.extend(self.suffixes.drain(..).rev());
                continue;
            };
            self.step(cmd, &mut cmds);
        }
    }

    fn width_left(&self) -> isize {
        self.opts.line_width as isize - self.pos as isize
    }

    fn newline(&mut self, indent: usize) {
        let trimmed = self.out.trim_end_matches([' ', '\t']).len().max(self.keep);
        self.out.truncate(trimmed);
        self.out.push('\n');
        let level = &self.indents.levels[indent];
        self.out.push_str(&level.text);
        self.pos = level.width;
    }

    fn write(&mut self, s: &str) {
        self.out.push_str(s);
        match s.rfind('\n') {
            Some(i) => self.pos = str_width(&s[i + 1..], self.opts.indent_width),
            None => self.pos += str_width(s, self.opts.indent_width),
        }
    }

    fn step(&mut self, cmd: Cmd<'a>, cmds: &mut Vec<Cmd<'a>>) {
        let Cmd { indent, mode, item } = cmd;
        let doc = match item {
            Item::Doc(d) => d,
            Item::Seq(parts) => {
                cmds.extend(parts.iter().rev().map(|d| Cmd {
                    indent,
                    mode,
                    item: Item::Doc(d),
                }));
                return;
            }
            Item::Fill(parts) => return self.fill(indent, mode, parts, cmds),
        };
        let push = |cmds: &mut Vec<Cmd<'a>>, indent, mode, doc| {
            cmds.push(Cmd {
                indent,
                mode,
                item: Item::Doc(doc),
            })
        };
        match doc {
            Doc::Nil | Doc::BreakParent => {}
            Doc::Text(s) => self.write(s),
            Doc::Verbatim(s) => {
                self.write(s);
                self.keep = self.out.len();
            }
            Doc::Concat(parts) => cmds.push(Cmd {
                indent,
                mode,
                item: Item::Seq(parts),
            }),
            Doc::Indent(d) => {
                let i = self.indents.indent(indent);
                push(cmds, i, mode, d);
            }
            Doc::Align(n, d) => {
                let i = self.indents.align(indent, *n);
                push(cmds, i, mode, d);
            }
            Doc::IfBreak { broken, flat } => {
                push(
                    cmds,
                    indent,
                    mode,
                    if mode == Mode::Break { broken } else { flat },
                );
            }
            Doc::Fill(parts) => cmds.push(Cmd {
                indent,
                mode,
                item: Item::Fill(parts),
            }),
            Doc::LineSuffix(d) => self.suffixes.push(Cmd {
                indent,
                mode,
                item: Item::Doc(d),
            }),
            Doc::Line | Doc::SoftLine | Doc::HardLine => {
                if mode == Mode::Flat && !matches!(doc, Doc::HardLine) {
                    if matches!(doc, Doc::Line) {
                        self.write(" ");
                    }
                    return;
                }
                if mode == Mode::Flat {
                    self.remeasure = true;
                }
                if !self.suffixes.is_empty() {
                    // Trailing comments go out before the newline.
                    cmds.push(cmd);
                    cmds.extend(self.suffixes.drain(..).rev());
                    return;
                }
                self.newline(indent);
            }
            Doc::Group(g) => {
                let flat_cmd = |doc| Cmd {
                    indent,
                    mode: Mode::Flat,
                    item: Item::Doc(doc),
                };
                if mode == Mode::Flat && !self.remeasure {
                    let (m, d) = match (g.broken, g.states.last()) {
                        (true, Some(last)) => (Mode::Break, last),
                        (true, None) => (Mode::Break, &g.contents),
                        (false, _) => (Mode::Flat, &g.contents),
                    };
                    push(cmds, indent, m, d);
                    return;
                }
                self.remeasure = false;
                let next = flat_cmd(&g.contents);
                let width = self.width_left();
                if !g.broken && fits(&mut self.scratch, next, cmds, width, false) {
                    cmds.push(next);
                } else if let Some(most_expanded) = g.states.last() {
                    if g.broken {
                        push(cmds, indent, Mode::Break, most_expanded);
                        return;
                    }
                    let chosen = g.states[1..]
                        .iter()
                        .map(flat_cmd)
                        .find(|c| fits(&mut self.scratch, *c, cmds, width, false));
                    match chosen {
                        Some(c) => cmds.push(c),
                        None => push(cmds, indent, Mode::Break, most_expanded),
                    }
                } else {
                    push(cmds, indent, Mode::Break, &g.contents);
                }
            }
        }
    }

    fn fill(&mut self, indent: usize, mode: Mode, parts: &'a [Doc], cmds: &mut Vec<Cmd<'a>>) {
        let cmd = |mode, item| Cmd { indent, mode, item };
        let width = self.width_left();
        let [content, rest @ ..] = parts else { return };
        let content_flat = cmd(Mode::Flat, Item::Doc(content));
        let content_fits = fits(&mut self.scratch, content_flat, &[], width, true);
        let [whitespace, rest @ ..] = rest else {
            let m = if content_fits {
                Mode::Flat
            } else {
                Mode::Break
            };
            cmds.push(cmd(m, Item::Doc(content)));
            return;
        };
        let ws = |m| cmd(m, Item::Doc(whitespace));
        let content_cmd = |m| cmd(m, Item::Doc(content));
        if rest.is_empty() {
            if content_fits {
                cmds.extend([ws(Mode::Flat), content_cmd(Mode::Flat)]);
            } else {
                cmds.extend([ws(Mode::Break), content_cmd(Mode::Break)]);
            }
            return;
        }
        let remaining = cmd(mode, Item::Fill(rest));
        let first_two = cmd(Mode::Flat, Item::Seq(&parts[..3]));
        let first_two_fit = fits(&mut self.scratch, first_two, &[], width, true);
        if first_two_fit {
            cmds.extend([remaining, ws(Mode::Flat), content_cmd(Mode::Flat)]);
        } else if content_fits {
            cmds.extend([remaining, ws(Mode::Break), content_cmd(Mode::Flat)]);
        } else {
            cmds.extend([remaining, ws(Mode::Break), content_cmd(Mode::Break)]);
        }
    }
}

/// Does `next`, followed by whatever is already queued in `rest`, fit in `width` columns up to
/// the first line break in break mode?
fn fits<'a>(
    stack: &mut Vec<(Mode, Item<'a>)>,
    next: Cmd<'a>,
    rest: &[Cmd<'a>],
    width: isize,
    must_be_flat: bool,
) -> bool {
    let mut width = width;
    let mut rest_idx = rest.len();
    stack.clear();
    stack.push((next.mode, next.item));
    while width >= 0 {
        let Some((mode, item)) = stack.pop() else {
            if rest_idx == 0 {
                return true;
            }
            rest_idx -= 1;
            stack.push((rest[rest_idx].mode, rest[rest_idx].item));
            continue;
        };
        let doc = match item {
            Item::Doc(d) => d,
            Item::Seq(parts) | Item::Fill(parts) => {
                stack.extend(parts.iter().rev().map(|d| (mode, Item::Doc(d))));
                continue;
            }
        };
        match doc {
            Doc::Nil | Doc::BreakParent | Doc::LineSuffix(_) => {}
            Doc::Text(s) | Doc::Verbatim(s) => match s.find('\n') {
                // Only the first line of verbatim text counts against this line.
                Some(i) => return width - (str_width(&s[..i], 1) as isize) >= 0,
                None => width -= str_width(s, 1) as isize,
            },
            Doc::Concat(parts) => stack.push((mode, Item::Seq(parts))),
            Doc::Indent(d) | Doc::Align(_, d) => stack.push((mode, Item::Doc(d))),
            Doc::IfBreak { broken, flat } => stack.push((
                mode,
                Item::Doc(if mode == Mode::Break { broken } else { flat }),
            )),
            Doc::Fill(parts) => stack.push((mode, Item::Fill(parts))),
            Doc::Group(g) => {
                if must_be_flat && g.broken {
                    return false;
                }
                let m = if g.broken { Mode::Break } else { mode };
                let contents = match (m, g.states.last()) {
                    (Mode::Break, Some(last)) => last,
                    _ => &g.contents,
                };
                stack.push((m, Item::Doc(contents)));
            }
            Doc::Line | Doc::SoftLine | Doc::HardLine => {
                if mode == Mode::Break || matches!(doc, Doc::HardLine) {
                    return true;
                }
                if matches!(doc, Doc::Line) {
                    width -= 1;
                }
            }
        }
    }
    false
}


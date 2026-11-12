//! The shared formatting engine.
//!
//! * [`doc`]: the layout document; [`printer`]: the greedy, language-unaware printer.
//! * [`shapes`]: delimited list, block, key-value, statement sequence and verbatim.
//! * [`context`]: [`FormatCtx`] and the [`NodeFormatter`] trait providers implement.
//! * [`comments`], [`ignore`], [`directives`]: comment placement, `sudsy-ignore`, layout directives.
//! * [`safety`]: the re-parse and tree comparison check; [`testing`]: the shared test helper.
//!
//! A provider's formatter turns a node into a [`Doc`]; [`format_tree`] prints it.

pub mod comments;
pub mod context;
pub mod directives;
pub mod doc;
mod error;
pub mod ignore;
mod lines;
pub mod printer;
pub mod safety;
pub mod shapes;
pub mod testing;

pub use context::{DefaultFormatter, EmbedMode, Embedder, FormatCtx, NodeFormatter, Variants};
pub use directives::{DIRECTIVES_VERSION, Directive, DirectiveConflict, DirectiveTable, NodeKey};
pub use doc::Doc;
pub use error::FormatError;
pub use printer::PrintOptions;

use core_config::ResolvedOptions;
use core_syntax::SyntaxNode;

thread_local! {
    /// Levels of [`deep`] entered on this thread, and the most since the last [`take_peak`].
    static LEVEL: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static PEAK: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The deepest [`deep`] nesting since the previous call.
fn take_peak() -> u32 {
    PEAK.replace(0)
}

/// Runs `f` on a fresh stack segment when the current one is nearly used up. The recursion over
/// a syntax tree or a document is as deep as the source nests, which debug frames cannot hold on
/// a 2 MB thread stack; every recursive entry of the engine goes through here.
pub fn deep<R>(f: impl FnOnce() -> R) -> R {
    // Measuring the stack is far dearer than a counter, so only every 32nd level measures; 32
    // debug-build levels use well under the red zone.
    let level = LEVEL.get();
    LEVEL.set(level.wrapping_add(1));
    if level > PEAK.get() {
        PEAK.set(level);
    }
    let result = if level % 32 == 31 {
        stacker::maybe_grow(512 * 1024, 4 * 1024 * 1024, f)
    } else {
        f()
    };
    LEVEL.set(level);
    result
}

/// Printer settings from the shared options (`line_width`, `indent_size`, `indent_style`); an
/// option the provider's schema does not declare keeps the printer default.
pub fn print_options(options: &ResolvedOptions) -> PrintOptions {
    use core_config::{OptionValue, shared};
    let default = PrintOptions::default();
    let int = |name, fallback: usize| match options.get(name) {
        Some(OptionValue::Int(n)) => *n as usize,
        _ => fallback,
    };
    PrintOptions {
        line_width: int(shared::LINE_WIDTH, default.line_width),
        indent_width: int(shared::INDENT_SIZE, default.indent_width),
        use_tabs: matches!(options.get(shared::INDENT_STYLE),
            Some(OptionValue::String(s)) if s == "tab"),
    }
}

/// Line break style of a file: the first line break found decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub fn detect(text: &str) -> Self {
        match text.find('\n') {
            Some(i) if text[..i].ends_with('\r') => LineEnding::CrLf,
            _ => LineEnding::Lf,
        }
    }
}

impl LineEnding {
    /// Like [`LineEnding::detect`] for the text of a tree, without making the text.
    pub fn of_tree(root: &SyntaxNode) -> Self {
        let mut after_cr = false;
        for token in root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            let text = token.text();
            if let Some(i) = text.find('\n') {
                let cr = text[..i].ends_with('\r') || i == 0 && after_cr;
                return if cr { LineEnding::CrLf } else { LineEnding::Lf };
            }
            after_cr = text.ends_with('\r');
        }
        LineEnding::Lf
    }
}

/// Formats `root` (and everything below it) to text. The `end_of_line` option decides the line
/// ending (`auto`, the default and for providers without the option: the file's first one) and
/// the `final_newline` option, when the provider declares it, decides how the text ends.
pub fn format_tree(root: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<String, FormatError> {
    take_peak();
    let mut doc = cx.format_unguarded(root)?;
    let (printed, keep) = printer::print_protected(&mut doc, &print_options(cx.options));
    // A document of ordinary depth drops the usual way; only a pathological one needs this.
    if take_peak() > 64 {
        doc.dispose();
    }
    // Copies of the whole output only when verbatim text brought `\r\n` into it.
    let (mut out, keep) = if printed.contains("\r\n") {
        let keep = printed[..keep].replace("\r\n", "\n").len();
        (printed.replace("\r\n", "\n"), keep)
    } else {
        (printed, keep)
    };
    out.truncate(out.trim_end_matches(['\n', ' ', '\t']).len().max(keep));
    let final_newline = !matches!(
        cx.options.get(core_config::shared::FINAL_NEWLINE),
        Some(core_config::OptionValue::Bool(false))
    );
    if final_newline && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    let ending = match cx.options.get(core_config::shared::END_OF_LINE) {
        Some(core_config::OptionValue::String(s)) if s == "lf" => LineEnding::Lf,
        Some(core_config::OptionValue::String(s)) if s == "crlf" => LineEnding::CrLf,
        _ => LineEnding::of_tree(root),
    };
    Ok(match ending {
        LineEnding::Lf => out,
        LineEnding::CrLf => out.replace('\n', "\r\n"),
    })
}

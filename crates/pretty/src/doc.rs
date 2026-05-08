//! Doc IR for the Wadler-style pretty printer.
//!
//! A `Doc` is an intermediate representation built by the formatter / emitter
//! while walking the AST. Layout decisions (whether a `Group` fits on a line,
//! whether a `Line` becomes a space or a newline) are deferred to the renderer
//! in `render.rs`, which makes them based on the configured `max_width`.

use std::rc::Rc;

/// Single comment node attached to a token.
#[derive(Clone, Debug)]
pub struct CommentDoc {
    /// Comment text. Line comments are stored without their trailing `\n`;
    /// block comments are stored verbatim and may contain embedded newlines.
    pub text: Rc<str>,
    /// Number of newlines that separate this comment from the previous
    /// emitted entity (the main token, or the preceding comment in the same
    /// `Comments` vec). `0` means "same line, separated by a space" (unless
    /// the previous content ended with a newline already).
    pub leading_newlines: u32,
    /// True for `//` line comments, false for `/* */` block comments.
    pub is_line_comment: bool,
    /// 1-based source position of the comment. When non-zero, the renderer
    /// emits an anchor entry (so the emitter can rebuild a source map).
    pub src_line: u32,
    pub src_column: u32,
}

/// Pretty-printer document.
///
/// The variants are intentionally minimal: the few combinators below are
/// sufficient to express every layout choice the formatter / emitter need to
/// make. Anything more elaborate is layered on top via helper constructors.
#[derive(Clone, Debug, Default)]
pub enum Doc {
    /// Empty document.
    #[default]
    Nil,

    /// Plain text. Must not contain `\n` (newlines belong to `Line` /
    /// `Hardline`). Alignment padding is baked in here as well.
    Text(Rc<str>),

    /// Sequential composition of multiple documents.
    Concat(Rc<[Doc]>),

    /// Indent the inner document by `level` units (each unit is multiplied by
    /// `RenderOpts.indent_width` at render time). Negative values dedent.
    Indent(i32, Rc<Doc>),

    /// Wadler's `group`: try to render the inner document on a single line; if
    /// that exceeds the remaining width, break all `Line` nodes inside it
    /// into hard newlines.
    Group(Rc<Doc>),

    /// Force the inner document to render in flat mode regardless of width.
    /// Inside `ForceFlat`, `Hardline` / `UserBreak` / `DedentHardline`
    /// collapse to a single space so the entire region stays on one line,
    /// even when callers nested constructs that would normally break.
    ForceFlat(Rc<Doc>),

    /// Soft line. Becomes `sep` when the enclosing group fits, becomes a
    /// newline + indent otherwise. Common values: `" "` (space) or `""`.
    Line(&'static str),

    /// Forced newline. Always renders as a newline + indent and forces the
    /// enclosing group to break mode.
    Hardline,

    /// Same semantics as `Hardline` but distinguishes a break that originated
    /// from the user's source layout (preserved as a hint when reformatting).
    UserBreak,

    /// Forced newline that *also* strips a trailing run of exactly
    /// `level * indent_width` spaces from the output before emitting, so
    /// alignment padding sitting at the end of a block doesn't survive
    /// across a dedent.
    DedentHardline(u32),

    /// Comments attached to a token. The renderer compares each comment's
    /// `line` against its current line counter to decide spacing.
    Comments(Rc<[CommentDoc]>),

    /// Emit `text` only when the enclosing group is laid out in break mode.
    /// Used for things like trailing commas that should appear in
    /// multi-line layouts but disappear when the construct fits on one
    /// line. `fits_flat` ignores this node entirely (it contributes 0
    /// columns when flat).
    IfBreak(Rc<str>),

    /// Plain text identical in rendering to `Text`, but also records the
    /// (dst_line, dst_column, src_line, src_column, text) position when
    /// the renderer is asked to collect anchors, so callers can rebuild a
    /// source map.
    Anchored(Rc<AnchoredText>),
}

/// Source-mapped text. Carries the original (1-based) line/column so the
/// renderer can record where the text landed in the output.
#[derive(Clone, Debug)]
pub struct AnchoredText {
    pub text: Rc<str>,
    pub src_line: u32,
    pub src_column: u32,
}

// ----- Helper constructors -------------------------------------------------

/// Plain text node.
pub fn text(s: impl Into<Rc<str>>) -> Doc {
    Doc::Text(s.into())
}

/// `n` spaces.
pub fn space(n: usize) -> Doc {
    if n == 0 {
        Doc::Nil
    } else {
        Doc::Text(" ".repeat(n).into())
    }
}

/// Soft line: space when flat, newline when broken.
pub fn line() -> Doc {
    Doc::Line(" ")
}

/// Soft line with no flat separator: empty when flat, newline when broken.
pub fn softline() -> Doc {
    Doc::Line("")
}

/// Forced newline.
pub fn hard() -> Doc {
    Doc::Hardline
}

/// Forced newline that originated from the user's source layout.
pub fn user_break() -> Doc {
    Doc::UserBreak
}

/// Increase indent by one level inside `d`.
pub fn nest(d: Doc) -> Doc {
    Doc::Indent(1, Rc::new(d))
}

/// Decrease indent by one level inside `d`.
pub fn dedent(d: Doc) -> Doc {
    Doc::Indent(-1, Rc::new(d))
}

/// Indent `d` by an explicit number of levels.
pub fn indent_by(level: i32, d: Doc) -> Doc {
    if level == 0 {
        d
    } else {
        Doc::Indent(level, Rc::new(d))
    }
}

/// Wrap `d` in a Wadler group.
pub fn group(d: Doc) -> Doc {
    Doc::Group(Rc::new(d))
}

/// Wrap `d` in a `ForceFlat` so it renders flat regardless of width.
pub fn force_flat(d: Doc) -> Doc {
    Doc::ForceFlat(Rc::new(d))
}

/// Concatenate a vector of docs.
pub fn concat(docs: Vec<Doc>) -> Doc {
    let docs: Vec<Doc> = docs
        .into_iter()
        .filter(|d| !matches!(d, Doc::Nil))
        .collect();
    match docs.len() {
        0 => Doc::Nil,
        1 => docs.into_iter().next().unwrap(),
        _ => Doc::Concat(docs.into()),
    }
}

/// Pick the appropriate separator for content that may have been laid out by
/// the user across multiple lines. If `user_broken` is true the separator is
/// a hard newline (preserved from the source); otherwise it is a soft `Line`
/// that is decided by the enclosing group.
pub fn user_break_or(user_broken: bool, soft: &'static str) -> Doc {
    if user_broken {
        Doc::UserBreak
    } else {
        Doc::Line(soft)
    }
}

/// Comments node.
pub fn comments(cs: Vec<CommentDoc>) -> Doc {
    if cs.is_empty() {
        Doc::Nil
    } else {
        Doc::Comments(cs.into())
    }
}

/// Emit `text` only when the enclosing group is laid out in break mode.
pub fn if_break(text: impl Into<Rc<str>>) -> Doc {
    Doc::IfBreak(text.into())
}

/// Anchored text with a source location. Identical to `Text` for layout
/// purposes; the renderer additionally records the destination position so
/// callers can build source maps.
pub fn anchored(text: impl Into<Rc<str>>, src_line: u32, src_column: u32) -> Doc {
    Doc::Anchored(Rc::new(AnchoredText {
        text: text.into(),
        src_line,
        src_column,
    }))
}

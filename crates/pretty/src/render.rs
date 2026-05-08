//! Wadler-style renderer for `Doc`.
//!
//! The renderer walks a `Doc` tree, tracking the current column and indent
//! level, and decides each `Group` independently: if the group's flat
//! rendering fits within `max_width - col`, it is laid out flat; otherwise it
//! is laid out in break mode and every `Line` inside becomes a newline.
//!
//! Hard newlines (`Hardline` / `UserBreak`) and line comments propagate
//! "doesn't fit" out to the enclosing group, so a group that contains any
//! mandatory break is always laid out in break mode.
//!
//! Indent emission is deferred. After emitting a newline we record the
//! indent that should appear at the start of the next line; it is flushed
//! when actual content arrives (`Text`, comment text, etc.). If another
//! newline arrives first we discard the pending indent — that produces a
//! genuine blank line (without trailing whitespace) in cases where two
//! `Hardline`s sit back-to-back.

use crate::doc::{AnchoredText, CommentDoc, Doc};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// Renderer configuration.
#[derive(Clone, Debug)]
pub struct RenderOpts {
    pub max_width: usize,
    pub indent_width: usize,
    pub newline: &'static str,
    /// Strip trailing whitespace from each output line. Useful when
    /// aligner padding may leave stray trailing spaces; can be disabled
    /// when the consumer expects byte-for-byte preservation of padding.
    pub strip_trailing_whitespace: bool,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            max_width: 120,
            indent_width: 4,
            newline: "\n",
            strip_trailing_whitespace: true,
        }
    }
}

struct State {
    out: String,
    col: usize,
    /// Counts the (logical) line at which the renderer has emitted content.
    /// Used internally for SourceMap generation in the emitter.
    current_line: u32,
    /// When set, the next forced break (`Hardline` / `UserBreak`) skips its
    /// `\n` because the previous emission already ended the line. The break
    /// still updates the pending indent so the next content lands at the
    /// right column.
    swallow_next_break: bool,
    /// Indent (in spaces) that should be emitted before the next piece of
    /// content. `None` means we're in the middle of a line (no pending
    /// indent). Deferring indent emission this way lets back-to-back
    /// newlines produce a true blank line instead of `\n    \n` with
    /// trailing whitespace.
    pending_indent: Option<usize>,
    /// Recorded anchors. Populated by `Doc::Anchored` nodes when the
    /// renderer is run in anchor-collecting mode (`render_with_anchors`).
    anchors: Vec<RenderedAnchor>,
}

/// One source-map entry produced by the renderer when it walks a
/// `Doc::Anchored` node. Positions are 1-based to match the rest of the
/// emitter pipeline.
#[derive(Clone, Debug)]
pub struct RenderedAnchor {
    pub dst_line: u32,
    pub dst_column: u32,
    pub src_line: u32,
    pub src_column: u32,
    pub text: Rc<str>,
}

/// Output of `render_with_anchors`: the formatted text plus the list of
/// (src, dst) anchor records the renderer collected along the way.
#[derive(Clone, Debug, Default)]
pub struct Rendered {
    pub text: String,
    pub anchors: Vec<RenderedAnchor>,
}

/// Render a document into a string. Anchors (`Doc::Anchored` nodes) are
/// emitted as plain text; their source-map metadata is discarded. Use
/// `render_with_anchors` to capture them.
pub fn render(doc: &Doc, opts: &RenderOpts) -> String {
    render_inner(doc, opts).text
}

/// Like `render`, but also returns the list of (src, dst) anchor records
/// gathered from `Doc::Anchored` nodes. The emitter uses this to rebuild a
/// `SourceMap` after Doc-IR rendering.
pub fn render_with_anchors(doc: &Doc, opts: &RenderOpts) -> Rendered {
    render_inner(doc, opts)
}

fn render_inner(doc: &Doc, opts: &RenderOpts) -> Rendered {
    let mut state = State {
        out: String::new(),
        col: 0,
        current_line: 1,
        swallow_next_break: false,
        pending_indent: None,
        anchors: Vec::new(),
    };
    let mut stack: Vec<Frame<'_>> = Vec::with_capacity(64);
    stack.push(Frame {
        indent: 0,
        mode: Mode::Break,
        doc,
    });
    while let Some(frame) = stack.pop() {
        render_frame(frame, opts, &mut state, &mut stack);
    }
    let text = if opts.strip_trailing_whitespace {
        strip_trailing_whitespace(&state.out, opts.newline)
    } else {
        state.out
    };
    Rendered {
        text,
        anchors: state.anchors,
    }
}

struct Frame<'a> {
    indent: i32,
    mode: Mode,
    doc: &'a Doc,
}

fn flush_pending(state: &mut State) {
    if let Some(pad) = state.pending_indent.take() {
        for _ in 0..pad {
            state.out.push(' ');
        }
        state.col = pad;
    }
}

/// Flush a pending indent. If the current frame's indent is *shallower* than
/// the queued value, retarget downwards: the queued value was set in a
/// nested frame (e.g. inside `Indent(+1, Comments)`), but the actual content
/// lives in an outer frame, so the original line really starts at the outer
/// indent. We never retarget upwards — that would move text deeper than the
/// preceding `\n` set the line up to be (e.g. inside a `Group`'s nest while
/// rendering flat).
fn flush_pending_with_indent(state: &mut State, frame_indent: i32, opts: &RenderOpts) {
    if let Some(pad) = state.pending_indent.take() {
        let target = pad_for(frame_indent, opts).min(pad);
        for _ in 0..target {
            state.out.push(' ');
        }
        state.col = target;
    }
}

fn pad_for(indent: i32, opts: &RenderOpts) -> usize {
    (indent.max(0) as usize) * opts.indent_width
}

fn render_frame<'a>(
    frame: Frame<'a>,
    opts: &RenderOpts,
    state: &mut State,
    stack: &mut Vec<Frame<'a>>,
) {
    let Frame { indent, mode, doc } = frame;
    match doc {
        Doc::Nil => {}
        Doc::Text(s) => {
            flush_pending_with_indent(state, indent, opts);
            // Real content invalidates the swallow request: the comment's
            // trailing newline is no longer adjacent to the upcoming break.
            state.swallow_next_break = false;
            state.out.push_str(s);
            let nls = s.matches('\n').count() as u32;
            if nls == 0 {
                state.col += s.chars().count();
            } else {
                state.current_line += nls;
                let last_line = s.rsplit('\n').next().unwrap_or("");
                state.col = last_line.chars().count();
            }
        }
        Doc::Concat(items) => {
            for item in items.iter().rev() {
                stack.push(Frame {
                    indent,
                    mode,
                    doc: item,
                });
            }
        }
        Doc::Indent(off, inner) => {
            stack.push(Frame {
                indent: indent + off,
                mode,
                doc: inner,
            });
        }
        Doc::Group(inner) => {
            // Inherit `Mode::Flat` from a surrounding `ForceFlat` so
            // nested Groups don't independently break when the outer
            // wrapping demanded flat.
            let chosen = if matches!(mode, Mode::Flat) {
                Mode::Flat
            } else {
                let remaining = opts.max_width.saturating_sub(state.col);
                if fits_flat(inner, remaining as isize) {
                    Mode::Flat
                } else {
                    Mode::Break
                }
            };
            stack.push(Frame {
                indent,
                mode: chosen,
                doc: inner,
            });
        }
        Doc::ForceFlat(inner) => {
            stack.push(Frame {
                indent,
                mode: Mode::Flat,
                doc: inner,
            });
        }
        Doc::Line(sep) => match mode {
            Mode::Flat => {
                flush_pending(state);
                state.swallow_next_break = false;
                state.out.push_str(sep);
                state.col += sep.chars().count();
            }
            Mode::Break => {
                emit_break(state, indent, opts);
            }
        },
        Doc::Hardline | Doc::UserBreak => match mode {
            Mode::Flat => {
                // Reachable only when a `ForceFlat` overrode the enclosing
                // Group's natural break decision. Collapse to a space so
                // the region stays on one line.
                flush_pending(state);
                state.swallow_next_break = false;
                state.out.push(' ');
                state.col += 1;
            }
            Mode::Break => {
                emit_break(state, indent, opts);
            }
        },
        Doc::DedentHardline(level) => match mode {
            Mode::Flat => {
                flush_pending(state);
                state.swallow_next_break = false;
                state.out.push(' ');
                state.col += 1;
            }
            Mode::Break => {
                // Strip up to `level * indent_width` trailing spaces so
                // alignment padding at line ends doesn't survive the
                // dedent. Aligner padding longer than the indent is left
                // alone (only the indent's worth is stripped).
                let want = (*level as usize) * opts.indent_width;
                if want > 0 && state.pending_indent.is_none() {
                    let len = state.out.len();
                    let bytes = state.out.as_bytes();
                    if len >= want && bytes[len - want..].iter().all(|b| *b == b' ') {
                        state.out.truncate(len - want);
                        state.col = state.col.saturating_sub(want);
                    }
                }
                emit_break(state, indent, opts);
            }
        },
        Doc::Comments(cs) => {
            render_comments(cs, indent, opts, state);
        }
        Doc::IfBreak(s) => {
            if matches!(mode, Mode::Break) {
                flush_pending_with_indent(state, indent, opts);
                state.swallow_next_break = false;
                state.out.push_str(s);
                state.col += s.chars().count();
            }
        }
        Doc::Anchored(a) => {
            emit_anchored(a, indent, opts, state);
        }
    }
}

fn emit_anchored(a: &Rc<AnchoredText>, indent: i32, opts: &RenderOpts, state: &mut State) {
    flush_pending_with_indent(state, indent, opts);
    state.swallow_next_break = false;
    // Record the position where the text starts (before pushing it onto
    // the output). 1-based positions, matching the rest of the pipeline.
    state.anchors.push(RenderedAnchor {
        dst_line: state.current_line,
        dst_column: (state.col as u32) + 1,
        src_line: a.src_line,
        src_column: a.src_column,
        text: a.text.clone(),
    });
    state.out.push_str(&a.text);
    let nls = a.text.matches('\n').count() as u32;
    if nls == 0 {
        state.col += a.text.chars().count();
    } else {
        state.current_line += nls;
        let last = a.text.rsplit('\n').next().unwrap_or("");
        state.col = last.chars().count();
    }
}

fn emit_break(state: &mut State, indent: i32, opts: &RenderOpts) {
    if state.swallow_next_break {
        // A preceding line comment already ended the line.
        state.swallow_next_break = false;
    } else {
        state.out.push_str(opts.newline);
        state.current_line += 1;
        state.col = 0;
    }
    state.pending_indent = Some(pad_for(indent, opts));
}

fn render_comments(cs: &[CommentDoc], indent: i32, opts: &RenderOpts, state: &mut State) {
    let pad_width = pad_for(indent, opts);
    for c in cs {
        let pre_swallow = state.swallow_next_break;
        state.swallow_next_break = false;
        let pending = state.pending_indent.is_some();

        if c.leading_newlines == 0 && !pre_swallow && !pending {
            if state.col > 0 {
                state.out.push(' ');
                state.col += 1;
            }
        } else {
            // `pre_swallow` and `pending` each stand in for at most one
            // already-emitted `\n`; together they still count as one, so
            // subtract a single newline from the requested count rather
            // than double-counting.
            let already = if pre_swallow || pending { 1u32 } else { 0 };
            let to_emit = c.leading_newlines.max(1).saturating_sub(already);
            for _ in 0..to_emit {
                state.out.push_str(opts.newline);
                state.current_line += 1;
                state.col = 0;
            }
            for _ in 0..pad_width {
                state.out.push(' ');
            }
            state.col = pad_width;
            state.pending_indent = None;
        }

        if c.src_line != 0 && c.src_column != 0 {
            state.anchors.push(RenderedAnchor {
                dst_line: state.current_line,
                dst_column: (state.col as u32) + 1,
                src_line: c.src_line,
                src_column: c.src_column,
                text: c.text.clone(),
            });
        }

        state.out.push_str(&c.text);
        let nls = c.text.matches('\n').count() as u32;
        if c.is_line_comment {
            // `flush_pending_with_indent` retargets this pending indent if
            // the next content lives at a shallower frame than this comment.
            state.out.push_str(opts.newline);
            state.current_line += nls + 1;
            state.col = 0;
            state.pending_indent = Some(pad_width);
            state.swallow_next_break = true;
        } else if nls > 0 {
            state.current_line += nls;
            state.col = 0;
        } else {
            state.col += c.text.chars().count();
        }
    }
}

/// Returns true if `d` can be rendered flat within `budget` columns. Used by
/// `Group` to choose between `Mode::Flat` and `Mode::Break`.
///
/// We descend into children, accumulating widths of `Text` nodes and the flat
/// width of `Line` nodes. Any `Hardline`, `UserBreak`, or line comment forces
/// the answer to false (the document cannot be laid out on a single line).
fn fits_flat(d: &Doc, budget: isize) -> bool {
    let mut budget = budget;
    if budget < 0 {
        return false;
    }
    let mut work: Vec<&Doc> = vec![d];
    while let Some(x) = work.pop() {
        if budget < 0 {
            return false;
        }
        match x {
            Doc::Nil => {}
            Doc::Text(s) => {
                budget -= s.chars().count() as isize;
            }
            Doc::Concat(items) => {
                for item in items.iter().rev() {
                    work.push(item);
                }
            }
            Doc::Indent(_, inner) => {
                work.push(inner);
            }
            Doc::Group(inner) => {
                work.push(inner);
            }
            Doc::ForceFlat(inner) => {
                work.push(inner);
            }
            Doc::Line(sep) => {
                budget -= sep.chars().count() as isize;
            }
            Doc::Hardline | Doc::UserBreak | Doc::DedentHardline(_) => return false,
            Doc::IfBreak(_) => {
                // IfBreak contributes nothing in flat mode.
            }
            Doc::Anchored(a) => {
                budget -= a.text.chars().count() as isize;
            }
            Doc::Comments(cs) => {
                if cs.iter().any(|c| c.is_line_comment) {
                    return false;
                }
                for c in cs.iter() {
                    budget -= c.text.chars().count() as isize + 1;
                }
            }
        }
    }
    budget >= 0
}

/// Strip trailing whitespace from each line of `s`.
fn strip_trailing_whitespace(s: &str, newline: &'static str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, line) in s.split(newline).enumerate() {
        if i > 0 {
            out.push_str(newline);
        }
        let trimmed = line.trim_end_matches([' ', '\t']);
        out.push_str(trimmed);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::*;

    fn opts(max: usize) -> RenderOpts {
        RenderOpts {
            max_width: max,
            indent_width: 4,
            newline: "\n",
            strip_trailing_whitespace: true,
        }
    }

    #[test]
    fn text_only() {
        let d = text("hello");
        assert_eq!(render(&d, &opts(80)), "hello");
    }

    #[test]
    fn group_fits_flat() {
        let d = group(concat(vec![
            text("a"),
            line(),
            text("b"),
            line(),
            text("c"),
        ]));
        assert_eq!(render(&d, &opts(80)), "a b c");
    }

    #[test]
    fn group_breaks_when_too_wide() {
        let d = group(nest(concat(vec![
            text("aaaa"),
            line(),
            text("bbbb"),
            line(),
            text("cccc"),
        ])));
        assert_eq!(render(&d, &opts(8)), "aaaa\n    bbbb\n    cccc");
    }

    #[test]
    fn hardline_forces_break() {
        let d = concat(vec![text("a"), hard(), text("b")]);
        assert_eq!(render(&d, &opts(80)), "a\nb");
    }

    #[test]
    fn user_break_forces_break() {
        let d = concat(vec![text("a"), user_break(), text("b")]);
        assert_eq!(render(&d, &opts(80)), "a\nb");
    }

    #[test]
    fn nested_group_outer_breaks_inner_flat() {
        let inner = group(concat(vec![text("xx"), line(), text("yy")]));
        let outer = group(nest(concat(vec![
            text("[start]"),
            line(),
            inner.clone(),
            line(),
            text("[end]"),
        ])));
        assert_eq!(render(&outer, &opts(10)), "[start]\n    xx yy\n    [end]");
    }

    #[test]
    fn binary_op_break_when_overflow() {
        let parts = vec![
            text("aaa"),
            group(concat(vec![line(), text("+ bbb")])),
            group(concat(vec![line(), text("+ ccc")])),
            group(concat(vec![line(), text("+ ddd")])),
        ];
        let d = group(nest(concat(parts)));
        assert_eq!(render(&d, &opts(12)), "aaa + bbb\n    + ccc\n    + ddd");
    }

    #[test]
    fn outer_group_all_break() {
        let d = group(nest(concat(vec![
            text("aaa"),
            line(),
            text("+ bbb"),
            line(),
            text("+ ccc"),
            line(),
            text("+ ddd"),
        ])));
        assert_eq!(
            render(&d, &opts(12)),
            "aaa\n    + bbb\n    + ccc\n    + ddd"
        );
    }

    #[test]
    fn back_to_back_hardlines_produce_blank_line() {
        let d = concat(vec![text("a"), hard(), hard(), text("b")]);
        assert_eq!(render(&d, &opts(80)), "a\n\nb");
    }
}

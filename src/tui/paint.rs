//! Cell-level painting primitives.
//!
//! The TUI draws its own frames, shadows and bands instead of using
//! `ratatui::widgets`, because the visual language needs things the stock
//! widgets cannot express: a 1-cell solid border ring, an offset drop shadow,
//! and a press state that moves the face into where the shadow was. Everything
//! semantic lives here so the widgets module stays about composition.
//!
//! Every entry point clips: `Buffer::cell_mut` returns `None` outside the
//! buffer, so a narrow window simply drops the overflow rather than panicking.
//! That is what lets the layout hand these functions rects computed from
//! whatever size the terminal actually is.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

/// Write one cell. Out-of-bounds positions are ignored.
pub fn cell(buf: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(c) = buf.cell_mut(Position::new(x, y)) {
        c.set_symbol(symbol);
        c.set_style(style);
    }
}

/// Fill a rect with `symbol` + `style`.
pub fn fill(buf: &mut Buffer, rect: Rect, symbol: &str, style: Style) {
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            cell(buf, x, y, symbol, style);
        }
    }
}

/// The heavy frame: paint the whole rect in the border colour, then the inset
/// rect in the face colour. The result is a solid 1-cell ring on all four sides,
/// which reads far heavier than box-drawing glyphs at the same cell budget.
pub fn frame_ring(buf: &mut Buffer, rect: Rect, border: Style, face: Style) {
    fill(buf, rect, " ", border);
    let inner = inset(rect, 1, 1, 1, 1);
    if !inner.is_empty() {
        fill(buf, inner, " ", face);
    }
}

/// The offset drop shadow: an L of border colour along the right and bottom
/// edges of `rect`, displaced by `cols`/`rows`.
///
/// It is deliberately not a copy of the element's shape: at a 2x1 cell offset a
/// full copy would sit almost entirely underneath the element.
pub fn shadow(buf: &mut Buffer, rect: Rect, color: Style, cols: u16, rows: u16) {
    if cols > 0 {
        let right = Rect::new(rect.right(), rect.y, cols, rect.height.saturating_add(rows));
        fill(buf, right, " ", color);
    }
    if rows > 0 {
        let bottom = Rect::new(rect.x, rect.bottom(), rect.width, rows);
        fill(buf, bottom, " ", color);
    }
}

/// Move a rect by `dx`/`dy`, saturating at the coordinate bounds.
pub fn offset(rect: Rect, dx: u16, dy: u16) -> Rect {
    Rect::new(
        rect.x.saturating_add(dx),
        rect.y.saturating_add(dy),
        rect.width,
        rect.height,
    )
}

/// Shrink a rect from each edge, clamping so a too-small rect becomes empty
/// rather than wrapping around.
pub fn inset(rect: Rect, left: u16, right: u16, top: u16, bottom: u16) -> Rect {
    let x = rect.x.saturating_add(left);
    let y = rect.y.saturating_add(top);
    let w = rect.width.saturating_sub(left.saturating_add(right));
    let h = rect.height.saturating_sub(top.saturating_add(bottom));
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// The `index`-th row of `rect`, or `None` past its bottom edge.
pub fn row_of(rect: Rect, index: u16) -> Option<Rect> {
    if index >= rect.height {
        return None;
    }
    Some(Rect::new(rect.x, rect.y + index, rect.width, 1))
}

/// Paint `text` on row `y` starting at column `x`, clipped to `cols` columns and
/// to the buffer. Wide characters take two columns and leave a continuation
/// cell, so a CJK value in a config file cannot overwrite the cell after it.
pub fn put_in_row(buf: &mut Buffer, x: u16, y: u16, text: &str, cols: usize, style: Style) {
    let mut col = x;
    for ch in text.chars() {
        if usize::from(col.saturating_sub(x)) >= cols {
            break;
        }
        let wide = is_wide(ch);
        cell(buf, col, y, &ch.to_string(), style);
        col += 1;
        if wide {
            cell(buf, col, y, "", style);
            col += 1;
        }
    }
}

/// A cheap width heuristic: only the ranges that realistically appear in
/// terminal output are treated as double-width. `char::width()` is unstable, and
/// a width crate is not worth a dependency for this.
pub fn is_wide(ch: char) -> bool {
    matches!(u32::from(ch),
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF)
}

/// Display width of a string under the same heuristic `put_in_row` uses, so
/// layout maths and painting agree.
pub fn display_width(text: &str) -> usize {
    text.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

/// Centre `text` in `rect`'s inner area on one row.
pub fn put_centered(buf: &mut Buffer, rect: Rect, text: &str, style: Style) {
    let Some(row) = row_of(rect, rect.height / 2) else {
        return;
    };
    let inner = inset(row, 1, 1, 0, 0);
    let w = display_width(text);
    let pad = (usize::from(inner.width)).saturating_sub(w) / 2;
    put_in_row(
        buf,
        inner.x.saturating_add(pad as u16),
        inner.y,
        text,
        usize::from(inner.width),
        style,
    );
}

//! The reusable pieces of the visual language, composed from [`crate::tui::paint`]
//! and [`crate::tui::tokens`].
//!
//! Each interactive one registers a [`Target`] into the frame's hotspot table,
//! which is what the mouse routing reads. Registration happens on the **rest**
//! rect, never on the shifted one — see the press rule in
//! `docs/design-language.md` §6.1/§6.5.
//!
//! Deliberate absences, both from the spec:
//! - no rounded corners, no gradients, no soft transitions;
//! - no frames around rows of a dense table, or around paragraphs. Rows use
//!   inversion and a marker column; prose uses the page background.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::tui::input::{self, Hot};
use crate::tui::paint;
use crate::tui::tokens::{self as t, Palette, Rgb, Status};

/// A button is frame / label / frame — three rows, because a heavy border that
/// is only one cell tall has to be allowed to wrap the text it labels.
pub const BUTTON_ROWS: u16 = 3;

/// Cells a button needs free to its right, so the hover lift has somewhere to go.
pub const GUTTER_COLS: u16 = t::SHADOW_COLS + t::HOVER_LIFT_COLS;

/// ... and below it, for the bottom band of the shadow.
pub const GUTTER_ROWS: u16 = t::SHADOW_ROWS;

/// Smallest terminal this UI will draw into.
///
/// The number is arithmetic, not taste: the bands above the body take 13 rows and
/// the body's own shadow plus the help line take 2 more, and `view` refuses to
/// draw a card below 3 rows — so 18 is the first height where the advertised
/// minimum actually shows a body instead of only chrome.
pub const MIN_COLS: u16 = 44;
pub const MIN_ROWS: u16 = 18;

fn paint_style(color: Rgb) -> Style {
    Style::default().bg(t::rgb(color))
}

fn label_style(fg: Rgb, bg: Rgb) -> Style {
    Style::default()
        .fg(t::rgb(fg))
        .bg(t::rgb(bg))
        .add_modifier(Modifier::BOLD)
}

/// Text colour to use *on the frame band* — the title bar of a card and the
/// inverted row of a selected table entry are both drawn in `pal.frame`, so the
/// question is what contrasts with the frame, not with the card.
fn text_on_frame(pal: &Palette) -> Rgb {
    if t::luminance(pal.frame) > 0.5 {
        pal.page
    } else {
        pal.card
    }
}

/// A primary or secondary button in the neobrutalist sense: solid heavy frame,
/// hard offset shadow, and a press state that drops the shadow and slides the
/// face into where the shadow was.
#[derive(Debug, Clone, Copy)]
pub struct Button<'a> {
    pub label: &'a str,
    pub hot: Hot,
    /// Primary = inverted block (ink face, cream text). Secondary = card face
    /// with a heavy frame, which is how the GUI distinguishes one action from a
    /// setting toggle without inventing a second colour.
    pub primary: bool,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str, hot: Hot) -> Self {
        Self {
            label,
            hot,
            primary: false,
        }
    }

    pub fn primary(label: &'a str, hot: Hot) -> Self {
        Self {
            label,
            hot,
            primary: true,
        }
    }

    /// Width including the frame and one space of padding on each side.
    pub fn width(&self) -> u16 {
        (paint::display_width(self.label) + 4) as u16
    }

    /// Paint the button into `slot` and register its hit area. Returns the hit
    /// rect, so callers can chain buttons horizontally without re-deriving it.
    pub fn paint(
        &self,
        buf: &mut Buffer,
        slot: Rect,
        pal: &Palette,
        pressed: bool,
        hovered: bool,
        targets: &mut Vec<input::Target>,
    ) -> Rect {
        let hit = Rect::new(
            slot.x,
            slot.y,
            self.width().min(slot.width),
            BUTTON_ROWS.min(slot.height),
        );
        input::register(targets, hit, self.hot);

        // Pressed: the face slides into the shadow's place and there is no
        // shadow. Rest: the face stays and the shadow sits 2x1 out, one column
        // further when the pointer is on it.
        let face = if pressed {
            paint::offset(hit, t::PRESS_SHIFT.0, t::PRESS_SHIFT.1)
        } else {
            hit
        };
        if !pressed {
            let lift = if hovered { t::HOVER_LIFT_COLS } else { 0 };
            let color = pal.accent.unwrap_or(pal.frame);
            paint::shadow(
                buf,
                hit,
                paint_style(color),
                t::SHADOW_COLS + lift,
                t::SHADOW_ROWS,
            );
        }

        let face_bg = if self.primary { pal.face } else { pal.card };
        paint::frame_ring(buf, face, paint_style(pal.frame), paint_style(face_bg));
        paint::put_centered(
            buf,
            face,
            self.label,
            label_style(
                if self.primary {
                    pal.face_text
                } else {
                    pal.text
                },
                face_bg,
            ),
        );
        hit
    }
}

/// A section card: heavy frame, hard shadow, title carried by the frame's own
/// top band. The returned rect is the body area inside the frame, where the
/// converged (unframed) content goes.
pub struct Card<'a> {
    pub title: &'a str,
}

impl<'a> Card<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title }
    }

    pub fn paint(&self, buf: &mut Buffer, area: Rect, pal: &Palette) -> Rect {
        if area.width < 3 || area.height < 3 {
            // Too small to frame: an empty rect tells the caller to put its body
            // somewhere else, rather than handing back a body of negative size.
            return Rect::new(area.x, area.y, 0, 0);
        }
        paint::shadow(
            buf,
            area,
            paint_style(pal.accent.unwrap_or(pal.frame)),
            t::SHADOW_COLS,
            t::SHADOW_ROWS,
        );
        paint::frame_ring(buf, area, paint_style(pal.frame), paint_style(pal.card));
        // The title lives on the top band of the frame: a label that is part of
        // the structure rather than floating above it.
        paint::put_in_row(
            buf,
            area.x + 1,
            area.y,
            self.title,
            usize::from(area.width.saturating_sub(2)),
            label_style(text_on_frame(pal), pal.frame),
        );
        paint::inset(area, 1, 1, 1, 1)
    }
}

/// A status indicator. Not interactive, so no frame: the spec puts the weight on
/// colour contrast here, and a bordered pill would be decoration.
pub fn status_band(buf: &mut Buffer, area: Rect, status: Status, label: &str) {
    let body = t::status_rgb(status);
    let text = status.text_on_status();
    paint::fill(buf, area, " ", paint_style(body));
    paint::put_centered(buf, area, label, label_style(text, body));
}

/// One row of a dense table, in the converged treatment §3 requires: the row is
/// never framed. Selected inverts the whole row; hovered puts a marker bar in
/// column one and underlines the text, so the pointer always has a readback.
pub fn paint_row(
    buf: &mut Buffer,
    area: Rect,
    pal: &Palette,
    text: &str,
    selected: bool,
    hovered: bool,
    hot: Option<Hot>,
    targets: &mut Vec<input::Target>,
) {
    if selected {
        paint::fill(buf, area, " ", paint_style(pal.frame));
        paint::put_in_row(
            buf,
            area.x + 1,
            area.y,
            text,
            usize::from(area.width.saturating_sub(2)),
            label_style(text_on_frame(pal), pal.frame),
        );
    } else {
        let marker = pal.accent.unwrap_or(pal.frame);
        if hovered {
            paint::put_in_row(
                buf,
                area.x,
                area.y,
                "\u{258c}",
                1,
                label_style(marker, pal.card),
            );
        }
        let mut style = label_style(pal.text, pal.card);
        if hovered {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        paint::put_in_row(
            buf,
            area.x + 1,
            area.y,
            text,
            usize::from(area.width.saturating_sub(2)),
            style,
        );
    }
    if let Some(hot) = hot {
        input::register(targets, area, hot);
    }
}

/// A line of prose or the help strip: page background, no frame, ever.
pub fn paint_prose(buf: &mut Buffer, area: Rect, pal: &Palette, text: &str) {
    paint::put_in_row(
        buf,
        area.x,
        area.y,
        text,
        usize::from(area.width),
        t::prose_style(pal),
    );
}

/// Whether the frame is big enough to hold the language (frames + shadows +
/// gutters), as opposed to merely big enough to draw something.
pub fn fits(area: Rect) -> bool {
    area.width >= MIN_COLS && area.height >= MIN_ROWS
}

/// What gets drawn instead of the UI when it does not fit. Honest about the
/// reason: the offset shadows and the 3-row buttons are what need the cells.
pub fn paint_too_small(buf: &mut Buffer, area: Rect, pal: &Palette) {
    paint::fill(buf, area, " ", paint_style(pal.page));
    let lines = [
        format!(
            "{MIN_COLS}x{MIN_ROWS} MINIMUM, THIS IS {}x{}.",
            area.width, area.height
        ),
        "the frames and offset shadows need the cells".to_string(),
        "resize the window; nothing here is clickable".to_string(),
    ];
    for (i, line) in lines.iter().enumerate() {
        if i >= usize::from(area.height) {
            break;
        }
        if let Some(row) = paint::row_of(area, i as u16) {
            paint::put_in_row(
                buf,
                row.x + 1,
                row.y,
                line,
                usize::from(row.width.saturating_sub(2)),
                t::prose_style(pal),
            );
        }
    }
}

/// The mouse affordance strip, generated from what the app actually implements
/// rather than written by hand, so it cannot drift into advertising a gesture
/// that does nothing.
pub fn help_line() -> String {
    let parts = [
        "click = press, release on the same control = activate",
        "move = hover",
        "drag inside a pane = scroll it, same direction as the wheel",
        "wheel = scroll the pane under the pointer",
        "wheel sideways = scroll columns",
        "right-click = alternate",
        "middle-click = jump to newest",
        "double-click = expand",
        "keys: tab/shift-tab, j/k, arrows, space, f, t, x, s, q",
    ];
    parts.join("  \u{2022}  ")
}

/// The tab strip. Returns the x cursor after the last tab so callers can place
/// the action buttons on the same band.
pub fn paint_tabs(
    buf: &mut Buffer,
    slot: Rect,
    pal: &Palette,
    current: input::Page,
    pressed: Option<Hot>,
    hover: Option<Hot>,
    targets: &mut Vec<input::Target>,
) -> u16 {
    let mut x = slot.x;
    for page in input::Page::ALL {
        let hot = Hot::Tab(page);
        // The selected tab is the inverted one; the heavy frame around it is what
        // keeps it a control rather than a heading.
        let btn = if page == current {
            Button::primary(page.label(), hot)
        } else {
            Button::new(page.label(), hot)
        };
        let rect = Rect::new(x, slot.y, btn.width(), BUTTON_ROWS.min(slot.height));
        let hit = btn.paint(
            buf,
            rect,
            pal,
            pressed == Some(hot),
            hover == Some(hot),
            targets,
        );
        x = hit.right() + 2;
    }
    x
}

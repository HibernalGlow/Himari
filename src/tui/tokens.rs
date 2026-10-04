//! Neobrutalism tokens for the terminal — the same palette the Iced GUI uses,
//! expressed in the only two things a terminal gives you: cells and RGB colours.
//!
//! Ground truth for the colour values is `crates/himari-ui/src/tokens.rs` in the
//! Himari app repo (mirrored byte-for-byte below and pinned by
//! `tests/tui_visual_language.rs`). The mapping from GUI rule to cell rule is
//! documented in `docs/design-language.md` §6.
//!
//! Two rules from the spec drive the shape of this module:
//! - heavy borders go on interactive elements *only*, so `frame_*` is used by
//!   buttons/cards and never by paragraphs of text;
//! - the press animation is "shadow disappears + element moves", so a pressed
//!   element must paint its face at an offset and paint no shadow.

use ratatui::style::{Color, Modifier, Style};

/// An RGB triple, kept separate from `ratatui::style::Color` so the values can
/// be compared against the Iced tokens without pulling the terminal into it.
pub type Rgb = (u8, u8, u8);

// --- Palette: cream + black + status colours, identical to the GUI tokens ---
pub const CREAM: Rgb = (0xF4, 0xF1, 0xEA);
pub const INK: Rgb = (0x11, 0x11, 0x11);
pub const DARK_SURFACE: Rgb = (0x1A, 0x19, 0x17);
pub const DARK_TEXT: Rgb = (0xE8, 0xE6, 0xE1);

pub const SUCCESS: Rgb = (0x1A, 0x7A, 0x33);
pub const WARNING: Rgb = (0xB7, 0x7E, 0x33);
pub const DANGER: Rgb = (0xC0, 0x39, 0x2B);

/// How far a drop shadow sits from its element, in cells.
///
/// The GUI uses a 3px offset on both axes. A terminal cell is roughly twice as
/// tall as it is wide, so 2 columns x 1 row is the nearest square offset the
/// grid can express; that is also what keeps a shadow inside a narrow window.
pub const SHADOW_COLS: u16 = 2;
pub const SHADOW_ROWS: u16 = 1;

/// Hover lifts the shadow one column further from the element — the cell-grid
/// version of "this thing is closer to you than the page".
pub const HOVER_LIFT_COLS: u16 = 1;

/// A pressed element paints its face here instead of on its own rect: exactly
/// where its shadow was. The *hit area* stays on the unpressed rect, so a
/// press-release pair separated by the shift still activates the same control.
pub const PRESS_SHIFT: (u16, u16) = (SHADOW_COLS, SHADOW_ROWS);

/// The face colour of a card, deliberately different from the page colour.
/// A card that matches its page would erase the whole hierarchy of this style;
/// `tests/tui_visual_language.rs` fails if these two ever collide.
pub const fn card(dark: bool) -> Rgb {
    if dark {
        (0x24, 0x22, 0x1F)
    } else {
        (0xFF, 0xFF, 0xFF)
    }
}

/// Every colour one frame needs, resolved once per theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    /// Window background.
    pub page: Rgb,
    /// Card / panel face.
    pub card: Rgb,
    /// Text on `page` and `card`.
    pub text: Rgb,
    /// Borders and drop shadows.
    pub frame: Rgb,
    /// Face of a primary button (the inverted block).
    pub face: Rgb,
    /// Text on `face`.
    pub face_text: Rgb,
    /// User-supplied accent, or `None` while it stays the app's open decision.
    pub accent: Option<Rgb>,
}

impl Palette {
    /// Build a palette.
    ///
    /// `frame` is the one value that does *not* mirror the Iced tokens verbatim:
    /// `crates/himari-ui/src/tokens.rs::frame()` returns `INK` for both themes,
    /// and `#111111` on a `#1A1917` dark page is a ~1.1:1 border — invisible.
    /// This style's hierarchy is contrast, so the dark theme draws its frame in
    /// the text colour instead. That deviation is recorded in
    /// `docs/design-language.md` §6.3 rather than hidden.
    pub fn new(dark: bool, accent: Option<Rgb>) -> Self {
        if dark {
            Self {
                dark,
                page: DARK_SURFACE,
                card: card(true),
                text: DARK_TEXT,
                frame: DARK_TEXT,
                face: DARK_TEXT,
                face_text: DARK_SURFACE,
                accent,
            }
        } else {
            Self {
                dark,
                page: CREAM,
                card: card(false),
                text: INK,
                frame: INK,
                face: INK,
                face_text: CREAM,
                accent,
            }
        }
    }
}

pub const fn rgb(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// Parse a `#RRGGBB` / `RRGGBB` accent from the command line.
pub fn parse_hex(text: &str) -> Option<Rgb> {
    let text = text.strip_prefix('#').unwrap_or(text);
    if text.len() != 6 {
        return None;
    }
    let mut bytes = [0u8; 3];
    for (i, slot) in bytes.iter_mut().enumerate() {
        let octet = &text[i * 2..i * 2 + 2];
        *slot = u8::from_str_radix(octet, 16).ok()?;
    }
    Some((bytes[0], bytes[1], bytes[2]))
}

/// Text: bold, no soft anything. The terminal cannot choose the typeface, so
/// weight is the whole of the grotesque/condensed requirement that survives.
pub fn text_style(p: &Palette) -> Style {
    Style::default()
        .fg(rgb(p.text))
        .bg(rgb(p.card))
        .add_modifier(Modifier::BOLD)
}

/// A paragraph of prose — the one place the spec forbids borders, so it gets
/// the page background and no frame.
pub fn prose_style(p: &Palette) -> Style {
    Style::default()
        .fg(rgb(p.text))
        .bg(rgb(p.page))
        .add_modifier(Modifier::BOLD)
}

/// The face of an interactive element at rest.
pub fn face_style(p: &Palette) -> Style {
    Style::default()
        .fg(rgb(p.face_text))
        .bg(rgb(p.face))
        .add_modifier(Modifier::BOLD)
}

/// A status colour for the six states the app can be in, expressed as colour
/// contrast rather than icon decoration (brief §35: the UI only renders state).
pub fn status_rgb(state: Status) -> Rgb {
    match state {
        Status::Running => SUCCESS,
        Status::Starting | Status::Degraded => WARNING,
        Status::Stopped => INK,
        Status::Failed => DANGER,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Starting,
    Running,
    Stopped,
    Degraded,
    Failed,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "STARTING",
            Self::Running => "RUNNING",
            Self::Stopped => "STOPPED",
            Self::Degraded => "DEGRADED",
            Self::Failed => "FAILED",
        }
    }

    /// Ink or cream, whichever measures better against the status colour.
    ///
    /// This is a measurement, not a rule of thumb. The first version asked
    /// "is the body dark? then cream", and that loses on `WARNING`: cream text on
    /// `#B77E33` measures 3.11:1 while ink measures 4.83:1. The amber pill would
    /// have shipped below the same standard this file applies everywhere else, so
    /// the candidate is now chosen by `contrast`, and
    /// `tests/tui_visual_language.rs` pins the winner per state — a future
    /// shortcut has to beat the measurement to pass.
    pub fn text_on_status(self) -> Rgb {
        let body = status_rgb(self);
        [INK, CREAM]
            .into_iter()
            .max_by(|a, b| {
                contrast(body, *a)
                    .partial_cmp(&contrast(body, *b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(CREAM)
    }

    /// What the chosen label colour actually measures against the body.
    pub fn label_contrast(self) -> f64 {
        contrast(status_rgb(self), self.text_on_status())
    }
}

/// Relative luminance approximation, used for contrast decisions only.
pub fn luminance(c: Rgb) -> f64 {
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
}

/// Whether two colours are far enough apart to carry *text*. The threshold is the
/// WCAG small-text ratio, not a made-up number.
pub fn distinguishable(a: Rgb, b: Rgb) -> bool {
    contrast(a, b) >= 4.5
}

/// Non-text contrast (WCAG 1.4.11): the gauge for a *boundary* — a frame, a
/// shadow, an inverted band — as opposed to glyphs, which use `distinguishable`.
///
/// The two are deliberately separate. A cream page with a white card face
/// measures 1.13:1 apart, and no surface-only choice fixes that with a text-level
/// threshold; what makes the card read as a card is the solid frame around it, so
/// that is what gets measured (`docs/design-language.md` §6.2).
pub fn boundary(a: Rgb, b: Rgb) -> bool {
    contrast(a, b) >= 3.0
}

/// A contrast ratio as the style guide quotes it, so the number in the doc comes
/// from code rather than from a note that can drift.
pub fn ratio_text(a: Rgb, b: Rgb) -> String {
    format!("{:.2}:1", contrast(a, b))
}

pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (hi, lo) = (luminance(a), luminance(b));
    let (hi, lo) = if hi >= lo { (hi, lo) } else { (lo, hi) };
    (hi + 0.05) / (lo + 0.05)
}

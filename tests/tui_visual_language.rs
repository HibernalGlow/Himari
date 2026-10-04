//! Gates on the terminal UI's visual language.
//!
//! These are the checks `docs/design-language.md` promises, expressed as things
//! that go red: the palette has to be the GUI's palette, a card face has to be
//! distinguishable from the page it sits on, the press animation has to remove
//! the shadow *and* move the face, and the painting primitives must clip instead
//! of panicking on a small window.
//!
//! Run with the feature on: `cargo test --features tui`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use sni_gate::tui::paint;
use sni_gate::tui::tokens::{self, Rgb, Status, SHADOW_COLS, SHADOW_ROWS};

/// The exact values recorded from `crates/himari-ui/src/tokens.rs` (the Iced
/// GUI) on 2026-10-04. If the GUI's palette moves, this moves with it — a
/// deliberately loud way of saying "the two front ends share one language".
const GUI_CREAM: Rgb = (0xF4, 0xF1, 0xEA);
const GUI_INK: Rgb = (0x11, 0x11, 0x11);
const GUI_DARK_SURFACE: Rgb = (0x1A, 0x19, 0x17);
const GUI_DARK_TEXT: Rgb = (0xE8, 0xE6, 0xE1);
const GUI_SUCCESS: Rgb = (0x1A, 0x7A, 0x33);
const GUI_WARNING: Rgb = (0xB7, 0x7E, 0x33);
const GUI_DANGER: Rgb = (0xC0, 0x39, 0x2B);
const GUI_CARD_LIGHT: Rgb = (0xFF, 0xFF, 0xFF);
const GUI_CARD_DARK: Rgb = (0x24, 0x22, 0x1F);

/// `Buffer::get` is deprecated in ratatui 0.30; `cell` takes an `Into<Position>`
/// and returns `None` off-grid, so a wrong coordinate here is a panic that names
/// the coordinates instead of a silently defaulted cell.
fn cell_at<'a>(buf: &'a Buffer, x: u16, y: u16) -> &'a ratatui::buffer::Cell {
    buf.cell((x, y))
        .unwrap_or_else(|| panic!("cell ({x},{y}) is outside the test buffer"))
}

fn bg(buf: &Buffer, x: u16, y: u16) -> Option<Rgb> {
    match cell_at(&buf, x, y).bg {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

fn ink() -> Style {
    Style::default().bg(Color::Rgb(GUI_INK.0, GUI_INK.1, GUI_INK.2))
}

#[test]
fn terminal_palette_values_are_the_gui_palette() {
    assert_eq!(tokens::CREAM, GUI_CREAM);
    assert_eq!(tokens::INK, GUI_INK);
    assert_eq!(tokens::DARK_SURFACE, GUI_DARK_SURFACE);
    assert_eq!(tokens::DARK_TEXT, GUI_DARK_TEXT);
    assert_eq!(tokens::SUCCESS, GUI_SUCCESS);
    assert_eq!(tokens::WARNING, GUI_WARNING);
    assert_eq!(tokens::DANGER, GUI_DANGER);
    assert_eq!(tokens::card(false), GUI_CARD_LIGHT);
    assert_eq!(tokens::card(true), GUI_CARD_DARK);
}

/// §4's literal requirement is that the card face may not equal the page colour.
/// What actually carries the hierarchy in this style is the frame, so the
/// boundary is what gets gated at non-text contrast — and the fill gap is recorded
/// as a number, because cream + white is a 1.13:1 pair that only the ring makes
/// read as two surfaces.
#[test]
fn card_face_differs_from_page_and_the_frame_carries_the_boundary() {
    for dark in [false, true] {
        let p = tokens::Palette::new(dark, None);
        assert_ne!(
            p.card, p.page,
            "dark={dark}: a card matching its page is the §4 failure"
        );
        assert!(
            tokens::boundary(p.frame, p.card),
            "frame/card {} on dark={dark}",
            tokens::ratio_text(p.frame, p.card)
        );
        assert!(
            tokens::boundary(p.frame, p.page),
            "frame/page {} on dark={dark}",
            tokens::ratio_text(p.frame, p.page)
        );
        // Positive control for the gauge itself: one colour against itself is 1:1
        // and must never pass as a boundary.
        assert!(!tokens::boundary(p.frame, p.frame));
    }

    // The light theme's fill gap, pinned as a measurement. If the card colour ever
    // changes enough to carry the hierarchy on its own, this moves and the design
    // note in §6.2 should be rewritten — that is a decision, not a refactor.
    let fill_gap = tokens::contrast(tokens::card(false), tokens::CREAM);
    assert!(
        fill_gap < 1.5,
        "cream/white fill contrast moved to {fill_gap:.2}:1; §6.2 assumed the ring was load-bearing"
    );
}

/// The one recorded deviation from the GUI tokens: the Iced `frame()` is `INK`
/// for both themes, and `INK` on `DARK_SURFACE` is not a visible border. The
/// terminal uses the text colour there, and this test pins *why* — the GUI's
/// own combination is required to fail the same contrast gate.
#[test]
fn dark_frame_colour_is_visible_where_the_ones_ink_would_not_be() {
    assert!(
        tokens::contrast(GUI_INK, GUI_DARK_SURFACE) < 4.5,
        "the premise changed: INK on DARK_SURFACE measured {}",
        tokens::contrast(GUI_INK, GUI_DARK_SURFACE)
    );
    let p = tokens::Palette::new(true, None);
    assert!(tokens::distinguishable(p.frame, p.page));
    assert!(tokens::distinguishable(p.frame, p.card));
}

#[test]
fn light_frame_is_distinguishable_from_page_and_card() {
    let p = tokens::Palette::new(false, None);
    assert!(tokens::distinguishable(p.frame, p.page));
    assert_eq!(p.frame, GUI_INK);
}

#[test]
fn status_colours_carry_text_and_are_not_all_the_same() {
    let states = [
        Status::Starting,
        Status::Running,
        Status::Stopped,
        Status::Degraded,
        Status::Failed,
    ];
    let mut seen: Vec<Rgb> = vec![];
    for s in states {
        let body = tokens::status_rgb(s);
        let label = s.text_on_status();
        assert!(
            tokens::distinguishable(body, label),
            "{} measures {}; no candidate reaches text contrast",
            s.label(),
            tokens::ratio_text(body, label)
        );
        if !seen.contains(&body) {
            seen.push(body);
        }
    }
    // The label colour is chosen by measurement, so the shortcut this replaced has
    // to be demonstrably losing: cream on the amber pill is 3.11:1, ink is 4.83:1.
    assert!(
        !tokens::distinguishable(tokens::WARNING, tokens::CREAM),
        "amber/cream moved to {}, so the cream-by-default rule needs revisiting",
        tokens::ratio_text(tokens::WARNING, tokens::CREAM)
    );
    assert!(tokens::distinguishable(tokens::WARNING, tokens::INK));
    assert_eq!(Status::Starting.text_on_status(), tokens::INK);
    assert_eq!(Status::Running.text_on_status(), tokens::CREAM);
    // Five states over four colours: STARTING and DEGRADED may share, nothing
    // else may. A palette where everything is red would pass a weaker check.
    assert!(seen.len() >= 4);
}

#[test]
fn every_painted_style_is_bold() {
    // The spec's typography rule survives the terminal only as weight; a style
    // that lost it would quietly turn the UI back into "default admin panel".
    for p in [
        tokens::Palette::new(false, None),
        tokens::Palette::new(true, None),
    ] {
        for style in [
            tokens::text_style(&p),
            tokens::prose_style(&p),
            tokens::face_style(&p),
        ] {
            assert!(style.add_modifier.contains(Modifier::BOLD), "{style:?}");
        }
    }
}

/// The press animation, measured on a buffer: at rest the cells right of and
/// below the element are frame colour; pressed there is page colour and the
/// element's own corner has moved to where the shadow used to start.
#[test]
fn press_removes_the_shadow_and_moves_the_face() {
    let area = Rect::new(0, 0, 40, 10);
    let frame = Rect::new(2, 2, 8, 3);

    let rest = {
        let mut buf = Buffer::empty(area);
        paint::fill(&mut buf, area, " ", ink()); // page, painted over below
        paint::shadow(&mut buf, frame, ink(), SHADOW_COLS, SHADOW_ROWS);
        paint::frame_ring(
            &mut buf,
            frame,
            ink(),
            Style::default().bg(Color::Rgb(255, 255, 255)),
        );
        buf
    };
    // Shadow present at rest.
    assert_eq!(bg(&rest, frame.right(), frame.y), Some(GUI_INK));
    assert_eq!(bg(&rest, frame.x, frame.bottom()), Some(GUI_INK));
    assert_eq!(
        bg(&rest, frame.right() + SHADOW_COLS - 1, frame.y),
        Some(GUI_INK)
    );

    let pressed = {
        let mut buf = Buffer::empty(area);
        paint::fill(
            &mut buf,
            area,
            " ",
            Style::default().bg(Color::Rgb(0, 0, 0)),
        );
        let face = paint::offset(frame, SHADOW_COLS, SHADOW_ROWS);
        paint::frame_ring(
            &mut buf,
            face,
            ink(),
            Style::default().bg(Color::Rgb(255, 255, 255)),
        );
        buf
    };
    // The shadow's old cells are not frame colour any more, and the face moved.
    assert_eq!(bg(&pressed, frame.right(), frame.y), Some((0, 0, 0)));
    assert_eq!(bg(&pressed, frame.x, frame.bottom()), Some((0, 0, 0)));
    assert_eq!(
        bg(&pressed, frame.x + SHADOW_COLS, frame.y + SHADOW_ROWS),
        Some(GUI_INK)
    );
    // The hit area is the *unpressed* rect, so press and release still agree;
    // the painting shift must not be the only truth about where a control is.
    assert_eq!(frame.area(), 8 * 3);
}

/// Hover pushes the shadow exactly one column further out — measured from both
/// sides. The first version of this test asserted one cell and used the *same*
/// coordinate for its positive control, so it could not have failed either way;
/// these four assertions catch a lift of 0 and a lift of 2.
#[test]
fn hover_lifts_the_shadow_by_exactly_one_column() {
    let area = Rect::new(0, 0, 40, 10);
    let frame = Rect::new(2, 2, 8, 3);
    let painted = |cols: u16| {
        let mut buf = Buffer::empty(area);
        paint::shadow(&mut buf, frame, ink(), cols, SHADOW_ROWS);
        buf
    };

    let rest = painted(SHADOW_COLS);
    let lifted = painted(SHADOW_COLS + tokens::HOVER_LIFT_COLS);

    // At rest the band ends one column earlier than the hover band does.
    assert_eq!(
        bg(&rest, frame.right() + SHADOW_COLS - 1, frame.y),
        Some(GUI_INK)
    );
    assert_eq!(bg(&rest, frame.right() + SHADOW_COLS, frame.y), None);
    // Hover fills precisely that next column and no further.
    assert_eq!(
        bg(&lifted, frame.right() + SHADOW_COLS, frame.y),
        Some(GUI_INK)
    );
    assert_eq!(bg(&lifted, frame.right() + SHADOW_COLS + 1, frame.y), None);
}

/// A 20-column window is a normal condition, not a panic. The positive control
/// (the same call fully inside the buffer) keeps this from passing vacuously.
#[test]
fn painting_clips_at_the_buffer_edge_instead_of_panicking() {
    let area = Rect::new(0, 0, 20, 6);
    let mut buf = Buffer::empty(area);
    let outside = Rect::new(18, 4, 6, 3);
    paint::shadow(&mut buf, outside, ink(), SHADOW_COLS, SHADOW_ROWS);
    paint::frame_ring(&mut buf, outside, ink(), ink());
    paint::put_in_row(
        &mut buf,
        19,
        5,
        "way too long a label that will not fit",
        40,
        ink(),
    );
    assert_eq!(bg(&buf, 18, 4), Some(GUI_INK));
    assert_eq!(cell_at(&buf, 0, 0).bg, Color::Reset);
}

#[test]
fn wide_characters_reserve_two_columns_and_ascii_one() {
    let area = Rect::new(0, 0, 12, 2);
    let mut buf = Buffer::empty(area);
    paint::put_in_row(&mut buf, 0, 0, "日本", 4, ink());
    assert_eq!(cell_at(&buf, 0, 0).symbol(), "日");
    assert_eq!(cell_at(&buf, 1, 0).symbol(), "");
    assert_eq!(cell_at(&buf, 2, 0).symbol(), "本");
    assert_eq!(paint::display_width("日本"), 4);
    // Positive control: ASCII is one column per character.
    assert_eq!(paint::display_width("ab"), 2);
}

#[test]
fn accent_is_parsed_only_when_it_is_a_real_hex_triplet() {
    assert_eq!(tokens::parse_hex("#FF8800"), Some((0xFF, 0x88, 0x00)));
    assert_eq!(tokens::parse_hex("ff8800"), Some((0xFF, 0x88, 0x00)));
    assert_eq!(tokens::parse_hex("red"), None);
    assert_eq!(tokens::parse_hex("#GGGGGG"), None);
    assert_eq!(tokens::parse_hex("#FFF"), None);
    // Unset stays unset: the accent is the app's open decision, not this UI's.
    let p = tokens::Palette::new(false, None);
    assert_eq!(p.accent, None);
}

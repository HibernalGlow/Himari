//! The mouse contract, tested without a terminal.
//!
//! `docs/design-language.md` §6.5 makes four promises about pointer behaviour:
//! press and release are separate steps, the hit area is the unpressed rect, the
//! wheel aims at whatever is under the cursor, and the keyboard can do everything
//! the mouse can. Each is a thing that can go red, so each is a test here.
//!
//! Run with: `cargo test --features tui`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::Color;

use sni_gate::tui::input::{self, Hot, MouseCmd, Page, Target, Wheel, DOUBLE_CLICK_MS};
use sni_gate::tui::paint;
use sni_gate::tui::tokens::{self, Palette, Status};
use sni_gate::tui::widgets;

/// Log pane on rows 0-5, config pane on rows 6-11, a Start button at (20,13),
/// and a row of the table at (1,7) drawn *after* the config pane so it is on top.
fn frame_table() -> Vec<Target> {
    vec![
        Target {
            rect: Rect::new(0, 0, 40, 6),
            hot: Hot::LogPane,
        },
        Target {
            rect: Rect::new(0, 6, 40, 6),
            hot: Hot::ConfigPane,
        },
        Target {
            rect: Rect::new(1, 7, 30, 1),
            hot: Hot::Row(2),
        },
        Target {
            rect: Rect::new(20, 13, 9, 3),
            hot: Hot::Start,
        },
    ]
}

fn route(
    kind: MouseEventKind,
    x: u16,
    y: u16,
    targets: &[Target],
    pointer: &mut input::Pointer,
    now_ms: u64,
) -> MouseCmd {
    input::route_mouse(kind, x, y, targets, pointer, now_ms)
}

#[test]
fn hit_returns_the_topmost_region() {
    let targets = frame_table();
    // (1,7) is inside both ConfigPane and Row(2); Row(2) was registered later.
    assert_eq!(input::hit(&targets, 1, 7), Some(Hot::Row(2)));
    // Positive control: one row up, the row does not cover it.
    assert_eq!(input::hit(&targets, 1, 6), Some(Hot::ConfigPane));
    assert_eq!(input::hit(&targets, 39, 0), Some(Hot::LogPane));
    assert_eq!(input::hit(&targets, 39, 12), None);
}

#[test]
fn empty_rects_are_never_hitable() {
    let mut targets = Vec::new();
    input::register(&mut targets, Rect::new(5, 5, 0, 3), Hot::Start);
    input::register(&mut targets, Rect::new(5, 9, 0, 0), Hot::Stop);
    assert!(targets.is_empty());
    // Positive control.
    input::register(&mut targets, Rect::new(5, 5, 3, 1), Hot::Start);
    assert_eq!(input::hit(&targets, 6, 5), Some(Hot::Start));
}

#[test]
fn a_click_is_press_then_release_on_the_same_region() {
    let targets = frame_table();
    let mut pointer = input::Pointer::default();
    assert_eq!(
        route(
            MouseEventKind::Down(MouseButton::Left),
            22,
            14,
            &targets,
            &mut pointer,
            100
        ),
        MouseCmd::Press(Hot::Start)
    );
    // While pressed, the app knows what to draw in the pressed style.
    assert_eq!(pointer.pressed(), Some(Hot::Start));
    assert_eq!(
        route(
            MouseEventKind::Up(MouseButton::Left),
            22,
            14,
            &targets,
            &mut pointer,
            160
        ),
        MouseCmd::Activate(Hot::Start)
    );
    assert_eq!(pointer.pressed(), None);
}

#[test]
fn releasing_outside_the_control_cancels_it() {
    let targets = frame_table();
    let mut pointer = input::Pointer::default();
    route(
        MouseEventKind::Down(MouseButton::Left),
        22,
        14,
        &targets,
        &mut pointer,
        100,
    );
    assert_eq!(
        route(
            MouseEventKind::Up(MouseButton::Left),
            3,
            3,
            &targets,
            &mut pointer,
            160
        ),
        MouseCmd::Release
    );
    assert_eq!(pointer.pressed(), None);

    // And a release onto a *different* control does not activate that one either.
    let mut pointer = input::Pointer::default();
    route(
        MouseEventKind::Down(MouseButton::Left),
        22,
        14,
        &targets,
        &mut pointer,
        100,
    );
    assert_eq!(
        route(
            MouseEventKind::Up(MouseButton::Left),
            5,
            5,
            &targets,
            &mut pointer,
            160
        ),
        MouseCmd::Release
    );
}

#[test]
fn double_click_window_is_exactly_double_click_ms() {
    let targets = frame_table();
    let inside = DOUBLE_CLICK_MS - 1;
    let mut pointer = input::Pointer::default();
    route(
        MouseEventKind::Down(MouseButton::Left),
        22,
        14,
        &targets,
        &mut pointer,
        0,
    );
    assert_eq!(
        route(
            MouseEventKind::Down(MouseButton::Left),
            22,
            14,
            &targets,
            &mut pointer,
            inside
        ),
        MouseCmd::Double(Hot::Start)
    );

    // The other side of the boundary: at the window size it is a fresh press.
    let mut pointer = input::Pointer::default();
    route(
        MouseEventKind::Down(MouseButton::Left),
        22,
        14,
        &targets,
        &mut pointer,
        0,
    );
    assert_eq!(
        route(
            MouseEventKind::Down(MouseButton::Left),
            22,
            14,
            &targets,
            &mut pointer,
            DOUBLE_CLICK_MS
        ),
        MouseCmd::Press(Hot::Start)
    );

    // A double click elsewhere in time on a different region is not a double.
    let mut pointer = input::Pointer::default();
    route(
        MouseEventKind::Down(MouseButton::Left),
        22,
        14,
        &targets,
        &mut pointer,
        0,
    );
    assert_eq!(
        route(
            MouseEventKind::Down(MouseButton::Left),
            5,
            5,
            &targets,
            &mut pointer,
            10
        ),
        MouseCmd::Press(Hot::LogPane)
    );
}

#[test]
fn drag_scroll_is_incremental_and_stays_inside_the_pressed_region() {
    let targets = frame_table();
    let mut pointer = input::Pointer::default();
    route(
        MouseEventKind::Down(MouseButton::Left),
        5,
        2,
        &targets,
        &mut pointer,
        0,
    );
    assert_eq!(
        route(
            MouseEventKind::Drag(MouseButton::Left),
            5,
            5,
            &targets,
            &mut pointer,
            10
        ),
        MouseCmd::Drag {
            over: Hot::LogPane,
            dx: 0,
            dy: 3
        }
    );
    // The next delta is measured from the previous position, not from the origin.
    assert_eq!(
        route(
            MouseEventKind::Drag(MouseButton::Left),
            5,
            3,
            &targets,
            &mut pointer,
            20
        ),
        MouseCmd::Drag {
            over: Hot::LogPane,
            dx: 0,
            dy: -2
        }
    );
    // Dragged off the region: no scroll, and the press is still tracked so the
    // eventual release cancels rather than activating.
    assert_eq!(
        route(
            MouseEventKind::Drag(MouseButton::Left),
            5,
            20,
            &targets,
            &mut pointer,
            30
        ),
        MouseCmd::Ignore
    );
    assert_eq!(
        route(
            MouseEventKind::Up(MouseButton::Left),
            5,
            20,
            &targets,
            &mut pointer,
            40
        ),
        MouseCmd::Release
    );
    // A drag without a press is not a gesture at all.
    let mut idle = input::Pointer::default();
    assert_eq!(
        route(
            MouseEventKind::Drag(MouseButton::Left),
            5,
            5,
            &targets,
            &mut idle,
            0
        ),
        MouseCmd::Ignore
    );
}

#[test]
fn the_wheel_scrolls_what_is_under_the_pointer() {
    let targets = frame_table();
    let mut pointer = input::Pointer::default();
    assert_eq!(
        route(MouseEventKind::ScrollDown, 5, 1, &targets, &mut pointer, 0),
        MouseCmd::Wheel {
            over: Hot::LogPane,
            dir: Wheel::Down
        }
    );
    assert_eq!(
        route(MouseEventKind::ScrollDown, 5, 7, &targets, &mut pointer, 0),
        MouseCmd::Wheel {
            over: Hot::Row(2),
            dir: Wheel::Down
        }
    );
    assert_eq!(
        route(MouseEventKind::ScrollUp, 5, 9, &targets, &mut pointer, 0),
        MouseCmd::Wheel {
            over: Hot::ConfigPane,
            dir: Wheel::Up
        }
    );
    // Sideways is horizontal scrolling, not a no-op.
    assert_eq!(
        route(MouseEventKind::ScrollLeft, 5, 9, &targets, &mut pointer, 0),
        MouseCmd::Wheel {
            over: Hot::ConfigPane,
            dir: Wheel::Left
        }
    );
    assert_eq!(
        route(MouseEventKind::ScrollRight, 5, 9, &targets, &mut pointer, 0),
        MouseCmd::Wheel {
            over: Hot::ConfigPane,
            dir: Wheel::Right
        }
    );
    // Over nothing: reported, not silently attributed to the focused pane.
    assert_eq!(
        route(
            MouseEventKind::ScrollDown,
            39,
            20,
            &targets,
            &mut pointer,
            0
        ),
        MouseCmd::Ignore
    );
}

#[test]
fn hover_reads_the_pointer_position_including_off_control() {
    let targets = frame_table();
    let mut pointer = input::Pointer::default();
    assert_eq!(
        route(MouseEventKind::Moved, 22, 14, &targets, &mut pointer, 0),
        MouseCmd::Hover(Some(Hot::Start))
    );
    assert_eq!(
        route(MouseEventKind::Moved, 39, 20, &targets, &mut pointer, 0),
        MouseCmd::Hover(None)
    );
}

#[test]
fn right_click_alternates_and_middle_click_jumps() {
    let targets = frame_table();
    let mut pointer = input::Pointer::default();
    assert_eq!(
        route(
            MouseEventKind::Down(MouseButton::Right),
            5,
            9,
            &targets,
            &mut pointer,
            0
        ),
        MouseCmd::Alternate(Hot::ConfigPane)
    );
    assert_eq!(
        route(
            MouseEventKind::Down(MouseButton::Middle),
            5,
            1,
            &targets,
            &mut pointer,
            0
        ),
        MouseCmd::Jump(Hot::LogPane)
    );
    assert_eq!(
        route(
            MouseEventKind::Down(MouseButton::Right),
            39,
            20,
            &targets,
            &mut pointer,
            0
        ),
        MouseCmd::Ignore
    );
    // Neither of them enters the press state, so neither paints a pressed face.
    assert_eq!(pointer.pressed(), None);
}

/// The exhaustive-name function: add a `MouseCmd` variant without naming it here
/// and this stops compiling. That is what makes the set equality below a gate and
/// not a wish — the alternative (a hand-written list of names) would happily keep
/// passing while a new variant sat unreachable.
fn tag(cmd: &MouseCmd) -> &'static str {
    match cmd {
        MouseCmd::Press(_) => "Press",
        MouseCmd::Activate(_) => "Activate",
        MouseCmd::Alternate(_) => "Alternate",
        MouseCmd::Jump(_) => "Jump",
        MouseCmd::Hover(_) => "Hover",
        MouseCmd::Drag { .. } => "Drag",
        MouseCmd::Wheel { .. } => "Wheel",
        MouseCmd::Double(_) => "Double",
        MouseCmd::Release => "Release",
        MouseCmd::Ignore => "Ignore",
    }
}

/// The completeness gate for "支持鼠标交互": one scripted session must produce
/// every command the router can name. A command nobody can produce is an
/// interface promising a gesture that does not exist.
#[test]
fn every_mouse_command_has_a_producer() {
    use std::collections::BTreeSet;

    let targets = frame_table();
    let mut pointer = input::Pointer::default();
    let script: [(MouseEventKind, u16, u16, u64); 13] = [
        (MouseEventKind::Down(MouseButton::Left), 22, 14, 0),
        (MouseEventKind::Up(MouseButton::Left), 22, 14, 50),
        (MouseEventKind::Down(MouseButton::Left), 22, 14, 100),
        (MouseEventKind::Down(MouseButton::Left), 22, 14, 200),
        (MouseEventKind::Up(MouseButton::Left), 22, 14, 260),
        (MouseEventKind::Moved, 5, 1, 300),
        (MouseEventKind::ScrollDown, 5, 1, 310),
        (MouseEventKind::Down(MouseButton::Left), 5, 1, 320),
        (MouseEventKind::Drag(MouseButton::Left), 5, 3, 330),
        (MouseEventKind::Up(MouseButton::Left), 39, 19, 340),
        (MouseEventKind::Down(MouseButton::Right), 5, 9, 350),
        (MouseEventKind::Down(MouseButton::Middle), 5, 1, 360),
        (MouseEventKind::ScrollDown, 39, 19, 370),
    ];

    let mut produced: BTreeSet<&str> = BTreeSet::new();
    for (kind, x, y, at) in script {
        produced.insert(tag(&route(kind, x, y, &targets, &mut pointer, at)));
    }

    let expected: BTreeSet<&str> = [
        "Activate",
        "Alternate",
        "Double",
        "Drag",
        "Hover",
        "Ignore",
        "Jump",
        "Press",
        "Release",
        "Wheel",
    ]
    .into_iter()
    .collect();
    assert_eq!(produced, expected);
    assert_eq!(produced.len(), 10);
}

#[test]
fn page_navigation_wraps_in_both_directions() {
    let mut p = Page::Status;
    for _ in 0..Page::ALL.len() {
        p = p.next();
    }
    assert_eq!(p, Page::Status);
    for _ in 0..Page::ALL.len() {
        p = p.prev();
    }
    assert_eq!(p, Page::Status);
    assert_eq!(Page::ALL.len(), 4);
    // Every page has a label the tab strip can paint.
    for page in Page::ALL {
        assert!(!page.label().is_empty());
    }
}

// --- the widget layer's share of the contract -------------------------------

/// `Buffer::get` is deprecated in ratatui 0.30; `cell` takes an `Into<Position>`
/// and returns `None` off-grid, so a wrong coordinate in these tests is a panic
/// with coordinates in the message rather than a silent `Default::default()` cell.
fn cell_at<'a>(buf: &'a Buffer, x: u16, y: u16) -> &'a ratatui::buffer::Cell {
    buf.cell((x, y))
        .unwrap_or_else(|| panic!("cell ({x},{y}) is outside the test buffer"))
}

fn bg(buf: &Buffer, x: u16, y: u16) -> Option<(u8, u8, u8)> {
    match cell_at(&buf, x, y).bg {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

fn page_area() -> Rect {
    Rect::new(0, 0, 60, 20)
}

#[test]
fn a_pressed_button_keeps_the_hit_area_where_it_was() {
    // The invariant §6.5 states: painting shifts, registering does not.
    let pal = Palette::new(false, None);
    let btn = widgets::Button::primary("START", Hot::Start);
    let slot = Rect::new(2, 2, 20, 3);

    let mut rest: Vec<Target> = Vec::new();
    let mut buf = Buffer::empty(page_area());
    let hit_rest = btn.paint(&mut buf, slot, &pal, false, false, &mut rest);

    let mut pressed: Vec<Target> = Vec::new();
    let mut buf2 = Buffer::empty(page_area());
    let hit_press = btn.paint(&mut buf2, slot, &pal, true, false, &mut pressed);

    assert_eq!(hit_rest, hit_press);
    assert_eq!(
        input::hit(&pressed, hit_rest.x + 1, hit_rest.y + 1),
        Some(Hot::Start)
    );
    // The vacated cells stay hittable (the control did not move)...
    assert_eq!(
        input::hit(&pressed, hit_rest.x + 1, hit_rest.bottom() - 1),
        Some(Hot::Start)
    );
    // ...and the cells the shifted face newly covers stay *not* hittable.
    let face = paint::offset(hit_rest, tokens::SHADOW_COLS, tokens::SHADOW_ROWS);
    assert_eq!(
        input::hit(&pressed, face.right() - 1, face.bottom() - 1),
        None
    );
    assert_eq!(
        input::hit(&pressed, hit_rest.right() + tokens::SHADOW_COLS, hit_rest.y),
        None
    );
}

#[test]
fn press_paints_no_shadow_and_rest_paints_one() {
    let pal = Palette::new(false, None);
    let btn = widgets::Button::primary("START", Hot::Start);
    let slot = Rect::new(2, 2, 20, 3);

    let mut targets = Vec::new();
    let mut rest = Buffer::empty(page_area());
    let hit = btn.paint(&mut rest, slot, &pal, false, false, &mut targets);
    assert_eq!(bg(&rest, hit.right(), hit.y), Some(tokens::INK));
    assert_eq!(bg(&rest, hit.x, hit.bottom()), Some(tokens::INK));

    let mut pressed = Buffer::empty(page_area());
    btn.paint(&mut pressed, slot, &pal, true, false, &mut targets);
    assert_ne!(bg(&pressed, hit.right(), hit.y), Some(tokens::INK));
    assert_eq!(
        bg(
            &pressed,
            hit.x + tokens::SHADOW_COLS,
            hit.y + tokens::SHADOW_ROWS
        ),
        Some(tokens::INK)
    );
}

#[test]
fn hover_widens_the_shadow_by_the_lift() {
    let pal = Palette::new(false, None);
    let btn = widgets::Button::new("THEME", Hot::Theme);
    let slot = Rect::new(2, 2, 20, 3);
    let mut targets = Vec::new();
    let mut buf = Buffer::empty(page_area());
    let hit = btn.paint(&mut buf, slot, &pal, false, true, &mut targets);
    let lifted = hit.right() + tokens::SHADOW_COLS + tokens::HOVER_LIFT_COLS - 1;
    assert_eq!(bg(&buf, lifted, hit.y), Some(tokens::INK));
    // Positive control: at rest that cell is untouched.
    let mut rest = Buffer::empty(page_area());
    btn.paint(&mut rest, slot, &pal, false, false, &mut targets);
    assert_eq!(bg(&rest, lifted, hit.y), None);
}

#[test]
fn an_accent_recolours_the_shadow_only_when_the_user_asks_for_it() {
    let accent = (0xFF, 0x88, 0x00);
    let pal = Palette::new(false, Some(accent));
    let btn = widgets::Button::primary("START", Hot::Start);
    let mut targets = Vec::new();
    let mut buf = Buffer::empty(page_area());
    let hit = btn.paint(
        &mut buf,
        Rect::new(2, 2, 20, 3),
        &pal,
        false,
        false,
        &mut targets,
    );
    assert_eq!(bg(&buf, hit.right(), hit.y), Some(accent));
    // The face itself stays ink: the GUI's parity is the default, and the accent
    // is the user's open decision, not this UI's opinion.
    assert_eq!(bg(&buf, hit.x + 2, hit.y + 1), Some(tokens::INK));
}

#[test]
fn dense_rows_and_prose_never_get_frames() {
    // §2: heavy borders belong to interactive elements only; the border-overload
    // trap is exactly what this test catches if someone "improves" the tables.
    let pal = Palette::new(false, None);
    let mut buf = Buffer::empty(page_area());
    let mut targets = Vec::new();
    for i in 0..6u16 {
        widgets::paint_row(
            &mut buf,
            Rect::new(1, 1 + i, 40, 1),
            &pal,
            "route cdn .example.com",
            i == 0,
            i == 1,
            Some(Hot::Row(usize::from(i))),
            &mut targets,
        );
    }
    widgets::paint_prose(
        &mut buf,
        Rect::new(1, 8, 40, 1),
        &pal,
        "no frame here either",
    );

    let frame = Color::Rgb(tokens::INK.0, tokens::INK.1, tokens::INK.2);
    // A 1-cell ring would show up as frame colour along the left edge of every
    // row. Only the selected row may be frame-coloured, and only across itself.
    for i in 1..6u16 {
        assert_ne!(cell_at(&buf, 1, 1 + i).bg, frame, "row {i} grew a frame");
    }
    assert_ne!(cell_at(&buf, 1, 8).bg, frame, "prose grew a frame");
    // Positive control: the selected row *is* an inverted band.
    assert_eq!(cell_at(&buf, 3, 1).bg, frame);
    assert_eq!(targets.len(), 6);
}

#[test]
fn card_title_contrasts_with_the_band_it_sits_on() {
    for dark in [false, true] {
        let pal = Palette::new(dark, None);
        let mut buf = Buffer::empty(page_area());
        let body = widgets::Card::new("GATEWAY").paint(&mut buf, Rect::new(1, 1, 30, 6), &pal);
        assert!(!body.is_empty());
        // The title glyph cell must not share its background with its foreground.
        let cell = cell_at(&buf, 2, 1);
        assert_ne!(cell.fg, cell.bg, "dark={dark}");
        // And the card face has to stay distinct from the page it was drawn on.
        assert_ne!(
            cell_at(&buf, body.x, body.y).bg,
            Color::Rgb(pal.page.0, pal.page.1, pal.page.2)
        );
    }
}

#[test]
fn too_small_says_so_instead_of_drawing_a_broken_layout() {
    let pal = Palette::new(false, None);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 8));
    assert!(!widgets::fits(Rect::new(0, 0, 20, 8)));
    widgets::paint_too_small(&mut buf, Rect::new(0, 0, 20, 8), &pal);
    let painted: Vec<String> = (0..8)
        .map(|y| {
            (0..20)
                .map(|x| cell_at(&buf, x, y).symbol().to_string())
                .collect()
        })
        .collect();
    let text = painted.concat();
    assert!(text.contains("MINIMUM"), "the message disappeared: {text}");
    assert!(text.contains(&format!("{}x{}", widgets::MIN_COLS, widgets::MIN_ROWS)));
    // Positive control: a real window does fit.
    assert!(widgets::fits(Rect::new(
        0,
        0,
        widgets::MIN_COLS,
        widgets::MIN_ROWS
    )));
}

#[test]
fn status_bands_carry_state_by_contrast_and_have_no_frame() {
    for status in [
        Status::Starting,
        Status::Running,
        Status::Stopped,
        Status::Degraded,
        Status::Failed,
    ] {
        let mut buf = Buffer::empty(page_area());
        widgets::status_band(&mut buf, Rect::new(2, 2, 12, 1), status, status.label());
        let painted = cell_at(&buf, 3, 2);
        assert_ne!(painted.fg, painted.bg, "{status:?} label is unreadable");
        // Neighbours stay untouched: a pill is not a framed control.
        assert_eq!(
            cell_at(&buf, 1, 2).bg,
            Color::Reset,
            "{status:?} leaked a frame left"
        );
        assert_eq!(
            cell_at(&buf, 3, 1).bg,
            Color::Reset,
            "{status:?} leaked a frame above"
        );
    }
    // Positive control: the band itself really is filled, edge to edge.
    let mut buf = Buffer::empty(page_area());
    widgets::status_band(
        &mut buf,
        Rect::new(2, 2, 12, 1),
        Status::Running,
        Status::Running.label(),
    );
    assert_ne!(cell_at(&buf, 2, 2).bg, Color::Reset);
    assert_ne!(cell_at(&buf, 13, 2).bg, Color::Reset);
    assert_eq!(cell_at(&buf, 14, 2).bg, Color::Reset);
}

/// The app-level half of the mouse contract: a gesture must move *state*, not just
/// produce a command object. These drive `App` the way the event loop would.
///
/// The config is the repository's own `sni-gate.example.toml`, which is guaranteed
/// to load. That means `App::new` starts the port-probe worker against the
/// listeners it declares; those are loopback/wildcard addresses, so the only thing
/// that happens is a refused connect to this machine. Nothing is spawned and
/// nothing is written: `snapshot.rs` only stats paths.
fn app() -> sni_gate::tui::app::App {
    use std::path::PathBuf;
    use std::time::Duration;

    let opts = sni_gate::tui::app::Options {
        config: PathBuf::from("sni-gate.example.toml"),
        // A path that cannot exist: no test below may start a real gateway.
        gate_bin: PathBuf::from("/nonexistent/sni-gate-for-tests"),
        dark: false,
        accent: None,
        tick: Duration::from_millis(50),
    };
    sni_gate::tui::app::App::new(opts)
}

fn wheel(app: &mut sni_gate::tui::app::App, x: u16, y: u16, kind: MouseEventKind) {
    let ev = MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    };
    app.apply_mouse(ev);
}

/// The bug this pins: the log view stores how far *above the newest line* it sits,
/// while the config and status views store how far down from the top they skipped.
/// One direction rule for both makes the wheel scroll the log backwards, and no
/// compiler can see that because both offsets are plain integers.
#[test]
fn the_log_pane_and_the_text_panes_run_in_opposite_directions() {
    use sni_gate::tui::gate::LogLine;
    use std::collections::VecDeque;

    let mut app = app();
    let at = app.started_at;
    app.logs = VecDeque::from([
        LogLine {
            text: "one".into(),
            stream: "stderr",
            at,
        },
        LogLine {
            text: "two".into(),
            stream: "stderr",
            at,
        },
        LogLine {
            text: "three".into(),
            stream: "stderr",
            at,
        },
    ]);
    app.set_targets(vec![
        Target {
            rect: Rect::new(0, 0, 40, 6),
            hot: Hot::LogPane,
        },
        Target {
            rect: Rect::new(0, 8, 40, 6),
            hot: Hot::ConfigPane,
        },
    ]);

    // Wheel down over the logs moves toward the newest line: the stored number
    // must fall, and from 0 it cannot go below zero.
    app.log_scroll = 2;
    wheel(&mut app, 5, 2, MouseEventKind::ScrollDown);
    assert_eq!(
        app.log_scroll, 1,
        "wheel down moved the log away from the newest line"
    );
    wheel(&mut app, 5, 2, MouseEventKind::ScrollUp);
    assert_eq!(
        app.log_scroll, 2,
        "wheel up must move back toward the older lines"
    );

    // Wheel down over the config tree moves *forward* through the file.
    let before = app.config_scroll;
    wheel(&mut app, 5, 10, MouseEventKind::ScrollDown);
    assert_eq!(
        app.config_scroll,
        before + 1,
        "the config pane must grow downward"
    );
}

#[test]
fn end_and_home_mean_the_document_ends_not_the_number_zero() {
    let mut app = app();
    use sni_gate::tui::gate::LogLine;
    use std::collections::VecDeque;
    let at = app.started_at;
    app.logs = VecDeque::from([
        LogLine {
            text: "a".into(),
            stream: "stderr",
            at,
        },
        LogLine {
            text: "b".into(),
            stream: "stderr",
            at,
        },
        LogLine {
            text: "c".into(),
            stream: "stderr",
            at,
        },
    ]);
    app.page = Page::Logs;

    // End on the log page is the newest line = offset 0.
    app.log_scroll = 1;
    app.apply_cmd(input::KeyCmd::Jump);
    assert_eq!(app.log_scroll, 0, "End must land on the newest line");

    // Home on the log page is the *oldest* line = the largest offset, not zero.
    app.apply_cmd(input::KeyCmd::Top);
    assert_eq!(
        app.log_scroll, 2,
        "Home must go to the top of the document, which is the far end of a reverse offset"
    );
}

#[test]
fn a_missing_gateway_is_reported_not_thrown() {
    let mut app = app();
    // Stop with nothing running: an honest refusal, and no process was created.
    app.activate(Hot::Stop);
    assert!(
        app.message.contains("nothing is running"),
        "message was {:?}",
        app.message
    );
    // Start with a binary that does not exist: the failure is text on screen.
    app.activate(Hot::Start);
    assert!(
        app.message.contains("start failed"),
        "message was {:?}",
        app.message
    );
    assert!(
        app.message.contains("/nonexistent/sni-gate-for-tests"),
        "the message must name what it tried"
    );
}

#[test]
fn the_help_strip_names_the_commands_the_router_produces() {
    // The footer must not advertise a gesture the router cannot deliver, and must
    // not hide one it can. Each clause is checked against a command name.
    let help = widgets::help_line();
    for (clause, cmd) in [
        ("press", "Press"),
        ("release", "Activate"),
        ("hover", "Hover"),
        ("drag", "Drag"),
        ("wheel", "Wheel"),
        ("right-click", "Alternate"),
        ("middle-click", "Jump"),
        ("double-click", "Double"),
    ] {
        assert!(help.contains(clause), "help line lost {clause} ({cmd})");
    }
}

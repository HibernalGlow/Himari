//! One frame: where every element goes, and what its cells must look like.
//!
//! Rects are computed by hand rather than with `Layout`, because this style needs
//! *reserved* space that a layout solver does not know about: one row under every
//! framed element for its shadow, and [`widgets::GUTTER_COLS`] columns to the right
//! for the hover lift. A control whose shadow has nowhere to go would paint over
//! its neighbour, which is the border-overload failure the spec warns about.
//!
//! Every interactive element registers its hit area while it paints, so the table
//! the router reads is always the table that produced what the user is looking at.
//! Panes are registered *before* the rows on top of them, because
//! [`crate::tui::input::hit`] takes the topmost match.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;

use crate::tui::app::App;
use crate::tui::input::{self, Hot, Page, Target};
use crate::tui::paint;
use crate::tui::snapshot::Reading;
use crate::tui::tokens::{self, Palette};
use crate::tui::widgets::{self, Button, Card};

/// Framed elements start one cell in, and the focus caret is painted in that
/// column, so a keyboard-focused control is always visible.
const MARGIN: u16 = 1;

/// The band rows, counted from the top of the screen. Everything here is
/// explicit so the shadow reservations are visible in the numbers themselves.
const MASTHEAD_TOP: u16 = 0;
const MASTHEAD_ROWS: u16 = 3;
/// masthead(3) + its shadow(1) + gap(1)
const CONTROLS_TOP: u16 = MASTHEAD_TOP + MASTHEAD_ROWS + 2;
/// controls(3) + their shadow(1) + gap(1)
const MESSAGE_TOP: u16 = CONTROLS_TOP + MASTHEAD_ROWS + 2;
/// message(1) + gap(1)
const BODY_GAP: u16 = 2;
/// the body's own shadow row, and the help line
const FOOT_ROWS: u16 = 2;

/// A control band: frame, label, frame. The row under it is its shadow, which is
/// why the message band sits two rows below rather than one.
const BUTTON_BAND: u16 = 3;

struct Bands {
    masthead: Rect,
    controls: Rect,
    message: Rect,
    body: Rect,
    help: Rect,
}

impl Bands {
    fn of(area: Rect) -> Self {
        let x = area.x + MARGIN;
        let width = area.width.saturating_sub(MARGIN + widgets::GUTTER_COLS + 1);
        let row = |top: u16, height: u16| Rect::new(x, area.y + top, width, height);
        let body_top = MESSAGE_TOP + BODY_GAP;
        Self {
            masthead: row(MASTHEAD_TOP, MASTHEAD_ROWS),
            controls: row(CONTROLS_TOP, BUTTON_BAND),
            message: row(MESSAGE_TOP, 1),
            body: Rect::new(
                x,
                area.y + body_top,
                width,
                area.height.saturating_sub(body_top + FOOT_ROWS),
            ),
            help: Rect::new(x, area.y + area.height.saturating_sub(1), width, 1),
        }
    }
}

fn card_text(pal: &Palette) -> Style {
    Style::default()
        .fg(tokens::rgb(pal.text))
        .bg(tokens::rgb(pal.card))
        .add_modifier(Modifier::BOLD)
}

fn page_bg(pal: &Palette) -> Style {
    Style::default().bg(tokens::rgb(pal.page))
}

pub fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    app.frames += 1;
    let area = frame.area();
    let pal = app.pal;
    let buf = frame.buffer_mut();

    paint::fill(buf, area, " ", page_bg(&pal));

    if !widgets::fits(area) {
        widgets::paint_too_small(buf, area, &pal);
        app.set_targets(Vec::new());
        return;
    }

    let bands = Bands::of(area);
    let mut targets: Vec<Target> = Vec::new();

    draw_masthead(buf, bands.masthead, app, &pal);
    draw_controls(buf, bands.controls, app, &pal, &mut targets);
    widgets::paint_prose(buf, bands.message, &pal, &app.message);
    draw_body(buf, bands.body, app, &pal, &mut targets);
    widgets::paint_prose(buf, bands.help, &pal, &widgets::help_line());

    // Replaced, never merged: a stale table would let a click land on something
    // that is no longer drawn.
    app.set_targets(targets);
}

/// The wordmark, the version, and the status pill — whose *words* come from the
/// process and whose *colour* comes from severity. They are never merged into one
/// confident guess.
fn draw_masthead(buf: &mut Buffer, area: Rect, app: &App, pal: &Palette) {
    let body = Card::new("HIMARI  ·  sni-gate").paint(buf, area, pal);
    if body.is_empty() {
        return;
    }
    let pill_width = 12u16.min(body.width / 2);
    let pill = Rect::new(
        body.right().saturating_sub(pill_width),
        body.y,
        pill_width,
        1,
    );
    widgets::status_band(buf, pill, app.status(), &app.status_label());

    let version = format!(
        "gateway {}  ·  {}",
        env!("CARGO_PKG_VERSION"),
        app.reading.headline()
    );
    let room = pill.x.saturating_sub(body.x).saturating_sub(1);
    paint::put_in_row(
        buf,
        body.x,
        body.y,
        &version,
        usize::from(room),
        card_text(pal),
    );
}

/// Tabs on the left, actions on the right, in one band. Both are framed buttons;
/// the message band below is where an action's readback lands.
/// The actions, placed from the right edge inwards: START outermost, TURN OFF next
/// to it, then the theme and quit.
fn action_buttons(pal: &Palette) -> Vec<Button<'static>> {
    vec![
        Button::primary("START", Hot::Start),
        Button::new("TURN OFF", Hot::Stop),
        Button::new(theme_label(pal), Hot::Theme),
        Button::new("QUIT", Hot::Quit),
    ]
}

/// Tabs on the left, actions on the right, one band.
///
/// If the window is too narrow for both groups, the leftovers are reported by name
/// instead of disappearing: an unpainted control is unreachable by pointer *and* by
/// keyboard focus, so the note is the only honest option short of growing logic here
/// to lay the band out adaptively.
fn draw_controls(
    buf: &mut Buffer,
    area: Rect,
    app: &App,
    pal: &Palette,
    targets: &mut Vec<Target>,
) {
    let after_tabs =
        widgets::paint_tabs(buf, area, pal, app.page, app.pressed(), app.hover, targets);

    let actions = action_buttons(pal);
    // One cell of the left margin stays free for the focus caret.
    let left_limit = after_tabs.saturating_add(1);
    let mut right = area.right();
    let mut hidden: Vec<&str> = Vec::new();

    for button in &actions {
        let width = button.width();
        let x = right.saturating_sub(width);
        if x < left_limit || width > area.width {
            hidden.push(button.label);
            continue;
        }
        let hot = button.hot;
        let slot = Rect::new(x, area.y, width, BUTTON_BAND);
        let hit = button.paint(
            buf,
            slot,
            pal,
            app.pressed() == Some(hot),
            app.hover == Some(hot),
            targets,
        );
        if app.focus == hot {
            paint_focus_caret(buf, hit, pal);
        }
        right = x.saturating_sub(1);
    }

    if !hidden.is_empty() {
        let note = format!(
            "{} action(s) hidden for lack of room: {}",
            hidden.len(),
            hidden.join(", ")
        );
        // The band's own shadow row is free; the note goes there, unframed.
        let row = area.y + BUTTON_BAND;
        paint::put_in_row(
            buf,
            area.x,
            row,
            &note,
            usize::from(area.width),
            card_text(pal),
        );
    }
}

fn theme_label(pal: &Palette) -> &'static str {
    if pal.dark {
        "LIGHT"
    } else {
        "DARK"
    }
}

/// The keyboard focus marker: a caret one cell left of the control. The pointer
/// has hover; the keyboard has this, and both are on screen at the same time.
fn paint_focus_caret(buf: &mut Buffer, hit: Rect, pal: &Palette) {
    let color = pal.accent.unwrap_or(pal.text);
    paint::cell(
        buf,
        hit.x.saturating_sub(1),
        hit.y + hit.height / 2,
        "\u{25c0}",
        Style::default()
            .fg(Color::Rgb(color.0, color.1, color.2))
            .bg(tokens::rgb(pal.page)),
    );
}

fn draw_body(
    buf: &mut Buffer,
    area: Rect,
    app: &mut App,
    pal: &Palette,
    targets: &mut Vec<Target>,
) {
    if area.height < 3 {
        return;
    }
    match app.page {
        Page::Status => draw_status(buf, area, app, pal, targets),
        Page::Listeners => draw_listeners(buf, area, app, pal, targets),
        Page::Logs => draw_logs(buf, area, app, pal, targets),
        Page::Config => draw_config(buf, area, app, pal, targets),
    }
}

fn draw_status(
    buf: &mut Buffer,
    area: Rect,
    app: &mut App,
    pal: &Palette,
    targets: &mut Vec<Target>,
) {
    let body = Card::new("GATEWAY").paint(buf, area, pal);
    if body.is_empty() {
        return;
    }
    // The status card is its own scrollable pane: it is long enough to overflow a
    // 30-row terminal, and a wheel over it must not scroll the log view that is
    // not even on screen.
    input::register(targets, body, Hot::StatusPane);
    paint_pane(buf, body, pal, &app.status_rows(), app.status_scroll);
}

/// Paint a window of text inside `body`, and when there is more below, spend the
/// last row saying so. Silently dropping the tail is how a long list ends up
/// looking short, which is the kind of quiet lie this UI keeps refusing.
fn paint_pane(buf: &mut Buffer, body: Rect, pal: &Palette, lines: &[String], scroll: usize) {
    let visible = body.height as usize;
    if visible == 0 {
        return;
    }
    let total = lines.len();
    let start = if total > visible {
        scroll.min(total - visible)
    } else {
        0
    };
    let overflow = total > start + visible;
    let shown = if overflow && visible > 1 {
        visible - 1
    } else {
        visible
    };

    for (offset, text) in lines.iter().skip(start).take(shown).enumerate() {
        let Some(row) = paint::row_of(body, offset as u16) else {
            break;
        };
        paint::put_in_row(
            buf,
            row.x,
            row.y,
            text,
            usize::from(row.width),
            card_text(pal),
        );
    }

    if overflow {
        let below = total - start - shown;
        let note = if start > 0 {
            format!("  {start} above · +{below} below — wheel, or PageUp/PageDown")
        } else {
            format!("  +{below} below — wheel, or PageUp/PageDown")
        };
        if let Some(row) = paint::row_of(body, (shown) as u16) {
            paint::put_in_row(
                buf,
                row.x,
                row.y,
                &note,
                usize::from(row.width),
                card_text(pal),
            );
        }
    }
}

fn draw_listeners(
    buf: &mut Buffer,
    area: Rect,
    app: &mut App,
    pal: &Palette,
    targets: &mut Vec<Target>,
) {
    let facts = match &app.reading {
        Reading::Loaded(facts) => facts,
        Reading::Failed { path, reason } => {
            let body = Card::new("LISTENERS").paint(buf, area, pal);
            let rows = vec![
                format!(
                    "{} did not load, so there is nothing to list:",
                    path.display()
                ),
                reason.clone(),
                "Fix the config and reopen, or press START to see the gateway's own complaint."
                    .to_string(),
            ];
            paint_text_rows(buf, body, pal, &rows);
            return;
        }
    };

    let count = facts.listeners.len();
    if count == 0 {
        let body = Card::new("LISTENERS").paint(buf, area, pal);
        paint_text_rows(
            buf,
            body,
            pal,
            &["  the config loaded and declares no listener".to_string()],
        );
        return;
    }

    // Two stacked cards: the selectable list, then the routes of the selection.
    let list_rows = count.min(area.height.saturating_sub(6).max(1) as usize);
    let list_rect = Rect::new(area.x, area.y, area.width, list_rows as u16 + 2);
    let list_body = Card::new("LISTENERS  ·  click a row to select").paint(buf, list_rect, pal);
    if list_body.is_empty() {
        return;
    }
    input::register(targets, list_body, Hot::RowPane);

    let window = visible_window(count, list_body.height as usize, app.selected);
    for (offset, index) in window.iter().enumerate() {
        let Some(row) = paint::row_of(list_body, offset as u16) else {
            break;
        };
        let hot = Hot::Row(*index);
        widgets::paint_row(
            buf,
            row,
            pal,
            &facts.listeners[*index].summary(),
            *index == app.selected,
            app.hover == Some(hot) || app.focus == hot,
            Some(hot),
            targets,
        );
    }

    // The list card's shadow owns the row below it, so the detail card starts two
    // rows down: shadow, then the gap this style always asks for.
    let detail_top = list_rect.bottom() + 2;
    if detail_top + 2 < area.bottom() {
        let selected = facts.listeners.get(app.selected).map(|l| l.addr.clone());
        let title = match &selected {
            Some(addr) => format!("ROUTES OF {addr}"),
            None => "ROUTES".to_string(),
        };
        let detail = Card::new(&title).paint(
            buf,
            Rect::new(area.x, detail_top, area.width, area.bottom() - detail_top),
            pal,
        );
        let rows: Vec<String> = facts
            .listeners
            .get(app.selected)
            .map(|listener| {
                let mut out: Vec<String> = Vec::new();
                for route in &listener.routes {
                    out.push(format!("  {}", route.row()));
                }
                if let Some(default) = &listener.default_route {
                    out.push(format!("  default {}", default.row()));
                }
                if out.is_empty() {
                    out.push("  this listener declares no routes".to_string());
                }
                out
            })
            .unwrap_or_default();
        paint_text_rows(buf, detail, pal, &rows);
    }
}

fn draw_logs(
    buf: &mut Buffer,
    area: Rect,
    app: &mut App,
    pal: &Palette,
    targets: &mut Vec<Target>,
) {
    let title = if app.following() {
        "LOGS  ·  following the newest line"
    } else {
        "LOGS  ·  pinned, wheel down or press End to follow"
    };
    let body = Card::new(title).paint(buf, area, pal);
    if body.is_empty() {
        return;
    }
    input::register(targets, body, Hot::LogPane);

    let visible = body.height as usize;
    let total = app.logs.len();
    // `log_scroll` counts lines *up* from the newest, so 0 is the tail.
    let end = total.saturating_sub(app.log_scroll);
    let start = end.saturating_sub(visible);

    let mut rows: Vec<String> = Vec::new();
    for line in app.logs.iter().take(end).skip(start) {
        let at = line
            .at
            .checked_duration_since(app.started_at)
            .unwrap_or_default();
        rows.push(format!(
            "t+{:>7.2} {:<7} {}",
            at.as_secs_f64(),
            line.stream,
            line.text
        ));
    }
    if rows.is_empty() {
        rows.push(
            "  no lines yet: press START, or the gateway has not logged anything".to_string(),
        );
    }
    let hidden = app.log_scroll;
    if hidden > 0 {
        rows.push(format!(
            "  {hidden} newer line(s) above this view — press End to follow"
        ));
    }
    paint_text_rows(buf, body, pal, &rows);
}

fn draw_config(
    buf: &mut Buffer,
    area: Rect,
    app: &mut App,
    pal: &Palette,
    targets: &mut Vec<Target>,
) {
    let body = Card::new("CONFIG FILE  ·  click a header to collapse, right-click to expand all")
        .paint(buf, area, pal);
    if body.is_empty() {
        return;
    }
    // The pane first, so the header rows painted on top of it win the hit test.
    input::register(targets, body, Hot::ConfigPane);

    let rows = app.config_rows();
    let visible = body.height as usize;
    // Clamp against the window rather than the list length: when the user jumps to
    // the end, the last line of the file has to actually be on screen.
    let last_start = rows.len().saturating_sub(visible.max(1));
    let start = app.config_scroll.min(last_start);

    for (offset, line) in rows.iter().skip(start).take(visible).enumerate() {
        let Some(row) = paint::row_of(body, offset as u16) else {
            break;
        };
        match (line.is_header, line.section) {
            (true, Some(section)) => {
                let hot = Hot::Section(section);
                let marker = if app.collapsed.contains(&section) {
                    "[-]"
                } else {
                    "[+]"
                };
                let text = format!("{marker} {}", line.text);
                widgets::paint_row(
                    buf,
                    row,
                    pal,
                    &text,
                    false,
                    app.hover == Some(hot) || app.focus == hot,
                    Some(hot),
                    targets,
                );
            }
            _ => paint::put_in_row(
                buf,
                row.x + 2,
                row.y,
                &line.text,
                usize::from(row.width.saturating_sub(2)),
                card_text(pal),
            ),
        }
    }
}

/// Plain body rows on a card face: prose, so no frames and no hit areas.
fn paint_text_rows(buf: &mut Buffer, body: Rect, pal: &Palette, rows: &[String]) {
    for (index, text) in rows.iter().enumerate() {
        let Some(row) = paint::row_of(body, index as u16) else {
            break;
        };
        paint::put_in_row(
            buf,
            row.x,
            row.y,
            text,
            usize::from(row.width),
            card_text(pal),
        );
    }
}

/// The visible index window that contains `selected`.
fn visible_window(total: usize, height: usize, selected: usize) -> Vec<usize> {
    if total == 0 || height == 0 {
        return Vec::new();
    }
    let height = height.min(total);
    let start = selected
        .saturating_sub(height / 2)
        .min(total.saturating_sub(height));
    (start..start + height).collect()
}

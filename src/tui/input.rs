//! What the pointer and the keyboard *mean*, as data.
//!
//! Two rules from `docs/design-language.md` §6.5 shape this module:
//!
//! - A click is press + release on the **same** hotspot. A press whose release
//!   lands elsewhere is cancelled, and the hit area is the *unpressed* rect, so
//!   the press animation (which shifts the painted face) can never make a
//!   control escape from under the cursor mid-click.
//! - The keyboard must reach everything the mouse reaches. A terminal without
//!   mouse support is a normal condition, not an error state, and a control that
//!   only answers to the pointer cannot be acceptance-tested (brief §30/§31).
//!
//! Nothing here paints or mutates the app; `widgets` registers the geometry,
//! `app` executes the effects. That split is what lets the whole mouse contract
//! be tested without a terminal.

use ratatui::crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};

/// Double-click window. crossterm 0.29 does not report double clicks, so two
/// presses of the same hotspot inside this many milliseconds are one double click.
pub const DOUBLE_CLICK_MS: u64 = 400;

/// The four surfaces of the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Status,
    Listeners,
    Logs,
    Config,
}

impl Page {
    pub const ALL: [Page; 4] = [Self::Status, Self::Listeners, Self::Logs, Self::Config];

    pub fn label(self) -> &'static str {
        match self {
            Self::Status => "STATUS",
            Self::Listeners => "LISTENERS",
            Self::Logs => "LOGS",
            Self::Config => "CONFIG",
        }
    }

    pub fn next(self) -> Page {
        let i = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Page {
        let i = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// An addressable region of the screen. `usize` payload indices are positions in
/// the current frame's list, so they are only meaningful against the same frame's
/// hotspot table — which is why the table is rebuilt every draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hot {
    Tab(Page),
    Start,
    Stop,
    Theme,
    Quit,
    /// A collapsible node of the config tree.
    Section(usize),
    /// A selectable row of the current page's table.
    Row(usize),
    /// A scrollable text surface, so the wheel can be aimed at whichever one
    /// is actually under the cursor. Each page owns its own pane: a gesture over
    /// the status card must not scroll the log view that is not even on screen.
    StatusPane,
    LogPane,
    ConfigPane,
    RowPane,
}

/// A painted region paired with its meaning.
#[derive(Debug, Clone, Copy)]
pub struct Target {
    pub rect: Rect,
    pub hot: Hot,
}

pub fn register(targets: &mut Vec<Target>, rect: Rect, hot: Hot) {
    if !rect.is_empty() {
        targets.push(Target { rect, hot });
    }
}

/// The topmost hotspot at a position. Later registrations painted over earlier
/// ones, so the table is searched back to front — a panel drawn on top of a row
/// must win, or clicking it would move the selection underneath.
pub fn hit(targets: &[Target], x: u16, y: u16) -> Option<Hot> {
    let pos = Position::new(x, y);
    targets
        .iter()
        .rev()
        .find(|t| t.rect.contains(pos))
        .map(|t| t.hot)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wheel {
    Up,
    Down,
    Left,
    Right,
}

/// Everything the mouse can ask the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseCmd {
    /// Left button down on a control: enter the press state, do not act yet.
    Press(Hot),
    /// Left button released on the control it was pressed on: act.
    Activate(Hot),
    /// Right button: the alternate action for that region.
    Alternate(Hot),
    /// Middle button: jump to the newest entry / the bottom of the region.
    Jump(Hot),
    /// Pointer moved with no button held, including leaving everything.
    Hover(Option<Hot>),
    /// Left button dragged inside the region it was pressed on, by these deltas
    /// in cells since the last drag event.
    Drag { over: Hot, dx: i32, dy: i32 },
    /// Wheel over a region.
    Wheel { over: Hot, dir: Wheel },
    /// Second left press on the same hotspot inside `DOUBLE_CLICK_MS`.
    Double(Hot),
    /// A release that did not activate anything — the press is withdrawn.
    Release,
    /// Any event outside an interesting region, or a button we do not use.
    Ignore,
}

/// Pointer bookkeeping between events.
#[derive(Debug, Default, Clone)]
pub struct Pointer {
    pressed: Option<Hot>,
    drag_last: Option<(u16, u16)>,
    last_click: Option<(Hot, u64)>,
}

impl Pointer {
    /// The hotspot currently in its press state, if any. `widgets` paints from
    /// this, which is why the press animation and the activation rule read the
    /// same piece of state.
    pub fn pressed(&self) -> Option<Hot> {
        self.pressed
    }
}

/// Translate one mouse event against the current frame's hotspot table.
///
/// `now_ms` is passed in rather than read from a clock so the double-click
/// boundary is testable at 399 and 401 ms.
pub fn route_mouse(
    kind: MouseEventKind,
    x: u16,
    y: u16,
    targets: &[Target],
    pointer: &mut Pointer,
    now_ms: u64,
) -> MouseCmd {
    let over = hit(targets, x, y);

    match kind {
        MouseEventKind::Down(button) => match button {
            MouseButton::Left => {
                let double = over
                    .zip(pointer.last_click)
                    .is_some_and(|(hot, (last, at))| {
                        hot == last && now_ms.saturating_sub(at) < DOUBLE_CLICK_MS
                    });
                pointer.pressed = over;
                pointer.drag_last = Some((x, y));
                pointer.last_click = over.map(|hot| (hot, now_ms));
                match (over, double) {
                    (Some(hot), true) => MouseCmd::Double(hot),
                    (Some(hot), false) => MouseCmd::Press(hot),
                    (None, _) => MouseCmd::Ignore,
                }
            }
            MouseButton::Right => over.map_or(MouseCmd::Ignore, MouseCmd::Alternate),
            MouseButton::Middle => over.map_or(MouseCmd::Ignore, MouseCmd::Jump),
        },

        MouseEventKind::Up(MouseButton::Left) => {
            let was = pointer.pressed.take();
            pointer.drag_last = None;
            match was.zip(over) {
                Some((hot, o)) if hot == o => MouseCmd::Activate(hot),
                _ => MouseCmd::Release,
            }
        }
        MouseEventKind::Up(_) => {
            pointer.drag_last = None;
            MouseCmd::Release
        }

        MouseEventKind::Drag(button) => {
            let last = pointer.drag_last.replace((x, y));
            let dx = i32::from(x) - i32::from(last.map_or(x, |(lx, _)| lx));
            let dy = i32::from(y) - i32::from(last.map_or(y, |(_, ly)| ly));
            match (button, pointer.pressed.zip(over)) {
                (MouseButton::Left, Some((hot, o))) if hot == o && (dx != 0 || dy != 0) => {
                    MouseCmd::Drag { over: hot, dx, dy }
                }
                _ => MouseCmd::Ignore,
            }
        }

        MouseEventKind::Moved => MouseCmd::Hover(over),

        MouseEventKind::ScrollUp => wheel(over, Wheel::Up),
        MouseEventKind::ScrollDown => wheel(over, Wheel::Down),
        MouseEventKind::ScrollLeft => wheel(over, Wheel::Left),
        MouseEventKind::ScrollRight => wheel(over, Wheel::Right),
    }
}

fn wheel(over: Option<Hot>, dir: Wheel) -> MouseCmd {
    over.map_or(MouseCmd::Ignore, |over| MouseCmd::Wheel { over, dir })
}

/// What a key press asks for. Keystrokes are named by intent, not by key, so the
/// app's effect functions stay shared with the mouse path: every variant here has
/// a twin in [`MouseCmd`], and `App` routes both into the same effect functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCmd {
    /// The keyboard twin of a completed click, on the focused control.
    Activate(Hot),
    /// The keyboard twin of a right click.
    Alternate(Hot),
    /// The keyboard twin of a middle click: jump to newest / bottom.
    Jump,
    /// The keyboard twin of a wheel notch on the active pane.
    Scroll(Wheel),
    /// The keyboard twin of dragging to the very top.
    Top,
    /// Move the highlight through the frame's hotspot table.
    FocusNext,
    FocusPrev,
    MoveSelection(i32),
    GotoPage(Page),
    NextPage,
    PrevPage,
    Start,
    Stop,
    ToggleTheme,
    Quit,
    Ignore,
}

/// Translate a key press into intent.
///
/// `focus` is the control the keyboard is currently pointing at, so `Enter` means
/// the same thing `Activate` does when a click lands on the same hotspot.
///
/// Unrecognised keys are `Ignore` rather than swallowed silently by the caller.
pub fn route_key(code: KeyCode, modifiers: KeyModifiers, focus: Hot) -> KeyCmd {
    use KeyCode as K;

    if modifiers.contains(KeyModifiers::CONTROL) {
        return match code {
            K::Char('c') => KeyCmd::Quit,
            _ => KeyCmd::Ignore,
        };
    }

    match code {
        K::Enter | K::Char(' ') => KeyCmd::Activate(focus),
        K::Char('r') => KeyCmd::Alternate(focus),
        K::Tab => KeyCmd::NextPage,
        K::BackTab => KeyCmd::PrevPage,
        K::Char('n') => KeyCmd::FocusNext,
        K::Char('p') => KeyCmd::FocusPrev,
        K::Down | K::Char('j') => KeyCmd::MoveSelection(1),
        K::Up | K::Char('k') => KeyCmd::MoveSelection(-1),
        K::PageDown | K::Char('d') => KeyCmd::Scroll(Wheel::Down),
        K::PageUp | K::Char('u') => KeyCmd::Scroll(Wheel::Up),
        K::Right | K::Char('l') => KeyCmd::Scroll(Wheel::Right),
        K::Left | K::Char('h') => KeyCmd::Scroll(Wheel::Left),
        K::End => KeyCmd::Jump,
        K::Home => KeyCmd::Top,
        K::Char('t') => KeyCmd::ToggleTheme,
        K::Char('s') => KeyCmd::Start,
        K::Char('x') => KeyCmd::Stop,
        K::Char('q') | K::Esc => KeyCmd::Quit,
        K::Char(digit @ '1'..='4') => {
            let index = usize::from(digit as u8 - b'1');
            match Page::ALL.get(index) {
                Some(page) => KeyCmd::GotoPage(*page),
                None => KeyCmd::Ignore,
            }
        }
        _ => KeyCmd::Ignore,
    }
}

/// The focus model: a single hotspot the keyboard acts on. `view` sets the
/// initial focus and `App` moves it, so `Enter` always means
/// `Activate(focus)` and therefore calls the same function a click calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Focus {
    pub hot: Hot,
}

impl Focus {
    pub const DEFAULT: Focus = Focus { hot: Hot::Start };
}

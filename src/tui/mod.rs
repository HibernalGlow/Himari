//! The terminal UI: a neobrutalist front end for the `sni-gate` gateway, built
//! on ratatui and driven by the mouse as much as by the keyboard.
//!
//! This module is the whole of the `tui` feature and is compiled only under it,
//! so the gateway binary and its dependency graph are untouched.
//!
//! The visual language is `docs/design-language.md`; §6 is the terminal half of
//! it, including the two places where a cell grid cannot honour a GUI rule
//! verbatim and what was done instead. The colour values come from [`tokens`],
//! which mirrors `crates/himari-ui/src/tokens.rs` from the app repo and is pinned
//! against drift by `tests/tui_visual_language.rs`.
//!
//! Layout of the crate, in the order the pieces depend on each other:
//! - [`tokens`]: palette, shadow/press geometry, status colours, contrast maths
//! - [`paint`]: cell primitives those tokens need (solid rings, offset shadows,
//!   clipped text)
//! - [`input`]: the hotspot table and what pointer/keyboard events mean
//! - [`widgets`]: the pieces of the language (button, card, status band, table
//!   row), each registering its own hit area
//! - [`gate`]: the gateway child process, its logs, its exit, and a port probe
//! - [`snapshot`]: what the configuration actually says, read by the gateway's
//!   own parser
//! - [`app`]: the state machine every effect funnels through
//! - [`view`]: one frame, and the hotspot table that frame produced
//!
//! Terminal lifetime is owned by [`run`]: ratatui's own init/restore plus a
//! [`MouseCapture`] guard that puts the pointer and the cursor back on any exit
//! path, including a panic.

pub mod app;
pub mod gate;
pub mod input;
pub mod paint;
pub mod snapshot;
pub mod tokens;
pub mod view;
pub mod widgets;

use std::io::stdout;

use ratatui::crossterm::cursor::{Hide, Show};
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::crossterm::tty::IsTty;
use ratatui::crossterm::{execute, terminal};

use crate::tui::app::App;

/// Refuse to start where there is no terminal to draw into.
///
/// A pipe or a `--help` style invocation would otherwise put the screen into
/// alternate-buffer mode, fail, and leave the caller's shell in a strange state.
pub fn stdout_is_a_terminal() -> bool {
    stdout().is_tty()
}

/// Mouse capture plus cursor hiding, as one RAII pair. Both are undone on drop,
/// so a panic inside the loop (ratatui's own panic hook restores the screen) does
/// not leave a terminal eating clicks with nothing drawn.
struct MouseCapture {
    captured: bool,
    hidden: bool,
}

impl MouseCapture {
    fn enable() -> Self {
        let captured = execute!(stdout(), EnableMouseCapture).is_ok();
        let hidden = execute!(stdout(), Hide).is_ok();
        Self { captured, hidden }
    }
}

impl Drop for MouseCapture {
    fn drop(&mut self) {
        if self.captured {
            let _ = execute!(stdout(), DisableMouseCapture);
        }
        if self.hidden {
            let _ = execute!(stdout(), Show);
        }
    }
}

/// Run the UI until the user quits or the frame budget is spent.
///
/// The terminal is restored before this returns on every path: the draw error and
/// the event error are captured rather than propagated with `?`, because a `?`
/// here would leave the caller's terminal in the alternate screen.
pub fn run(mut app: App) -> std::io::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let capture = MouseCapture::enable();

    let outcome = loop {
        app.tick();
        if let Err(e) = terminal.draw(|frame| view::draw(frame, &mut app)) {
            break Err(e);
        }
        if app.quit {
            break Ok(());
        }
        match event::poll(app.tick_duration()) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) => app.apply_key(key),
                Ok(Event::Mouse(ev)) => app.apply_mouse(ev),
                // Resize is handled by drawing the next frame at the new size;
                // focus and paste need no state here.
                Ok(_) => {}
                Err(e) => break Err(e),
            },
            Ok(false) => {}
            Err(e) => break Err(e),
        }
    };

    drop(capture);
    let restored = ratatui::try_restore();
    // Never lose the real failure behind a cleanup failure.
    match (outcome, restored) {
        (Err(e), _) => Err(e),
        (Ok(()), Err(e)) => Err(e),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// The terminal size, for callers that want to warn before drawing.
pub fn size() -> std::io::Result<(u16, u16)> {
    terminal::size().map(|(cols, rows)| (cols, rows))
}

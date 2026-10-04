//! The state machine.
//!
//! Division of labour, kept honest by where the code lives:
//! - [`crate::tui::input`] turns bytes into intent and knows nothing about state;
//! - this module applies intent and knows nothing about geometry;
//! - [`crate::tui::view`] and [`crate::tui::widgets`] decide what a state looks
//!   like and register the hotspots that this module then acts on.
//!
//! The rule that matters for the mouse/keyboard claim: **both paths funnel into
//! [`App::activate`]**. A click on the Start button and `Enter` while Start holds
//! the focus run the same function, so "the keyboard is a fallback" is not a
//! slogan here — it is the same code path with a different origin.
//!
//! Every effect writes a one-line readback into [`App::message`], because a
//! control the user cannot read back is a decoration.

use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use ratatui::crossterm::event::KeyEvent;
use ratatui::crossterm::event::KeyEventKind;
use ratatui::crossterm::event::MouseEvent;

use crate::tui::gate;
use crate::tui::gate::Gate;
use crate::tui::gate::GateState;
use crate::tui::gate::LogLine;
use crate::tui::input;
use crate::tui::input::Hot;
use crate::tui::input::KeyCmd;
use crate::tui::input::MouseCmd;
use crate::tui::input::Page;
use crate::tui::input::Pointer;
use crate::tui::input::Target;
use crate::tui::input::Wheel;
use crate::tui::snapshot::ConfigFacts;
use crate::tui::snapshot::FileTree;
use crate::tui::snapshot::Reading;
use crate::tui::tokens::Palette;
use crate::tui::tokens::Rgb;
use crate::tui::tokens::Status;

/// Lines of gateway log kept in memory. The screen shows a window of them; the
/// cap exists so a chatty gateway cannot grow the TUI without bound.
pub const LOG_CAPACITY: usize = 2_000;

/// A child that has been alive for less than this is *starting*, not running.
///
/// This is the measured settle window from `docs/audits/09`: TERMed before it,
/// the gateway dies from the signal (143); after it, it exits 0 on its own.
pub const SETTLE: Duration = Duration::from_secs(1);

/// Scroll one wheel notch / one key step.
pub const SCROLL_STEP: usize = 1;

/// A page of scrolling, for PageUp/PageDown and the horizontal wheel.
pub const SCROLL_PAGE: usize = 8;

/// Everything the binary was told on the command line.
#[derive(Debug, Clone)]
pub struct Options {
    pub config: PathBuf,
    pub gate_bin: PathBuf,
    pub dark: bool,
    pub accent: Option<Rgb>,
    /// How long the event loop waits for input per turn. Also the cadence at
    /// which the gateway is reaped and the log queue is drained, so it is the
    /// UI's heartbeat rather than only an input timeout.
    pub tick: Duration,
}

/// Not `Debug`: the supervisor holds child-process handles and channels that do
/// not implement it, and inventing a partial impl just to satisfy a derive would
/// be interesting only while it is wrong.
pub struct App {
    pub opts: Options,
    pub pal: Palette,
    /// The gateway's own parser's answer about the config file.
    pub reading: Reading,
    /// The same file as text, for the collapsible tree.
    pub tree: FileTree,
    pub gate: Gate,
    pub page: Page,
    pub focus: Hot,
    pub pointer: Pointer,
    pub hover: Option<Hot>,
    /// The hotspot table from the last drawn frame. Routing uses it, which is
    /// what makes "topmost wins" and "the hit area did not move" observable.
    pub targets: Vec<Target>,
    pub logs: VecDeque<LogLine>,
    pub log_scroll: usize,
    /// Scroll offset of the status page's own text pane. It is separate from the
    /// log scroll because the two pages show different things: one wheel notch
    /// cannot move a list the user is not looking at.
    pub status_scroll: usize,
    pub config_scroll: usize,
    pub collapsed: BTreeSet<usize>,
    /// Selected listener index on the Listeners page.
    pub selected: usize,
    /// Readback of the last effect, shown in its own band.
    pub message: String,
    pub started_at: Instant,
    pub quit: bool,
    pub frames: u64,
}

impl App {
    pub fn new(opts: Options) -> Self {
        let pal = Palette::new(opts.dark, opts.accent);
        let reading = ConfigFacts::load(&opts.config);
        let tree = FileTree::read(&opts.config);

        let gate = Gate::new(opts.gate_bin.clone(), opts.config.clone());

        let message = match &reading {
            Reading::Loaded(_) => format!("loaded {}", opts.config.display()),
            Reading::Failed { path, reason } => {
                format!("{} did not load: {reason}", path.display())
            }
        };

        // The first section starts collapsed only when the tree is long; a short
        // config should be readable without a click.
        let mut collapsed = BTreeSet::new();
        if tree.sections.len() > 8 {
            collapsed.insert(tree.sections.len() - 1);
        }

        Self {
            opts,
            pal,
            reading,
            tree,
            gate,
            page: Page::Status,
            focus: Hot::Start,
            pointer: Pointer::default(),
            hover: None,
            targets: Vec::new(),
            logs: VecDeque::new(),
            log_scroll: 0,
            status_scroll: 0,
            config_scroll: 0,
            collapsed,
            selected: 0,
            message,
            started_at: Instant::now(),
            quit: false,
            frames: 0,
        }
    }

    pub fn tick(&mut self) {
        self.gate.tick();
        for line in self.gate.drain_logs(LOG_CAPACITY) {
            if self.logs.len() >= LOG_CAPACITY {
                self.logs.pop_front();
            }
            self.logs.push_back(line);
        }
        // Follow mode means the newest line is on screen; anything else keeps the
        // user's position, which is why it must be clamped rather than trusted.
        if self.following() {
            self.log_scroll = 0;
        }
        self.clamp_scrolls();
    }

    pub fn following(&self) -> bool {
        self.page == Page::Logs && self.log_scroll == 0 && self.pointer.pressed().is_none()
    }

    /// The one place a completed gesture or a key becomes an effect.
    pub fn activate(&mut self, hot: Hot) {
        match hot {
            Hot::Start => self.message = self.start(),
            Hot::Stop => self.message = self.stop(),
            Hot::Theme => {
                self.opts.dark = !self.opts.dark;
                self.pal = Palette::new(self.opts.dark, self.opts.accent);
                self.message = format!("theme: {}", if self.opts.dark { "dark" } else { "light" });
            }
            Hot::Quit => {
                self.quit = true;
                self.message = "quit requested".to_string();
            }
            Hot::Tab(page) => self.set_page(page),
            Hot::Section(index) => self.toggle_section(index),
            Hot::Row(index) => {
                self.selected = index;
                self.message = match self.reading.facts() {
                    Some(facts) => match facts.listeners.get(index) {
                        Some(listener) => format!("listener {} selected", listener.addr),
                        None => format!("row {index} is not a listener on this page"),
                    },
                    None => "no listeners: the config did not load".to_string(),
                };
            }
            Hot::LogPane | Hot::StatusPane | Hot::ConfigPane | Hot::RowPane => {
                // Clicking inside a pane focuses it for scrolling; it is not a
                // button, and saying so is better than pretending nothing moved.
                self.message = format!("{} focused for scrolling", pane_name(hot));
            }
        }
        self.focus = hot;
    }

    /// The keyboard twin of a right click.
    pub fn alternate(&mut self, hot: Hot) {
        match hot {
            Hot::Section(index) => self.collapse_others(index),
            Hot::ConfigPane => {
                self.collapsed.clear();
                self.message = format!("all {} section(s) expanded", self.tree.sections.len());
            }
            Hot::LogPane => {
                self.logs.clear();
                self.message = "log view cleared (the gateway keeps writing)".to_string();
            }
            other => self.message = format!("no alternate action for {}", pane_name(other)),
        }
        self.focus = hot;
    }

    pub fn apply_mouse(&mut self, ev: MouseEvent) {
        let now_ms = u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        let cmd = input::route_mouse(
            ev.kind,
            ev.column,
            ev.row,
            &self.targets,
            &mut self.pointer,
            now_ms,
        );
        match cmd {
            MouseCmd::Press(hot) | MouseCmd::Double(hot) => {
                // A press does not act yet, but it does move the focus: this is
                // how the pointer hands over to the keyboard without a dead end.
                self.focus = hot;
                if let MouseCmd::Double(hot) = cmd {
                    self.activate(hot);
                }
            }
            MouseCmd::Activate(hot) => self.activate(hot),
            MouseCmd::Alternate(hot) => self.alternate(hot),
            MouseCmd::Jump(hot) => {
                self.message = self.jump(hot);
                self.focus = hot;
            }
            MouseCmd::Hover(hot) => self.hover = hot,
            MouseCmd::Wheel { over, dir } => self.scroll(over, dir, SCROLL_STEP),
            MouseCmd::Drag { over, dx, dy } => {
                // Drag and wheel share one direction rule on purpose: two gestures
                // that move the same pane opposite ways read as a bug, whichever
                // convention is "correct". So dragging down scrolls like wheeling
                // down — toward the newest end.
                if dy != 0 {
                    let dir = if dy > 0 { Wheel::Down } else { Wheel::Up };
                    self.scroll(over, dir, dy.unsigned_abs() as usize);
                }
                if dx != 0 {
                    let dir = if dx > 0 { Wheel::Right } else { Wheel::Left };
                    self.scroll(over, dir, dx.unsigned_abs() as usize);
                }
            }
            MouseCmd::Release => {
                // The press was withdrawn. Nothing acted; the paint already shows it.
            }
            MouseCmd::Ignore => {}
        }
    }

    pub fn apply_key(&mut self, ev: KeyEvent) {
        // Windows reports key release and repeat events. Acting on those would
        // fire the same control two or three times per physical press.
        if ev.kind != KeyEventKind::Press {
            return;
        }
        let cmd = input::route_key(ev.code, ev.modifiers, self.focus);
        self.apply_cmd(cmd);
    }

    pub fn apply_cmd(&mut self, cmd: KeyCmd) {
        match cmd {
            KeyCmd::Activate(hot) => self.activate(hot),
            KeyCmd::Alternate(hot) => self.alternate(hot),
            KeyCmd::Jump => self.message = self.jump(self.active_pane()),
            KeyCmd::Top => {
                // "Top" is the start of the document. On the log view the stored
                // number runs backwards from the newest line, so the start is its
                // largest offset, not zero — which is why this cannot be a literal.
                let pane = self.active_pane();
                let start = Self::pane_start(pane);
                self.set_scroll(pane, start);
                self.message = format!("top of {}", pane_name(pane));
            }
            KeyCmd::Scroll(dir) => self.scroll(self.active_pane(), dir, SCROLL_PAGE),
            KeyCmd::FocusNext => self.move_focus(1),
            KeyCmd::FocusPrev => self.move_focus(-1),
            KeyCmd::MoveSelection(step) => self.move_selection(step),
            KeyCmd::GotoPage(page) => self.set_page(page),
            KeyCmd::NextPage => self.set_page(self.page.next()),
            KeyCmd::PrevPage => self.set_page(self.page.prev()),
            KeyCmd::Start => self.message = self.start(),
            KeyCmd::Stop => self.message = self.stop(),
            KeyCmd::ToggleTheme => self.activate(Hot::Theme),
            KeyCmd::Quit => {
                self.quit = true;
                self.message = "quit requested".to_string();
            }
            KeyCmd::Ignore => {}
        }
    }

    /// The pane the keyboard scrolls on this page.
    pub fn active_pane(&self) -> Hot {
        match self.page {
            Page::Logs => Hot::LogPane,
            Page::Config => Hot::ConfigPane,
            Page::Listeners => Hot::RowPane,
            Page::Status => Hot::StatusPane,
        }
    }

    fn start(&mut self) -> String {
        match self.gate.start() {
            Ok(()) => match self.gate.pid() {
                Some(pid) => format!("started pid {pid} with {}", self.opts.config.display()),
                None => "started".to_string(),
            },
            Err(e) => format!("start failed: {e}"),
        }
    }

    fn stop(&mut self) -> String {
        match self.gate.request_stop() {
            Ok(()) => "SIGTERM sent; waiting for it to exit".to_string(),
            Err(e) => format!("stop failed: {e}"),
        }
    }

    fn set_page(&mut self, page: Page) {
        self.page = page;
        self.selected = self.selected.min(self.row_count().saturating_sub(1));
        self.message = format!("page {}", page.label());
    }

    fn toggle_section(&mut self, index: usize) {
        if index >= self.tree.sections.len() {
            self.message = format!("section {index} no longer exists");
            return;
        }
        if self.collapsed.contains(&index) {
            self.collapsed.remove(&index);
            self.message = format!("{} expanded", self.tree.sections[index].header);
        } else {
            self.collapsed.insert(index);
            self.message = format!("{} collapsed", self.tree.sections[index].header);
        }
    }

    fn collapse_others(&mut self, keep: usize) {
        let count = self.tree.sections.len();
        self.collapsed = (0..count).filter(|i| *i != keep).collect();
        self.message = format!(
            "only {} expanded",
            self.tree
                .sections
                .get(keep)
                .map(|s| s.header.clone())
                .unwrap_or_default()
        );
    }

    fn jump(&mut self, hot: Hot) -> String {
        match hot {
            Hot::LogPane | Hot::ConfigPane | Hot::StatusPane | Hot::RowPane => {
                let end = Self::pane_end(hot);
                self.set_scroll(hot, end);
                format!("at the newest end of the {}", pane_name(hot))
            }
            other => format!("nothing to jump to on {}", pane_name(other)),
        }
    }

    fn scroll(&mut self, over: Hot, dir: Wheel, step: usize) {
        let pane = match over {
            Hot::LogPane | Hot::ConfigPane | Hot::RowPane | Hot::StatusPane => over,
            // Over a button or a row: the wheel still scrolls the pane containing
            // it, so a row hit does not eat the gesture.
            _ => self.active_pane(),
        };
        let amount = step.max(1);
        let towards_end = matches!(dir, Wheel::Down | Wheel::Right);
        // One direction table, because the panes do not store their position the
        // same way: see `forward_increases`.
        let grows = towards_end == Self::forward_increases(pane);
        let current = self.scroll_of(pane);
        let next = if grows {
            current.saturating_add(amount)
        } else {
            current.saturating_sub(amount)
        };
        self.set_scroll(pane, next);
        // Horizontal scrolling of a text pane has no visible cursor of its own, so
        // the readback is the message band.
        if matches!(dir, Wheel::Left | Wheel::Right) {
            self.message = format!(
                "{} scrolled {} by {amount}",
                pane_name(pane),
                if towards_end { "right" } else { "left" }
            );
        }
    }

    /// Which way "forward" moves a pane's stored offset.
    /// The log view is stored as *lines above the newest*, so its number grows as
    /// the user moves away from the newest line; the config and status panes store
    /// *lines skipped from the top*, so theirs grows downward. Applying one
    /// direction rule to both would scroll the log view backwards — a bug no
    /// compiler can see, since both offsets are just `usize`.
    fn forward_increases(pane: Hot) -> bool {
        !matches!(pane, Hot::LogPane)
    }

    /// The offset that shows the first line of the pane. `usize::MAX / 2` stands in
    /// for "as far as it goes"; every setter clamps against the real content right
    /// after, so the sentinel never reaches the screen as a number.
    fn pane_start(pane: Hot) -> usize {
        if Self::forward_increases(pane) {
            0
        } else {
            usize::MAX / 2
        }
    }

    /// The offset that shows the newest / last line of the pane.
    fn pane_end(pane: Hot) -> usize {
        if Self::forward_increases(pane) {
            usize::MAX / 2
        } else {
            0
        }
    }

    /// Which offset belongs to which pane. Only panes reach these two functions:
    /// [`App::scroll`] maps any other hotspot onto the active pane first. The
    /// fallback is `0`, never another pane's offset — so if that ever stops being
    /// true, the worst case is a pane that does not move, not the wrong list
    /// scrolling under a gesture the user aimed somewhere else.
    fn scroll_of(&self, pane: Hot) -> usize {
        match pane {
            Hot::ConfigPane => self.config_scroll,
            Hot::RowPane => self.selected,
            Hot::StatusPane => self.status_scroll,
            Hot::LogPane => self.log_scroll,
            _ => 0,
        }
    }

    fn set_scroll(&mut self, pane: Hot, value: usize) {
        match pane {
            Hot::ConfigPane => self.config_scroll = value,
            Hot::LogPane => self.log_scroll = value,
            Hot::StatusPane => self.status_scroll = value,
            // The row pane's scroll *is* the selection, so the inverted band and
            // the visible window can never disagree.
            Hot::RowPane => self.selected = value,
            _ => {}
        }
        self.clamp_scrolls();
    }

    fn clamp_scrolls(&mut self) {
        let logs = self.logs.len();
        let max_log = logs.saturating_sub(1);
        if self.log_scroll > max_log {
            self.log_scroll = max_log;
        }
        let config = self.config_rows().len();
        let max_config = config.saturating_sub(1);
        if self.config_scroll > max_config {
            self.config_scroll = max_config;
        }
        let status = self.status_rows().len();
        let max_status = status.saturating_sub(1);
        if self.status_scroll > max_status {
            self.status_scroll = max_status;
        }
        let rows = self.row_count().saturating_sub(1);
        if self.selected > rows {
            self.selected = rows;
        }
    }

    fn move_selection(&mut self, step: i32) {
        let rows = self.row_count();
        if rows == 0 {
            self.message = "no rows on this page".to_string();
            return;
        }
        let moved = self.selected as i64 + i64::from(step);
        self.selected = moved.clamp(0, rows as i64 - 1) as usize;
        self.message = format!("row {} of {rows}", self.selected + 1);
    }

    /// Walk the focus through the last frame's hotspot table, wrapping. This is
    /// what makes every mouse-reachable control keyboard-reachable.
    fn move_focus(&mut self, step: i32) {
        if self.targets.is_empty() {
            self.message = "no controls were drawn in the last frame".to_string();
            return;
        }
        let ids: Vec<Hot> = self.targets.iter().map(|t| t.hot).collect();
        let at = ids.iter().position(|h| *h == self.focus).unwrap_or(0);
        let len = ids.len() as i64;
        let next = (at as i64 + i64::from(step)).rem_euclid(len) as usize;
        self.focus = ids[next];
        self.message = format!("focus: {}", hot_label(self.focus));
    }

    pub fn row_count(&self) -> usize {
        match self.page {
            Page::Listeners => self.reading.facts().map(|f| f.listeners.len()).unwrap_or(0),
            Page::Config => self.tree.sections.len(),
            Page::Logs => self.logs.len(),
            // The status page has no selectable rows — its body is one text pane,
            // scrolled by the wheel or PageUp/PageDown. Reporting 0 is what makes
            // `j`/`k` there say "no rows on this page" instead of moving a
            // highlight that is drawn nowhere.
            Page::Status => 0,
        }
    }

    /// The status page's body lines, built here rather than in the frame so that
    /// scrolling clamps against the same list the screen paints.
    ///
    /// Three blocks, kept visibly separate because they are three different kinds
    /// of claim: what *our* child process did; what the gateway's own parser read;
    /// and what a TCP connect could see. The last one is not evidence about the
    /// first, and the `WHAT IS NOT MEASURED` block exists so the page never passes
    /// off an absence of counters as a measurement of zero.
    pub fn status_rows(&self) -> Vec<String> {
        let mut rows: Vec<String> = Vec::new();
        rows.push(format!(
            "state {}   pid {}   uptime {}",
            self.status_label(),
            self.gate
                .pid()
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_string()),
            self.gate
                .uptime()
                .map(|up| format!("{:.1} s", up.as_secs_f64()))
                .unwrap_or_else(|| "-".to_string())
        ));
        if let GateState::Exited(info) = self.gate.state() {
            rows.push(format!("last exit {}", info.describe()));
        }
        rows.push(format!("binary {}", self.gate.gateway_bin().display()));
        rows.push(format!("config {}", self.gate.config_path().display()));
        rows.push(String::new());
        rows.push("AS THE GATEWAY'S OWN PARSER READS IT".to_string());
        match self.reading.facts() {
            Some(facts) => {
                for (key, value) in facts.status_rows() {
                    rows.push(format!("  {key:<22}{value}"));
                }
                for (key, value) in facts.ca_rows() {
                    rows.push(format!("  {key:<22}{value}"));
                }
            }
            None => rows.push(format!("  {}", self.reading.headline())),
        }
        rows.push(String::new());
        rows.push("WHAT IS NOT MEASURED".to_string());
        rows.push(
            "  connections, handshakes, bytes: the pinned core exposes no counters,".to_string(),
        );
        rows.push(
            "  so this page reports the absence instead of showing a confident 0".to_string(),
        );
        rows.push("  (docs/audits/09-session-handover, Gate AO).".to_string());
        rows
    }

    /// The config tree flattened to display rows, honouring the collapsed set.
    /// Rows keep their section index, because the frame needs it to register a
    /// clickable header per section.
    pub fn config_rows(&self) -> Vec<crate::tui::snapshot::Line> {
        if let Some(error) = &self.tree.error {
            let text = format!("could not read {}: {error}", self.tree.path.display());
            return vec![crate::tui::snapshot::Line {
                text,
                section: None,
                is_header: false,
            }];
        }
        let collapsed = &self.collapsed;
        self.tree.lines(&|index| collapsed.contains(&index))
    }

    /// The pill's colour comes from severity, its words from the process truth —
    /// never a merged guess about the other.
    pub fn status(&self) -> Status {
        match self.gate.state() {
            GateState::Stopped => Status::Stopped,
            GateState::Running => {
                if self.gate.uptime().is_some_and(|up| up < SETTLE) {
                    Status::Starting
                } else {
                    Status::Running
                }
            }
            // Not an error: a stop we asked for that has not come back yet.
            GateState::Stopping => Status::Degraded,
            GateState::Exited(info) => {
                let unsolicited = info.via == gate::Stop::Unsolicited;
                let failed = info.code.map(|c| c != 0).unwrap_or(info.signal.is_some());
                if unsolicited && failed {
                    Status::Failed
                } else {
                    Status::Stopped
                }
            }
        }
    }

    pub fn status_label(&self) -> String {
        self.gate.state().label().to_string()
    }

    /// The pointer's pressed control, for the painter.
    pub fn pressed(&self) -> Option<Hot> {
        self.pointer.pressed()
    }

    pub fn take_targets(&mut self) -> Vec<Target> {
        std::mem::take(&mut self.targets)
    }

    pub fn set_targets(&mut self, targets: Vec<Target>) {
        self.targets = targets;
    }

    pub fn tick_duration(&self) -> Duration {
        self.opts.tick
    }
}

fn pane_name(hot: Hot) -> &'static str {
    match hot {
        Hot::LogPane => "log pane",
        Hot::StatusPane => "status pane",
        Hot::ConfigPane => "config pane",
        Hot::RowPane => "row pane",
        Hot::Start => "Start",
        Hot::Stop => "Turn Off",
        Hot::Theme => "theme",
        Hot::Quit => "Quit",
        Hot::Tab(page) => page.label(),
        Hot::Section(_) => "config section",
        Hot::Row(_) => "table row",
    }
}

fn hot_label(hot: Hot) -> String {
    match hot {
        Hot::Section(index) => format!("section #{index}"),
        Hot::Row(index) => format!("row #{index}"),
        other => pane_name(other).to_string(),
    }
}

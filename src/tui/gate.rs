//! Supervision of the gateway child process.
//!
//! The TUI does not embed the gateway. It starts the real `sni-gate` binary as a
//! child and reads what it says, for two reasons:
//!
//! - the core is a pinned upstream whose runtime exposes no counters (see
//!   `docs/audits/09-session-handover-2026-10-04.md`, Gate AO), so anything the
//!   TUI invented in-process would be decoration;
//! - the process boundary is measurable: pid, exit code, termination signal, and
//!   the gateway's own log lines.
//!
//! Two facts stay apart on purpose, because merging them is the classic lie:
//! [`GateState`] is what *our* child did, [`Probe`] is whether a port accepts
//! TCP. A port accepting connections does not mean our gateway owns it — another
//! daemon can hold the same port, and the UI must show both readings rather than
//! a single confident word.

use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

/// How long to wait for a graceful stop before escalating to a hard kill.
///
/// The measured reason for waiting at all: `sni-gate` catches SIGTERM and exits 0
/// once it has had about a second to settle, but is *killed by* the signal (143)
/// when TERMed immediately after start. Telling "the user stopped it" apart from
/// "it died" needs that window, so the grace period is a measurement choice, not
/// politeness.
pub const STOP_GRACE: Duration = Duration::from_secs(3);

/// A line that arrived from one of the gateway's pipes.
#[derive(Debug, Clone)]
pub struct LogLine {
    pub text: String,
    /// Which pipe it came from. `tracing` writes to stderr by default, so this
    /// is shown rather than assumed away.
    pub stream: &'static str,
    pub at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    /// Nothing has been started, or the last child was reaped.
    Stopped,
    /// Spawned and still running.
    Running,
    /// A stop was requested; the child has not exited yet.
    Stopping,
    /// The child exited — by itself, on our signal, or forced.
    Exited(ExitInfo),
}

impl GateState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "STOPPED",
            Self::Running => "RUNNING",
            Self::Stopping => "STOPPING",
            Self::Exited(_) => "EXITED",
        }
    }

    /// `Running` only while our own child is alive. Nothing here infers liveness
    /// from a port.
    pub fn is_live(self) -> bool {
        matches!(self, Self::Running | Self::Stopping)
    }
}

/// How a child ended. `signal` is filled where the platform exposes it, which is
/// what distinguishes "the user stopped it" from "it died".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitInfo {
    pub code: Option<u8>,
    pub signal: Option<i32>,
    pub via: Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// We never asked it to stop; it left on its own.
    Unsolicited,
    /// We sent SIGTERM and it came back within `STOP_GRACE`.
    Term,
    /// `STOP_GRACE` expired and we escalated to a hard kill.
    Forced,
    /// Windows has no SIGTERM, so our stop is always a hard kill.
    Killed,
}

impl ExitInfo {
    pub fn from_status(status: ExitStatus, via: Stop) -> Self {
        Self {
            code: status.code().map(|c| c as u8),
            signal: signal_of(&status),
            via,
        }
    }

    /// A short, truthful one-liner for the status card.
    pub fn describe(self) -> String {
        let base = match (self.signal, self.code) {
            (Some(sig), _) => format!("signal {sig}"),
            (None, Some(code)) => format!("exit {code}"),
            (None, None) => "exit status unavailable".to_string(),
        };
        let via = match self.via {
            Stop::Unsolicited => "on its own",
            Stop::Term => "after our SIGTERM",
            Stop::Forced => "forced after the grace period",
            Stop::Killed => "killed (no SIGTERM on this platform)",
        };
        format!("{base}, {via}")
    }
}

#[cfg(unix)]
fn signal_of(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal_of(_status: &ExitStatus) -> Option<i32> {
    None
}

/// The supervisor. One per TUI process.
pub struct Gate {
    bin: PathBuf,
    config: PathBuf,
    child: Option<Child>,
    readers: Vec<JoinHandle<()>>,
    rx: Receiver<LogLine>,
    tx: Sender<LogLine>,
    state: GateState,
    started_at: Option<Instant>,
    stop_at: Option<Instant>,
    stop_kind: Stop,
}

impl Gate {
    pub fn new(bin: PathBuf, config: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel::<LogLine>();
        Self {
            bin,
            config,
            child: None,
            readers: Vec::new(),
            rx,
            tx,
            state: GateState::Stopped,
            started_at: None,
            stop_at: None,
            stop_kind: Stop::Unsolicited,
        }
    }

    pub fn state(&self) -> GateState {
        self.state
    }

    pub fn config_path(&self) -> &Path {
        &self.config
    }

    pub fn gateway_bin(&self) -> &Path {
        &self.bin
    }

    pub fn uptime(&self) -> Option<Duration> {
        self.started_at.map(|at| at.elapsed())
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// Drain queued log lines, keeping at most `limit` of the newest. The channel
    /// can outrun the screen, so the oldest are dropped rather than shown.
    pub fn drain_logs(&mut self, limit: usize) -> Vec<LogLine> {
        let mut out = Vec::new();
        while let Ok(line) = self.rx.try_recv() {
            out.push(line);
        }
        if out.len() > limit {
            let skip = out.len() - limit;
            out.drain(0..skip);
        }
        out
    }

    /// Spawn the gateway. The error is a string to *display*, not to crash on: a
    /// missing binary is a fact about the machine, and the UI has to say it
    /// instead of refusing to open.
    pub fn start(&mut self) -> Result<(), String> {
        if !matches!(self.state, GateState::Stopped | GateState::Exited(_)) {
            return Err(format!("cannot start while {}", self.state.label()));
        }
        if !self.bin.exists() {
            return Err(format!("no gateway binary at {}", self.bin.display()));
        }

        let mut cmd = Command::new(&self.bin);
        cmd.arg("--config").arg(&self.config);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Err(format!("could not start {}: {e}", self.bin.display())),
        };

        // One reader thread per pipe. Neither may block the UI, and neither is
        // closed by us: the child's exit ends the read.
        self.readers.clear();
        if let Some(out) = child.stdout.take() {
            let tx = self.tx.clone();
            self.readers
                .push(std::thread::spawn(move || pipe_reader(out, "stdout", tx)));
        }
        if let Some(err) = child.stderr.take() {
            let tx = self.tx.clone();
            self.readers
                .push(std::thread::spawn(move || pipe_reader(err, "stderr", tx)));
        }

        self.child = Some(child);
        self.started_at = Some(Instant::now());
        self.stop_at = None;
        self.stop_kind = Stop::Unsolicited;
        self.state = GateState::Running;
        Ok(())
    }

    /// Ask the gateway to stop. Unix gets SIGTERM because that is the path the
    /// gateway actually handles; Windows gets a kill and the UI says so.
    ///
    /// `/bin/kill` is used instead of a `libc` dependency. That is a real
    /// subprocess, so its failure is reported rather than assumed away.
    pub fn request_stop(&mut self) -> Result<(), String> {
        let Some(child) = self.child.as_mut() else {
            return Err("nothing is running".to_string());
        };
        let pid = child.id().to_string();

        #[cfg(unix)]
        {
            let outcome = Command::new("/bin/kill")
                .arg("-TERM")
                .arg(pid.as_str())
                .status();
            match outcome {
                Ok(status) if status.success() => self.stop_kind = Stop::Term,
                Ok(status) => return Err(format!("/bin/kill -TERM {pid} failed: {status}")),
                Err(e) => return Err(format!("could not run /bin/kill: {e}")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            let _ = child.kill();
            self.stop_kind = Stop::Killed;
        }

        self.stop_at = Some(Instant::now());
        self.state = GateState::Stopping;
        Ok(())
    }

    /// Advance the state machine once: reap the child, and escalate a process that
    /// is not answering after the grace period.
    pub fn tick(&mut self) {
        let mut finished: Option<(ExitStatus, Stop)> = None;
        let mut unwaitable: Option<String> = None;
        let mut escalate = false;

        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => finished = Some((status, self.stop_kind)),
                Ok(None) => {
                    if self.stop_at.is_some_and(|at| at.elapsed() > STOP_GRACE) {
                        escalate = true;
                    }
                }
                Err(e) => unwaitable = Some(format!("could not wait on the gateway: {e}")),
            }
        }

        if escalate {
            if let Some(child) = self.child.as_mut() {
                match child.kill() {
                    Ok(()) => self.stop_kind = Stop::Forced,
                    Err(e) => self.note(format!("force kill failed: {e}")),
                }
            }
        }

        if let Some((status, via)) = finished {
            self.reap();
            self.state = GateState::Exited(ExitInfo::from_status(status, via));
        } else if let Some(reason) = unwaitable {
            // No ExitStatus to read: report what is known and nothing more.
            self.note(reason);
            self.reap();
            self.state = GateState::Exited(ExitInfo {
                code: None,
                signal: None,
                via: self.stop_kind,
            });
        }
    }

    /// Push a message into the log stream, for failures that have no pipe.
    fn note(&self, text: String) {
        let _ = self.tx.send(LogLine {
            text,
            stream: "tui",
            at: Instant::now(),
        });
    }

    fn reap(&mut self) {
        self.child = None;
        self.started_at = None;
        self.stop_at = None;
        // Handles are dropped, not joined: the reader threads end by themselves
        // once the child's pipes close, and joining here could block a redraw.
        self.readers.clear();
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        // A TUI that exits must not leave a gateway bound to privileged ports.
        // TERM first, then wait inside the grace period, then kill: the same
        // order a user pressing Turn Off gets.
        #[cfg(unix)]
        if let Some(child) = self.child.as_ref() {
            let _ = Command::new("/bin/kill")
                .arg("-TERM")
                .arg(child.id().to_string())
                .status();
        }
        if let Some(child) = self.child.as_mut() {
            let deadline = Instant::now() + STOP_GRACE;
            while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn pipe_reader<R: std::io::Read + Send + 'static>(
    read: R,
    stream: &'static str,
    tx: Sender<LogLine>,
) {
    // `read_line` rather than `lines()`: the loop must end on EOF (0 bytes) and
    // on a read error alike, and a line that arrives without a trailing newline
    // at the moment the child exits still has to reach the screen.
    let mut reader = BufReader::new(read);
    let mut buffer = String::new();
    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => break,
            Ok(_) => {
                let text = buffer.trim_end_matches(['\r', '\n']);
                if tx
                    .send(LogLine {
                        text: text.to_string(),
                        stream,
                        at: Instant::now(),
                    })
                    .is_err()
                {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

/// Where the gateway binary is: an explicit path, else PATH, else the sibling of
/// this executable inside the same `target/{profile}` directory, else the bare
/// name so the error message still says what was tried.
///
/// Deliberately no `CARGO_MANIFEST_DIR`: that bakes this checkout's absolute path
/// into the binary, and a moved repository would report a search path that never
/// existed on this machine.
pub fn resolve_binary(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Some(found) = which("sni-gate") {
        return found;
    }
    if let Ok(me) = std::env::current_exe() {
        if let Some(dir) = me.parent() {
            let sibling = dir.join(exe_name("sni-gate"));
            if sibling.exists() {
                return sibling;
            }
        }
    }
    PathBuf::from(exe_name("sni-gate"))
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Look up a program on PATH without adding a dependency.
fn which(name: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    let probe = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .output()
        .ok()?;
    #[cfg(not(unix))]
    let probe = Command::new("where").arg(name).output().ok()?;
    if !probe.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&probe.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    }
}

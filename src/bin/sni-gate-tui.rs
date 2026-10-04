//! `sni-gate-tui` — the neobrutalist terminal front end for the gateway.
//!
//! It supervises the real `sni-gate` binary as a child process; it does not
//! contain a second copy of the gateway. See `src/tui/gate.rs` for why that
//! distinction is the honest one, and `docs/design-language.md` §6 for the
//! visual language this renders.
//!
//! Naming, as elsewhere in this repository: the product is **Himari**, the
//! command, the config file and the supervised binary are still `sni-gate`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;

use sni_gate::tui;
use sni_gate::tui::app::App;
use sni_gate::tui::app::Options;
use sni_gate::tui::gate;
use sni_gate::tui::tokens;

#[derive(Debug, Parser)]
#[command(
    name = "sni-gate-tui",
    version,
    about = "Neobrutalist terminal UI for the sni-gate gateway, mouse-driven"
)]
struct Cli {
    /// TOML configuration to show and to hand to the gateway. Same file, same
    /// parser as `sni-gate --config`.
    #[arg(short, long, default_value = "sni-gate.toml")]
    config: PathBuf,

    /// Which gateway binary to supervise. Defaults to `sni-gate` on PATH, then
    /// the sibling of this executable in the same `target/<profile>` directory.
    #[arg(long)]
    gate_bin: Option<PathBuf>,

    /// Start in the dark palette. The theme button switches it at any time.
    #[arg(long)]
    dark: bool,

    /// Accent colour as `#RRGGBB`. It paints the drop shadows only, so the
    /// button faces stay the same as the GUI's. Left unset by default: the
    /// accent is the project's open decision, not this UI's (`docs/
    /// design-language.md` §6.4).
    #[arg(long)]
    accent: Option<String>,

    /// Event poll interval in milliseconds, which is also how often the child is
    /// reaped and its log queue drained.
    #[arg(long, default_value_t = 200)]
    tick_ms: u64,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let accent = match cli.accent.as_deref() {
        Some(text) => match tokens::parse_hex(text) {
            Some(rgb) => Some(rgb),
            None => {
                // Refuse loudly. Silently falling back to "no accent" would make
                // the flag a decoration that appears to work.
                eprintln!("sni-gate-tui: --accent wants #RRGGBB or RRGGBB, got {text:?}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };

    if !tui::stdout_is_a_terminal() {
        eprintln!("sni-gate-tui: stdout is not a terminal, so it would take over a screen nobody is looking at");
        eprintln!("(it enters the alternate buffer and captures the mouse; run it in a terminal, or pipe its output nowhere)");
        return ExitCode::FAILURE;
    }

    let opts = Options {
        config: cli.config.clone(),
        gate_bin: gate::resolve_binary(cli.gate_bin.clone()),
        dark: cli.dark,
        accent,
        // A tick below 20 ms would spin the loop against an idle input queue.
        tick: Duration::from_millis(cli.tick_ms.max(20)),
    };

    let app = App::new(opts);
    match tui::run(app) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sni-gate-tui: {e}");
            ExitCode::FAILURE
        }
    }
}

//! Source-level gates over the terminal UI.
//!
//! These read the files as text instead of running them, which is deliberate: they
//! hold whether the `tui` feature is on, so the properties below stay guarded even
//! in a build that never compiles ratatui. They are the machine-checkable form of
//! claims that would otherwise live only in prose:
//!
//! - every gesture the docs promise has a producer in the router and an effect in
//!   the state machine (no control that goes nowhere, no variant nobody can reach);
//! - the UI does not touch the disk except to read the config it displays, and does
//!   not generate CA material (opening a status screen must have no side effects);
//! - the feature really is isolated, and CI really builds it — because a green
//!   default build says nothing about code it never compiled.
//!
//! Each scan carries a positive control: the checker is shown to fire on a sample it
//! was not pointed at, so "no violations" is not the same thing as "no eyes".

use std::path::Path;

fn read(rel: &str) -> String {
    std::fs::read_to_string(Path::new(rel))
        .unwrap_or_else(|e| panic!("{rel} is not readable from the package root: {e}"))
}

/// Variant names of `pub enum <name> { ... }`, handling both `Start,` and
/// `Tab(Page),` shapes. Test-local parsing: if this silently returns nothing, the
/// gates that use it become vacuous, so `variant_parser_sees_both_shapes` checks it
/// directly.
fn variants(source: &str, enum_name: &str) -> Vec<String> {
    let header = format!("pub enum {enum_name} {{");
    let Some(start) = source.find(&header) else {
        panic!("{enum_name} not found; the enum was renamed or the file moved");
    };
    let body = &source[start + header.len()..];
    let Some(end) = body.find("\n}") else {
        panic!("{enum_name} has no closing brace at line start");
    };
    let mut out = Vec::new();
    for line in body[..end].lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        let name = trimmed
            .split(['(', ',', ':', ' '])
            .next()
            .unwrap_or("")
            .trim();
        if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_uppercase()) {
            let name = name.split("::").next().unwrap_or(name).to_string();
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

#[test]
fn variant_parser_sees_both_shapes() {
    let sample = "pub enum Hot {\n    /// doc\n    Tab(Page),\n    Start,\n}\n";
    assert_eq!(variants(sample, "Hot"), ["Tab", "Start"]);
    // The guard against a vacuous parser: an enum with no body must yield nothing,
    // and a name that is not an enum must panic rather than return [].
    assert!(variants("pub enum Empty {\n}\n", "Empty").is_empty());
    let panicked =
        std::panic::catch_unwind(|| variants("struct NotAnEnum {}", "NotAnEnum")).is_err();
    assert!(
        panicked,
        "the extractor must complain loudly when the enum is missing"
    );
}

/// Every hotspot the router can name must be both *reachable* (some frame paints
/// and registers it) and *acted on* (the state machine matches on it). A variant
/// with no producer is a gesture nobody can make; one with no handler is a control
/// that eats the click.
#[test]
fn every_hotspot_has_a_producer_and_an_effect() {
    let input = read("src/tui/input.rs");
    let hot = variants(&input, "Hot");
    assert!(
        hot.len() >= 11,
        "expected the full hotspot set, parsed {hot:?}"
    );

    let app = read("src/tui/app.rs");
    let painted = format!("{}{}", read("src/tui/view.rs"), read("src/tui/widgets.rs"));

    for name in &hot {
        let token = format!("Hot::{name}");
        assert!(
            painted.contains(&token),
            "{token} is never constructed by the frame ⇒ it can never be registered ⇒ nothing can be clicked there"
        );
        assert!(
            app.contains(&token),
            "{token} is never matched in app.rs ⇒ a click on it does nothing at all"
        );
    }

    // Positive control for the scan: the same assertions must fail on an invented
    // variant, otherwise this test cannot go red for a real one.
    assert!(!painted.contains("Hot::DefinitelyNotReal"));
    assert!(!app.contains("Hot::DefinitelyNotReal"));
}

/// Same property for the two command enums, so the pointer and the keyboard cannot
/// grow an arm that no page implements.
#[test]
fn every_command_has_a_producer_and_a_consumer() {
    let input = read("src/tui/input.rs");
    let app = read("src/tui/app.rs");

    let mouse = variants(&input, "MouseCmd");
    assert!(mouse.len() >= 10, "MouseCmd variants parsed: {mouse:?}");
    let routed = read("src/tui/input.rs");
    for name in &mouse {
        let token = format!("MouseCmd::{name}");
        assert!(
            routed.contains(&token),
            "{token} is declared but never produced by route_mouse"
        );
        assert!(
            app.contains(&token),
            "{token} is produced but app.rs never acts on it"
        );
    }

    let keys = variants(&input, "KeyCmd");
    assert!(keys.len() >= 15, "KeyCmd variants parsed: {keys:?}");
    for name in &keys {
        let token = format!("KeyCmd::{name}");
        assert!(
            routed.contains(&token),
            "{token} is declared but route_key never returns it"
        );
        assert!(
            app.contains(&token),
            "{token} is returned by route_key but app.rs ignores it"
        );
    }
}

/// The UI's only side effect is starting the gateway it supervises. Reading the
/// config is fine; writing, generating, or deleting anything is not — opening a
/// status screen must not be able to create a CA key, refresh a PSL file, or touch
/// the cert store.
/// Strip line comments and doc comments, so a gate forbidding an API cannot fire on
/// prose that merely *names* it. `src/tui/snapshot.rs` documents why the UI refuses
/// to call `load_or_generate`; a scan that flagged that sentence would be a scan that
/// cannot tell a promise from a violation.
fn code_only(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_ui_writes_nothing_to_disk() {
    let forbidden = [
        "File::create",
        "fs::write",
        "fs::remove",
        "OpenOptions",
        "set_permissions",
        "load_or_generate",
        "ensure_installed",
        "std::process::Command::new(\"sh\"",
    ];
    let tui = [
        "src/tui/mod.rs",
        "src/tui/app.rs",
        "src/tui/view.rs",
        "src/tui/widgets.rs",
        "src/tui/paint.rs",
        "src/tui/tokens.rs",
        "src/tui/input.rs",
        "src/tui/gate.rs",
        "src/tui/snapshot.rs",
        "src/bin/sni-gate-tui.rs",
    ];
    let mut haystack = String::new();
    for file in tui {
        haystack.push_str(&code_only(&read(file)));
    }
    assert!(
        haystack.contains("let mut reader"),
        "the scan lost the code it should keep"
    );

    for needle in forbidden {
        assert!(
            !haystack.contains(needle),
            "the terminal UI must not do this: {needle}"
        );
    }
    // Positive control: the scan can see the strings it is looking for.
    for needle in forbidden {
        assert!(
            format!("clean body; {needle}() here").contains(needle),
            "the scan cannot even match {needle}"
        );
    }
    // And the one write-like path that *is* allowed is the supervised process
    // launch itself, which must exist or the Start button is a decoration.
    assert!(
        haystack.contains("Command::new(&self.bin)"),
        "Start must spawn the real gateway"
    );
    assert!(
        haystack.contains("/bin/kill"),
        "stop must be a real signal, not a flag"
    );
}

/// The help strip, the gesture docs, and the router must describe one set of
/// gestures. Three sources of truth that drift is exactly how a UI ends up
/// advertising a hidden gesture that does nothing.
#[test]
fn help_line_docs_and_router_name_the_same_gestures() {
    let docs = read("docs/tui.md");
    let widgets = read("src/tui/widgets.rs");
    let input = read("src/tui/input.rs");

    // Each gesture lives in three places, in three different registers: the English
    // strip the terminal shows, the Chinese table the user reads, and the variant
    // name the router uses. Checking one language against another would be a gate on
    // translation, not on behaviour — so each side gets its own token.
    let gestures = [
        ("press", "按下", "Press"),
        ("release", "松开", "Activate"),
        ("hover", "hover", "Hover"),
        ("drag", "拖拽", "Drag"),
        ("wheel", "滚轮", "Wheel"),
        ("right-click", "右键", "Alternate"),
        ("middle-click", "中键", "Jump"),
        ("double-click", "双击", "Double"),
    ];

    for (clause, doc_term, variant) in gestures {
        assert!(
            widgets.contains(clause),
            "help_line() lost the clause {clause:?}"
        );
        assert!(
            docs.contains(doc_term),
            "docs/tui.md no longer documents {doc_term:?}"
        );
        assert!(
            input.contains(&format!("MouseCmd::{variant}")),
            "the router has no MouseCmd::{variant} the strip and the docs both promise"
        );
    }

    // Positive control: a promise that is not implemented anywhere would be caught,
    // because the router scan is a real substring test.
    assert!(
        !input.contains("MouseCmd::TripleClick"),
        "the fixture moved; pick a genuinely absent gesture"
    );
}

/// The isolation claim, checked in the manifest rather than asserted in prose:
/// ratatui is optional, the `tui` feature is what turns it on, and the TUI binary
/// requires the feature — so `cargo build` for the gateway never sees any of it.
#[test]
fn the_feature_really_is_isolated() {
    let manifest = read("Cargo.toml");
    assert!(
        manifest.contains("ratatui = { version = \"0.30.2\", optional = true }"),
        "ratatui must stay optional"
    );
    assert!(
        manifest.contains("tui = [\"dep:ratatui\"]"),
        "the tui feature must gate the dependency itself"
    );
    assert!(
        manifest.contains("name = \"sni-gate-tui\""),
        "the TUI binary target disappeared"
    );
    assert!(
        manifest.contains("required-features = [\"tui\"]"),
        "the TUI binary must not build by default"
    );
    // The gateway's own target list must not have grown a ratatui dependency.
    let gate_bin = read("src/main.rs");
    assert!(
        !gate_bin.contains("ratatui"),
        "src/main.rs must not reach into the TUI"
    );
    assert!(
        !gate_bin.contains("mod tui"),
        "the gateway binary must not pull in the TUI modules"
    );
}

/// The terminal UI is shelved, and CI is where that has to show: no step may
/// compile it. Re-adding a `--features tui` step is a decision to un-shelve, and
/// it should be said out loud rather than smuggled in through a workflow edit.
#[test]
fn ci_does_not_build_the_shelved_ui() {
    let ci = read(".github/workflows/ci.yml");
    assert!(
        !ci.contains("--features tui"),
        "CI builds the tui feature again; that un-shelves the terminal UI"
    );
    // The gateway's own gates are still the ones CI runs.
    assert!(
        ci.contains("cargo build --locked"),
        "the default build step disappeared"
    );
    assert!(
        ci.contains("cargo test --locked"),
        "the default test step disappeared"
    );
    assert!(
        ci.contains("clippy --all-targets --locked"),
        "the lint step changed shape"
    );
    // The shelf itself is four targets of `required-features`, plus an optional
    // dependency: bin, three test files.
    let manifest = read("Cargo.toml");
    assert_eq!(
        manifest.matches("required-features = [\"tui\"]").count(),
        4,
        "expected the bin and three test targets to stay off the default build"
    );
    assert!(
        manifest.contains("ratatui = { version = \"0.30.2\", optional = true }"),
        "ratatui must stay optional"
    );
    assert!(
        manifest.contains("tui = [\"dep:ratatui\"]"),
        "the tui feature must gate the dependency itself"
    );
}

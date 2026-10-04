//! Real readings for the screens: the configuration the gateway would load, and
//! the file it came from.
//!
//! Two independent views, because they answer different questions and the UI must
//! not confuse them:
//!
//! - [`ConfigFacts`] is the **effective** config, produced by the gateway's own
//!   parser (`crate::config::Config::load`), so template references, shorthand
//!   ports and defaults are resolved exactly as the running process would resolve
//!   them. Re-implementing that reading here would be a second source of truth.
//! - [`FileTree`] is the **text** of the file, split into sections, which is what
//!   an "advanced config tree" should show — including comments, which carry the
//!   reasons for a value.
//!
//! What this module deliberately does *not* do: it never calls
//! `CertificateAuthority::load_or_generate`. That function writes a key and a
//! cert when they are missing, and opening a status screen must have no side
//! effects on disk. The CA is therefore described by what can be `stat`ed.

use std::path::Path;
use std::path::PathBuf;

use crate::config::Config;
use crate::config::Route;
use crate::config::RouteType;

/// The result of loading the config: facts, or the parser's own complaint.
#[derive(Debug, Clone)]
pub enum Reading {
    Loaded(ConfigFacts),
    Failed { path: PathBuf, reason: String },
}

impl Reading {
    pub fn facts(&self) -> Option<&ConfigFacts> {
        match self {
            Self::Loaded(facts) => Some(facts),
            Self::Failed { .. } => None,
        }
    }

    /// The status card's headline: a failed config is shown as itself, not as a
    /// confident zero.
    pub fn headline(&self) -> String {
        match self {
            Self::Loaded(facts) => format!(
                "{} listener(s), {} route(s)",
                facts.listeners.len(),
                facts.route_count()
            ),
            Self::Failed { reason, .. } => format!("config did not load: {reason}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigFacts {
    pub path: PathBuf,
    /// The `tracing` filter directive from `[global]`.
    pub log_directive: String,
    pub listeners: Vec<ListenerFacts>,
    pub ca: CaFacts,
    pub store: StoreFacts,
    pub cache: CacheFacts,
    pub psl: PslFacts,
    /// Named tables, by declared name, sorted — the config page lists them and
    /// the file tree shows their bodies.
    pub pools: Vec<String>,
    pub resolvers: Vec<String>,
    pub regexes: Vec<String>,
    pub templates: Vec<String>,
}

impl ConfigFacts {
    /// Load with the gateway's own parser. Any error becomes [`Reading::Failed`]
    /// with the parser's message, which is what the screen should show.
    pub fn load(path: &Path) -> Reading {
        match Config::load(path) {
            Ok(cfg) => Reading::Loaded(ConfigFacts::from_cfg(path, &cfg)),
            Err(e) => Reading::Failed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            },
        }
    }

    pub fn from_cfg(path: &Path, cfg: &Config) -> Self {
        let names = |map_keys: Vec<String>| {
            let mut v = map_keys;
            v.sort();
            v
        };

        Self {
            path: path.to_path_buf(),
            log_directive: cfg.global.log.clone(),
            listeners: cfg
                .listeners
                .iter()
                .map(ListenerFacts::from_listener)
                .collect(),
            ca: CaFacts::from_cfg(&cfg.ca),
            store: StoreFacts {
                enabled: cfg.store.enabled,
                dir: path_text(&cfg.store.dir),
                renew_margin_days: cfg.store.renew_margin_days,
            },
            cache: CacheFacts {
                capacity: cfg.cache.capacity,
                ttl_secs: cfg.cache.ttl_secs,
            },
            psl: PslFacts {
                source: format!("{:?}", cfg.psl.source),
                path: path_text(&cfg.psl.path),
                auto_update: cfg.psl.auto_update,
                file_present: cfg.psl.path.exists(),
            },
            pools: names(cfg.pools.keys().cloned().collect()),
            resolvers: names(cfg.resolvers.keys().cloned().collect()),
            regexes: names(cfg.regexes.keys().cloned().collect()),
            templates: names(cfg.templates.keys().cloned().collect()),
        }
    }

    pub fn route_count(&self) -> usize {
        self.listeners
            .iter()
            .map(|l| l.routes.len() + usize::from(l.default_route.is_some()))
            .sum()
    }

    /// The status page's first card, as rows. Every value here came from the
    /// parser or from a `stat`, never from a default the UI invented.
    pub fn status_rows(&self) -> Vec<(String, String)> {
        vec![
            ("config".to_string(), path_text(&self.path)),
            ("log filter".to_string(), self.log_directive.clone()),
            ("listeners".to_string(), self.listeners.len().to_string()),
            ("routes".to_string(), self.route_count().to_string()),
            ("pools".to_string(), count_list(&self.pools)),
            ("resolvers".to_string(), count_list(&self.resolvers)),
            ("regexes".to_string(), count_list(&self.regexes)),
            ("templates".to_string(), count_list(&self.templates)),
        ]
    }

    /// Rows of the `[ca]` card.
    pub fn ca_rows(&self) -> Vec<(String, String)> {
        let ca = &self.ca;
        vec![
            (
                "cert".to_string(),
                format!("{} ({})", ca.cert_path, present(ca.cert_present)),
            ),
            (
                "key".to_string(),
                format!("{} ({})", ca.key_path, present(ca.key_present)),
            ),
            ("common name".to_string(), ca.common_name.clone()),
            (
                "organization".to_string(),
                or_unset(ca.organization.clone()),
            ),
            ("country".to_string(), or_unset(ca.country.clone())),
            (
                "leaf validity".to_string(),
                format!("{} d", ca.leaf_validity_days),
            ),
            (
                "install to system root".to_string(),
                yes_no(ca.install_to_system_root).to_string(),
            ),
            (
                "store".to_string(),
                format!(
                    "{} ({}), margin {} d",
                    yes_no(self.store.enabled),
                    self.store.dir,
                    self.store.renew_margin_days
                ),
            ),
            (
                "cache".to_string(),
                format!(
                    "{} entries, {} s ttl",
                    self.cache.capacity, self.cache.ttl_secs
                ),
            ),
            (
                "psl".to_string(),
                format!("{} ({})", self.psl.source, present(self.psl.file_present)),
            ),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct ListenerFacts {
    pub addr: String,
    /// The parsed bind address, kept typed so the port probe never has to
    /// re-parse a string and silently drop a listener it cannot read.
    pub socket_addr: std::net::SocketAddr,
    pub template: Option<String>,
    pub routes: Vec<RouteFacts>,
    pub default_route: Option<RouteFacts>,
}

impl ListenerFacts {
    fn from_listener(l: &crate::config::Listener) -> Self {
        Self {
            addr: l.addr.to_string(),
            socket_addr: l.addr,
            template: l.use_template.clone(),
            routes: l.routes.iter().map(RouteFacts::from_route).collect(),
            default_route: l.default_route.as_ref().map(RouteFacts::from_route),
        }
    }

    /// One line per listener for the Listeners page.
    pub fn summary(&self) -> String {
        let template = self
            .template
            .as_deref()
            .map(|t| format!(" use={t}"))
            .unwrap_or_default();
        format!("{}  {} route(s){}", self.addr, self.routes.len(), template)
    }
}

#[derive(Debug, Clone)]
pub struct RouteFacts {
    pub name: String,
    pub kind: String,
    pub matches: Vec<String>,
    pub upstream: String,
    pub template: Option<String>,
}

impl RouteFacts {
    fn from_route(r: &Route) -> Self {
        Self {
            name: r.name.clone().unwrap_or_else(|| "(unnamed)".to_string()),
            // `type` may be omitted when a template supplies it; the parser has
            // resolved it by now, but the struct keeps the option, so say so
            // rather than inventing a default.
            kind: r
                .route_type
                .map(kind_text)
                .unwrap_or("(from template)")
                .to_string(),
            matches: r.match_sni.clone(),
            upstream: r
                .upstream
                .clone()
                // Documented behaviour of an omitted `upstream` (config.rs,
                // `Route::upstream`): host = the matched SNI/Host, port = this
                // listener's port. Shown as the rule, not as a made-up address.
                .unwrap_or_else(|| "(matched name, listener port)".to_string()),
            template: r.use_template.clone(),
        }
    }

    pub fn row(&self) -> String {
        let matches = if self.matches.is_empty() {
            "(default)".to_string()
        } else {
            self.matches.join(",")
        };
        format!(
            "{}  {}  {}  ->  {}",
            self.name, self.kind, matches, self.upstream
        )
    }
}

fn kind_text(kind: RouteType) -> &'static str {
    match kind {
        RouteType::Ech => "ech",
        RouteType::Tls => "tls",
        RouteType::Http => "http",
        RouteType::Raw => "raw",
    }
}

#[derive(Debug, Clone)]
pub struct CaFacts {
    pub cert_path: String,
    pub key_path: String,
    pub cert_present: bool,
    pub key_present: bool,
    pub common_name: String,
    pub organization: String,
    pub country: String,
    pub leaf_validity_days: u32,
    pub install_to_system_root: bool,
}

impl CaFacts {
    fn from_cfg(ca: &crate::config::CaConfig) -> Self {
        Self {
            cert_path: path_text(&ca.cert_path),
            key_path: path_text(&ca.key_path),
            // `stat` only. Loading the CA would generate material if absent.
            cert_present: ca.cert_path.exists(),
            key_present: ca.key_path.exists(),
            common_name: ca.common_name.clone(),
            organization: ca.organization.clone(),
            country: ca.country.clone(),
            leaf_validity_days: ca.leaf_validity_days,
            install_to_system_root: ca.install_to_system_root,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoreFacts {
    pub enabled: bool,
    pub dir: String,
    pub renew_margin_days: u32,
}

#[derive(Debug, Clone)]
pub struct CacheFacts {
    pub capacity: u64,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone)]
pub struct PslFacts {
    pub source: String,
    pub path: String,
    pub auto_update: bool,
    pub file_present: bool,
}

/// A section of the config file, by header, with its body lines kept in order.
#[derive(Debug, Clone)]
pub struct Section {
    /// The header text, e.g. `[listener.route]` — or `(top)` before the first.
    pub header: String,
    pub lines: Vec<String>,
}

/// The file's own structure, for the collapsible config tree.
#[derive(Debug, Clone)]
pub struct FileTree {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    /// Set when the file could not be read; the tree is then empty and the screen
    /// says why instead of showing a blank panel.
    pub error: Option<String>,
}

impl FileTree {
    /// Split on TOML table headers. Comments stay in the section they belong to,
    /// because in this project's config files they are the reason for the value.
    pub fn read(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                return Self {
                    path: path.to_path_buf(),
                    sections: Vec::new(),
                    error: Some(e.to_string()),
                };
            }
        };
        Self {
            path: path.to_path_buf(),
            sections: sections_of(&text),
            error: None,
        }
    }

    pub fn from_text(path: &Path, text: &str) -> Self {
        Self {
            path: path.to_path_buf(),
            sections: sections_of(text),
            error: None,
        }
    }

    /// Flatten to display lines, honouring which sections are collapsed. Row 0 of
    /// each section is its header, which is also the clickable node.
    pub fn lines(&self, collapsed: &impl Fn(usize) -> bool) -> Vec<Line> {
        let mut out = Vec::new();
        for (index, section) in self.sections.iter().enumerate() {
            out.push(Line {
                text: section.header.clone(),
                section: Some(index),
                is_header: true,
            });
            if collapsed(index) {
                out.push(Line {
                    text: format!("{} line(s) hidden", section.lines.len()),
                    section: Some(index),
                    is_header: false,
                });
                continue;
            }
            for raw in &section.lines {
                out.push(Line {
                    text: raw.clone(),
                    section: Some(index),
                    is_header: false,
                });
            }
        }
        out
    }
}

fn sections_of(text: &str) -> Vec<Section> {
    let mut sections = vec![Section {
        header: "(top)".to_string(),
        lines: Vec::new(),
    }];
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.starts_with('[') && line.ends_with(']') {
            sections.push(Section {
                header: line.to_string(),
                lines: Vec::new(),
            });
        } else if let Some(section) = sections.last_mut() {
            section.lines.push(line.to_string());
        }
    }
    // A file with no leading header leaves an empty `(top)`; do not show it.
    if sections
        .first()
        .is_some_and(|s| s.lines.iter().all(|l| l.trim().is_empty()))
    {
        sections.remove(0);
    }
    sections
}

/// One display row of the config tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub section: Option<usize>,
    pub is_header: bool,
}

fn path_text(path: &Path) -> String {
    path.display().to_string()
}

fn present(yes: bool) -> &'static str {
    if yes {
        "present"
    } else {
        "missing"
    }
}

fn yes_no(flag: bool) -> &'static str {
    if flag {
        "yes"
    } else {
        "no"
    }
}

fn or_unset(value: String) -> String {
    if value.trim().is_empty() {
        "(unset)".to_string()
    } else {
        value
    }
}

fn count_list(names: &[String]) -> String {
    if names.is_empty() {
        "0".to_string()
    } else {
        format!("{}: {}", names.len(), names.join(","))
    }
}

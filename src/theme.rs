use ratatui::style::Color;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{OnceLock, RwLock};

/// Semantic color palette used across the whole UI. Every color the UI draws
/// resolves through one of these slots, so a theme fully reskins the app.
#[derive(Clone)]
pub struct Theme {
    pub accent: Color,    // primary highlight: focused labels, borders, titles
    pub dim: Color,       // secondary text, inactive borders, separators
    pub highlight: Color, // background fill behind the focused field row
    pub text: Color,      // primary readable values
    pub muted: Color,     // de-emphasized values (e.g. derived ws url)
    pub success: Color,   // healthy/positive: balances, price, reachable
    pub warning: Color,   // in-progress / caution: editing cursor, spinner
    pub error: Color,     // failures and danger (mainnet hazard, bad keypair)
    pub on_accent: Color, // text drawn on top of a colored background (badges)
    pub mainnet: Color,   // per-cluster accents
    pub devnet: Color,
    pub testnet: Color,
    pub localhost: Color,
    pub custom: Color, // any non-builtin RPC endpoint
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    /// The default — a conventional cyan/grey TUI palette.
    fn default_theme() -> Self {
        Self {
            accent: Color::Cyan,
            dim: Color::DarkGray,
            highlight: rgb(38, 40, 56),
            text: Color::White,
            muted: Color::Gray,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            on_accent: Color::Black,
            mainnet: Color::Red,
            devnet: Color::Magenta,
            testnet: Color::Yellow,
            localhost: Color::Blue,
            custom: Color::Cyan,
        }
    }

    /// Neon synthwave — a dracula base with hot-pink/cyan highlights.
    fn synthwave() -> Self {
        Self {
            accent: rgb(255, 106, 193), // hot pink
            dim: rgb(98, 114, 164),
            highlight: rgb(68, 71, 90),
            text: rgb(248, 248, 242),
            muted: rgb(98, 114, 164),
            success: rgb(54, 249, 246), // neon cyan
            warning: rgb(254, 222, 93),
            error: rgb(254, 68, 80),
            on_accent: rgb(40, 42, 54),
            mainnet: rgb(255, 85, 85),
            devnet: rgb(114, 241, 184), // mint
            testnet: rgb(241, 250, 140),
            localhost: rgb(139, 233, 253),
            custom: rgb(184, 147, 206),
        }
    }

    fn dracula() -> Self {
        Self {
            accent: rgb(189, 147, 249),
            dim: rgb(98, 114, 164),
            highlight: rgb(68, 71, 90),
            text: rgb(248, 248, 242),
            muted: rgb(98, 114, 164),
            success: rgb(80, 250, 123),
            warning: rgb(241, 250, 140),
            error: rgb(255, 85, 85),
            on_accent: rgb(40, 42, 54),
            mainnet: rgb(255, 85, 85),
            devnet: rgb(189, 147, 249),
            testnet: rgb(241, 250, 140),
            localhost: rgb(139, 233, 253),
            custom: rgb(255, 121, 198),
        }
    }

    fn gruvbox() -> Self {
        Self {
            accent: rgb(250, 189, 47),
            dim: rgb(146, 131, 116),
            highlight: rgb(60, 56, 54),
            text: rgb(235, 219, 178),
            muted: rgb(168, 153, 132),
            success: rgb(184, 187, 38),
            warning: rgb(254, 128, 25),
            error: rgb(251, 73, 52),
            on_accent: rgb(40, 40, 40),
            mainnet: rgb(251, 73, 52),
            devnet: rgb(211, 134, 155),
            testnet: rgb(250, 189, 47),
            localhost: rgb(131, 165, 152),
            custom: rgb(142, 192, 124),
        }
    }

    fn solarized() -> Self {
        Self {
            accent: rgb(38, 139, 210),
            dim: rgb(88, 110, 117),
            highlight: rgb(7, 54, 66),
            text: rgb(147, 161, 161),
            muted: rgb(101, 123, 131),
            success: rgb(133, 153, 0),
            warning: rgb(181, 137, 0),
            error: rgb(220, 50, 47),
            on_accent: rgb(0, 43, 54),
            mainnet: rgb(220, 50, 47),
            devnet: rgb(211, 54, 130),
            testnet: rgb(181, 137, 0),
            localhost: rgb(42, 161, 152),
            custom: rgb(108, 113, 196),
        }
    }

    fn nord() -> Self {
        Self {
            accent: rgb(136, 192, 208),
            dim: rgb(76, 86, 106),
            highlight: rgb(59, 66, 82),
            text: rgb(236, 239, 244),
            muted: rgb(216, 222, 233),
            success: rgb(163, 190, 140),
            warning: rgb(235, 203, 139),
            error: rgb(191, 97, 106),
            on_accent: rgb(46, 52, 64),
            mainnet: rgb(191, 97, 106),
            devnet: rgb(180, 142, 173),
            testnet: rgb(235, 203, 139),
            localhost: rgb(129, 161, 193),
            custom: rgb(143, 188, 187),
        }
    }

    /// Phosphor-green terminal look.
    fn matrix() -> Self {
        Self {
            accent: rgb(0, 255, 102),
            dim: rgb(0, 90, 40),
            highlight: rgb(0, 28, 14),
            text: rgb(0, 220, 90),
            muted: rgb(0, 120, 50),
            success: rgb(0, 255, 102),
            warning: rgb(180, 255, 120),
            error: rgb(255, 80, 80),
            on_accent: Color::Black,
            mainnet: rgb(255, 80, 80),
            devnet: rgb(120, 255, 160),
            testnet: rgb(180, 255, 120),
            localhost: rgb(0, 200, 120),
            custom: rgb(0, 255, 180),
        }
    }

    fn lookup(name: &str) -> Option<Self> {
        Some(match name.trim().to_lowercase().as_str() {
            "default" => Self::default_theme(),
            "synthwave" => Self::synthwave(),
            "dracula" => Self::dracula(),
            "gruvbox" => Self::gruvbox(),
            "solarized" => Self::solarized(),
            "nord" => Self::nord(),
            "matrix" => Self::matrix(),
            _ => return None,
        })
    }
}

/// Built-in palette names, in the order the switcher cycles them first.
const BUILTIN_NAMES: [&str; 7] = [
    "default",
    "synthwave",
    "dracula",
    "gruvbox",
    "solarized",
    "nord",
    "matrix",
];

/// A built-in palette by name, falling back to the default.
fn builtin(name: &str) -> Theme {
    Theme::lookup(name).unwrap_or_else(Theme::default_theme)
}

// ── on-disk config ──────────────────────────────────────────────────────

/// The per-color override fields shared by the top-level config and each
/// user-defined theme. Any `Some` value replaces that slot.
#[derive(Deserialize, Default, Clone)]
struct RawColors {
    accent: Option<String>,
    dim: Option<String>,
    highlight: Option<String>,
    text: Option<String>,
    muted: Option<String>,
    success: Option<String>,
    warning: Option<String>,
    error: Option<String>,
    on_accent: Option<String>,
    mainnet: Option<String>,
    devnet: Option<String>,
    testnet: Option<String>,
    localhost: Option<String>,
    custom: Option<String>,
}

/// A user-defined theme: starts from `base` (default `default`) then overrides.
#[derive(Deserialize, Default, Clone)]
struct UserTheme {
    base: Option<String>,
    #[serde(flatten)]
    colors: RawColors,
}

/// `~/.config/solfig/theme.yml`: active `name`, top-level overrides, and a map
/// of user-defined `themes`.
#[derive(Deserialize, Default)]
struct RawTheme {
    name: Option<String>,
    #[serde(flatten)]
    overrides: RawColors,
    #[serde(default)]
    themes: BTreeMap<String, UserTheme>,
}

fn file() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_default();
    p.push(".config/solfig/theme.yml");
    p
}

/// A documented starter file written on first run so the option is discoverable.
const EXAMPLE: &str = "\
# SOLFIG theme.
# Pick a palette by name (built-in or one you define under `themes:` below).
# Built-ins: default · synthwave · dracula · gruvbox · solarized · nord · matrix
name: default

# Optionally override individual colors on top of the chosen palette.
# Values are color names (cyan, light-red, gray, …) or hex (#1e2025).
# Uncomment any line to tweak it:
# accent:    \"#26e0c0\"   # focused labels, borders, titles
# dim:       darkgray     # secondary text & separators
# highlight: \"#262838\"   # background behind the focused row
# text:      white        # primary values
# muted:     gray         # de-emphasized values
# success:   green        # balances, price, reachable
# warning:   yellow       # editing cursor, spinners
# error:     red          # failures & mainnet hazard
# on_accent: black        # text drawn on colored badges
# mainnet:   red          # per-cluster accents
# devnet:    magenta
# testnet:   yellow
# localhost: blue
# custom:    cyan

# Define your own named themes — they show up in the in-app `T` switcher.
# Each starts from `base` (default) and overrides any slots:
# themes:
#   mytheme:
#     base: nord
#     accent: \"#ff79c6\"
#     error:  \"#ff5555\"
";

fn apply(slot: &mut Color, raw: &Option<String>) {
    if let Some(s) = raw {
        if let Ok(c) = Color::from_str(s) {
            *slot = c;
        }
    }
}

fn apply_colors(t: &mut Theme, c: &RawColors) {
    apply(&mut t.accent, &c.accent);
    apply(&mut t.dim, &c.dim);
    apply(&mut t.highlight, &c.highlight);
    apply(&mut t.text, &c.text);
    apply(&mut t.muted, &c.muted);
    apply(&mut t.success, &c.success);
    apply(&mut t.warning, &c.warning);
    apply(&mut t.error, &c.error);
    apply(&mut t.on_accent, &c.on_accent);
    apply(&mut t.mainnet, &c.mainnet);
    apply(&mut t.devnet, &c.devnet);
    apply(&mut t.testnet, &c.testnet);
    apply(&mut t.localhost, &c.localhost);
    apply(&mut t.custom, &c.custom);
}

/// The fully-resolved theme config, parsed once from disk.
struct ThemeConfig {
    /// All selectable names: built-ins first, then user themes (sorted).
    names: Vec<String>,
    /// Resolved user themes, keyed by lowercased name.
    user: HashMap<String, Theme>,
    /// The configured active name (a known built-in or user theme; else `default`).
    configured: String,
    /// The startup theme: resolved `configured` with top-level overrides applied.
    active: Theme,
}

fn resolve_in(user: &HashMap<String, Theme>, name: &str) -> Theme {
    let key = name.trim().to_lowercase();
    user.get(&key).cloned().unwrap_or_else(|| builtin(&key))
}

/// Read and fully resolve `theme.yml`, seeding a documented example on first run.
fn load_config() -> ThemeConfig {
    let path = file();
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&path, EXAMPLE);
            EXAMPLE.to_string()
        }
    };
    let raw: RawTheme = serde_yaml::from_str(&contents).unwrap_or_default();

    // Resolve each user theme: base palette + its overrides.
    let mut user: HashMap<String, Theme> = HashMap::new();
    let mut user_names: Vec<String> = Vec::new();
    for (name, ut) in &raw.themes {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }
        let mut t = builtin(ut.base.as_deref().unwrap_or("default"));
        apply_colors(&mut t, &ut.colors);
        if !BUILTIN_NAMES.contains(&key.as_str()) {
            user_names.push(key.clone());
        }
        user.insert(key, t);
    }
    user_names.sort();
    user_names.dedup();

    let mut names: Vec<String> = BUILTIN_NAMES.iter().map(|s| s.to_string()).collect();
    names.extend(user_names);

    let configured = raw
        .name
        .as_deref()
        .map(|n| n.trim().to_lowercase())
        .filter(|n| names.contains(n))
        .unwrap_or_else(|| "default".to_string());

    let mut active = resolve_in(&user, &configured);
    apply_colors(&mut active, &raw.overrides);

    ThemeConfig {
        names,
        user,
        configured,
        active,
    }
}

static CONFIG: OnceLock<ThemeConfig> = OnceLock::new();

fn config() -> &'static ThemeConfig {
    CONFIG.get_or_init(load_config)
}

/// All selectable theme names (built-ins followed by user-defined themes).
pub fn names() -> &'static [String] {
    &config().names
}

/// Resolve any theme by name — user-defined themes take precedence over
/// built-ins of the same name; unknown names fall back to the default.
pub fn resolve(name: &str) -> Theme {
    resolve_in(&config().user, name)
}

/// The active theme name configured in `theme.yml` (seeds the switcher).
pub fn configured_name() -> String {
    config().configured.clone()
}

static CURRENT: OnceLock<RwLock<Theme>> = OnceLock::new();

fn current() -> &'static RwLock<Theme> {
    CURRENT.get_or_init(|| RwLock::new(config().active.clone()))
}

/// The active theme. Swappable at runtime via [`set`] (the `T` switcher).
pub fn theme() -> Theme {
    current().read().unwrap().clone()
}

/// Swap the active theme (e.g. live preview while cycling in the switcher).
pub fn set(t: Theme) {
    *current().write().unwrap() = t;
}

/// Persist the chosen theme name to `theme.yml`, preserving the user's comments,
/// per-color overrides, and `themes:` definitions (only the `name:` line moves).
pub fn persist_name(name: &str) -> anyhow::Result<()> {
    let path = file();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in existing.lines() {
        let t = line.trim_start();
        if !replaced && !t.starts_with('#') && t.starts_with("name:") {
            out.push(format!("name: {name}"));
            replaced = true;
        } else {
            out.push(line.to_string());
        }
    }
    if !replaced {
        out.insert(0, format!("name: {name}"));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, out.join("\n") + "\n")?;
    Ok(())
}

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Mirrors the Solana CLI config file (`config.yml`).
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct SolanaConfig {
    #[serde(default)]
    pub json_rpc_url: String,
    #[serde(default)]
    pub websocket_url: String,
    #[serde(default)]
    pub keypair_path: String,
    #[serde(default)]
    pub commitment: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub address_labels: BTreeMap<String, String>,
}

/// Known cluster monikers and their canonical RPC URLs.
pub const MONIKERS: [(&str, &str); 4] = [
    ("mainnet-beta", "https://api.mainnet-beta.solana.com"),
    ("devnet", "https://api.devnet.solana.com"),
    ("testnet", "https://api.testnet.solana.com"),
    ("localhost", "http://127.0.0.1:8899"),
];

pub const COMMITMENTS: [&str; 3] = ["processed", "confirmed", "finalized"];

/// Resolve the active config path: `$SOLANA_CONFIG_FILE` or the CLI default.
pub fn default_config_path() -> PathBuf {
    if let Ok(p) = std::env::var("SOLANA_CONFIG_FILE") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let mut p = dirs::home_dir().unwrap_or_default();
    p.push(".config/solana/cli/config.yml");
    p
}

/// Load a config file, falling back to sensible defaults if missing/empty.
pub fn load(path: &Path) -> SolanaConfig {
    let mut cfg: SolanaConfig = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_yaml::from_str(&s).ok())
        .unwrap_or_default();
    if cfg.json_rpc_url.is_empty() {
        cfg.json_rpc_url = MONIKERS[0].1.to_string();
    }
    if cfg.commitment.is_empty() {
        cfg.commitment = "confirmed".to_string();
    }
    if cfg.keypair_path.is_empty() {
        cfg.keypair_path = default_keypair_path();
    } else {
        // The Solana CLI does not expand `~`; self-heal to an absolute path.
        cfg.keypair_path = absolute(&cfg.keypair_path);
    }
    cfg
}

/// Default keypair path as an absolute string.
pub fn default_keypair_path() -> String {
    let mut p = dirs::home_dir().unwrap_or_default();
    p.push(".config/solana/id.json");
    p.to_string_lossy().into_owned()
}

/// Expand a leading `~` and return an absolute path string the CLI can open.
pub fn absolute(path: &str) -> String {
    expand_tilde(path).to_string_lossy().into_owned()
}

/// Write the config atomically (temp file + rename).
pub fn save(path: &Path, cfg: &SolanaConfig) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Persist an absolute keypair path so the Solana CLI can read it.
    let mut out = cfg.clone();
    out.keypair_path = absolute(&out.keypair_path);
    let yaml = serde_yaml::to_string(&out)?;
    let tmp = path.with_extension("yml.tmp");
    std::fs::write(&tmp, yaml)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// The moniker name for a URL, if it matches a known cluster.
pub fn moniker_for_url(url: &str) -> Option<&'static str> {
    MONIKERS.iter().find(|(_, u)| *u == url).map(|(m, _)| *m)
}

/// Derive the WebSocket URL the CLI computes from an RPC URL:
/// `http`→`ws`, `https`→`wss`, and port (if present) incremented by one.
pub fn derive_ws_url(rpc: &str) -> String {
    let (rest, ws_scheme) = if let Some(r) = rpc.strip_prefix("https://") {
        (r, "wss://")
    } else if let Some(r) = rpc.strip_prefix("http://") {
        (r, "ws://")
    } else {
        return String::new();
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let authority = match authority.rfind(':') {
        Some(colon) => {
            let (host, port) = authority.split_at(colon);
            match port[1..].parse::<u16>() {
                Ok(p) => format!("{host}:{}", p + 1),
                Err(_) => authority.to_string(),
            }
        }
        None => authority.to_string(),
    };
    format!("{ws_scheme}{authority}{path}")
}

/// Expand a leading `~/` to the home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Derive the base58 pubkey from a keypair JSON file (64-byte array).
pub fn pubkey_from_keypair(path: &str) -> Option<String> {
    let data = std::fs::read_to_string(expand_tilde(path)).ok()?;
    let bytes: Vec<u8> = serde_json::from_str(&data).ok()?;
    if bytes.len() != 64 {
        return None;
    }
    Some(bs58::encode(&bytes[32..]).into_string())
}

/// Render a path with a leading `~` when it lives under the home directory.
pub fn display_path(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if let Some(home) = dirs::home_dir() {
        let home = home.to_string_lossy().to_string();
        if let Some(rest) = s.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    s
}

/// A discovered keypair file with its derived pubkey.
pub struct KeyFile {
    pub display: String,
    pub path: String,
    pub pubkey: String,
}

/// Scan common locations for valid keypair files (64-byte JSON arrays).
pub fn scan_keypairs(current: &str) -> Vec<KeyFile> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".config/solana"));
        dirs.push(home.join("Downloads"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    if let Some(parent) = expand_tilde(current).parent() {
        dirs.push(parent.to_path_buf());
    }

    let mut out: Vec<KeyFile> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                // Keypair files are tiny; skip anything large to avoid slow reads.
                let small = entry.metadata().map(|m| m.len() < 2048).unwrap_or(false);
                if !small {
                    continue;
                }
                let display = display_path(&path);
                if !seen.insert(display.clone()) {
                    continue;
                }
                if let Some(pubkey) = pubkey_from_keypair(&path.to_string_lossy()) {
                    out.push(KeyFile {
                        display,
                        path: path.to_string_lossy().into_owned(),
                        pubkey,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.display.cmp(&b.display));
    out
}

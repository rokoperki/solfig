use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A user-saved named RPC endpoint (e.g. a Helius/Triton URL).
#[derive(Serialize, Deserialize, Clone)]
pub struct Endpoint {
    pub name: String,
    pub url: String,
}

fn file() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_default();
    p.push(".config/solfig/endpoints.yml");
    p
}

/// Load saved custom endpoints (empty if none).
pub fn load() -> Vec<Endpoint> {
    std::fs::read_to_string(file())
        .ok()
        .and_then(|s| serde_yaml::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist the custom endpoint list.
pub fn save(list: &[Endpoint]) -> anyhow::Result<()> {
    let path = file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_yaml::to_string(list)?)?;
    Ok(())
}

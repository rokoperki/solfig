use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A web faucet opened in the browser (HTTP UI, not a JSON-RPC endpoint).
#[derive(Serialize, Deserialize, Clone)]
pub struct Faucet {
    pub name: String,
    pub url: String,
}

fn file() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_default();
    p.push(".config/solfig/faucets.yml");
    p
}

fn defaults() -> Vec<Faucet> {
    vec![
        Faucet {
            name: "solana".to_string(),
            url: "https://faucet.solana.com".to_string(),
        },
        Faucet {
            name: "quicknode".to_string(),
            url: "https://faucet.quicknode.com/solana/devnet".to_string(),
        },
    ]
}

/// Load saved faucets, seeding sensible defaults on first run.
pub fn load() -> Vec<Faucet> {
    match std::fs::read_to_string(file()) {
        Ok(s) => serde_yaml::from_str(&s).unwrap_or_default(),
        Err(_) => {
            let d = defaults();
            let _ = save(&d);
            d
        }
    }
}

/// Persist the faucet list.
pub fn save(list: &[Faucet]) -> anyhow::Result<()> {
    let path = file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_yaml::to_string(list)?)?;
    Ok(())
}

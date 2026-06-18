use crate::config::{self, SolanaConfig};
use std::path::{Path, PathBuf};

/// A saved named config environment.
pub struct Profile {
    pub name: String,
    pub path: PathBuf,
    pub cfg: SolanaConfig,
}

/// Where profile files live: `~/.config/solfig/profiles`.
pub fn profiles_dir() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_default();
    p.push(".config/solfig/profiles");
    p
}

/// List all saved profiles, sorted by name.
pub fn list() -> Vec<Profile> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(profiles_dir()) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yml") {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let cfg = config::load(&path);
                out.push(Profile { name, path, cfg });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Save the given config as a new (or overwritten) profile.
pub fn save(name: &str, cfg: &SolanaConfig) -> anyhow::Result<()> {
    let clean = name.trim().replace(['/', '\\'], "-");
    if clean.is_empty() {
        anyhow::bail!("empty profile name");
    }
    let dir = profiles_dir();
    std::fs::create_dir_all(&dir)?;
    config::save(&dir.join(format!("{clean}.yml")), cfg)
}

/// Activate a profile by copying it over the active config path.
pub fn activate(profile: &Profile, active_path: &Path) -> anyhow::Result<()> {
    if let Some(dir) = active_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::copy(&profile.path, active_path)?;
    Ok(())
}

/// Delete a profile file.
pub fn delete(profile: &Profile) -> anyhow::Result<()> {
    std::fs::remove_file(&profile.path)?;
    Ok(())
}

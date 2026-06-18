//! Integration tests for the config / keypair layer.

use solfig::config::{
    default_keypair_path, derive_ws_url, display_path, expand_tilde, load, moniker_for_url,
    pubkey_from_keypair, save, write_secret, SolanaConfig, MONIKERS,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A unique temp path that does not touch the user's real config tree.
fn tmp(name: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut p = std::env::temp_dir();
    p.push(format!("solfig-test-{}-{n}-{name}", std::process::id()));
    p
}

#[test]
fn derive_ws_url_handles_scheme_and_port() {
    // No port: scheme flips, authority untouched.
    assert_eq!(
        derive_ws_url("https://api.devnet.solana.com"),
        "wss://api.devnet.solana.com"
    );
    // Port present: incremented by one.
    assert_eq!(
        derive_ws_url("http://127.0.0.1:8899"),
        "ws://127.0.0.1:8900"
    );
    // Port + path: path preserved, port bumped.
    assert_eq!(
        derive_ws_url("https://example.com:443/rpc"),
        "wss://example.com:444/rpc"
    );
    // Path without a port.
    assert_eq!(derive_ws_url("http://host/path"), "ws://host/path");
    // Non-http schemes are unsupported → empty.
    assert_eq!(derive_ws_url("ftp://example.com"), "");
}

#[test]
fn expand_tilde_only_expands_leading_tilde_slash() {
    let home = dirs::home_dir().unwrap();
    assert_eq!(expand_tilde("~/foo/bar"), home.join("foo/bar"));
    assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
    assert_eq!(expand_tilde("relative"), PathBuf::from("relative"));
    // A bare "~" (no slash) is left alone.
    assert_eq!(expand_tilde("~tilde"), PathBuf::from("~tilde"));
}

#[test]
fn moniker_for_url_round_trips_known_clusters() {
    assert_eq!(
        moniker_for_url("https://api.mainnet-beta.solana.com"),
        Some("mainnet-beta")
    );
    assert_eq!(moniker_for_url("http://127.0.0.1:8899"), Some("localhost"));
    assert_eq!(moniker_for_url("https://my.custom.rpc"), None);
}

#[test]
fn display_path_collapses_home_to_tilde() {
    let home = dirs::home_dir().unwrap();
    let inside = home.join(".config/solana/id.json");
    assert_eq!(display_path(&inside), "~/.config/solana/id.json");
    assert_eq!(display_path(Path::new("/etc/hosts")), "/etc/hosts");
}

#[test]
fn pubkey_from_keypair_reads_trailing_32_bytes() {
    // Solana keypair JSON = [secret(32) | public(32)]. All-zero public
    // bytes encode to the well-known system-program address.
    let mut bytes = vec![1u8; 32];
    bytes.extend(vec![0u8; 32]);
    let path = tmp("kp.json");
    std::fs::write(&path, serde_json::to_string(&bytes).unwrap()).unwrap();

    assert_eq!(
        pubkey_from_keypair(&path.to_string_lossy()),
        Some("11111111111111111111111111111111".to_string())
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn pubkey_from_keypair_rejects_bad_input() {
    // Wrong length.
    let short = tmp("short.json");
    std::fs::write(&short, serde_json::to_string(&vec![0u8; 10]).unwrap()).unwrap();
    assert_eq!(pubkey_from_keypair(&short.to_string_lossy()), None);
    std::fs::remove_file(&short).ok();

    // Not JSON.
    let garbage = tmp("garbage.json");
    std::fs::write(&garbage, "not json at all").unwrap();
    assert_eq!(pubkey_from_keypair(&garbage.to_string_lossy()), None);
    std::fs::remove_file(&garbage).ok();

    // Missing file.
    assert_eq!(pubkey_from_keypair("/no/such/keypair.json"), None);
}

#[test]
fn load_falls_back_to_defaults_when_missing() {
    let cfg = load(&tmp("does-not-exist.yml"));
    assert_eq!(cfg.json_rpc_url, MONIKERS[0].1); // mainnet-beta
    assert_eq!(cfg.commitment, "confirmed");
    assert_eq!(cfg.keypair_path, default_keypair_path());
}

#[test]
fn save_then_load_round_trips_values() {
    let path = tmp("config.yml");
    let cfg = SolanaConfig {
        json_rpc_url: "https://api.devnet.solana.com".to_string(),
        websocket_url: "wss://api.devnet.solana.com".to_string(),
        keypair_path: "/tmp/some/id.json".to_string(),
        commitment: "finalized".to_string(),
        ..Default::default()
    };
    save(&path, &cfg).unwrap();
    let back = load(&path);
    assert_eq!(back.json_rpc_url, cfg.json_rpc_url);
    assert_eq!(back.websocket_url, cfg.websocket_url);
    assert_eq!(back.commitment, "finalized");
    // Keypair path is persisted as an absolute path the CLI can open.
    assert_eq!(back.keypair_path, "/tmp/some/id.json");
    std::fs::remove_file(&path).ok();
}

#[cfg(unix)]
#[test]
fn write_secret_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let path = tmp("secret.json");
    write_secret(&path, b"[0,1,2,3]").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    // Only the low 9 permission bits matter; the secret must be 0600.
    assert_eq!(mode & 0o777, 0o600, "got {:o}", mode & 0o777);
    std::fs::remove_file(&path).ok();
}

#[test]
fn save_expands_tilde_keypair_to_absolute() {
    let path = tmp("config-tilde.yml");
    let cfg = SolanaConfig {
        keypair_path: "~/wallet.json".to_string(),
        ..Default::default()
    };
    save(&path, &cfg).unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    let home = dirs::home_dir().unwrap();
    assert!(raw.contains(&home.join("wallet.json").to_string_lossy().to_string()));
    assert!(!raw.contains("~/wallet.json"));
    std::fs::remove_file(&path).ok();
}

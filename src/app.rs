use crate::config::{self, KeyFile, SolanaConfig, COMMITMENTS, MONIKERS};
use crate::endpoints::{self, Endpoint};
use crate::faucets::{self, Faucet};
use crate::profiles::{self, Profile};
use crate::theme::{self, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub enum Field {
    Cluster,
    Keypair,
    Commitment,
    Websocket,
}

pub const FIELDS: [Field; 4] = [
    Field::Cluster,
    Field::Keypair,
    Field::Commitment,
    Field::Websocket,
];

#[derive(PartialEq)]
pub enum Mode {
    Normal,
    EditField,
    KeyPicker,
    Profiles,
    NewProfile,
    Endpoints,
    NewEndpoint,
    Faucets,
    NewFaucet,
    Transfer,
    TransferConfirm,
    Themes,
    Help,
}

pub struct App {
    pub cfg: SolanaConfig,
    pub path: PathBuf,
    pub sel: usize,
    pub dirty: bool,
    pub mode: Mode,
    pub buf: String,
    pub status: String,
    pub should_quit: bool,
    pub confirm_quit: bool,
    pub need_health_check: bool,
    pub need_balance_check: bool,
    pub need_airdrop: bool,
    pub profiles: Vec<Profile>,
    pub prof_sel: usize,
    pub key_files: Vec<KeyFile>,
    pub key_filter: String,
    pub key_sel: usize,
    pub endpoints: Vec<Endpoint>,
    pub ep_sel: usize,
    pub airdrop_sol: f64,
    pub faucets: Vec<Faucet>,
    pub faucet_sel: usize,
    pub need_transfer: bool,
    pub tx_to: String,
    pub tx_amount: String,
    pub tx_focus: usize,
    pub tx_key_idx: usize,
    pub theme_name: String,
    pub theme_sel: usize,
    pub theme_prev: Theme,
}

/// Selectable airdrop amounts (SOL).
pub const AIRDROP_STEPS: [f64; 4] = [0.5, 1.0, 2.0, 5.0];

impl App {
    pub fn new(path: PathBuf) -> Self {
        let cfg = config::load(&path);
        Self {
            cfg,
            path,
            sel: 0,
            dirty: false,
            mode: Mode::Normal,
            buf: String::new(),
            status: "loaded".to_string(),
            should_quit: false,
            confirm_quit: false,
            need_health_check: true,
            need_balance_check: true,
            need_airdrop: false,
            profiles: Vec::new(),
            prof_sel: 0,
            key_files: Vec::new(),
            key_filter: String::new(),
            key_sel: 0,
            endpoints: endpoints::load(),
            ep_sel: 0,
            airdrop_sol: 1.0,
            faucets: Vec::new(),
            faucet_sel: 0,
            need_transfer: false,
            tx_to: String::new(),
            tx_amount: String::new(),
            tx_focus: 0,
            tx_key_idx: 0,
            theme_name: theme::configured_name(),
            theme_sel: 0,
            theme_prev: theme::theme(),
        }
    }

    pub fn field(&self) -> Field {
        FIELDS[self.sel]
    }

    /// All selectable clusters: built-in monikers followed by custom endpoints.
    pub fn clusters(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = MONIKERS
            .iter()
            .map(|(n, u)| (n.to_string(), u.to_string()))
            .collect();
        for e in &self.endpoints {
            v.push((e.name.clone(), e.url.clone()));
        }
        v
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            Mode::Normal => self.key_normal(key),
            Mode::EditField => self.key_edit(key),
            Mode::KeyPicker => self.key_picker(key),
            Mode::Profiles => self.key_profiles(key),
            Mode::NewProfile => self.key_new_profile(key),
            Mode::Endpoints => self.key_endpoints(key),
            Mode::NewEndpoint => self.key_new_endpoint(key),
            Mode::Faucets => self.key_faucets(key),
            Mode::NewFaucet => self.key_new_faucet(key),
            Mode::Transfer => self.key_transfer(key),
            Mode::TransferConfirm => self.key_transfer_confirm(key),
            Mode::Themes => self.key_themes(key),
            Mode::Help => self.key_help(key),
        }
    }

    fn key_help(&mut self, key: KeyEvent) {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
        ) {
            self.mode = Mode::Normal;
        }
    }

    fn key_normal(&mut self, key: KeyEvent) {
        // Pending unsaved-quit confirmation intercepts the next keystroke.
        if self.confirm_quit {
            self.confirm_quit = false;
            match key.code {
                KeyCode::Char('q') => {
                    self.should_quit = true;
                    return;
                }
                KeyCode::Char('s') => {
                    self.save();
                    self.should_quit = true;
                    return;
                }
                KeyCode::Esc => {
                    self.status = "quit cancelled".to_string();
                    return;
                }
                _ => {} // any other key cancels and is handled normally below
            }
        }

        match key.code {
            KeyCode::Char('q') => {
                if self.dirty {
                    self.confirm_quit = true;
                    self.status =
                        "unsaved changes — q again to discard, s to save & quit, esc cancel"
                            .to_string();
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.sel = wrap_index(self.sel, 1, FIELDS.len());
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.sel = wrap_index(self.sel, -1, FIELDS.len());
            }
            KeyCode::Left | KeyCode::Char('h') => self.adjust(-1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust(1),
            KeyCode::Enter => self.activate_field(),
            KeyCode::Char('a') => self.request_airdrop(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.cycle_airdrop(1),
            KeyCode::Char('-') | KeyCode::Char('_') => self.cycle_airdrop(-1),
            KeyCode::Char('y') => self.copy_field(),
            KeyCode::Char('o') => self.open_explorer(),
            KeyCode::Char('t') => self.open_transfer(),
            KeyCode::Char('e') => self.open_endpoints(),
            KeyCode::Char('f') => self.open_faucets(),
            KeyCode::Char('s') => self.save(),
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('p') => self.open_profiles(),
            KeyCode::Char('T') => self.open_themes(),
            KeyCode::Char('?') => self.mode = Mode::Help,
            _ => {}
        }
    }

    /// Cycle the airdrop amount through the preset steps.
    fn cycle_airdrop(&mut self, delta: i32) {
        let cur = AIRDROP_STEPS
            .iter()
            .position(|s| (*s - self.airdrop_sol).abs() < f64::EPSILON)
            .unwrap_or(1);
        self.airdrop_sol = AIRDROP_STEPS[wrap_index(cur, delta, AIRDROP_STEPS.len())];
        self.status = format!("airdrop amount: {} SOL", self.airdrop_sol);
    }

    /// Request an airdrop of the current amount to the keypair (non-mainnet only).
    fn request_airdrop(&mut self) {
        if config::moniker_for_url(&self.cfg.json_rpc_url) == Some("mainnet-beta") {
            self.status = "airdrop unavailable on mainnet-beta".to_string();
            return;
        }
        if config::pubkey_from_keypair(&self.cfg.keypair_path).is_none() {
            self.status = "no valid keypair to airdrop to".to_string();
            return;
        }
        self.need_airdrop = true;
        self.status = "requesting airdrop…".to_string();
    }

    /// Copy the focused field's resolved value to the system clipboard.
    fn copy_field(&mut self) {
        let (what, value) = match self.field() {
            Field::Cluster => ("RPC URL", self.cfg.json_rpc_url.clone()),
            Field::Keypair => (
                "pubkey",
                config::pubkey_from_keypair(&self.cfg.keypair_path).unwrap_or_default(),
            ),
            Field::Commitment => ("commitment", self.cfg.commitment.clone()),
            Field::Websocket => (
                "websocket URL",
                if self.cfg.websocket_url.is_empty() {
                    self.cfg.json_rpc_url.clone()
                } else {
                    self.cfg.websocket_url.clone()
                },
            ),
        };
        if value.is_empty() {
            self.status = format!("nothing to copy for {what}");
            return;
        }
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(value)) {
            Ok(()) => self.status = format!("copied {what}"),
            Err(e) => self.status = format!("copy failed: {e}"),
        }
    }

    /// Open the keypair's address in Solana Explorer for the active cluster.
    fn open_explorer(&mut self) {
        let Some(pubkey) = config::pubkey_from_keypair(&self.cfg.keypair_path) else {
            self.status = "no valid keypair to open".to_string();
            return;
        };
        let url = format!(
            "https://explorer.solana.com/address/{pubkey}{}",
            self.explorer_cluster_param()
        );
        self.status = match open_url(&url) {
            Ok(()) => "opened explorer".to_string(),
            Err(e) => format!("open failed: {e}"),
        };
    }

    /// The `?cluster=…` suffix Explorer needs for the active RPC.
    fn explorer_cluster_param(&self) -> String {
        match config::moniker_for_url(&self.cfg.json_rpc_url) {
            Some("mainnet-beta") => String::new(),
            Some("devnet") => "?cluster=devnet".to_string(),
            Some("testnet") => "?cluster=testnet".to_string(),
            // localhost or any custom endpoint → point Explorer at the raw URL
            _ => format!(
                "?cluster=custom&customUrl={}",
                url_encode(&self.cfg.json_rpc_url)
            ),
        }
    }

    /// Cycle presets for the focused field (left/right arrows).
    fn adjust(&mut self, delta: i32) {
        match self.field() {
            Field::Cluster => {
                let list = self.clusters();
                let cur = list
                    .iter()
                    .position(|(_, u)| *u == self.cfg.json_rpc_url)
                    .unwrap_or(0);
                self.cfg.json_rpc_url = list[wrap_index(cur, delta, list.len())].1.clone();
                self.dirty = true;
                self.need_health_check = true;
                self.need_balance_check = true;
            }
            Field::Commitment => {
                let cur = COMMITMENTS
                    .iter()
                    .position(|c| *c == self.cfg.commitment)
                    .unwrap_or(1);
                self.cfg.commitment =
                    COMMITMENTS[wrap_index(cur, delta, COMMITMENTS.len())].to_string();
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// Enter: cycle commitment, or open a text editor for free-text fields.
    fn activate_field(&mut self) {
        match self.field() {
            Field::Commitment => self.adjust(1),
            Field::Cluster => self.begin_edit(self.cfg.json_rpc_url.clone()),
            Field::Keypair => self.open_key_picker(),
            Field::Websocket => self.begin_edit(self.cfg.websocket_url.clone()),
        }
    }

    fn begin_edit(&mut self, initial: String) {
        self.buf = initial;
        self.mode = Mode::EditField;
    }

    fn key_edit(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                let val = self.buf.trim().to_string();
                match self.field() {
                    Field::Cluster => {
                        self.cfg.json_rpc_url = val;
                        self.need_health_check = true;
                        self.need_balance_check = true;
                    }
                    Field::Keypair => {
                        self.cfg.keypair_path = val;
                        self.need_balance_check = true;
                    }
                    Field::Websocket => self.cfg.websocket_url = val,
                    Field::Commitment => {}
                }
                self.dirty = true;
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.buf.pop();
            }
            KeyCode::Char(c) => self.buf.push(c),
            _ => {}
        }
    }

    fn save(&mut self) {
        match config::save(&self.path, &self.cfg) {
            Ok(()) => {
                self.dirty = false;
                self.status = "saved".to_string();
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
    }

    fn reload(&mut self) {
        self.cfg = config::load(&self.path);
        self.dirty = false;
        self.need_health_check = true;
        self.need_balance_check = true;
        self.status = "reloaded".to_string();
    }

    fn open_key_picker(&mut self) {
        self.key_files = config::scan_keypairs(&self.cfg.keypair_path);
        self.key_filter.clear();
        self.key_sel = 0;
        self.mode = Mode::KeyPicker;
    }

    /// Indices of `key_files` matching the current filter (subsequence match).
    pub fn filtered_keys(&self) -> Vec<usize> {
        if self.key_filter.is_empty() {
            return (0..self.key_files.len()).collect();
        }
        let needle = self.key_filter.to_lowercase();
        self.key_files
            .iter()
            .enumerate()
            .filter(|(_, kf)| {
                let hay = format!("{} {}", kf.display, kf.pubkey).to_lowercase();
                is_subsequence(&needle, &hay)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn key_picker(&mut self, key: KeyEvent) {
        let matches = self.filtered_keys();
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Down => self.key_sel = wrap_index(self.key_sel, 1, matches.len()),
            KeyCode::Up => self.key_sel = wrap_index(self.key_sel, -1, matches.len()),
            KeyCode::Enter => {
                if let Some(&idx) = matches.get(self.key_sel) {
                    self.cfg.keypair_path = self.key_files[idx].path.clone();
                    self.dirty = true;
                    self.need_balance_check = true;
                    self.status = "keypair selected".to_string();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.key_filter.pop();
                self.key_sel = 0;
            }
            KeyCode::Char('g') if self.key_filter.is_empty() => self.generate_keypair(),
            KeyCode::Char(c) => {
                self.key_filter.push(c);
                self.key_sel = 0;
            }
            _ => {}
        }
    }

    /// Generate a fresh keypair, select it, and refresh the picker list.
    fn generate_keypair(&mut self) {
        match config::generate_keypair() {
            Ok((path, pubkey)) => {
                self.cfg.keypair_path = path;
                self.dirty = true;
                self.need_balance_check = true;
                let short = format!(
                    "{}...{}",
                    &pubkey[..4.min(pubkey.len())],
                    &pubkey[pubkey.len().saturating_sub(4)..]
                );
                self.status = format!("generated {short}");
                self.mode = Mode::Normal;
            }
            Err(e) => self.status = format!("keygen failed: {e}"),
        }
    }

    fn open_profiles(&mut self) {
        self.profiles = profiles::list();
        self.prof_sel = 0;
        self.mode = Mode::Profiles;
    }

    fn key_profiles(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('p') | KeyCode::Char('q') => {
                self.mode = Mode::Normal;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.prof_sel = wrap_index(self.prof_sel, 1, self.profiles.len());
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.prof_sel = wrap_index(self.prof_sel, -1, self.profiles.len());
            }
            KeyCode::Enter => self.activate_profile(),
            KeyCode::Char('n') => {
                self.buf.clear();
                self.mode = Mode::NewProfile;
            }
            KeyCode::Char('d') => self.delete_profile(),
            _ => {}
        }
    }

    fn activate_profile(&mut self) {
        if let Some(p) = self.profiles.get(self.prof_sel) {
            match profiles::activate(p, &self.path) {
                Ok(()) => {
                    self.status = format!("activated '{}'", p.name);
                    self.cfg = config::load(&self.path);
                    self.dirty = false;
                    self.need_health_check = true;
                    self.need_balance_check = true;
                    self.mode = Mode::Normal;
                }
                Err(e) => self.status = format!("activate failed: {e}"),
            }
        }
    }

    fn delete_profile(&mut self) {
        if let Some(p) = self.profiles.get(self.prof_sel) {
            match profiles::delete(p) {
                Ok(()) => {
                    self.status = format!("deleted '{}'", p.name);
                    self.profiles = profiles::list();
                    if self.prof_sel >= self.profiles.len() {
                        self.prof_sel = self.profiles.len().saturating_sub(1);
                    }
                }
                Err(e) => self.status = format!("delete failed: {e}"),
            }
        }
    }

    fn key_new_profile(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Profiles,
            KeyCode::Enter => {
                match profiles::save(&self.buf, &self.cfg) {
                    Ok(()) => {
                        self.status = format!("saved profile '{}'", self.buf.trim());
                        self.profiles = profiles::list();
                    }
                    Err(e) => self.status = format!("profile save failed: {e}"),
                }
                self.mode = Mode::Profiles;
            }
            KeyCode::Backspace => {
                self.buf.pop();
            }
            KeyCode::Char(c) => self.buf.push(c),
            _ => {}
        }
    }

    fn open_endpoints(&mut self) {
        self.endpoints = endpoints::load();
        self.ep_sel = 0;
        self.mode = Mode::Endpoints;
    }

    fn key_endpoints(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('e') | KeyCode::Char('q') => {
                self.mode = Mode::Normal;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.ep_sel = wrap_index(self.ep_sel, 1, self.endpoints.len());
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.ep_sel = wrap_index(self.ep_sel, -1, self.endpoints.len());
            }
            KeyCode::Enter => {
                if let Some(ep) = self.endpoints.get(self.ep_sel) {
                    self.cfg.json_rpc_url = ep.url.clone();
                    self.dirty = true;
                    self.need_health_check = true;
                    self.need_balance_check = true;
                    self.status = format!("using '{}'", ep.name);
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Char('n') => {
                self.buf.clear();
                self.mode = Mode::NewEndpoint;
            }
            KeyCode::Char('d') => {
                if self.ep_sel < self.endpoints.len() {
                    let removed = self.endpoints.remove(self.ep_sel);
                    let _ = endpoints::save(&self.endpoints);
                    self.ep_sel = self.ep_sel.min(self.endpoints.len().saturating_sub(1));
                    self.status = format!("removed '{}'", removed.name);
                }
            }
            _ => {}
        }
    }

    fn key_new_endpoint(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Endpoints,
            KeyCode::Enter => {
                match self.buf.split_once('=') {
                    Some((name, url)) if !name.trim().is_empty() && !url.trim().is_empty() => {
                        self.endpoints.push(Endpoint {
                            name: name.trim().to_string(),
                            url: url.trim().to_string(),
                        });
                        match endpoints::save(&self.endpoints) {
                            Ok(()) => self.status = format!("added '{}'", name.trim()),
                            Err(e) => self.status = format!("save failed: {e}"),
                        }
                    }
                    _ => self.status = "format: name=url".to_string(),
                }
                self.mode = Mode::Endpoints;
            }
            KeyCode::Backspace => {
                self.buf.pop();
            }
            KeyCode::Char(c) => self.buf.push(c),
            _ => {}
        }
    }

    fn open_faucets(&mut self) {
        self.faucets = faucets::load();
        self.faucet_sel = 0;
        self.mode = Mode::Faucets;
    }

    fn key_faucets(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('f') | KeyCode::Char('q') => {
                self.mode = Mode::Normal;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.faucet_sel = wrap_index(self.faucet_sel, 1, self.faucets.len());
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.faucet_sel = wrap_index(self.faucet_sel, -1, self.faucets.len());
            }
            KeyCode::Enter => self.launch_faucet(),
            KeyCode::Char('n') => {
                self.buf.clear();
                self.mode = Mode::NewFaucet;
            }
            KeyCode::Char('d') => {
                if self.faucet_sel < self.faucets.len() {
                    let removed = self.faucets.remove(self.faucet_sel);
                    let _ = faucets::save(&self.faucets);
                    self.faucet_sel = self.faucet_sel.min(self.faucets.len().saturating_sub(1));
                    self.status = format!("removed '{}'", removed.name);
                }
            }
            _ => {}
        }
    }

    /// Copy the pubkey to the clipboard and open the faucet in the browser.
    fn launch_faucet(&mut self) {
        let Some(faucet) = self.faucets.get(self.faucet_sel) else {
            return;
        };
        let name = faucet.name.clone();
        let url = faucet.url.clone();
        let copied = config::pubkey_from_keypair(&self.cfg.keypair_path)
            .and_then(|pk| {
                arboard::Clipboard::new()
                    .and_then(|mut c| c.set_text(pk))
                    .ok()
            })
            .is_some();
        self.status = match open_url(&url) {
            Ok(()) if copied => format!("opened {name} — pubkey copied to clipboard"),
            Ok(()) => format!("opened {name}"),
            Err(e) => format!("open failed: {e}"),
        };
        self.mode = Mode::Normal;
    }

    fn key_new_faucet(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Faucets,
            KeyCode::Enter => {
                match self.buf.split_once('=') {
                    Some((name, url)) if !name.trim().is_empty() && !url.trim().is_empty() => {
                        self.faucets.push(Faucet {
                            name: name.trim().to_string(),
                            url: url.trim().to_string(),
                        });
                        match faucets::save(&self.faucets) {
                            Ok(()) => self.status = format!("added '{}'", name.trim()),
                            Err(e) => self.status = format!("save failed: {e}"),
                        }
                    }
                    _ => self.status = "format: name=url".to_string(),
                }
                self.mode = Mode::Faucets;
            }
            KeyCode::Backspace => {
                self.buf.pop();
            }
            KeyCode::Char(c) => self.buf.push(c),
            _ => {}
        }
    }
    // --- Transfer SOL ---------------------------------------------------

    fn open_transfer(&mut self) {
        if config::pubkey_from_keypair(&self.cfg.keypair_path).is_none() {
            self.status = "no valid keypair to send from".to_string();
            return;
        }
        self.key_files = config::scan_keypairs(&self.cfg.keypair_path);
        self.tx_to.clear();
        self.tx_amount.clear();
        self.tx_focus = 0;
        self.tx_key_idx = 0;
        self.mode = Mode::Transfer;
    }

    /// Display name of the local wallet matching the current recipient, if any.
    pub fn recipient_local_name(&self) -> Option<String> {
        self.key_files
            .iter()
            .find(|k| k.pubkey == self.tx_to)
            .map(|k| k.display.clone())
    }

    fn key_transfer(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Tab | KeyCode::Up | KeyCode::Down => self.tx_focus = 1 - self.tx_focus,
            // On the recipient field, ←→ toggles through local keypairs.
            KeyCode::Left | KeyCode::Right if self.tx_focus == 0 => {
                if !self.key_files.is_empty() {
                    let delta = if key.code == KeyCode::Left {
                        self.key_files.len() - 1
                    } else {
                        1
                    };
                    self.tx_key_idx = (self.tx_key_idx + delta) % self.key_files.len();
                    self.tx_to = self.key_files[self.tx_key_idx].pubkey.clone();
                }
            }
            KeyCode::Enter => {
                if !self.tx_to.is_empty() && self.tx_amount.parse::<f64>().is_ok_and(|a| a > 0.0) {
                    self.mode = Mode::TransferConfirm;
                } else {
                    self.status = "enter a recipient and a positive amount".to_string();
                }
            }
            KeyCode::Backspace => {
                if self.tx_focus == 0 {
                    self.tx_to.pop();
                } else {
                    self.tx_amount.pop();
                }
            }
            KeyCode::Char(c) => {
                if self.tx_focus == 0 {
                    self.tx_to.push(c);
                } else {
                    self.tx_amount.push(c);
                }
            }
            _ => {}
        }
    }

    fn key_transfer_confirm(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                self.need_transfer = true;
                self.status = "transfer queued…".to_string();
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => {
                self.status = "transfer cancelled".to_string();
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    // --- Themes ---------------------------------------------------------

    fn open_themes(&mut self) {
        // Remember the active theme so Esc can cleanly revert the preview.
        self.theme_prev = theme::theme();
        self.theme_sel = theme::names()
            .iter()
            .position(|n| *n == self.theme_name)
            .unwrap_or(0);
        self.preview_theme();
        self.mode = Mode::Themes;
    }

    /// Apply the highlighted theme to the live UI (preview, not yet persisted).
    fn preview_theme(&self) {
        if let Some(name) = theme::names().get(self.theme_sel) {
            theme::set(theme::resolve(name));
        }
    }

    fn key_themes(&mut self, key: KeyEvent) {
        let count = theme::names().len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                theme::set(self.theme_prev.clone());
                self.status = "theme unchanged".to_string();
                self.mode = Mode::Normal;
            }
            KeyCode::Down | KeyCode::Char('j') if count > 0 => {
                self.theme_sel = wrap_index(self.theme_sel, 1, count);
                self.preview_theme();
            }
            KeyCode::Up | KeyCode::Char('k') if count > 0 => {
                self.theme_sel = wrap_index(self.theme_sel, -1, count);
                self.preview_theme();
            }
            KeyCode::Enter => {
                if let Some(name) = theme::names().get(self.theme_sel).cloned() {
                    self.theme_name = name.clone();
                    self.status = match theme::persist_name(&name) {
                        Ok(()) => format!("theme: {name}"),
                        Err(e) => format!("theme set (save failed: {e})"),
                    };
                }
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }
}

/// Step an index by `delta` within `[0, len)`, wrapping around both ends.
/// Returns 0 for an empty list. Used everywhere a selection cycles.
pub fn wrap_index(cur: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let n = len as i32;
    (((cur as i32 + delta) % n + n) % n) as usize
}

/// True if every char of `needle` appears in `hay` in order.
pub fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut chars = hay.chars();
    needle.chars().all(|c| chars.any(|h| h == c))
}

/// Percent-encode a string for use as a URL query-parameter value.
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Open a URL in the system default browser.
fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }
    Ok(())
}

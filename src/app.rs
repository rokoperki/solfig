use crate::config::{self, KeyFile, SolanaConfig, COMMITMENTS, MONIKERS};
use crate::endpoints::{self, Endpoint};
use crate::faucets::{self, Faucet};
use crate::profiles::{self, Profile};
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
                self.sel = (self.sel + 1) % FIELDS.len();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.sel = (self.sel + FIELDS.len() - 1) % FIELDS.len();
            }
            KeyCode::Left | KeyCode::Char('h') => self.adjust(-1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust(1),
            KeyCode::Enter => self.activate_field(),
            KeyCode::Char('a') => self.request_airdrop(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.cycle_airdrop(1),
            KeyCode::Char('-') | KeyCode::Char('_') => self.cycle_airdrop(-1),
            KeyCode::Char('y') => self.copy_field(),
            KeyCode::Char('e') => self.open_endpoints(),
            KeyCode::Char('f') => self.open_faucets(),
            KeyCode::Char('s') => self.save(),
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('p') => self.open_profiles(),
            _ => {}
        }
    }

    /// Cycle the airdrop amount through the preset steps.
    fn cycle_airdrop(&mut self, delta: i32) {
        let cur = AIRDROP_STEPS
            .iter()
            .position(|s| (*s - self.airdrop_sol).abs() < f64::EPSILON)
            .unwrap_or(1);
        let n = AIRDROP_STEPS.len() as i32;
        let next = ((cur as i32 + delta) % n + n) % n;
        self.airdrop_sol = AIRDROP_STEPS[next as usize];
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

    /// Cycle presets for the focused field (left/right arrows).
    fn adjust(&mut self, delta: i32) {
        match self.field() {
            Field::Cluster => {
                let list = self.clusters();
                let cur = list
                    .iter()
                    .position(|(_, u)| *u == self.cfg.json_rpc_url)
                    .unwrap_or(0);
                let n = list.len() as i32;
                let next = ((cur as i32 + delta) % n + n) % n;
                self.cfg.json_rpc_url = list[next as usize].1.clone();
                self.dirty = true;
                self.need_health_check = true;
                self.need_balance_check = true;
            }
            Field::Commitment => {
                let cur = COMMITMENTS
                    .iter()
                    .position(|c| *c == self.cfg.commitment)
                    .unwrap_or(1);
                let n = COMMITMENTS.len() as i32;
                let next = ((cur as i32 + delta) % n + n) % n;
                self.cfg.commitment = COMMITMENTS[next as usize].to_string();
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
            KeyCode::Down => {
                if !matches.is_empty() {
                    self.key_sel = (self.key_sel + 1) % matches.len();
                }
            }
            KeyCode::Up => {
                if !matches.is_empty() {
                    self.key_sel = (self.key_sel + matches.len() - 1) % matches.len();
                }
            }
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
            KeyCode::Char(c) => {
                self.key_filter.push(c);
                self.key_sel = 0;
            }
            _ => {}
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
                if !self.profiles.is_empty() {
                    self.prof_sel = (self.prof_sel + 1) % self.profiles.len();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.profiles.is_empty() {
                    self.prof_sel =
                        (self.prof_sel + self.profiles.len() - 1) % self.profiles.len();
                }
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
                if !self.endpoints.is_empty() {
                    self.ep_sel = (self.ep_sel + 1) % self.endpoints.len();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.endpoints.is_empty() {
                    self.ep_sel =
                        (self.ep_sel + self.endpoints.len() - 1) % self.endpoints.len();
                }
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
                if !self.faucets.is_empty() {
                    self.faucet_sel = (self.faucet_sel + 1) % self.faucets.len();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.faucets.is_empty() {
                    self.faucet_sel =
                        (self.faucet_sel + self.faucets.len() - 1) % self.faucets.len();
                }
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
            .and_then(|pk| arboard::Clipboard::new().and_then(|mut c| c.set_text(pk)).ok())
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
}

/// True if every char of `needle` appears in `hay` in order.
fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut chars = hay.chars();
    needle.chars().all(|c| chars.any(|h| h == c))
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

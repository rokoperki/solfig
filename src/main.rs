use crossterm::event::{self, Event, KeyEventKind};
use solfig::app::App;
use solfig::rpc::{Balance, ClusterStats, Health, Price, Request, Response, Rpc, Telemetry};
use solfig::{config, ui};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How often to poll the bare current slot (drives the live heartbeat).
const SLOT_INTERVAL: Duration = Duration::from_millis(800);
/// How often to refresh live cluster telemetry (slot/epoch/tps).
const TELEMETRY_INTERVAL: Duration = Duration::from_secs(2);
/// How often to refresh slower stats (validators/supply).
const STATS_INTERVAL: Duration = Duration::from_secs(20);
/// How often to refresh the (cluster-independent) SOL price.
const PRICE_INTERVAL: Duration = Duration::from_secs(60);

fn main() -> anyhow::Result<()> {
    let path = config::default_config_path();
    let mut app = App::new(path);
    let rpc = Rpc::new();
    let mut health = Health::Unknown;
    let mut balance = Balance::Unknown;
    let mut telemetry: Option<Telemetry> = None;
    let mut stats: Option<ClusterStats> = None;
    let mut price: Option<Price> = None;

    let mut term = ratatui::init();
    let result = run(
        &mut term,
        &mut app,
        &rpc,
        &mut health,
        &mut balance,
        &mut telemetry,
        &mut stats,
        &mut price,
    );
    ratatui::restore();
    result
}

#[allow(clippy::too_many_arguments)]
fn run(
    term: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rpc: &Rpc,
    health: &mut Health,
    balance: &mut Balance,
    telemetry: &mut Option<Telemetry>,
    stats: &mut Option<ClusterStats>,
    price: &mut Option<Price>,
) -> anyhow::Result<()> {
    // Per-cluster caches so switching back shows last values instantly.
    let mut tele_cache: HashMap<String, Telemetry> = HashMap::new();
    let mut stats_cache: HashMap<String, ClusterStats> = HashMap::new();
    let mut health_cache: HashMap<String, Health> = HashMap::new();

    let mut last_telemetry = Instant::now() - TELEMETRY_INTERVAL;
    let mut last_stats = Instant::now() - STATS_INTERVAL;
    let mut last_price = Instant::now() - PRICE_INTERVAL;
    let mut last_slot_poll = Instant::now() - SLOT_INTERVAL;
    // Live-heartbeat state: the highest slot seen on the active cluster and when
    // it last advanced. `None` beat = no pulse yet (or chain/RPC stalled).
    let mut last_slot: u64 = 0;
    let mut last_beat: Option<Instant> = None;
    loop {
        while let Some(resp) = rpc.poll() {
            match resp {
                Response::Health(url, h) => {
                    let current = url == app.cfg.json_rpc_url;
                    if let Health::Checking = h {
                        // Only show the spinner if we have nothing cached yet.
                        if current && !health_cache.contains_key(&url) {
                            *health = Health::Checking;
                        }
                    } else {
                        health_cache.insert(url.clone(), h.clone());
                        if current {
                            *health = h;
                        }
                    }
                }
                Response::Slot(url, s) => {
                    if let (Some(s), true) = (s, url == app.cfg.json_rpc_url) {
                        if s > last_slot {
                            last_slot = s;
                            last_beat = Some(Instant::now());
                        }
                        // Keep the displayed slot ticking between full refreshes.
                        if let Some(t) = telemetry.as_mut() {
                            t.slot = t.slot.max(s);
                        }
                    }
                }
                Response::Telemetry(url, t) => {
                    if let Some(t) = &t {
                        tele_cache.insert(url.clone(), t.clone());
                    }
                    if url == app.cfg.json_rpc_url && t.is_some() {
                        *telemetry = t;
                    }
                }
                Response::Stats(url, s) => {
                    if let Some(s) = &s {
                        stats_cache.insert(url.clone(), s.clone());
                    }
                    if url == app.cfg.json_rpc_url && s.is_some() {
                        *stats = s;
                    }
                }
                Response::Price(p) => {
                    // Keep the last good price if a refresh fails (rate-limited).
                    if p.is_some() {
                        *price = p;
                    }
                }
                Response::Balance(b) => {
                    app.balance_lamports = match &b {
                        Balance::Lamports(l) => Some(*l),
                        _ => None,
                    };
                    *balance = b;
                }
                Response::Notice(msg) => app.status = msg,
            }
        }
        if app.need_health_check {
            app.need_health_check = false;
            // Reset the heartbeat: the new cluster's slot count is unrelated.
            last_slot = 0;
            last_beat = None;
            let url = app.cfg.json_rpc_url.clone();
            if url.is_empty() {
                *health = Health::Unknown;
                *telemetry = None;
                *stats = None;
            } else {
                // Show cached values immediately, then refresh in the background.
                *telemetry = tele_cache.get(&url).cloned();
                *stats = stats_cache.get(&url).cloned();
                *health = health_cache.get(&url).cloned().unwrap_or(Health::Checking);
                rpc.request(Request::Slot(url.clone()));
                rpc.request(Request::Version(url.clone()));
                rpc.request(Request::Telemetry(url.clone()));
                rpc.request(Request::Stats(url));
                last_slot_poll = Instant::now();
                last_telemetry = Instant::now();
                last_stats = Instant::now();
            }
        }
        if app.need_balance_check {
            app.need_balance_check = false;
            // Drop the cached amount until the fresh balance arrives.
            app.balance_lamports = None;
            match config::pubkey_from_keypair(&app.cfg.keypair_path) {
                Some(pubkey) if !app.cfg.json_rpc_url.is_empty() => {
                    *balance = Balance::Loading;
                    rpc.request(Request::Balance {
                        url: app.cfg.json_rpc_url.clone(),
                        pubkey,
                    });
                }
                _ => *balance = Balance::Unknown,
            }
        }
        if app.need_airdrop {
            app.need_airdrop = false;
            if let Some(pubkey) = config::pubkey_from_keypair(&app.cfg.keypair_path) {
                rpc.request(Request::Airdrop {
                    url: app.cfg.json_rpc_url.clone(),
                    pubkey,
                    lamports: (app.airdrop_sol * 1_000_000_000.0) as u64,
                });
            }
        }
        if app.need_transfer {
            app.need_transfer = false;
            if let Some(pubkey) = config::pubkey_from_keypair(&app.cfg.keypair_path) {
                rpc.request(Request::Transfer {
                    url: app.cfg.json_rpc_url.clone(),
                    keypair: app.cfg.keypair_path.clone(),
                    pubkey,
                    to: app.tx_to.clone(),
                    amount: app.tx_amount.clone(),
                });
            }
        }

        // Fast current-slot poll — the heartbeat ticker between full refreshes.
        if !app.cfg.json_rpc_url.is_empty() && last_slot_poll.elapsed() >= SLOT_INTERVAL {
            rpc.request(Request::Slot(app.cfg.json_rpc_url.clone()));
            last_slot_poll = Instant::now();
        }
        // Periodic live telemetry refresh (the slot ticker).
        if !app.cfg.json_rpc_url.is_empty() && last_telemetry.elapsed() >= TELEMETRY_INTERVAL {
            rpc.request(Request::Telemetry(app.cfg.json_rpc_url.clone()));
            last_telemetry = Instant::now();
        }
        // Slower stats refresh (validators/supply).
        if !app.cfg.json_rpc_url.is_empty() && last_stats.elapsed() >= STATS_INTERVAL {
            rpc.request(Request::Stats(app.cfg.json_rpc_url.clone()));
            last_stats = Instant::now();
        }
        // SOL price — cluster-independent, slow cadence, survives cluster switches.
        if last_price.elapsed() >= PRICE_INTERVAL {
            rpc.request(Request::Price);
            last_price = Instant::now();
        }

        let beat_age = last_beat.map(|b| b.elapsed());
        term.draw(|f| ui::render(f, app, health, balance, telemetry, stats, price, beat_age))?;

        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

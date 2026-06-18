mod app;
mod config;
mod endpoints;
mod faucets;
mod profiles;
mod rpc;
mod ui;

use app::App;
use crossterm::event::{self, Event, KeyEventKind};
use rpc::{Balance, Health, Request, Response, Rpc};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let path = config::default_config_path();
    let mut app = App::new(path);
    let rpc = Rpc::new();
    let mut health = Health::Unknown;
    let mut balance = Balance::Unknown;

    let mut term = ratatui::init();
    let result = run(&mut term, &mut app, &rpc, &mut health, &mut balance);
    ratatui::restore();
    result
}

fn run(
    term: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rpc: &Rpc,
    health: &mut Health,
    balance: &mut Balance,
) -> anyhow::Result<()> {
    loop {
        while let Some(resp) = rpc.poll() {
            match resp {
                Response::Health(h) => *health = h,
                Response::Balance(b) => *balance = b,
                Response::Notice(msg) => app.status = msg,
            }
        }
        if app.need_health_check {
            app.need_health_check = false;
            if app.cfg.json_rpc_url.is_empty() {
                *health = Health::Unknown;
            } else {
                *health = Health::Checking;
                rpc.request(Request::Version(app.cfg.json_rpc_url.clone()));
            }
        }
        if app.need_balance_check {
            app.need_balance_check = false;
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

        term.draw(|f| ui::render(f, app, health, balance))?;

        if event::poll(Duration::from_millis(200))? {
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

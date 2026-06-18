use serde_json::{json, Value};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Result of an RPC `getVersion` probe.
#[derive(Clone)]
pub enum Health {
    Unknown,
    Checking,
    Ok { version: String, ms: u128 },
    Err(String),
}

/// Result of an RPC `getBalance` probe.
#[derive(Clone)]
pub enum Balance {
    Unknown,
    Loading,
    Lamports(u64),
    Err(String),
}

/// A unit of work for the background worker.
pub enum Request {
    Version(String),
    Balance { url: String, pubkey: String },
    Airdrop {
        url: String,
        pubkey: String,
        lamports: u64,
    },
}

/// An update produced by the background worker.
pub enum Response {
    Health(Health),
    Balance(Balance),
    Notice(String),
}

/// Background worker that runs RPC probes off the UI thread.
pub struct Rpc {
    req_tx: Sender<Request>,
    res_rx: Receiver<Response>,
}

impl Rpc {
    pub fn new() -> Self {
        let (req_tx, req_rx) = channel::<Request>();
        let (res_tx, res_rx) = channel::<Response>();
        thread::spawn(move || {
            while let Ok(first) = req_rx.recv() {
                // Collapse a burst of version/balance requests to the latest of
                // each kind; airdrops are side-effecting so every one is kept.
                let mut latest_version: Option<String> = None;
                let mut latest_balance: Option<(String, String)> = None;
                let mut airdrops: Vec<(String, String, u64)> = Vec::new();
                let mut cur = Some(first);
                while let Some(req) = cur.take() {
                    match req {
                        Request::Version(url) => latest_version = Some(url),
                        Request::Balance { url, pubkey } => {
                            latest_balance = Some((url, pubkey))
                        }
                        Request::Airdrop {
                            url,
                            pubkey,
                            lamports,
                        } => airdrops.push((url, pubkey, lamports)),
                    }
                    cur = req_rx.try_recv().ok();
                }
                for (url, pubkey, lamports) in airdrops {
                    run_airdrop(&res_tx, &url, &pubkey, lamports);
                }
                if let Some(url) = latest_version {
                    let _ = res_tx.send(Response::Health(Health::Checking));
                    let _ = res_tx.send(Response::Health(probe_version(&url)));
                }
                if let Some((url, pubkey)) = latest_balance {
                    let _ = res_tx.send(Response::Balance(Balance::Loading));
                    let _ = res_tx.send(Response::Balance(probe_balance(&url, &pubkey)));
                }
            }
        });
        Self { req_tx, res_rx }
    }

    pub fn request(&self, req: Request) {
        let _ = self.req_tx.send(req);
    }

    pub fn poll(&self) -> Option<Response> {
        self.res_rx.try_recv().ok()
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()
}

fn rpc_call(url: &str, method: &str, params: Value) -> Result<Value, String> {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    match agent().post(url).send_json(body) {
        Ok(resp) => resp
            .into_json::<Value>()
            .map_err(|e| format!("bad response: {e}")),
        Err(e) => Err(short_err(&e.to_string())),
    }
}

fn probe_version(url: &str) -> Health {
    let start = Instant::now();
    match rpc_call(url, "getVersion", json!([])) {
        Ok(v) => {
            let version = v["result"]["solana-core"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            Health::Ok {
                version,
                ms: start.elapsed().as_millis(),
            }
        }
        Err(e) => Health::Err(e),
    }
}

fn probe_balance(url: &str, pubkey: &str) -> Balance {
    match rpc_call(url, "getBalance", json!([pubkey])) {
        Ok(v) => {
            if let Some(lamports) = v["result"]["value"].as_u64() {
                Balance::Lamports(lamports)
            } else if v["error"].is_object() {
                Balance::Err(
                    v["error"]["message"]
                        .as_str()
                        .unwrap_or("rpc error")
                        .to_string(),
                )
            } else {
                Balance::Err("no balance in response".to_string())
            }
        }
        Err(e) => Balance::Err(e),
    }
}

fn run_airdrop(res_tx: &Sender<Response>, url: &str, pubkey: &str, lamports: u64) {
    let sol = lamports as f64 / 1_000_000_000.0;
    let _ = res_tx.send(Response::Notice(format!("requesting airdrop ({sol} SOL)…")));
    match rpc_call(url, "requestAirdrop", json!([pubkey, lamports])) {
        Ok(v) => {
            if let Some(sig) = v["result"].as_str() {
                let short: String = sig.chars().take(8).collect();
                let _ = res_tx.send(Response::Notice(format!(
                    "airdrop sent ({short}…), confirming"
                )));
                thread::sleep(Duration::from_secs(3));
                let _ = res_tx.send(Response::Balance(probe_balance(url, pubkey)));
                let _ = res_tx.send(Response::Notice("airdrop balance refreshed".to_string()));
            } else {
                let msg = v["error"]["message"].as_str().unwrap_or("airdrop rejected");
                let _ = res_tx.send(Response::Notice(airdrop_error(msg)));
            }
        }
        Err(e) => {
            let _ = res_tx.send(Response::Notice(airdrop_error(&e)));
        }
    }
}

/// Turn a raw airdrop error into a friendlier, actionable message.
fn airdrop_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("rate")
        || lower.contains("limit")
        || lower.contains("429")
        || lower.contains("too many")
    {
        "airdrop rate-limited — lower the amount, wait, switch faucet (e), or use faucet.solana.com"
            .to_string()
    } else {
        format!("airdrop failed: {raw}")
    }
}

fn short_err(s: &str) -> String {
    s.lines().next().unwrap_or(s).chars().take(60).collect()
}

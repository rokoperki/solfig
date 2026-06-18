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
    Transfer {
        url: String,
        keypair: String,
        pubkey: String,
        to: String,
        amount: String,
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
                let mut transfers: Vec<Request> = Vec::new();
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
                        t @ Request::Transfer { .. } => transfers.push(t),
                    }
                    cur = req_rx.try_recv().ok();
                }
                for (url, pubkey, lamports) in airdrops {
                    run_airdrop(&res_tx, &url, &pubkey, lamports);
                }
                for t in transfers {
                    if let Request::Transfer {
                        url,
                        keypair,
                        pubkey,
                        to,
                        amount,
                    } = t
                    {
                        run_transfer(&res_tx, &url, &keypair, &pubkey, &to, &amount);
                    }
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
    // Use `confirmed` so balances reflect just-confirmed txs (default is finalized).
    match rpc_call(url, "getBalance", json!([pubkey, {"commitment": "confirmed"}])) {
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
                let outcome = confirm_signature(url, sig);
                let _ = res_tx.send(Response::Balance(probe_balance(url, pubkey)));
                let _ = res_tx.send(Response::Notice(match outcome {
                    Confirm::Confirmed => "airdrop confirmed".to_string(),
                    Confirm::Failed(e) => format!("airdrop tx failed: {e}"),
                    Confirm::Timeout => "airdrop still pending — balance may lag".to_string(),
                }));
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

/// Send SOL by shelling out to the `solana transfer` CLI (it signs the tx).
fn run_transfer(
    res_tx: &Sender<Response>,
    url: &str,
    keypair: &str,
    pubkey: &str,
    to: &str,
    amount: &str,
) {
    let to_short: String = to.chars().take(8).collect();
    let _ = res_tx.send(Response::Notice(format!(
        "sending {amount} SOL → {to_short}…"
    )));
    let output = std::process::Command::new("solana")
        .args([
            "transfer",
            to,
            amount,
            "--keypair",
            keypair,
            "--url",
            url,
            "--allow-unfunded-recipient",
            "--commitment",
            "confirmed",
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let sig = stdout
                .lines()
                .find_map(|l| l.trim().strip_prefix("Signature: "))
                .unwrap_or("")
                .trim();
            let sig_short: String = sig.chars().take(8).collect();
            let _ = res_tx.send(Response::Balance(probe_balance(url, pubkey)));
            let _ = res_tx.send(Response::Notice(format!("transfer confirmed ({sig_short}…)")));
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let msg = err.lines().next().unwrap_or("transfer failed");
            let _ = res_tx.send(Response::Notice(format!("transfer failed: {msg}")));
        }
        Err(e) => {
            let _ = res_tx.send(Response::Notice(format!(
                "transfer failed (solana CLI?): {e}"
            )));
        }
    }
}

enum Confirm {
    Confirmed,
    Failed(String),
    Timeout,
}

/// Poll `getSignatureStatuses` until the tx confirms, fails, or times out (~15s).
fn confirm_signature(url: &str, sig: &str) -> Confirm {
    for _ in 0..15 {
        thread::sleep(Duration::from_secs(1));
        let Ok(v) = rpc_call(url, "getSignatureStatuses", json!([[sig]])) else {
            continue;
        };
        let status = &v["result"]["value"][0];
        if !status.is_object() {
            continue; // not yet visible to the RPC
        }
        if status["err"].is_object() {
            return Confirm::Failed(status["err"].to_string());
        }
        match status["confirmationStatus"].as_str() {
            Some("confirmed") | Some("finalized") => return Confirm::Confirmed,
            _ => {}
        }
    }
    Confirm::Timeout
}

/// Turn a raw airdrop error into a friendlier, actionable message.
fn airdrop_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("rate")
        || lower.contains("limit")
        || lower.contains("429")
        || lower.contains("too many")
    {
        "airdrop rate-limited — lower amount (-), wait a bit, or open a web faucet (f)".to_string()
    } else {
        format!("airdrop failed: {raw}")
    }
}

fn short_err(s: &str) -> String {
    s.lines().next().unwrap_or(s).chars().take(60).collect()
}

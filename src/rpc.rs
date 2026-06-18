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

/// Fast-moving cluster telemetry (refreshed often).
#[derive(Clone, Default)]
pub struct Telemetry {
    pub slot: u64,
    pub epoch: u64,
    pub slot_index: u64,
    pub slots_in_epoch: u64,
    pub block_height: u64,
    pub tps: u64,
}

/// SOL/USD spot price with its 24-hour change (percent).
#[derive(Clone, Copy)]
pub struct Price {
    pub usd: f64,
    pub change_24h: f64,
}

/// Slower-moving, cluster-specific stats (refreshed occasionally).
#[derive(Clone, Default)]
pub struct ClusterStats {
    pub validators: u64,
    pub txn_count: u64,
    pub supply_sol: u64,
}

/// A unit of work for the background worker.
pub enum Request {
    Version(String),
    Telemetry(String),
    Stats(String),
    Price,
    Balance {
        url: String,
        pubkey: String,
    },
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
    // The leading String is the RPC URL the result is for (used for caching).
    Health(String, Health),
    Telemetry(String, Option<Telemetry>),
    Stats(String, Option<ClusterStats>),
    Price(Option<Price>),
    Balance(Balance),
    Notice(String),
}

/// Background worker that runs RPC probes off the UI thread.
///
/// Balance has its own dedicated thread so it is fetched with top priority and
/// never waits behind slow telemetry/stats/price probes on the general worker.
pub struct Rpc {
    gen_tx: Sender<Request>,
    bal_tx: Sender<Request>,
    res_rx: Receiver<Response>,
}

impl Rpc {
    // `new` spawns background worker threads, so a trivial `Default` would
    // misrepresent the cost; construct it explicitly.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let (gen_tx, gen_rx) = channel::<Request>();
        let (bal_tx, bal_rx) = channel::<Request>();
        let (res_tx, res_rx) = channel::<Response>();

        // Dedicated balance worker — always responsive.
        let bal_res = res_tx.clone();
        thread::spawn(move || {
            while let Ok(first) = bal_rx.recv() {
                // Keep only the most recent balance request.
                let mut latest = first;
                while let Ok(next) = bal_rx.try_recv() {
                    latest = next;
                }
                if let Request::Balance { url, pubkey } = latest {
                    let _ = bal_res.send(Response::Balance(Balance::Loading));
                    let _ = bal_res.send(Response::Balance(probe_balance(&url, &pubkey)));
                }
            }
        });

        // General worker — everything else.
        thread::spawn(move || {
            while let Ok(first) = gen_rx.recv() {
                let mut latest_version: Option<String> = None;
                let mut latest_telemetry: Option<String> = None;
                let mut latest_stats: Option<String> = None;
                let mut want_price = false;
                let mut airdrops: Vec<(String, String, u64)> = Vec::new();
                let mut transfers: Vec<Request> = Vec::new();
                let mut cur = Some(first);
                while let Some(req) = cur.take() {
                    match req {
                        Request::Version(url) => latest_version = Some(url),
                        Request::Telemetry(url) => latest_telemetry = Some(url),
                        Request::Stats(url) => latest_stats = Some(url),
                        Request::Price => want_price = true,
                        Request::Airdrop {
                            url,
                            pubkey,
                            lamports,
                        } => airdrops.push((url, pubkey, lamports)),
                        t @ Request::Transfer { .. } => transfers.push(t),
                        Request::Balance { .. } => {} // handled by the balance thread
                    }
                    cur = gen_rx.try_recv().ok();
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
                    let _ = res_tx.send(Response::Health(url.clone(), Health::Checking));
                    let h = probe_version(&url);
                    let _ = res_tx.send(Response::Health(url, h));
                }
                if let Some(url) = latest_telemetry {
                    let t = probe_telemetry(&url);
                    let _ = res_tx.send(Response::Telemetry(url, t));
                }
                if let Some(url) = latest_stats {
                    let s = probe_stats(&url);
                    let _ = res_tx.send(Response::Stats(url, s));
                }
                if want_price {
                    let _ = res_tx.send(Response::Price(fetch_sol_price()));
                }
            }
        });

        Self {
            gen_tx,
            bal_tx,
            res_rx,
        }
    }

    pub fn request(&self, req: Request) {
        let tx = if matches!(req, Request::Balance { .. }) {
            &self.bal_tx
        } else {
            &self.gen_tx
        };
        let _ = tx.send(req);
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

fn probe_telemetry(url: &str) -> Option<Telemetry> {
    let epoch = rpc_call(url, "getEpochInfo", json!([])).ok()?;
    let e = &epoch["result"];
    let slot = e["absoluteSlot"].as_u64()?;
    Some(Telemetry {
        slot,
        epoch: e["epoch"].as_u64().unwrap_or(0),
        slot_index: e["slotIndex"].as_u64().unwrap_or(0),
        slots_in_epoch: e["slotsInEpoch"].as_u64().unwrap_or(0),
        block_height: e["blockHeight"].as_u64().unwrap_or(0),
        tps: probe_tps(url),
    })
}

fn probe_stats(url: &str) -> Option<ClusterStats> {
    let validators = rpc_call(url, "getClusterNodes", json!([]))
        .ok()
        .and_then(|v| v["result"].as_array().map(|a| a.len() as u64))
        .unwrap_or(0);
    let txn_count = rpc_call(url, "getTransactionCount", json!([]))
        .ok()
        .and_then(|v| v["result"].as_u64())
        .unwrap_or(0);
    let supply_sol = rpc_call(url, "getSupply", json!([]))
        .ok()
        .and_then(|v| v["result"]["value"]["circulating"].as_u64())
        .map(|l| l / 1_000_000_000)
        .unwrap_or(0);
    Some(ClusterStats {
        validators,
        txn_count,
        supply_sol,
    })
}

/// SOL/USD spot price and 24h change from CoinGecko (best-effort; external API).
fn fetch_sol_price() -> Option<Price> {
    let v: Value = agent()
        .get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd&include_24hr_change=true")
        .call()
        .ok()?
        .into_json()
        .ok()?;
    let usd = v["solana"]["usd"].as_f64()?;
    let change_24h = v["solana"]["usd_24h_change"].as_f64().unwrap_or(0.0);
    Some(Price { usd, change_24h })
}

fn probe_tps(url: &str) -> u64 {
    match rpc_call(url, "getRecentPerformanceSamples", json!([1])) {
        Ok(v) => {
            let s = &v["result"][0];
            let n = s["numTransactions"].as_u64().unwrap_or(0);
            let p = s["samplePeriodSecs"].as_u64().unwrap_or(0);
            n.checked_div(p).unwrap_or(0)
        }
        Err(_) => 0,
    }
}

fn probe_balance(url: &str, pubkey: &str) -> Balance {
    // Use `confirmed` so balances reflect just-confirmed txs (default is finalized).
    match rpc_call(
        url,
        "getBalance",
        json!([pubkey, {"commitment": "confirmed"}]),
    ) {
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
            let _ = res_tx.send(Response::Notice(format!(
                "transfer confirmed ({sig_short}…)"
            )));
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

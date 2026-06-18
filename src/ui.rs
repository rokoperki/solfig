use crate::app::{App, Field, Mode, FIELDS};
use crate::config::{self, COMMITMENTS, MONIKERS};
use crate::rpc::{Balance, ClusterStats, Health, Price, Telemetry};
use crate::theme::{self, theme};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use std::path::Path;

/// ANSI Shadow "SOLFIG" wordmark.
fn wordmark() -> [&'static str; 6] {
    [
        "███████╗ ██████╗ ██╗     ███████╗██╗ ██████╗",
        "██╔════╝██╔═══██╗██║     ██╔════╝██║██╔════╝",
        "███████╗██║   ██║██║     █████╗  ██║██║  ███╗",
        "╚════██║██║   ██║██║     ██╔══╝  ██║██║   ██║",
        "███████║╚██████╔╝███████╗██║     ██║╚██████╔╝",
        "╚══════╝ ╚═════╝ ╚══════╝╚═╝     ╚═╝ ╚═════╝",
    ]
}

/// Network-specific accent: a glance tells you which cluster you're on.
fn cluster_color(url: &str) -> Color {
    let t = theme();
    match config::moniker_for_url(url) {
        Some("mainnet-beta") => t.mainnet,
        Some("devnet") => t.devnet,
        Some("testnet") => t.testnet,
        Some("localhost") => t.localhost,
        _ => t.custom,
    }
}

/// Braille spinner frame derived from wall-clock time.
fn spinner() -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    FRAMES[(ms / 100) as usize % FRAMES.len()]
}

/// Shared modal chrome: clear, rounded accent border, titled. Returns inner area.
fn modal(f: &mut Frame, title: &str, width: u16, height: u16) -> Rect {
    let area = centered(width, height, f.area());
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme().accent))
        .title(format!(" {title} "))
        .title_style(Style::new().fg(theme().accent).add_modifier(Modifier::BOLD));
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

pub fn render(
    f: &mut Frame,
    app: &App,
    health: &Health,
    balance: &Balance,
    telemetry: &Option<Telemetry>,
    stats: &Option<ClusterStats>,
    price: &Option<Price>,
) {
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .split(f.area());

    // Split off a telemetry sidebar when the terminal is wide enough.
    if chunks[0].width >= 90 {
        let cols =
            Layout::horizontal([Constraint::Min(50), Constraint::Length(26)]).split(chunks[0]);
        render_body(f, app, balance, cols[0]);
        render_sidebar(f, app, health, telemetry, stats, price, cols[1]);
    } else {
        render_body(f, app, balance, chunks[0]);
    }
    render_status(f, app, health, chunks[1]);
    render_help(f, app, chunks[2]);

    if app.mode == Mode::KeyPicker {
        render_key_picker(f, app);
    }
    if app.mode == Mode::Profiles || app.mode == Mode::NewProfile {
        render_profiles(f, app);
    }
    if app.mode == Mode::Endpoints || app.mode == Mode::NewEndpoint {
        render_endpoints(f, app);
    }
    if app.mode == Mode::Faucets || app.mode == Mode::NewFaucet {
        render_faucets(f, app);
    }
    if app.mode == Mode::Transfer || app.mode == Mode::TransferConfirm {
        render_transfer(f, app);
    }
    if app.mode == Mode::Themes {
        render_themes(f, app);
    }
    if app.mode == Mode::Help {
        render_help_panel(f);
    }
}

fn render_body(f: &mut Frame, app: &App, balance: &Balance, area: Rect) {
    let net = config::moniker_for_url(&app.cfg.json_rpc_url).unwrap_or("custom");
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme().dim))
        .title(Span::styled(
            format!(" {} ", app.path.display()),
            Style::new().fg(theme().dim),
        ))
        .title(
            Line::from(Span::styled(
                format!(" {net} "),
                Style::new()
                    .fg(theme().on_accent)
                    .bg(cluster_color(&app.cfg.json_rpc_url))
                    .add_modifier(Modifier::BOLD),
            ))
            .right_aligned(),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let inner_w = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();

    // Compact wordmark (hidden on very short terminals to keep fields visible).
    if inner.height >= 16 {
        lines.push(Line::from(""));
        for row in wordmark() {
            lines.push(Line::from(Span::styled(
                format!("  {row}"),
                Style::new().fg(theme().accent).add_modifier(Modifier::BOLD),
            )));
        }
    }
    lines.push(Line::from(""));

    if net == "mainnet-beta" {
        lines.push(hazard_banner(inner_w));
    }
    lines.push(Line::from(""));
    for (i, field) in FIELDS.iter().enumerate() {
        let focused = i == app.sel;
        let editing = focused && app.mode == Mode::EditField;
        let mut flines = field_lines(app, *field, focused, editing, balance);
        if focused {
            for line in flines.iter_mut() {
                let used = line.width();
                if used < inner_w {
                    line.spans.push(Span::raw(" ".repeat(inner_w - used)));
                }
                line.style = Style::new().bg(theme().highlight);
            }
        }
        lines.extend(flines);
        lines.push(Line::from(""));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn render_sidebar(
    f: &mut Frame,
    app: &App,
    health: &Health,
    telemetry: &Option<Telemetry>,
    stats: &Option<ClusterStats>,
    price: &Option<Price>,
    area: Rect,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme().dim))
        .title(Span::styled(
            " cluster ",
            Style::new().fg(theme().accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let w = inner.width as usize;

    let kv = |label: &str, value: Span<'static>| {
        Line::from(vec![
            Span::styled(format!(" {label:<8}"), Style::new().fg(theme().dim)),
            value,
        ])
    };

    let net = config::moniker_for_url(&app.cfg.json_rpc_url).unwrap_or("custom");
    let (ping, version) = match health {
        Health::Ok { version, ms } => (format!("{ms}ms"), version.clone()),
        Health::Checking => (spinner().to_string(), "…".to_string()),
        Health::Err(_) => ("down".to_string(), "—".to_string()),
        Health::Unknown => ("—".to_string(), "—".to_string()),
    };

    let mut lines = vec![
        Line::from(""),
        kv(
            "network",
            Span::styled(
                net.to_string(),
                Style::new()
                    .fg(cluster_color(&app.cfg.json_rpc_url))
                    .add_modifier(Modifier::BOLD),
            ),
        ),
        Line::from(""),
    ];

    match telemetry {
        Some(t) => {
            lines.push(kv(
                "slot",
                Span::styled(group_thousands(t.slot), Style::new().fg(theme().text)),
            ));
            lines.push(kv(
                "block",
                Span::styled(
                    group_thousands(t.block_height),
                    Style::new().fg(theme().text),
                ),
            ));
            lines.push(kv(
                "epoch",
                Span::styled(t.epoch.to_string(), Style::new().fg(theme().text)),
            ));
            let ratio = if t.slots_in_epoch > 0 {
                t.slot_index as f64 / t.slots_in_epoch as f64
            } else {
                0.0
            };
            lines.push(Line::from(Span::styled(
                format!(" {}", progress_bar(ratio, w.saturating_sub(2))),
                Style::new().fg(theme().accent),
            )));
            let eta = format_eta(t.slots_in_epoch.saturating_sub(t.slot_index));
            lines.push(kv("eta", Span::styled(eta, Style::new().fg(theme().text))));
            lines.push(kv(
                "tps",
                Span::styled(group_thousands(t.tps), Style::new().fg(theme().text)),
            ));
        }
        None => {
            for k in ["slot", "block", "epoch", "eta", "tps"] {
                lines.push(kv(k, Span::styled("—", Style::new().fg(theme().dim))));
            }
        }
    }

    lines.push(Line::from(""));
    match stats {
        Some(s) => {
            lines.push(kv(
                "nodes",
                Span::styled(group_thousands(s.validators), Style::new().fg(theme().text)),
            ));
            lines.push(kv(
                "txns",
                Span::styled(abbrev(s.txn_count), Style::new().fg(theme().text)),
            ));
            let supply = if s.supply_sol > 0 {
                format!("{} SOL", abbrev(s.supply_sol))
            } else {
                "—".to_string()
            };
            lines.push(kv(
                "supply",
                Span::styled(supply, Style::new().fg(theme().text)),
            ));
        }
        None => {
            for k in ["nodes", "txns", "supply"] {
                lines.push(kv(k, Span::styled("—", Style::new().fg(theme().dim))));
            }
        }
    }

    // SOL price is global (not per-cluster), so it persists across switches.
    match price {
        Some(p) => {
            let (arrow, color) = if p.change_24h > 0.0 {
                ("▲", theme().success)
            } else if p.change_24h < 0.0 {
                ("▼", theme().error)
            } else {
                ("·", theme().muted)
            };
            lines.push(Line::from(vec![
                Span::styled(" SOL     ", Style::new().fg(theme().dim)),
                Span::styled(format!("${:.2} ", p.usd), Style::new().fg(theme().text)),
                Span::styled(
                    format!("{arrow}{:.1}%", p.change_24h.abs()),
                    Style::new().fg(color),
                ),
            ]));
        }
        None => lines.push(kv("SOL", Span::styled("—", Style::new().fg(theme().dim)))),
    }

    lines.push(Line::from(""));
    lines.push(kv(
        "ping",
        Span::styled(ping, Style::new().fg(theme().text)),
    ));
    lines.push(kv(
        "version",
        Span::styled(version, Style::new().fg(theme().text)),
    ));

    f.render_widget(Paragraph::new(lines), inner);
}

/// Format a slot count as a rough epoch ETA (slots × ~0.4s).
pub fn format_eta(slots: u64) -> String {
    let secs = (slots as f64 * 0.4) as u64;
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// Abbreviate large counts: 1.23B / 4.56M / else thousands-separated.
pub fn abbrev(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1e6)
    } else {
        group_thousands(n)
    }
}

/// `▓▓▓░░░ 42%` style progress bar filling the given width.
pub fn progress_bar(ratio: f64, width: usize) -> String {
    let label = format!(" {:>3.0}%", (ratio * 100.0).clamp(0.0, 100.0));
    let bar_w = width.saturating_sub(label.chars().count());
    let filled = ((ratio * bar_w as f64).round() as usize).min(bar_w);
    format!(
        "{}{}{}",
        "▓".repeat(filled),
        "░".repeat(bar_w - filled),
        label
    )
}

/// Format a number with thousands separators.
pub fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Full-width red warning bar shown when the active cluster is mainnet.
fn hazard_banner(width: usize) -> Line<'static> {
    let text = "⚠  MAINNET-BETA — real funds at risk";
    let pad = width.saturating_sub(text.chars().count() + 2);
    let left = pad / 2;
    let right = pad - left;
    let content = format!("{}{text}{}", " ".repeat(left + 1), " ".repeat(right + 1));
    Line::from(Span::raw(content)).style(
        Style::new()
            .bg(theme().error)
            .fg(theme().text)
            .add_modifier(Modifier::BOLD),
    )
}

fn marker(focused: bool) -> Span<'static> {
    if focused {
        Span::styled(
            " ◆ ",
            Style::new().fg(theme().accent).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("   ")
    }
}

fn label(text: &str, focused: bool) -> Span<'static> {
    let style = if focused {
        Style::new().fg(theme().accent).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme().text)
    };
    Span::styled(format!("{text:<11}"), style)
}

fn field_lines<'a>(
    app: &'a App,
    field: Field,
    focused: bool,
    editing: bool,
    balance: &Balance,
) -> Vec<Line<'a>> {
    match field {
        Field::Cluster => cluster_lines(app, focused, editing),
        Field::Keypair => keypair_lines(app, focused, balance),
        Field::Commitment => vec![commitment_line(app, focused)],
        Field::Websocket => websocket_lines(app, focused, editing),
    }
}

fn balance_span(balance: &Balance) -> Span<'static> {
    match balance {
        Balance::Unknown => Span::raw(""),
        Balance::Loading => Span::styled(
            format!("  ◎ {}", spinner()),
            Style::new().fg(theme().warning),
        ),
        Balance::Lamports(l) => Span::styled(
            format!("  ◎ {:.4} SOL", *l as f64 / 1_000_000_000.0),
            Style::new().fg(theme().success),
        ),
        Balance::Err(e) => {
            let short: String = e.chars().take(28).collect();
            Span::styled(format!("  ◎ n/a ({short})"), Style::new().fg(theme().dim))
        }
    }
}

fn radio(name: &str, active_url: &str, on: bool) -> Span<'static> {
    let bullet = if on { "● " } else { "○ " };
    let style = if on {
        Style::new()
            .fg(cluster_color(active_url))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme().dim)
    };
    Span::styled(format!("{bullet}{name}   "), style)
}

/// Shorten built-in moniker names so the radio row fits a narrow pane.
fn short_net(name: &str) -> &str {
    match name {
        "mainnet-beta" => "mainnet",
        "localhost" => "local",
        other => other,
    }
}

fn cluster_lines(app: &App, focused: bool, editing: bool) -> Vec<Line<'static>> {
    let list = app.clusters();
    let active = list.iter().position(|(_, u)| *u == app.cfg.json_rpc_url);
    let moniker_count = MONIKERS.len();

    let mut head = vec![marker(focused), label("Cluster", focused)];
    for (i, (name, _)) in list.iter().take(moniker_count).enumerate() {
        head.push(radio(
            short_net(name),
            &app.cfg.json_rpc_url,
            active == Some(i),
        ));
    }
    let mut lines = vec![Line::from(head)];

    // Second row for user-saved custom endpoints, if any.
    if list.len() > moniker_count {
        let mut row = vec![Span::raw(" ".repeat(14))];
        for (i, (name, _)) in list.iter().enumerate().skip(moniker_count) {
            row.push(radio(name, &app.cfg.json_rpc_url, active == Some(i)));
        }
        lines.push(Line::from(row));
    }

    let detail = if editing {
        Line::from(vec![
            Span::styled(
                format!("{}└ ", " ".repeat(12)),
                Style::new().fg(theme().dim),
            ),
            Span::styled(format!("{}▊", app.buf), Style::new().fg(theme().warning)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!("{}└ ", " ".repeat(12)),
                Style::new().fg(theme().dim),
            ),
            Span::styled(app.cfg.json_rpc_url.clone(), Style::new().fg(theme().dim)),
        ])
    };
    lines.push(detail);
    lines
}

fn keypair_lines(app: &App, focused: bool, balance: &Balance) -> Vec<Line<'static>> {
    let head = Line::from(vec![
        marker(focused),
        label("Keypair", focused),
        Span::raw(config::display_path(Path::new(&app.cfg.keypair_path))),
    ]);

    let detail = match config::pubkey_from_keypair(&app.cfg.keypair_path) {
        Some(pk) => {
            let short = format!(
                "{}...{}",
                &pk[..4.min(pk.len())],
                &pk[pk.len().saturating_sub(4)..]
            );
            Line::from(vec![
                Span::styled("              └ ", Style::new().fg(theme().dim)),
                Span::styled(short, Style::new().fg(theme().success)),
                balance_span(balance),
            ])
        }
        None => Line::from(vec![
            Span::styled("              └ ", Style::new().fg(theme().dim)),
            Span::styled(
                "no readable keypair at this path",
                Style::new().fg(theme().error),
            ),
        ]),
    };
    vec![head, detail]
}

fn commitment_line(app: &App, focused: bool) -> Line<'static> {
    let mut spans = vec![marker(focused), label("Commitment", focused)];
    for c in COMMITMENTS.iter() {
        if *c == app.cfg.commitment {
            spans.push(Span::styled(
                format!("[{c}] "),
                Style::new().fg(theme().accent).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {c}  "),
                Style::new().fg(theme().dim),
            ));
        }
    }
    Line::from(spans)
}

fn websocket_lines(app: &App, focused: bool, editing: bool) -> Vec<Line<'static>> {
    let mut spans = vec![marker(focused), label("WebSocket", focused)];
    if editing {
        spans.push(Span::styled(
            format!("{}▊", app.buf),
            Style::new().fg(theme().warning),
        ));
    } else if app.cfg.websocket_url.is_empty() {
        spans.push(Span::styled("(auto) ", Style::new().fg(theme().dim)));
        spans.push(Span::styled(
            config::derive_ws_url(&app.cfg.json_rpc_url),
            Style::new().fg(theme().muted),
        ));
    } else {
        spans.push(Span::raw(app.cfg.websocket_url.clone()));
    }
    vec![Line::from(spans)]
}

fn render_status(f: &mut Frame, app: &App, health: &Health, area: Rect) {
    let (dot, text, color) = match health {
        Health::Unknown => ("○".to_string(), "no endpoint".to_string(), theme().dim),
        Health::Checking => (
            spinner().to_string(),
            "checking…".to_string(),
            theme().warning,
        ),
        Health::Ok { version, ms } => (
            "●".to_string(),
            format!("reachable · v{version} · {ms}ms"),
            theme().success,
        ),
        Health::Err(e) => ("●".to_string(), format!("unreachable — {e}"), theme().error),
    };
    let saved = if app.dirty {
        Span::styled("unsaved", Style::new().fg(theme().warning))
    } else {
        Span::styled("saved", Style::new().fg(theme().success))
    };
    let left = Line::from(vec![
        Span::styled(format!(" {dot} "), Style::new().fg(color)),
        Span::styled(text, Style::new().fg(color)),
        Span::raw("   "),
        Span::styled(format!("[{}]", app.status), Style::new().fg(theme().dim)),
    ]);
    f.render_widget(Paragraph::new(left), area);
    // right-aligned saved indicator
    f.render_widget(
        Paragraph::new(Line::from(vec![saved, Span::raw(" ")])).right_aligned(),
        area,
    );
}

fn render_help(f: &mut Frame, app: &App, area: Rect) {
    let text: String = if app.confirm_quit {
        " q discard & quit   s save & quit   esc cancel".to_string()
    } else {
        match app.mode {
            Mode::Normal => format!(
                " ↑↓ move   ←→ pick   ⏎ edit   a airdrop {}◎(±)   t transfer   s save   ? all keys   q quit",
                app.airdrop_sol
            ),
            Mode::EditField => " type value   ⏎ confirm   esc cancel".to_string(),
            Mode::KeyPicker => {
                " type to filter   ↑↓ select   ⏎ choose   g generate new   esc back".to_string()
            }
            Mode::Profiles => " ↑↓ select   ⏎ activate   n new   d delete   esc back".to_string(),
            Mode::NewProfile => {
                " type name   ⏎ save current config as profile   esc back".to_string()
            }
            Mode::Endpoints => " ↑↓ select   ⏎ use   n new   d delete   esc back".to_string(),
            Mode::NewEndpoint => " type  name=url   ⏎ save   esc back".to_string(),
            Mode::Faucets => {
                " ↑↓ select   ⏎ open in browser (copies pubkey)   n new   d delete   esc back"
                    .to_string()
            }
            Mode::NewFaucet => " type  name=url   ⏎ save   esc back".to_string(),
            Mode::Transfer => {
                " tab switch field   ←→ pick local wallet   ⏎ review   esc cancel".to_string()
            }
            Mode::TransferConfirm => " ⏎ send   esc cancel".to_string(),
            Mode::Themes => " ↑↓ preview   ⏎ apply & save   esc cancel".to_string(),
            Mode::Help => " esc / ? close".to_string(),
        }
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(text, Style::new().fg(theme().dim))))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_key_picker(f: &mut Frame, app: &App) {
    let inner = modal(f, "select keypair", 72, 16);

    let matches = app.filtered_keys();
    let header = vec![
        Line::from(vec![
            Span::styled(" search: ", Style::new().fg(theme().accent)),
            Span::styled(
                format!("{}▊", app.key_filter),
                Style::new().fg(theme().warning),
            ),
        ]),
        Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::new().fg(theme().dim),
        )),
    ];

    let items: Vec<Line> = if matches.is_empty() {
        vec![Line::from(Span::styled(
            "  no matching keypairs found",
            Style::new().fg(theme().dim),
        ))]
    } else {
        matches
            .iter()
            .enumerate()
            .map(|(row, &idx)| {
                let kf = &app.key_files[idx];
                let sel = row == app.key_sel;
                let prefix = if sel { " > " } else { "   " };
                let style = if sel {
                    Style::new().fg(theme().accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(theme().text)
                };
                let pk = &kf.pubkey;
                let short = format!(
                    "{}...{}",
                    &pk[..4.min(pk.len())],
                    &pk[pk.len().saturating_sub(4)..]
                );
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(format!("{:<40}", kf.display), style),
                    Span::styled(short, Style::new().fg(theme().success)),
                ])
            })
            .collect()
    };

    render_list(f, inner, header, items, app.key_sel, vec![]);
}

fn render_endpoints(f: &mut Frame, app: &App) {
    let inner = modal(f, "custom RPC endpoints", 72, 14);

    let items: Vec<Line> = if app.endpoints.is_empty() {
        vec![Line::from(Span::styled(
            "  no custom endpoints — press 'n' to add one (name=url)",
            Style::new().fg(theme().dim),
        ))]
    } else {
        app.endpoints
            .iter()
            .enumerate()
            .map(|(i, ep)| {
                let sel = i == app.ep_sel;
                let prefix = if sel { " > " } else { "   " };
                let style = if sel {
                    Style::new().fg(theme().accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(theme().text)
                };
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(format!("{:<16}", ep.name), style),
                    Span::styled(ep.url.clone(), Style::new().fg(theme().dim)),
                ])
            })
            .collect()
    };

    let footer = if app.mode == Mode::NewEndpoint {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  name=url: ", Style::new().fg(theme().text)),
                Span::styled(format!("{}▊", app.buf), Style::new().fg(theme().warning)),
            ]),
        ]
    } else {
        vec![]
    };

    render_list(f, inner, vec![], items, app.ep_sel, footer);
}

fn render_help_panel(f: &mut Frame) {
    let inner = modal(f, "SOLFIG · keybindings", 64, 26);

    let section = |title: &str| {
        Line::from(Span::styled(
            format!(" {title}"),
            Style::new().fg(theme().accent).add_modifier(Modifier::BOLD),
        ))
    };
    let row = |key: &str, desc: &str| {
        Line::from(vec![
            Span::styled(format!("   {key:<14}"), Style::new().fg(theme().warning)),
            Span::styled(desc.to_string(), Style::new().fg(theme().text)),
        ])
    };

    let lines: Vec<Line> = vec![
        section("Navigate"),
        row("↑ ↓ / j k", "move between fields"),
        row("← → / h l", "change value (cluster, commitment)"),
        row("⏎", "edit field / open keypair picker"),
        Line::from(""),
        section("Wallet"),
        row("⏎", "on Keypair: pick from local wallets"),
        row("g", "in picker: generate a new keypair"),
        row("a", "airdrop to current keypair"),
        row("+ / -", "change airdrop amount"),
        row("t", "transfer SOL (toggle local wallets)"),
        row("y", "copy focused field value"),
        row("o", "open address in Solana Explorer"),
        Line::from(""),
        section("RPC & config"),
        row("e", "custom RPC endpoints"),
        row("f", "web faucets (open in browser)"),
        row("p", "profiles (switch environments)"),
        row("T", "themes (live preview)"),
        Line::from(""),
        section("File"),
        row("s / r", "save / reload config"),
        row("q", "quit (asks if unsaved)"),
    ];

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_transfer(f: &mut Frame, app: &App) {
    let inner = modal(f, "transfer SOL", 110, 15);

    let from = config::pubkey_from_keypair(&app.cfg.keypair_path).unwrap_or_default();
    let cluster = config::moniker_for_url(&app.cfg.json_rpc_url).unwrap_or("custom");
    let mut lines = vec![Line::from("")];
    if cluster == "mainnet-beta" {
        lines.push(Line::from(Span::styled(
            "  ⚠  MAINNET — this sends REAL SOL",
            Style::new().fg(theme().error).add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(vec![
        Span::styled("  from:    ", Style::new().fg(theme().dim)),
        Span::styled(from, Style::new().fg(theme().success)),
        Span::styled(format!("  ({cluster})"), Style::new().fg(theme().dim)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    if app.mode == Mode::Transfer {
        let to_focus = app.tx_focus == 0;
        let mut to_spans = vec![
            field_marker(to_focus),
            Span::styled("to:      ", Style::new().fg(theme().dim)),
        ];
        if app.tx_to.is_empty() && to_focus {
            to_spans.push(Span::styled("▊", Style::new().fg(theme().warning)));
            to_spans.push(Span::styled(
                "  (type, or ←→ to pick a local wallet)",
                Style::new().fg(theme().dim),
            ));
        } else {
            to_spans.push(Span::styled(
                if to_focus {
                    format!("{}▊", app.tx_to)
                } else {
                    app.tx_to.clone()
                },
                Style::new().fg(theme().text),
            ));
            if let Some(name) = app.recipient_local_name() {
                to_spans.push(Span::styled(
                    format!("  ({name})"),
                    Style::new().fg(theme().success),
                ));
            }
        }
        lines.push(Line::from(to_spans));
        lines.push(Line::from(""));

        let amt_focus = app.tx_focus == 1;
        lines.push(Line::from(vec![
            field_marker(amt_focus),
            Span::styled("amount:  ", Style::new().fg(theme().dim)),
            Span::styled(
                if amt_focus {
                    format!("{}▊", app.tx_amount)
                } else {
                    app.tx_amount.clone()
                },
                Style::new().fg(theme().text),
            ),
            Span::styled(" SOL", Style::new().fg(theme().dim)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  to:      ", Style::new().fg(theme().dim)),
            Span::styled(app.tx_to.clone(), Style::new().fg(theme().text)),
            match app.recipient_local_name() {
                Some(name) => Span::styled(format!("  ({name})"), Style::new().fg(theme().success)),
                None => Span::raw(""),
            },
        ]));
        lines.push(Line::from(vec![
            Span::styled("  amount:  ", Style::new().fg(theme().dim)),
            Span::styled(
                format!("{} SOL", app.tx_amount),
                Style::new().fg(theme().text).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ⏎ send   esc cancel",
            Style::new().fg(theme().warning),
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn field_marker(focused: bool) -> Span<'static> {
    if focused {
        Span::styled(
            "  › ",
            Style::new().fg(theme().accent).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("    ")
    }
}

fn render_faucets(f: &mut Frame, app: &App) {
    let inner = modal(f, "web faucets", 72, 14);

    let items: Vec<Line> = if app.faucets.is_empty() {
        vec![Line::from(Span::styled(
            "  no faucets — press 'n' to add one (name=url)",
            Style::new().fg(theme().dim),
        ))]
    } else {
        app.faucets
            .iter()
            .enumerate()
            .map(|(i, fct)| {
                let sel = i == app.faucet_sel;
                let prefix = if sel { " > " } else { "   " };
                let style = if sel {
                    Style::new().fg(theme().accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(theme().text)
                };
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(format!("{:<12}", fct.name), style),
                    Span::styled(fct.url.clone(), Style::new().fg(theme().dim)),
                ])
            })
            .collect()
    };

    let footer = if app.mode == Mode::NewFaucet {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  name=url: ", Style::new().fg(theme().text)),
                Span::styled(format!("{}▊", app.buf), Style::new().fg(theme().warning)),
            ]),
        ]
    } else {
        vec![]
    };

    render_list(f, inner, vec![], items, app.faucet_sel, footer);
}

fn render_profiles(f: &mut Frame, app: &App) {
    let inner = modal(f, "profiles", 60, 14);

    let items: Vec<Line> = if app.profiles.is_empty() {
        vec![Line::from(Span::styled(
            "  no profiles yet — press 'n' to save the current config",
            Style::new().fg(theme().dim),
        ))]
    } else {
        app.profiles
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let sel = i == app.prof_sel;
                let prefix = if sel { " > " } else { "   " };
                let moniker = config::moniker_for_url(&p.cfg.json_rpc_url)
                    .unwrap_or("custom")
                    .to_string();
                let style = if sel {
                    Style::new().fg(theme().accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(theme().text)
                };
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(format!("{:<16}", p.name), style),
                    Span::styled(moniker, Style::new().fg(theme().dim)),
                ])
            })
            .collect()
    };

    let footer = if app.mode == Mode::NewProfile {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  name: ", Style::new().fg(theme().text)),
                Span::styled(format!("{}▊", app.buf), Style::new().fg(theme().warning)),
            ]),
        ]
    } else {
        vec![]
    };

    render_list(f, inner, vec![], items, app.prof_sel, footer);
}

fn render_themes(f: &mut Frame, app: &App) {
    let names = theme::names();
    let height = (names.len() as u16).clamp(1, 14) + 4;
    let inner = modal(f, "themes", 48, height);

    let swatch = |c: Color| Span::styled("●", Style::new().fg(c));
    let items: Vec<Line> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let sel = i == app.theme_sel;
            let prefix = if sel { " > " } else { "   " };
            let style = if sel {
                Style::new().fg(theme().accent).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme().text)
            };
            // Preview each palette's key colors inline, regardless of the
            // theme currently driving the rest of the UI.
            let p = theme::resolve(name);
            Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{name:<12}"), style),
                swatch(p.accent),
                Span::raw(" "),
                swatch(p.success),
                Span::raw(" "),
                swatch(p.warning),
                Span::raw(" "),
                swatch(p.error),
                Span::raw(" "),
                swatch(p.devnet),
            ])
        })
        .collect();

    render_list(f, inner, vec![], items, app.theme_sel, vec![]);
}

/// Render a modal body as: fixed header, a scrollable item list (kept in view
/// around `selected`, with a scrollbar when it overflows), and a fixed footer.
fn render_list(
    f: &mut Frame,
    inner: Rect,
    header: Vec<Line>,
    items: Vec<Line>,
    selected: usize,
    footer: Vec<Line>,
) {
    let header_h = header.len() as u16;
    let footer_h = footer.len() as u16;
    let body_h = inner.height.saturating_sub(header_h + footer_h);

    if header_h > 0 {
        let a = Rect::new(inner.x, inner.y, inner.width, header_h);
        f.render_widget(Paragraph::new(header), a);
    }

    let body = Rect::new(inner.x, inner.y + header_h, inner.width, body_h);
    let h = body_h as usize;
    let total = items.len();
    let offset = if total <= h {
        0
    } else {
        selected.saturating_sub(h.saturating_sub(1)).min(total - h)
    };
    f.render_widget(Paragraph::new(items).scroll((offset as u16, 0)), body);

    if total > h {
        let mut state = ScrollbarState::new(total).position(offset);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            body,
            &mut state,
        );
    }

    if footer_h > 0 {
        let a = Rect::new(inner.x, inner.y + header_h + body_h, inner.width, footer_h);
        f.render_widget(Paragraph::new(footer), a);
    }
}

pub fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

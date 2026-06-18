use crate::app::{App, Field, Mode, FIELDS};
use crate::config::{self, COMMITMENTS, MONIKERS};
use crate::rpc::{Balance, Health};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use std::path::Path;

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn render(f: &mut Frame, app: &App, health: &Health, balance: &Balance) {
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(f.area());

    render_body(f, app, balance, chunks[0]);
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
}

fn render_body(f: &mut Frame, app: &App, balance: &Balance, area: Rect) {
    let title = format!(" solcfg — {} ", app.path.display());
    let block = Block::bordered()
        .title(title)
        .border_style(Style::new().fg(DIM));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, field) in FIELDS.iter().enumerate() {
        let focused = i == app.sel;
        let editing = focused && app.mode == Mode::EditField;
        lines.extend(field_lines(app, *field, focused, editing, balance));
        lines.push(Line::from(""));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn marker(focused: bool) -> Span<'static> {
    if focused {
        Span::styled(" ◆ ", Style::new().fg(ACCENT).add_modifier(Modifier::BOLD))
    } else {
        Span::raw("   ")
    }
}

fn label(text: &str, focused: bool) -> Span<'static> {
    let style = if focused {
        Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::White)
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
        Balance::Loading => Span::styled("  ◎ …", Style::new().fg(DIM)),
        Balance::Lamports(l) => Span::styled(
            format!("  ◎ {:.4} SOL", *l as f64 / 1_000_000_000.0),
            Style::new().fg(Color::Green),
        ),
        Balance::Err(e) => {
            let short: String = e.chars().take(28).collect();
            Span::styled(format!("  ◎ n/a ({short})"), Style::new().fg(DIM))
        }
    }
}

fn radio(name: &str, on: bool) -> Span<'static> {
    let bullet = if on { "(●) " } else { "( ) " };
    let style = if on {
        Style::new().fg(ACCENT)
    } else {
        Style::new().fg(DIM)
    };
    Span::styled(format!("{bullet}{name}   "), style)
}

fn cluster_lines(app: &App, focused: bool, editing: bool) -> Vec<Line<'static>> {
    let list = app.clusters();
    let active = list.iter().position(|(_, u)| *u == app.cfg.json_rpc_url);
    let moniker_count = MONIKERS.len();

    let mut head = vec![marker(focused), label("Cluster", focused)];
    for (i, (name, _)) in list.iter().take(moniker_count).enumerate() {
        head.push(radio(name, active == Some(i)));
    }
    let mut lines = vec![Line::from(head)];

    // Second row for user-saved custom endpoints, if any.
    if list.len() > moniker_count {
        let mut row = vec![Span::raw(" ".repeat(14))];
        for (i, (name, _)) in list.iter().enumerate().skip(moniker_count) {
            row.push(radio(name, active == Some(i)));
        }
        lines.push(Line::from(row));
    }

    let detail = if editing {
        Line::from(vec![
            Span::styled(format!("{}└ ", " ".repeat(12)), Style::new().fg(DIM)),
            Span::styled(format!("{}▊", app.buf), Style::new().fg(Color::Yellow)),
        ])
    } else {
        Line::from(vec![
            Span::styled(format!("{}└ ", " ".repeat(12)), Style::new().fg(DIM)),
            Span::styled(app.cfg.json_rpc_url.clone(), Style::new().fg(DIM)),
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
            let short = format!("{}...{}", &pk[..4.min(pk.len())], &pk[pk.len().saturating_sub(4)..]);
            Line::from(vec![
                Span::styled("              └ ", Style::new().fg(DIM)),
                Span::styled(short, Style::new().fg(Color::Green)),
                balance_span(balance),
            ])
        }
        None => Line::from(vec![
            Span::styled("              └ ", Style::new().fg(DIM)),
            Span::styled("no readable keypair at this path", Style::new().fg(Color::Red)),
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
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(format!(" {c}  "), Style::new().fg(DIM)));
        }
    }
    Line::from(spans)
}

fn websocket_lines(app: &App, focused: bool, editing: bool) -> Vec<Line<'static>> {
    let value = if editing {
        Span::styled(format!("{}▊", app.buf), Style::new().fg(Color::Yellow))
    } else if app.cfg.websocket_url.is_empty() {
        Span::styled("(auto from RPC)", Style::new().fg(DIM))
    } else {
        Span::raw(app.cfg.websocket_url.clone())
    };
    vec![Line::from(vec![
        marker(focused),
        label("WebSocket", focused),
        value,
    ])]
}

fn render_status(f: &mut Frame, app: &App, health: &Health, area: Rect) {
    let (dot, text, color) = match health {
        Health::Unknown => ("○", "no endpoint".to_string(), DIM),
        Health::Checking => ("◐", "checking…".to_string(), Color::Yellow),
        Health::Ok { version, ms } => {
            ("●", format!("reachable · v{version} · {ms}ms"), Color::Green)
        }
        Health::Err(e) => ("●", format!("unreachable — {e}"), Color::Red),
    };
    let saved = if app.dirty {
        Span::styled("✎ unsaved", Style::new().fg(Color::Yellow))
    } else {
        Span::styled("✓ saved", Style::new().fg(Color::Green))
    };
    let left = Line::from(vec![
        Span::styled(format!(" {dot} "), Style::new().fg(color)),
        Span::styled(text, Style::new().fg(color)),
        Span::raw("   "),
        Span::styled(format!("[{}]", app.status), Style::new().fg(DIM)),
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
                " ↑↓ field  ←→ pick  ⏎ edit  a airdrop {}◎ (±)  y copy  e endpoints  p profiles  s save  q quit",
                app.airdrop_sol
            ),
            Mode::EditField => " type value   ⏎ confirm   esc cancel".to_string(),
            Mode::KeyPicker => " type to filter   ↑↓ select   ⏎ choose   esc back".to_string(),
            Mode::Profiles => " ↑↓ select   ⏎ activate   n new   d delete   esc back".to_string(),
            Mode::NewProfile => {
                " type name   ⏎ save current config as profile   esc back".to_string()
            }
            Mode::Endpoints => " ↑↓ select   ⏎ use   n new   d delete   esc back".to_string(),
            Mode::NewEndpoint => " type  name=url   ⏎ save   esc back".to_string(),
        }
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(text, Style::new().fg(DIM)))),
        area,
    );
}

fn render_key_picker(f: &mut Frame, app: &App) {
    let area = centered(72, 16, f.area());
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" select keypair ")
        .border_style(Style::new().fg(ACCENT));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let matches = app.filtered_keys();
    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled(" 🔍 ", Style::new().fg(ACCENT)),
        Span::styled(format!("{}▊", app.key_filter), Style::new().fg(Color::Yellow)),
    ])];
    lines.push(Line::from(Span::styled(
        "─".repeat(inner.width as usize),
        Style::new().fg(DIM),
    )));

    if matches.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no matching keypairs found",
            Style::new().fg(DIM),
        )));
    } else {
        for (row, &idx) in matches.iter().enumerate() {
            let kf = &app.key_files[idx];
            let sel = row == app.key_sel;
            let prefix = if sel { " > " } else { "   " };
            let style = if sel {
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::White)
            };
            let pk = &kf.pubkey;
            let short = format!("{}...{}", &pk[..4.min(pk.len())], &pk[pk.len().saturating_sub(4)..]);
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{:<40}", kf.display), style),
                Span::styled(short, Style::new().fg(Color::Green)),
            ]));
        }
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn render_endpoints(f: &mut Frame, app: &App) {
    let area = centered(72, 14, f.area());
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" custom RPC endpoints ")
        .border_style(Style::new().fg(ACCENT));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if app.endpoints.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no custom endpoints — press 'n' to add one (name=url)",
            Style::new().fg(DIM),
        )));
    } else {
        for (i, ep) in app.endpoints.iter().enumerate() {
            let sel = i == app.ep_sel;
            let prefix = if sel { " > " } else { "   " };
            let style = if sel {
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::White)
            };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{:<16}", ep.name), style),
                Span::styled(ep.url.clone(), Style::new().fg(DIM)),
            ]));
        }
    }

    if app.mode == Mode::NewEndpoint {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  name=url: ", Style::new().fg(Color::White)),
            Span::styled(format!("{}▊", app.buf), Style::new().fg(Color::Yellow)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_profiles(f: &mut Frame, app: &App) {
    let area = centered(60, 14, f.area());
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" profiles ")
        .border_style(Style::new().fg(ACCENT));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if app.profiles.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no profiles yet — press 'n' to save the current config",
            Style::new().fg(DIM),
        )));
    } else {
        for (i, p) in app.profiles.iter().enumerate() {
            let sel = i == app.prof_sel;
            let prefix = if sel { " > " } else { "   " };
            let moniker = config::moniker_for_url(&p.cfg.json_rpc_url)
                .unwrap_or("custom")
                .to_string();
            let style = if sel {
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::White)
            };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{:<16}", p.name), style),
                Span::styled(moniker, Style::new().fg(DIM)),
            ]));
        }
    }

    if app.mode == Mode::NewProfile {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  name: ", Style::new().fg(Color::White)),
            Span::styled(format!("{}▊", app.buf), Style::new().fg(Color::Yellow)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

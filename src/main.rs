use chrono::{Local, Utc};
use chrono_tz::Tz;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Terminal,
};
use solardash::{api, data::{AuroraSource, DashboardData}, map::WorldMap};
use std::collections::HashMap;
use std::io;
use std::time::Duration;

const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const LAUNCH_REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 60);

use solardash::colors;

fn get_local_tz_abbrev() -> String {
    if let Ok(tz_name) = iana_time_zone::get_timezone() {
        if let Ok(tz) = tz_name.parse::<Tz>() {
            return Utc::now().with_timezone(&tz).format("%Z").to_string();
        }
    }
    Local::now().format("%Z").to_string()
}

fn get_severity_color(severity: solardash::data::ScaleSeverity) -> Color {
    use solardash::data::ScaleSeverity;
    match severity {
        ScaleSeverity::None => colors::SEVERITY_NONE,
        ScaleSeverity::Minor => colors::SEVERITY_MINOR,
        ScaleSeverity::Moderate => colors::SEVERITY_MODERATE,
        ScaleSeverity::Strong => colors::SEVERITY_STRONG,
        ScaleSeverity::Severe => colors::SEVERITY_SEVERE,
    }
}

// === Braille Sparkline Graph Renderer ===

/// Render a braille sparkline graph with Y-axis labels
fn render_braille_graph(
    data: &[f64],
    min_val: f64,
    max_val: f64,
    width: usize,
    height: usize,
    color: Color,
) -> Vec<Line<'static>> {
    let px_w = width * 2;
    let px_h = height * 4;
    let mut pixels = vec![false; px_w * px_h];

    if data.is_empty() || px_w == 0 || px_h == 0 {
        return vec![Line::from(""); height];
    }

    let range = if (max_val - min_val).abs() < 0.001 { 1.0 } else { max_val - min_val };

    // Map data to pixel positions and draw
    let n = data.len();
    for i in 0..n {
        let x = if n > 1 { i * (px_w - 1) / (n - 1) } else { px_w / 2 };
        let normalized = ((data[i] - min_val) / range).clamp(0.0, 1.0);
        let y = ((1.0 - normalized) * (px_h - 1) as f64) as usize;
        let y = y.min(px_h - 1);
        let x = x.min(px_w - 1);
        pixels[y * px_w + x] = true;

        // Draw line to next point
        if i + 1 < n {
            let x2 = if n > 1 { (i + 1) * (px_w - 1) / (n - 1) } else { px_w / 2 };
            let normalized2 = ((data[i + 1] - min_val) / range).clamp(0.0, 1.0);
            let y2 = ((1.0 - normalized2) * (px_h - 1) as f64) as usize;
            let y2 = y2.min(px_h - 1);
            let x2 = x2.min(px_w - 1);
            draw_line_pixels(&mut pixels, px_w, px_h, x as i32, y as i32, x2 as i32, y2 as i32);
        }
    }

    // Encode to braille
    let mut lines = Vec::with_capacity(height);
    for cy in 0..height {
        let mut s = String::with_capacity(width);
        for cx in 0..width {
            let mut code: u32 = 0x2800;
            for dx in 0..2usize {
                for dy in 0..4usize {
                    let px = cx * 2 + dx;
                    let py = cy * 4 + dy;
                    if px < px_w && py < px_h && pixels[py * px_w + px] {
                        let bit = match (dx, dy) {
                            (0, 0) => 0x01,
                            (0, 1) => 0x02,
                            (0, 2) => 0x04,
                            (0, 3) => 0x40,
                            (1, 0) => 0x08,
                            (1, 1) => 0x10,
                            (1, 2) => 0x20,
                            (1, 3) => 0x80,
                            _ => 0,
                        };
                        code |= bit;
                    }
                }
            }
            s.push(char::from_u32(code).unwrap_or(' '));
        }
        lines.push(Line::from(Span::styled(s, Style::default().fg(color))));
    }
    lines
}

/// Render a braille graph with positive (green) / negative (red) coloring split at zero line
fn render_bz_braille_graph(
    data: &[f64],
    min_val: f64,
    max_val: f64,
    width: usize,
    height: usize,
) -> Vec<Line<'static>> {
    let px_w = width * 2;
    let px_h = height * 4;

    if data.is_empty() || px_w == 0 || px_h == 0 {
        return vec![Line::from(""); height];
    }

    let range = if (max_val - min_val).abs() < 0.001 { 1.0 } else { max_val - min_val };
    let zero_py = ((1.0 - ((0.0 - min_val) / range).clamp(0.0, 1.0)) * (px_h - 1) as f64) as usize;

    // Separate pixel arrays: data points vs zero reference line
    let mut data_pixels = vec![false; px_w * px_h];
    let mut zero_pixels = vec![false; px_w * px_h];

    // Map data points onto data_pixels
    let n = data.len();
    for i in 0..n {
        let x = if n > 1 { i * (px_w - 1) / (n - 1) } else { px_w / 2 };
        let normalized = ((data[i] - min_val) / range).clamp(0.0, 1.0);
        let y = ((1.0 - normalized) * (px_h - 1) as f64) as usize;
        let y = y.min(px_h - 1);
        let x = x.min(px_w - 1);
        data_pixels[y * px_w + x] = true;

        if i + 1 < n {
            let x2 = if n > 1 { (i + 1) * (px_w - 1) / (n - 1) } else { px_w / 2 };
            let normalized2 = ((data[i + 1] - min_val) / range).clamp(0.0, 1.0);
            let y2 = ((1.0 - normalized2) * (px_h - 1) as f64) as usize;
            let y2 = y2.min(px_h - 1);
            let x2 = x2.min(px_w - 1);
            draw_line_pixels(&mut data_pixels, px_w, px_h, x as i32, y as i32, x2 as i32, y2 as i32);
        }
    }

    // Draw dotted zero reference line on separate buffer
    if zero_py < px_h {
        for x in (0..px_w).step_by(3) {
            zero_pixels[zero_py * px_w + x] = true;
        }
    }

    // Encode to braille with per-character coloring based on data position
    let mut lines = Vec::with_capacity(height);
    for cy in 0..height {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for cx in 0..width {
            let mut code: u32 = 0x2800;
            let mut has_positive_data = false;
            let mut has_negative_data = false;

            for dx in 0..2usize {
                for dy in 0..4usize {
                    let px = cx * 2 + dx;
                    let py = cy * 4 + dy;
                    if px < px_w && py < px_h {
                        let is_data = data_pixels[py * px_w + px];
                        let is_zero = zero_pixels[py * px_w + px];

                        if is_data || is_zero {
                            let bit = match (dx, dy) {
                                (0, 0) => 0x01,
                                (0, 1) => 0x02,
                                (0, 2) => 0x04,
                                (0, 3) => 0x40,
                                (1, 0) => 0x08,
                                (1, 1) => 0x10,
                                (1, 2) => 0x20,
                                (1, 3) => 0x80,
                                _ => 0,
                            };
                            code |= bit;
                        }

                        if is_data {
                            if py < zero_py {
                                has_positive_data = true;
                            } else if py > zero_py {
                                has_negative_data = true;
                            }
                        }
                    }
                }
            }

            let ch = char::from_u32(code).unwrap_or(' ');
            let color = if has_negative_data && !has_positive_data {
                colors::SOLAR_WIND_BZ_NEGATIVE
            } else if has_positive_data {
                colors::SOLAR_WIND_BZ_POSITIVE
            } else {
                colors::GRAPH_AXIS
            };

            spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn draw_line_pixels(pixels: &mut [bool], w: usize, h: usize, x0: i32, y0: i32, x1: i32, y1: i32) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;
    loop {
        if cx >= 0 && cy >= 0 && (cx as usize) < w && (cy as usize) < h {
            pixels[cy as usize * w + cx as usize] = true;
        }
        if cx == x1 && cy == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { if cx == x1 { break; } err += dy; cx += sx; }
        if e2 <= dx { if cy == y1 { break; } err += dx; cy += sy; }
    }
}

// === RSG Scale Block Builder ===

fn build_rsg_scale_line(label: &str, scale: &str, text: &str, level: i32) -> Line<'static> {
    use solardash::data::ScaleSeverity;
    let color = get_severity_color(ScaleSeverity::from_level(level));
    let clamped = level.clamp(0, 5) as usize;

    // Build a visually prominent scale indicator
    let filled = "\u{2588}".repeat(clamped);  // Full block chars
    let empty = "\u{2591}".repeat(5 - clamped); // Light shade chars

    Line::from(vec![
        Span::styled(
            format!(" {} ", label),
            Style::default().fg(colors::TEXT_PRIMARY).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            filled,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            empty,
            Style::default().fg(colors::BORDER_DIM),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{}", scale),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", text),
            Style::default().fg(colors::TEXT_SECONDARY),
        ),
    ])
}

fn build_forecast_cell(scale: &str, level: i32) -> Vec<Span<'static>> {
    use solardash::data::ScaleSeverity;
    let color = get_severity_color(ScaleSeverity::from_level(level));
    vec![
        Span::styled(
            format!(" {:<3}", scale),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]
}

// === Kp Gauge Renderer ===

fn render_kp_gauge(kp_value: f64) -> Vec<Line<'static>> {
    let kp_clamped = kp_value.clamp(0.0, 9.0);

    let kp_color = match kp_clamped as usize {
        0..=3 => colors::KP_LOW,
        4..=5 => colors::KP_MEDIUM,
        6..=7 => colors::KP_HIGH,
        _ => colors::KP_EXTREME,
    };

    let kp_label = match kp_clamped as usize {
        0 => "Quiet",
        1 => "Quiet",
        2 => "Unsettled",
        3 => "Unsettled",
        4 => "Active",
        5 => "Minor Storm",
        6 => "Moderate Storm",
        7 => "Strong Storm",
        8 => "Severe Storm",
        9 => "Extreme Storm",
        _ => "",
    };

    // Proportional gauge: 28 chars spanning Kp 0-9 (aligned with label positions)
    // Label: " 0  1  2  3  4  5  6  7  8  9" (29 chars)
    // Bar:   " " + 28 gauge chars = 29 chars
    let bar_width: usize = 28;
    let fill_exact = kp_clamped * bar_width as f64 / 9.0;
    let full_chars = fill_exact.floor() as usize;
    let partial = fill_exact - full_chars as f64;

    let mut bar_spans = vec![Span::raw(" ")];
    for i in 0..bar_width {
        // Color based on Kp region at this position
        let kp_at = ((i as f64 + 0.5) / bar_width as f64 * 9.0) as usize;
        let seg_color = match kp_at {
            0..=3 => colors::KP_LOW,
            4..=5 => colors::KP_MEDIUM,
            6..=7 => colors::KP_HIGH,
            _ => colors::KP_EXTREME,
        };

        if i < full_chars {
            bar_spans.push(Span::styled(
                "\u{2588}",
                Style::default().fg(seg_color),
            ));
        } else if i == full_chars && partial > 0.125 {
            // Partial fill using left-aligned block elements
            let partial_char = if partial > 0.875 {
                "\u{2589}" // 7/8
            } else if partial > 0.75 {
                "\u{258A}" // 3/4
            } else if partial > 0.625 {
                "\u{258B}" // 5/8
            } else if partial > 0.5 {
                "\u{258C}" // 1/2
            } else if partial > 0.375 {
                "\u{258D}" // 3/8
            } else if partial > 0.25 {
                "\u{258E}" // 1/4
            } else {
                "\u{258F}" // 1/8
            };
            bar_spans.push(Span::styled(
                partial_char,
                Style::default().fg(seg_color),
            ));
        } else {
            bar_spans.push(Span::styled(
                "\u{2591}",
                Style::default().fg(colors::BORDER_DIM),
            ));
        }
    }

    vec![
        Line::from(vec![
            Span::styled(
                format!(" {:.1} ", kp_value),
                Style::default().fg(kp_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                kp_label,
                Style::default().fg(colors::TEXT_SECONDARY),
            ),
        ]),
        Line::from(bar_spans),
        Line::from(vec![
            Span::styled(
                " 0  1  2  3  4  5  6  7  8  9",
                Style::default().fg(colors::GRAPH_LABEL),
            ),
        ]),
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_client = api::NoaaClient::new()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, api_client).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    api_client: api::NoaaClient,
) -> io::Result<()> {
    let mut dashboard_data = DashboardData::new();
    let mut alert_state = solardash::alerts::AlertState::new();
    let mut last_refresh = std::time::Instant::now();
    let mut refresh_requested = true;
    let mut show_info = false;
    let mut show_launch_lines = false;
    let mut audio_alerts = false;
    let mut last_launch_refresh = std::time::Instant::now()
        .checked_sub(LAUNCH_REFRESH_INTERVAL)
        .unwrap_or(std::time::Instant::now());

    loop {
        if last_refresh.elapsed() >= AUTO_REFRESH_INTERVAL {
            refresh_requested = true;
        }

        if refresh_requested {
            refresh_requested = false;
            let saved_launch = dashboard_data.upcoming_launch.take();
            match api_client.fetch_all_data().await {
                Ok(mut data) => {
                    data.upcoming_launch = saved_launch;
                    dashboard_data = data;
                    alert_state.check_and_play(&dashboard_data, audio_alerts);
                    last_refresh = std::time::Instant::now();
                }
                Err(e) => {
                    eprintln!("Error fetching data: {:?}", e);
                    dashboard_data.upcoming_launch = saved_launch;
                    last_refresh = std::time::Instant::now();
                }
            }
        }

        if last_launch_refresh.elapsed() >= LAUNCH_REFRESH_INTERVAL {
            match api_client.get_upcoming_launch().await {
                Ok(launch) => dashboard_data.upcoming_launch = launch,
                Err(e) => eprintln!("Warning: Failed to fetch upcoming launch: {:?}", e),
            }
            last_launch_refresh = std::time::Instant::now();
        }

        terminal.draw(|f| {
            let size = f.area();

            if size.width < 120 || size.height < 30 {
                // Portrait-ish terminals (phone SSH clients, vertically split
                // panes) need a different hint than merely-small ones
                let portrait = size.width < size.height * 2;
                let advice = if portrait {
                    "SolarDash needs a wide (landscape) terminal."
                } else {
                    "Enlarge the terminal window or reduce the font size."
                };
                let hint = if portrait {
                    "Rotate the display, widen the window, or close vertical splits."
                } else {
                    "On most terminals: Ctrl+- (or Cmd+-) shrinks the font."
                };
                let warning_text = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "TERMINAL TOO SMALL",
                        Style::default().fg(colors::SEVERITY_SEVERE).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("Current: {}x{} | Required: 120x30", size.width, size.height),
                        Style::default().fg(colors::TEXT_SECONDARY),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(advice, Style::default().fg(colors::TEXT_PRIMARY))),
                    Line::from(Span::styled(hint, Style::default().fg(colors::TEXT_SECONDARY))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press q to quit.",
                        Style::default().fg(colors::TEXT_DIM),
                    )),
                ];
                let w = Paragraph::new(warning_text)
                    .alignment(Alignment::Center)
                    .wrap(ratatui::widgets::Wrap { trim: true })
                    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::SEVERITY_SEVERE)));
                f.render_widget(w, size);
                return;
            }

            // === MAIN LAYOUT ===
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Length(7),  // NOAA Storm Scales + 3-Day Forecast
                    Constraint::Min(0),    // Content (3-panel)
                    Constraint::Length(1), // Footer
                ])
                .split(size);

            render_header(f, main_chunks[0], &dashboard_data);
            render_storm_scales(f, main_chunks[1], &dashboard_data);
            render_content(f, main_chunks[2], &dashboard_data);
            render_footer(f, main_chunks[3], last_refresh, &dashboard_data, show_info, show_launch_lines, audio_alerts);
            if show_launch_lines {
                render_launch_lines(f, size);
            }
            // Launch site markers rendered last so they overwrite any braille lines
            render_launch_markers(f, size);
            if show_info {
                render_info_overlay(f, size);
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('r') | KeyCode::Char('R') => refresh_requested = true,
                    KeyCode::Char('i') | KeyCode::Char('I') => show_info = !show_info,
                    KeyCode::Char('l') | KeyCode::Char('L') => show_launch_lines = !show_launch_lines,
                    KeyCode::Char('a') | KeyCode::Char('A') => audio_alerts = !audio_alerts,
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

// === HEADER SECTION ===

fn render_header(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),
            Constraint::Min(0),
            Constraint::Length(22),
        ])
        .split(area);

    let now_utc = Utc::now();
    let now_local = Local::now();

    // Left: Last Update
    let update_str = if let Some(t) = data.last_update {
        format!("{}", t.format("%Y-%m-%d %H:%M"))
    } else {
        "Never".to_string()
    };

    let left = Paragraph::new(vec![
        Line::from(Span::styled(
            format!(" {}", update_str),
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors::BORDER_DIM))
            .title(Span::styled(" Last Update ", Style::default().fg(colors::LAST_UPDATE))),
    );
    f.render_widget(left, chunks[0]);

    // Center: SPACE WEATHER (left third) | UTC clock (center) | DASHBOARD (right third)
    let center_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors::BORDER_HIGHLIGHT));
    let center_inner = center_block.inner(chunks[1]);
    f.render_widget(center_block, chunks[1]);

    let third = center_inner.width / 3;
    let center_third = center_inner.width - third * 2;

    // Left third: "SPACE WEATHER"
    f.render_widget(
        Paragraph::new(Span::styled(
            "SPACE WEATHER",
            Style::default().fg(colors::TITLE).add_modifier(Modifier::BOLD),
        )).alignment(Alignment::Center),
        Rect::new(center_inner.x, center_inner.y, third, 1),
    );

    // Center third: UTC clock + date
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{}", now_utc.format("%H:%M:%S")),
                Style::default().fg(colors::UTC_TIME).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " UTC ",
                Style::default().fg(colors::TEXT_SECONDARY),
            ),
            Span::styled(
                now_utc.format("%Y-%m-%d").to_string(),
                Style::default().fg(colors::TEXT_DIM),
            ),
        ])).alignment(Alignment::Center),
        Rect::new(center_inner.x + third, center_inner.y, center_third, 1),
    );

    // Right third: "DASHBOARD"
    f.render_widget(
        Paragraph::new(Span::styled(
            "DASHBOARD",
            Style::default().fg(colors::TITLE).add_modifier(Modifier::BOLD),
        )).alignment(Alignment::Center),
        Rect::new(center_inner.x + third + center_third, center_inner.y, third, 1),
    );

    // Right: Local Time with timezone abbreviation after the clock
    let tz_abbrev = get_local_tz_abbrev();
    let right = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                format!("{}", now_local.format("%H:%M:%S")),
                Style::default().fg(colors::LOCAL_TIME).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} ", tz_abbrev),
                Style::default().fg(colors::TEXT_SECONDARY),
            ),
        ]),
    ])
    .alignment(Alignment::Right)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors::BORDER_DIM))
            .title(Span::styled(" Local ", Style::default().fg(colors::LOCAL_TIME))),
    );
    f.render_widget(right, chunks[2]);
}

// === LAUNCH SITE CLOCKS ===

fn format_utc_offset(offset_raw: &str) -> String {
    if offset_raw.len() < 5 { return "UTC".to_string(); }
    let sign: i32 = if offset_raw.starts_with('-') { -1 } else { 1 };
    let h: i32 = offset_raw[1..3].parse().unwrap_or(0);
    let m: i32 = offset_raw[3..5].parse().unwrap_or(0);
    if m > 0 {
        format!("UTC{:+}:{:02}", sign * h, m)
    } else {
        format!("UTC{:+}", sign * h)
    }
}

fn render_launch_clocks(f: &mut ratatui::Frame, area: Rect) {
    let now_utc = Utc::now();

    let sites: &[(&str, &str, &str)] = &[
        ("Vandenberg SFB",      "California",    "America/Los_Angeles"),
        ("Starbase",             "Texas",         "America/Chicago"),
        ("Cape Canaveral",       "Florida",       "America/New_York"),
        ("Guiana Space Centre",  "French Guiana", "America/Cayenne"),
        ("Baikonur Cosmodrome",  "Kazakhstan",    "Asia/Qyzylorda"),
        ("Jiuquan Launch Ctr",   "China",         "Asia/Shanghai"),
        ("Tanegashima SC",       "Japan",         "Asia/Tokyo"),
    ];

    let num = sites.len() as u16;
    if area.width < num * 2 || area.height < 4 {
        return;
    }

    let col_w = area.width / num;

    // Center 4 content rows within available height
    let content_rows: u16 = 4;
    let y_offset = (area.height.saturating_sub(content_rows)) / 2;

    for (i, &(name, location, tz_str)) in sites.iter().enumerate() {
        let x = area.x + i as u16 * col_w;
        let w = if i as u16 == num - 1 {
            area.width - i as u16 * col_w
        } else {
            col_w
        };

        if let Ok(tz) = tz_str.parse::<Tz>() {
            let local = now_utc.with_timezone(&tz);
            let time_str = local.format("%H:%M:%S").to_string();
            let abbrev = local.format("%Z").to_string();
            let offset_raw = local.format("%z").to_string();
            let utc_str = format_utc_offset(&offset_raw);
            // If abbreviation is just a numeric offset (e.g. "+05"), show only "UTC+5"
            let tz_display = if abbrev.starts_with('+') || abbrev.starts_with('-') {
                utc_str
            } else {
                format!("{} ({})", abbrev, utc_str)
            };

            let base_y = area.y + y_offset;

            // Row 0: Site name (Group A - prominent)
            f.render_widget(
                Paragraph::new(Span::styled(
                    name,
                    Style::default().fg(colors::LAUNCH_NAME),
                )).alignment(Alignment::Center),
                Rect::new(x, base_y, w, 1),
            );
            // Row 1: Local time (Group A - brightest)
            f.render_widget(
                Paragraph::new(Span::styled(
                    time_str,
                    Style::default().fg(colors::LAUNCH_TIME).add_modifier(Modifier::BOLD),
                )).alignment(Alignment::Center),
                Rect::new(x, base_y + 1, w, 1),
            );
            // Row 2: Location (Group B - muted)
            f.render_widget(
                Paragraph::new(Span::styled(
                    location,
                    Style::default().fg(colors::LAUNCH_LOCATION),
                )).alignment(Alignment::Center),
                Rect::new(x, base_y + 2, w, 1),
            );
            // Row 3: Timezone (Group B - subtle)
            if base_y + 3 < area.y + area.height {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        tz_display,
                        Style::default().fg(colors::LAUNCH_TIMEZONE),
                    )).alignment(Alignment::Center),
                    Rect::new(x, base_y + 3, w, 1),
                );
            }
        }
    }
}

// === NOAA STORM SCALES ===

fn render_storm_scales(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    use solardash::data::parse_scale_level;

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28), // Current scales (aligned with left panel)
            Constraint::Min(0),    // Center (above map)
            Constraint::Length(32), // 3-Day Forecast matrix (aligned with right panel)
        ])
        .split(area);

    // Current R/S/G levels
    let r_level = parse_scale_level(&data.noaa_scales.radio_blackout.scale);
    let s_level = parse_scale_level(&data.noaa_scales.solar_radiation.scale);
    let g_level = parse_scale_level(&data.noaa_scales.geomagnetic_storm.scale);

    let r_line = build_rsg_scale_line("R", &data.noaa_scales.radio_blackout.scale, &data.noaa_scales.radio_blackout.text, r_level);
    let s_line = build_rsg_scale_line("S", &data.noaa_scales.solar_radiation.scale, &data.noaa_scales.solar_radiation.text, s_level);
    let g_line = build_rsg_scale_line("G", &data.noaa_scales.geomagnetic_storm.scale, &data.noaa_scales.geomagnetic_storm.text, g_level);

    let scales_widget = Paragraph::new(vec![
            Line::from(Span::styled(
                " Latest Observed",
                Style::default().fg(colors::TEXT_SECONDARY),
            )),
            r_line, s_line, g_line,
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::BORDER))
                .title(Span::styled(
                    " NOAA SWPC Storm Scales ",
                    Style::default().fg(colors::TITLE).add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(scales_widget, chunks[0]);

    // Center block with launch site clocks
    let center_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors::BORDER));
    let center_inner = center_block.inner(chunks[1]);
    f.render_widget(center_block, chunks[1]);
    render_launch_clocks(f, center_inner);

    // 3-Day Forecast Matrix (Radio/Solar/Geo all fit now)
    let day1_r = parse_scale_level(&data.noaa_scales.forecast_day1.radio_blackout.scale);
    let day1_s = parse_scale_level(&data.noaa_scales.forecast_day1.solar_radiation.scale);
    let day1_g = parse_scale_level(&data.noaa_scales.forecast_day1.geomagnetic_storm.scale);
    let day2_r = parse_scale_level(&data.noaa_scales.forecast_day2.radio_blackout.scale);
    let day2_s = parse_scale_level(&data.noaa_scales.forecast_day2.solar_radiation.scale);
    let day2_g = parse_scale_level(&data.noaa_scales.forecast_day2.geomagnetic_storm.scale);
    let day3_r = parse_scale_level(&data.noaa_scales.forecast_day3.radio_blackout.scale);
    let day3_s = parse_scale_level(&data.noaa_scales.forecast_day3.solar_radiation.scale);
    let day3_g = parse_scale_level(&data.noaa_scales.forecast_day3.geomagnetic_storm.scale);

    let header = Line::from(vec![
        Span::styled("        ", Style::default()),
        Span::styled("+1d ", Style::default().fg(colors::TEXT_SECONDARY)),
        Span::styled("+2d ", Style::default().fg(colors::TEXT_SECONDARY)),
        Span::styled("+3d", Style::default().fg(colors::TEXT_SECONDARY)),
    ]);

    let mut r_row = vec![Span::styled(" Radio: ", Style::default().fg(colors::TEXT_SECONDARY))];
    r_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day1.radio_blackout.scale, day1_r));
    r_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day2.radio_blackout.scale, day2_r));
    r_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day3.radio_blackout.scale, day3_r));

    let mut s_row = vec![Span::styled(" Solar: ", Style::default().fg(colors::TEXT_SECONDARY))];
    s_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day1.solar_radiation.scale, day1_s));
    s_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day2.solar_radiation.scale, day2_s));
    s_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day3.solar_radiation.scale, day3_s));

    let mut g_row = vec![Span::styled(" Geo:   ", Style::default().fg(colors::TEXT_SECONDARY))];
    g_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day1.geomagnetic_storm.scale, day1_g));
    g_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day2.geomagnetic_storm.scale, day2_g));
    g_row.extend(build_forecast_cell(&data.noaa_scales.forecast_day3.geomagnetic_storm.scale, day3_g));

    let forecast = Paragraph::new(vec![header, Line::from(r_row), Line::from(s_row), Line::from(g_row)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::BORDER))
                .title(Span::styled(
                    " 3-Day Forecast ",
                    Style::default().fg(colors::FORECAST).add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(forecast, chunks[2]);
}

// === MAIN CONTENT (3 panels) ===

fn render_content(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),  // Left: Solar Wind
            Constraint::Min(0),     // Center: World Map
            Constraint::Length(32),  // Right: Events/Forecasts
        ])
        .split(area);

    render_solar_wind_panel(f, chunks[0], data);
    render_world_map(f, chunks[1], data);
    render_right_panel(f, chunks[2], data);
}

// === LEFT PANEL: Solar Wind ===

fn render_solar_wind_panel(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let panel_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let solar_wind_6h = data.solar_wind.get_last_hours(6);
    let mag = data.solar_wind.get_current_magnetic();
    let plasma = data.solar_wind.get_current_plasma();

    let current_bt = mag.map(|m| m.bt).unwrap_or(0.0);
    let current_bz = mag.map(|m| m.bz_gsm).unwrap_or(0.0);
    let current_speed = plasma.map(|p| p.speed).unwrap_or(0.0);
    let current_density = plasma.map(|p| p.density).unwrap_or(0.0);

    // Bt Graph
    let bt_vals: Vec<f64> = solar_wind_6h.magnetic.iter().rev().take(60).rev().map(|m| m.bt).collect();
    render_solar_param(f, panel_chunks[0], "Bt", &format!("{:.1} nT", current_bt),
        &bt_vals, 0.0, 30.0, colors::SOLAR_WIND_BT, "0", "15", "30", false, "-6", "-3", "now");

    // Bz Graph (uses special green/red polarity rendering)
    let bz_vals: Vec<f64> = solar_wind_6h.magnetic.iter().rev().take(60).rev().map(|m| m.bz_gsm).collect();
    let bz_color = if current_bz < 0.0 { colors::SOLAR_WIND_BZ_NEGATIVE } else { colors::SOLAR_WIND_BZ_POSITIVE };
    render_solar_param(f, panel_chunks[1], "Bz", &format!("{:+.1} nT", current_bz),
        &bz_vals, -20.0, 20.0, bz_color, "-20", "0", "+20", true, "-6", "-3", "now");

    // Speed Graph
    let speed_vals: Vec<f64> = solar_wind_6h.plasma.iter().rev().take(60).rev().map(|p| p.speed).collect();
    render_solar_param(f, panel_chunks[2], "V", &format!("{:.0} km/s", current_speed),
        &speed_vals, 200.0, 800.0, colors::SOLAR_WIND_SPEED, "200", "500", "800", false, "-6", "-3", "now");

    // Density Graph
    let density_vals: Vec<f64> = solar_wind_6h.plasma.iter().rev().take(60).rev().map(|p| p.density).collect();
    render_solar_param(f, panel_chunks[3], "n", &format!("{:.1} p/cm\u{b3}", current_density),
        &density_vals, 0.0, 20.0, colors::SOLAR_WIND_DENSITY, "0", "10", "20", false, "-6", "-3", "now");
}

fn render_solar_param(
    f: &mut ratatui::Frame,
    area: Rect,
    label: &str,
    value_str: &str,
    data: &[f64],
    min_v: f64,
    max_v: f64,
    color: Color,
    y_min_label: &str,
    y_mid_label: &str,
    y_max_label: &str,
    bz_mode: bool,
    time_start_label: &str,
    time_mid_label: &str,
    time_end_label: &str,
) {
    let inner = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors::BORDER_DIM))
        .title(Span::styled(
            format!(" {} ", label),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .inner(area);

    // Draw the block border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors::BORDER_DIM))
        .title(Span::styled(
            format!(" {} ", label),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 8 {
        return;
    }

    // Layout: Y-axis labels (4 chars) | Graph area
    let y_axis_width = 4;
    let graph_width = (inner.width as usize).saturating_sub(y_axis_width + 1);
    let graph_height = inner.height.saturating_sub(3) as usize; // 1 row value, 1 row axis line, 1 row time labels

    if graph_width < 2 || graph_height < 1 {
        return;
    }

    // Auto-scale if data exceeds default range
    let data_min = data.iter().cloned().fold(f64::INFINITY, f64::min);
    let data_max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let actual_min = if data_min < min_v && data_min.is_finite() { data_min } else { min_v };
    let actual_max = if data_max > max_v && data_max.is_finite() { data_max } else { max_v };

    // Current value line (top)
    let value_line = Line::from(vec![
        Span::styled(
            format!(" {}", value_str),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(
        Paragraph::new(value_line),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    // Y-axis labels
    let graph_area_y = inner.y + 1;
    if graph_height >= 3 {
        // Top label
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("{:>3}", y_max_label),
                Style::default().fg(colors::GRAPH_LABEL),
            )).alignment(Alignment::Right),
            Rect::new(inner.x, graph_area_y, y_axis_width as u16, 1),
        );
        // Mid label - compute actual position based on value in the graph
        let mid_val = (min_v + max_v) / 2.0;
        let range = actual_max - actual_min;
        let mid_normalized = if range.abs() > 0.001 {
            (mid_val - actual_min) / range
        } else {
            0.5
        };
        let mid_pixel_y = ((1.0 - mid_normalized) * (graph_height as f64 * 4.0 - 1.0)) as usize;
        let mid_char_row = (mid_pixel_y / 4).min(graph_height.saturating_sub(1));
        let mid_y = graph_area_y + mid_char_row as u16;
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("{:>3}", y_mid_label),
                Style::default().fg(colors::GRAPH_LABEL),
            )).alignment(Alignment::Right),
            Rect::new(inner.x, mid_y, y_axis_width as u16, 1),
        );
        // Bottom label
        let bot_y = graph_area_y + graph_height as u16 - 1;
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("{:>3}", y_min_label),
                Style::default().fg(colors::GRAPH_LABEL),
            )).alignment(Alignment::Right),
            Rect::new(inner.x, bot_y, y_axis_width as u16, 1),
        );
    }

    // Darker background for graph area
    let graph_rect = Rect::new(
        inner.x + y_axis_width as u16 + 1,
        graph_area_y,
        graph_width as u16,
        graph_height as u16,
    );
    let bg_block = Block::default().style(Style::default().bg(Color::Rgb(8, 18, 28)));
    f.render_widget(bg_block, graph_rect);

    // Draw left axis line (thin vertical line)
    let axis_x = inner.x + y_axis_width as u16;
    for row in 0..graph_height {
        f.render_widget(
            Paragraph::new(Span::styled(
                "\u{2502}",
                Style::default().fg(colors::GRAPH_AXIS),
            )),
            Rect::new(axis_x, graph_area_y + row as u16, 1, 1),
        );
    }
    // Draw bottom axis line with corner and tick marks
    let bottom_y = graph_area_y + graph_height as u16;
    if bottom_y < inner.y + inner.height {
        // Corner piece
        f.render_widget(
            Paragraph::new(Span::styled(
                "\u{2514}",
                Style::default().fg(colors::GRAPH_AXIS),
            )),
            Rect::new(axis_x, bottom_y, 1, 1),
        );

        // Build bottom axis with prominent ticks at -6h, -3h, now and small 1h ticks
        let gw = graph_width;
        let last = gw.saturating_sub(1) as f64;

        // Compute all 7 tick positions on a uniform grid: -6h, -5h, ..., -1h, now
        let all_ticks: Vec<usize> = (0..=6)
            .map(|i| (last * i as f64 / 6.0).round() as usize)
            .collect();

        // Prominent: -6h (index 0), -3h (index 3), now (index 6)
        let prominent = [all_ticks[0], all_ticks[3], all_ticks[6]];

        let mut axis_chars: Vec<char> = vec!['\u{2500}'; gw]; // ─
        for &pos in &all_ticks {
            if pos < gw {
                if prominent.contains(&pos) {
                    axis_chars[pos] = '\u{253C}'; // ┼ (prominent: axis crosses through middle)
                } else {
                    axis_chars[pos] = '\u{252C}'; // ┬ (small: tick extending down)
                }
            }
        }

        // Render axis line
        let axis_str: String = axis_chars.iter().collect();
        f.render_widget(
            Paragraph::new(Span::styled(axis_str, Style::default().fg(colors::GRAPH_AXIS))),
            Rect::new(axis_x + 1, bottom_y, graph_width as u16, 1),
        );

        // Render time labels on the row below, centered on their tick marks
        let label_y = bottom_y + 1;
        if label_y < inner.y + inner.height {
            let labels: Vec<(usize, &str)> = if gw >= 12 {
                vec![
                    (all_ticks[0], time_start_label),
                    (all_ticks[3], time_mid_label),
                    (all_ticks[6], time_end_label),
                ]
            } else {
                vec![
                    (all_ticks[0], time_start_label),
                    (all_ticks[6], time_end_label),
                ]
            };

            let mut label_line = vec![' '; gw];
            for &(tick_pos, text) in &labels {
                let len = text.len();
                // Center label under its tick mark, clamped to bounds
                let start = tick_pos.saturating_sub(len / 2);
                let start = if start + len > gw { gw.saturating_sub(len) } else { start };
                for (j, ch) in text.chars().enumerate() {
                    let p = start + j;
                    if p < gw {
                        label_line[p] = ch;
                    }
                }
            }
            let label_str: String = label_line.iter().collect();
            f.render_widget(
                Paragraph::new(Span::styled(label_str, Style::default().fg(colors::GRAPH_LABEL))),
                Rect::new(axis_x + 1, label_y, graph_width as u16, 1),
            );
        }
    }

    // Render braille graph
    let graph_lines = if bz_mode {
        render_bz_braille_graph(data, actual_min, actual_max, graph_width, graph_height)
    } else {
        render_braille_graph(data, actual_min, actual_max, graph_width, graph_height, color)
    };
    let graph_widget = Paragraph::new(graph_lines);
    f.render_widget(
        graph_widget,
        graph_rect,
    );
}

// === CENTER PANEL: World Map ===

fn render_world_map(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let kp_value = data.kp_index.get_current_value();
    let aurora_label = match data.aurora_boundary.source {
        AuroraSource::Ovation => match data.aurora_boundary.forecast_time {
            Some(t) => format!("Aurora OVATION {}", t.format("%H:%MZ")),
            None => "Aurora OVATION".to_string(),
        },
        AuroraSource::KpModel => format!("Aurora Kp={:.1}", kp_value),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors::BORDER))
        .title(Span::styled(
            format!(" World Map \u{2502} {} \u{2502} Day/Night Terminator ", aurora_label),
            Style::default().fg(colors::AURORA).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(
            Line::from(vec![
                Span::styled(" aurora ", Style::default().fg(colors::TEXT_SECONDARY)),
                Span::styled("\u{25cf}", Style::default().fg(Color::Rgb(0, 220, 90))),
                Span::styled("\u{25cf}", Style::default().fg(Color::Rgb(240, 210, 0))),
                Span::styled("\u{25cf}", Style::default().fg(Color::Rgb(255, 50, 40))),
                Span::styled(" low\u{2192}high ", Style::default().fg(colors::TEXT_SECONDARY)),
            ])
            .right_aligned(),
        );

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 16 || inner.height < 7 {
        return;
    }

    // Reserve space for geomag lat scale (left) and UTC offset axis (bottom)
    let lat_label_width: u16 = 5; // e.g. "62\u{b0}N"
    let time_axis_height: u16 = 1;

    let map_width = inner.width.saturating_sub(lat_label_width);
    let map_height = inner.height.saturating_sub(time_axis_height);

    if map_width < 10 || map_height < 5 {
        return;
    }

    // Render the map
    let world_map = WorldMap::new();
    let map_lines = world_map.render_to_size(map_width, map_height, Some(&data.aurora_boundary));

    let map_widget = Paragraph::new(map_lines);
    f.render_widget(
        map_widget,
        Rect::new(inner.x + lat_label_width, inner.y, map_width, map_height),
    );

    // Geographic latitude scale on the left, aligned with grid lines every 10°
    for lat_deg in (-80i32..=80).step_by(10) {
        let y_frac = (90.0 - lat_deg as f64) / 180.0;
        let y_pos = (y_frac * map_height as f64) as u16;
        if y_pos >= map_height { continue; }

        let is_major = lat_deg % 30 == 0;
        if is_major {
            let label = if lat_deg == 0 {
                " 0\u{b0} ".to_string()
            } else {
                let dir = if lat_deg > 0 { "N" } else { "S" };
                format!("{:>2}\u{b0}{}", lat_deg.abs(), dir)
            };
            f.render_widget(
                Paragraph::new(Span::styled(
                    label,
                    Style::default().fg(colors::MAP_AXIS_LABEL),
                )),
                Rect::new(inner.x, inner.y + y_pos, lat_label_width, 1),
            );
        } else {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "  \u{2500}\u{2500}",
                    Style::default().fg(colors::MAP_AXIS_LABEL),
                )),
                Rect::new(inner.x, inner.y + y_pos, lat_label_width, 1),
            );
        }
    }

    // UTC offset axis on the bottom
    let utc_y = inner.y + map_height;
    let offsets: &[i32] = &[-12, -9, -6, -3, 0, 3, 6, 9, 12];
    for &offset in offsets {
        // longitude = offset * 15
        let lon = offset as f64 * 15.0;
        let x_frac = (lon + 180.0) / 360.0;
        let x_pos = (x_frac * map_width as f64) as u16;

        if x_pos < map_width {
            let label = if offset == 0 {
                "UTC".to_string()
            } else if offset > 0 {
                format!("+{}", offset)
            } else {
                format!("{}", offset)
            };
            let label_len = label.len() as u16;
            let draw_x = (inner.x + lat_label_width + x_pos).saturating_sub(label_len / 2);
            let max_x = inner.x + inner.width;
            if draw_x + label_len <= max_x {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        label,
                        Style::default().fg(colors::MAP_AXIS_LABEL),
                    )),
                    Rect::new(draw_x, utc_y, label_len, 1),
                );
            }
        }
    }

}

// === RIGHT PANEL: Events & Forecasts ===

fn render_right_panel(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let panel_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Kp Index
            Constraint::Length(5),  // X-Ray Flux
            Constraint::Length(8),  // Latest Flare Event
            Constraint::Length(10), // Dst Graph
            Constraint::Length(10), // Band Conditions
            Constraint::Min(0),    // Upcoming Launch
        ])
        .split(area);

    render_kp_index(f, panel_chunks[0], data);
    render_xray_flux(f, panel_chunks[1], data);
    render_flare_event(f, panel_chunks[2], data);
    render_dst_graph(f, panel_chunks[3], data);
    render_band_conditions(f, panel_chunks[4], data);
    render_upcoming_launch(f, panel_chunks[5], data);
}

fn render_kp_index(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let kp_value = data.kp_index.get_current_value();
    let gauge_lines = render_kp_gauge(kp_value);

    let widget = Paragraph::new(gauge_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::BORDER))
                .title(Span::styled(
                    " Kp Index ",
                    Style::default().fg(colors::FORECAST).add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(widget, area);
}

fn render_xray_flux(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let flare = data.flares.get_latest();
    let class = flare.map(|f| f.class_type.as_str()).unwrap_or("A0.0");
    let class_letter = class.chars().next().unwrap_or('A');

    let xray_color = match class_letter {
        'X' => colors::FLARE_X,
        'M' => colors::FLARE_M,
        'C' => colors::FLARE_C,
        'B' => colors::FLARE_B,
        _ => colors::FLARE_A,
    };

    let class_desc = match class_letter {
        'X' => "Extreme",
        'M' => "Strong",
        'C' => "Moderate",
        'B' => "Low",
        _ => "Minimal",
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(" Level: ", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled(
                class.to_string(),
                Style::default().fg(xray_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Class: ", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled(
                class_desc,
                Style::default().fg(xray_color),
            ),
        ]),
    ];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::BORDER_DIM))
                .title(Span::styled(
                    " X-Ray Flux ",
                    Style::default().fg(colors::FORECAST_SECONDARY).add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(widget, area);
}

fn render_flare_event(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let flare = data.flares.get_latest();

    let lines = if let Some(fl) = flare {
        let start_date = fl.begin_time.split('T').next().unwrap_or("--");
        let start_time = fl.begin_time.split('T').nth(1)
            .and_then(|t| t.split('.').next())
            .unwrap_or("--:--:--");

        let end_str = if fl.is_ongoing() {
            "Ongoing".to_string()
        } else {
            fl.end_time.as_ref()
                .and_then(|t| t.split('T').nth(1))
                .and_then(|t| t.split('.').next())
                .unwrap_or("--:--:--")
                .to_string()
        };

        let peak_str = fl.peak_time.as_ref()
            .and_then(|t| t.split('T').nth(1))
            .and_then(|t| t.split('.').next())
            .unwrap_or("--:--:--")
            .to_string();

        let max_color = match fl.class_letter() {
            'X' => colors::FLARE_X,
            'M' => colors::FLARE_M,
            'C' => colors::FLARE_C,
            'B' => colors::FLARE_B,
            _ => colors::FLARE_A,
        };

        let status_color = if fl.is_ongoing() { colors::STATUS_ONLINE } else { colors::TEXT_DIM };

        vec![
            Line::from(vec![
                Span::styled(" Start: ", Style::default().fg(colors::TEXT_SECONDARY)),
                Span::styled(format!("{} {}", start_date, start_time), Style::default().fg(colors::TEXT_PRIMARY)),
            ]),
            Line::from(vec![
                Span::styled(" Peak:  ", Style::default().fg(colors::TEXT_SECONDARY)),
                Span::styled(peak_str, Style::default().fg(colors::TEXT_PRIMARY)),
            ]),
            Line::from(vec![
                Span::styled(" End:   ", Style::default().fg(colors::TEXT_SECONDARY)),
                Span::styled(end_str, Style::default().fg(status_color)),
            ]),
            Line::from(vec![
                Span::styled(" Class: ", Style::default().fg(colors::TEXT_SECONDARY)),
                Span::styled(
                    fl.class_type.clone(),
                    Style::default().fg(max_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  (min: {})", fl.begin_class.as_deref().unwrap_or("--")),
                    Style::default().fg(colors::TEXT_DIM),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Region: ", Style::default().fg(colors::TEXT_SECONDARY)),
                Span::styled(
                    fl.active_region.map(|r| format!("AR{}", r)).unwrap_or_else(|| "--".to_string()),
                    Style::default().fg(colors::TEXT_PRIMARY),
                ),
                Span::raw("  "),
                Span::styled(
                    fl.source_location.as_deref().unwrap_or(""),
                    Style::default().fg(colors::TEXT_DIM),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(Span::styled(" No recent flare events", Style::default().fg(colors::TEXT_DIM))),
        ]
    };

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::BORDER_DIM))
                .title(Span::styled(
                    " Latest Flare Event ",
                    Style::default().fg(colors::FLARE_M).add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(widget, area);
}

fn render_dst_graph(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let dst_72h = data.dst.get_last_hours(72);
    let dst_vals: Vec<f64> = dst_72h.iter().map(|m| m.dst).collect();
    let current_dst = data.dst.get_current_value();

    let value_color = if current_dst < 0.0 {
        colors::SOLAR_WIND_BZ_NEGATIVE
    } else {
        colors::SOLAR_WIND_BZ_POSITIVE
    };

    render_solar_param(
        f, area, "Dst", &format!("{:+.0} nT", current_dst),
        &dst_vals, -100.0, 50.0, value_color,
        "-100", "-25", "+50", true,
        "-3d", "-1d", "now",
    );

    // Override the title color to DST_INDEX (coral-orange)
    // Re-render just the block border with the correct title color
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors::BORDER_DIM))
        .title(Span::styled(
            " Dst ",
            Style::default().fg(colors::DST_INDEX).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(block, area);
}

fn render_band_conditions(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    use solardash::data::BandQuality;

    let conditions = data.get_band_conditions();
    let sfi = if data.solar_flux > 0.0 { data.solar_flux } else { 0.0 };
    let kp = data.kp_index.get_current_value();

    let quality_color = |q: &BandQuality| -> Color {
        match q {
            BandQuality::Good => colors::BAND_GOOD,
            BandQuality::Fair => colors::BAND_FAIR,
            BandQuality::Poor => colors::BAND_POOR,
        }
    };

    // Build band condition rows: 2 bands per row
    let mut lines: Vec<Line<'static>> = Vec::new();

    let mut i = 0;
    while i < conditions.len() {
        let mut spans = Vec::new();

        // First band in the row
        let c1 = &conditions[i];
        let color1 = quality_color(&c1.quality);
        spans.push(Span::styled(
            format!(" {:>4} ", c1.band),
            Style::default().fg(colors::BAND_LABEL),
        ));
        spans.push(Span::styled(
            format!("{:<4}", c1.quality.label()),
            Style::default().fg(color1).add_modifier(Modifier::BOLD),
        ));

        // Second band in the row (if exists)
        if i + 1 < conditions.len() {
            let c2 = &conditions[i + 1];
            let color2 = quality_color(&c2.quality);
            spans.push(Span::styled(
                format!("  {:>4} ", c2.band),
                Style::default().fg(colors::BAND_LABEL),
            ));
            spans.push(Span::styled(
                format!("{:<4}", c2.quality.label()),
                Style::default().fg(color2).add_modifier(Modifier::BOLD),
            ));
        }

        lines.push(Line::from(spans));
        i += 2;
    }

    // Footer: SFI and Kp summary
    let sfi_str = if sfi > 0.0 {
        format!(" SFI {:.0}", sfi)
    } else {
        " SFI --".to_string()
    };
    lines.push(Line::from(vec![
        Span::styled(
            sfi_str,
            Style::default().fg(colors::TEXT_DIM),
        ),
        Span::styled(
            format!(" \u{2502} Kp {:.1}", kp),
            Style::default().fg(colors::TEXT_DIM),
        ),
    ]));

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::BORDER_DIM))
                .title(Span::styled(
                    " Band Conditions ",
                    Style::default().fg(colors::FORECAST_SECONDARY).add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(widget, area);
}

// === UPCOMING LAUNCH ===

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}..", &s[..max_len.saturating_sub(2)])
    }
}

fn render_upcoming_launch(f: &mut ratatui::Frame, area: Rect, data: &DashboardData) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors::BORDER_DIM))
        .title(Span::styled(
            " Next Launch ",
            Style::default().fg(colors::LAUNCH_NAME).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 3 {
        return;
    }

    let launch = match &data.upcoming_launch {
        Some(l) => l,
        None => {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " No upcoming launches",
                    Style::default().fg(colors::TEXT_DIM),
                )),
                inner,
            );
            return;
        }
    };

    let now = Utc::now();
    let diff = launch.net.signed_duration_since(now);
    let total_secs = diff.num_seconds();

    let max_w = inner.width as usize;

    // Countdown line
    let countdown_line = if total_secs >= 0 {
        let days = total_secs / 86400;
        let hours = (total_secs % 86400) / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;

        if total_secs >= 3600 {
            let countdown_str = format!(" T- {:02}:{:02}:{:02}:{:02}", days, hours, mins, secs);
            Line::from(Span::styled(
                countdown_str,
                Style::default().fg(colors::LAUNCH_TIME).add_modifier(Modifier::BOLD),
            ))
        } else {
            let countdown_str = format!(" T- {:02}:{:02}", mins, secs);
            Line::from(Span::styled(
                countdown_str,
                Style::default().fg(colors::SEVERITY_SEVERE).add_modifier(Modifier::BOLD),
            ))
        }
    } else {
        let abs_secs = total_secs.unsigned_abs();
        let mins = abs_secs / 60;
        let secs = abs_secs % 60;
        let countdown_str = format!(" T+ {:02}:{:02}", mins, secs);
        Line::from(Span::styled(
            countdown_str,
            Style::default().fg(colors::SEVERITY_SEVERE).add_modifier(Modifier::BOLD),
        ))
    };

    let window_str = launch.window_start.format("%Y-%m-%d %H:%M UTC").to_string();
    let label_w = 9; // " Window: " length
    let val_w = max_w.saturating_sub(label_w);

    let mut lines = vec![countdown_line];

    if inner.height >= 4 {
        lines.push(Line::from(vec![
            Span::styled(" Window: ", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled(
                truncate_str(&window_str, val_w),
                Style::default().fg(colors::TEXT_PRIMARY),
            ),
        ]));
    }
    if inner.height >= 5 {
        lines.push(Line::from(vec![
            Span::styled(" Vehicle:", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled(
                truncate_str(&format!(" {}", launch.vehicle), val_w),
                Style::default().fg(colors::TEXT_PRIMARY),
            ),
        ]));
    }
    if inner.height >= 6 {
        lines.push(Line::from(vec![
            Span::styled(" Mission:", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled(
                truncate_str(&format!(" {}", launch.mission), val_w),
                Style::default().fg(colors::TEXT_PRIMARY),
            ),
        ]));
    }
    if inner.height >= 7 {
        lines.push(Line::from(vec![
            Span::styled(" Orbit:  ", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled(
                format!(" {}", launch.orbit),
                Style::default().fg(colors::FORECAST),
            ),
        ]));
    }
    if inner.height >= 8 {
        lines.push(Line::from(vec![
            Span::styled(" Site:   ", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled(
                format!(" {}", launch.site),
                Style::default().fg(colors::LAUNCH_NAME),
            ),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// === LAUNCH SITE CONNECTOR LINES ===

/// Geographic coordinates matching the order in render_launch_clocks
const LAUNCH_SITE_COORDS: &[(&str, f64, f64)] = &[
    ("Vandenberg SFB",       34.58, -120.62),
    ("Starbase",              25.99,  -97.16),
    ("Cape Canaveral",       28.49,  -80.58),
    ("Guiana Space Centre",   5.23,  -52.77),
    ("Baikonur Cosmodrome",  45.92,   63.34),
    ("Jiuquan Launch Ctr",   40.96,  100.29),
    ("Tanegashima SC",       30.40,  130.98),
];

fn render_launch_lines(f: &mut ratatui::Frame, size: Rect) {
    // Reproduce the exact same layout splits as the main render
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    let scale_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),
            Constraint::Min(0),
            Constraint::Length(32),
        ])
        .split(main_chunks[1]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),
            Constraint::Min(0),
            Constraint::Length(32),
        ])
        .split(main_chunks[2]);

    // Clock inner area: center block of storm scales has borders
    let clock_outer = scale_chunks[1];
    let clock_inner = Rect::new(
        clock_outer.x + 1,
        clock_outer.y + 1,
        clock_outer.width.saturating_sub(2),
        clock_outer.height.saturating_sub(2),
    );

    // Map inner area: map block has borders
    let map_outer = content_chunks[1];
    let map_inner = Rect::new(
        map_outer.x + 1,
        map_outer.y + 1,
        map_outer.width.saturating_sub(2),
        map_outer.height.saturating_sub(2),
    );

    // Map coordinate params — must match render_world_map exactly
    let lat_label_w: u16 = 5;
    let time_axis_h: u16 = 1;
    let map_w = map_inner.width.saturating_sub(lat_label_w);
    let map_h = map_inner.height.saturating_sub(time_axis_h);
    let map_x0 = map_inner.x + lat_label_w;
    let map_y0 = map_inner.y;

    let num = LAUNCH_SITE_COORDS.len() as u16;
    if clock_inner.width < num * 2 || map_w < 10 || map_h < 5 {
        return;
    }

    let col_w = clock_inner.width / num;

    // y_offset within clock inner area — same calculation as render_launch_clocks
    let content_rows: u16 = 4;
    let y_offset = (clock_inner.height.saturating_sub(content_rows)) / 2;
    // Line starts at the row just BELOW the UTC+/- timezone line (row 3 = last content row)
    let line_start_y = clock_inner.y + y_offset + content_rows;

    let line_color = Color::Rgb(200, 35, 35);

    for (i, &(_name, lat, lon)) in LAUNCH_SITE_COORDS.iter().enumerate() {
        let col_x = clock_inner.x + i as u16 * col_w;
        let clock_cx = col_x + col_w / 2;

        let x_frac = ((lon + 180.0) / 360.0).clamp(0.0, 1.0);
        let y_frac = ((90.0 - lat) / 180.0).clamp(0.0, 1.0);
        let geo_x = map_x0 + (x_frac * (map_w as f64 - 1.0)).round() as u16;
        let geo_y = map_y0 + (y_frac * (map_h as f64 - 1.0)).round() as u16;

        // Line: from below the timezone row to one row above the marker
        let x0 = clock_cx as i32;
        let y0 = line_start_y as i32;
        let x1 = geo_x as i32;
        let y1 = geo_y as i32 - 1;
        if y1 > y0 {
            draw_braille_line(f, x0, y0, x1, y1, line_color);
        }
    }
}

/// Render launch site `+` markers on the map (called last to overwrite braille lines)
fn render_launch_markers(f: &mut ratatui::Frame, size: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),
            Constraint::Min(0),
            Constraint::Length(32),
        ])
        .split(main_chunks[2]);

    let map_outer = content_chunks[1];
    let map_inner = Rect::new(
        map_outer.x + 1,
        map_outer.y + 1,
        map_outer.width.saturating_sub(2),
        map_outer.height.saturating_sub(2),
    );

    let lat_label_w: u16 = 5;
    let time_axis_h: u16 = 1;
    let map_w = map_inner.width.saturating_sub(lat_label_w);
    let map_h = map_inner.height.saturating_sub(time_axis_h);
    let map_x0 = map_inner.x + lat_label_w;
    let map_y0 = map_inner.y;

    if map_w < 10 || map_h < 5 {
        return;
    }

    for &(_name, lat, lon) in LAUNCH_SITE_COORDS {
        let x_frac = ((lon + 180.0) / 360.0).clamp(0.0, 1.0);
        let y_frac = ((90.0 - lat) / 180.0).clamp(0.0, 1.0);
        let gx = map_x0 + (x_frac * (map_w as f64 - 1.0)).round() as u16;
        let gy = map_y0 + (y_frac * (map_h as f64 - 1.0)).round() as u16;
        if gx < map_x0 + map_w && gy < map_y0 + map_h {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "+",
                    Style::default().fg(colors::LAUNCH_SITE).add_modifier(Modifier::BOLD),
                )),
                Rect::new(gx, gy, 1, 1),
            );
        }
    }
}

fn draw_braille_line(
    f: &mut ratatui::Frame,
    x0: i32, y0: i32,  // start in character-cell coordinates
    x1: i32, y1: i32,  // end   in character-cell coordinates
    color: Color,
) {
    // Scale to braille sub-pixel space: each cell = 2 wide × 4 tall.
    // Use the horizontal centre and vertical middle of each cell as anchor.
    let (bx0, by0) = (x0 * 2 + 1, y0 * 4 + 2);
    let (bx1, by1) = (x1 * 2 + 1, y1 * 4 + 2);

    // Bresenham at braille-pixel resolution
    let adx = (bx1 - bx0).abs();
    let ady = (by1 - by0).abs();
    let sx: i32 = if bx0 < bx1 { 1 } else { -1 };
    let sy: i32 = if by0 < by1 { 1 } else { -1 };
    let mut err = adx - ady;
    let (mut bx, mut by) = (bx0, by0);

    // Accumulate braille bits per character cell
    let mut cells: HashMap<(i32, i32), u32> = HashMap::new();

    loop {
        if bx >= 0 && by >= 0 {
            let cx = bx / 2;
            let cy = by / 4;
            let dx = (bx % 2) as usize;
            let dy = (by % 4) as usize;
            let bit: u32 = match (dx, dy) {
                (0, 0) => 0x01,
                (0, 1) => 0x02,
                (0, 2) => 0x04,
                (0, 3) => 0x40,
                (1, 0) => 0x08,
                (1, 1) => 0x10,
                (1, 2) => 0x20,
                (1, 3) => 0x80,
                _      => 0,
            };
            *cells.entry((cx, cy)).or_insert(0x2800) |= bit;
        }

        if bx == bx1 && by == by1 { break; }
        let e2 = 2 * err;
        if e2 > -ady { err -= ady; bx += sx; }
        if e2 <  adx { err += adx; by += sy; }
    }

    // Render each touched cell once
    for ((cx, cy), code) in cells {
        if cx < 0 || cy < 0 { continue; }
        if let Some(ch) = char::from_u32(code) {
            f.render_widget(
                Paragraph::new(Span::styled(ch.to_string(), Style::default().fg(color))),
                Rect::new(cx as u16, cy as u16, 1, 1),
            );
        }
    }
}

// === INFO OVERLAY ===

fn render_info_overlay(f: &mut ratatui::Frame, size: Rect) {
    // Centered popup: 90% wide, 88% tall
    let popup_w = ((size.width as f32 * 0.90) as u16).min(130).max(80);
    let popup_h = ((size.height as f32 * 0.88) as u16).max(20);
    let popup_x = (size.width.saturating_sub(popup_w)) / 2;
    let popup_y = (size.height.saturating_sub(popup_h)) / 2;
    let area = Rect::new(popup_x, popup_y, popup_w, popup_h);

    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors::BORDER_HIGHLIGHT))
        .title(Span::styled(
            " METRIC GUIDE ",
            Style::default().fg(colors::TITLE).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            " [i] close ",
            Style::default().fg(colors::TEXT_DIM),
        )).alignment(Alignment::Center))
        .style(Style::default().bg(Color::Rgb(6, 11, 18)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Two equal columns
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    // ── LEFT COLUMN: Solar Wind + NOAA Scales ──
    let left_lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            "  SOLAR WIND  (left panel)",
            Style::default().fg(colors::TITLE).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Bt  ", Style::default().fg(colors::SOLAR_WIND_BT).add_modifier(Modifier::BOLD)),
            Span::styled("Total IMF field magnitude.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "        High Bt amplifies any storm when",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "        Bz turns southward.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Bz  ", Style::default().fg(colors::SOLAR_WIND_BZ_NEGATIVE).add_modifier(Modifier::BOLD)),
            Span::styled("N/S orientation of IMF. KEY metric.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "        Negative = southward → opens Earth's",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "        magnetosphere → aurora at mid-latitudes.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "        More negative & longer = stronger storm.",
            Style::default().fg(colors::TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  V   ", Style::default().fg(colors::SOLAR_WIND_SPEED).add_modifier(Modifier::BOLD)),
            Span::styled("Solar wind velocity (km/s).", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "        Higher speed amplifies energy transfer",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "        to magnetosphere; intensifies storms.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  n   ", Style::default().fg(colors::SOLAR_WIND_DENSITY).add_modifier(Modifier::BOLD)),
            Span::styled("Solar wind proton density (p/cm³).", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "        High density compresses magnetosphere",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "        and boosts storm effects.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "  NOAA STORM SCALES  (top bar)",
            Style::default().fg(colors::TITLE).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  R  ", Style::default().fg(colors::SEVERITY_STRONG).add_modifier(Modifier::BOLD)),
            Span::styled("Radio blackout. R1–R5.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "       Solar flares disrupt HF radio on sunlit side.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(vec![
            Span::styled("  S  ", Style::default().fg(colors::SEVERITY_STRONG).add_modifier(Modifier::BOLD)),
            Span::styled("Solar radiation storm. S1–S5.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "       High-energy proton events; polar aviation risk.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(vec![
            Span::styled("  G  ", Style::default().fg(colors::SEVERITY_STRONG).add_modifier(Modifier::BOLD)),
            Span::styled("Geomagnetic storm. G1–G5.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "       Kp-derived. Impacts power grid, GPS, aurora.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ─── DATA SOURCES ──────────────────────────",
            Style::default().fg(colors::BORDER_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Bt/Bz   ", Style::default().fg(colors::SOLAR_WIND_BT).add_modifier(Modifier::BOLD)),
            Span::styled("DSCOVR spacecraft at L1 (~1.5M km", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("  V/n     ", Style::default().fg(colors::SOLAR_WIND_SPEED).add_modifier(Modifier::BOLD)),
            Span::styled("sunward). 1-min cadence. ~15–60 min", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            propagation delay to Earth.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "            https://swpc.noaa.gov/products/solar-wind/",
            Style::default().fg(colors::TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  R/S/G   ", Style::default().fg(colors::SEVERITY_STRONG).add_modifier(Modifier::BOLD)),
            Span::styled("NOAA SWPC, Boulder CO. Real-time,", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            event-driven updates.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "            https://swpc.noaa.gov/products/noaa-scales.json",
            Style::default().fg(colors::TEXT_DIM),
        )),
        Line::from(vec![
            Span::styled("  Forecast ", Style::default().fg(colors::FORECAST).add_modifier(Modifier::BOLD)),
            Span::styled("NOAA SWPC. Issued 3× per day.", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            https://swpc.noaa.gov/text/3-day-forecast.txt",
            Style::default().fg(colors::TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  SFI     ", Style::default().fg(colors::FORECAST_SECONDARY).add_modifier(Modifier::BOLD)),
            Span::styled("DRAO Penticton, BC, Canada. Measured", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            daily at local noon (~20:00 UTC).",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "            https://swpc.noaa.gov/products/summary/10cm-flux",
            Style::default().fg(colors::TEXT_DIM),
        )),
    ];

    // ── RIGHT COLUMN: Activity + World Map ──
    let right_lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            "  ACTIVITY  (right panel)",
            Style::default().fg(colors::TITLE).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Kp     ", Style::default().fg(colors::KP_MEDIUM).add_modifier(Modifier::BOLD)),
            Span::styled("Planetary geomag activity 0–9.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "           Kp ≥ 5 = storm onset. Kp ≥ 7 = aurora",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "           visible at ~50°N. Kp 9 = extreme storm.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  X-Ray  ", Style::default().fg(colors::FLARE_M).add_modifier(Modifier::BOLD)),
            Span::styled("Solar flare class A/B/C/M/X.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "           Each step = ×10 energy. X-class causes",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "           major HF blackouts; may trigger CMEs.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Dst    ", Style::default().fg(colors::DST_INDEX).add_modifier(Modifier::BOLD)),
            Span::styled("Disturbance Storm Time (nT).", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "           Measures ring current strength.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "           < –50 nT = storm;  < –100 nT = severe.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Band   ", Style::default().fg(colors::FORECAST_SECONDARY).add_modifier(Modifier::BOLD)),
            Span::styled("HF radio propagation quality.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "  Cond   80m–10m bands rated Good/Fair/Poor.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "           SFI (solar flux) up = better; Kp up = worse.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "  WORLD MAP  (center)",
            Style::default().fg(colors::TITLE).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Aurora ", Style::default().fg(colors::AURORA_BRIGHT).add_modifier(Modifier::BOLD)),
            Span::styled("Oval from NOAA OVATION nowcast.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("           Activity: ", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled("green", Style::default().fg(Color::Rgb(0, 220, 90))),
            Span::styled(" \u{2192} ", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled("yellow", Style::default().fg(Color::Rgb(240, 210, 0))),
            Span::styled(" \u{2192} ", Style::default().fg(colors::TEXT_SECONDARY)),
            Span::styled("red", Style::default().fg(Color::Rgb(255, 50, 40))),
            Span::styled(" (low\u{2192}high).", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "           Falls back to Kp-based circle if feed down.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(vec![
            Span::styled("  Day/   ", Style::default().fg(colors::MAP_TERMINATOR_LINE).add_modifier(Modifier::BOLD)),
            Span::styled("Shaded = nightside. Bright line =", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "  Night  solar terminator (current UTC time).",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(vec![
            Span::styled("  Grid   ", Style::default().fg(colors::MAP_GRID).add_modifier(Modifier::BOLD)),
            Span::styled("10° lat / 15° lon graticule.", Style::default().fg(colors::TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(
            "           Left axis = geographic latitude.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "           Bottom axis = UTC timezone offset.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ─── DATA SOURCES ──────────────────────────",
            Style::default().fg(colors::BORDER_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Kp      ", Style::default().fg(colors::KP_MEDIUM).add_modifier(Modifier::BOLD)),
            Span::styled("Global magnetometer net (~13 stns).", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            3-hr averages, posted every 15 min.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "            NOAA SWPC, Boulder CO.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "            https://swpc.noaa.gov/products/noaa-planetary-k-index",
            Style::default().fg(colors::TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  X-Ray   ", Style::default().fg(colors::FLARE_M).add_modifier(Modifier::BOLD)),
            Span::styled("GOES geostationary sat (~35,800 km).", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            ~1-min cadence. NOAA SWPC Boulder CO.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "            https://swpc.noaa.gov/json/goes/primary/",
            Style::default().fg(colors::TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Dst     ", Style::default().fg(colors::DST_INDEX).add_modifier(Modifier::BOLD)),
            Span::styled("Kyoto WDC, Japan (4 equatorial stns).", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            Provisional values, ~1hr latency.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "            https://swpc.noaa.gov/products/kyoto-dst.json",
            Style::default().fg(colors::TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Band    ", Style::default().fg(colors::FORECAST_SECONDARY).add_modifier(Modifier::BOLD)),
            Span::styled("Derived in-dashboard from SFI + Kp.", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            No external endpoint.",
            Style::default().fg(colors::TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Launch  ", Style::default().fg(colors::LAUNCH_NAME).add_modifier(Modifier::BOLD)),
            Span::styled("The Space Devs – Launch Library 2.", Style::default().fg(colors::TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            "            Fetched hourly. Next tracked site only.",
            Style::default().fg(colors::TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            "            https://lldev.thespacedevs.com/2.3.0/launches/",
            Style::default().fg(colors::TEXT_DIM),
        )),
    ];

    f.render_widget(Paragraph::new(left_lines), cols[0]);
    f.render_widget(Paragraph::new(right_lines), cols[1]);
}

// === FOOTER ===

fn render_footer(f: &mut ratatui::Frame, area: Rect, last_refresh: std::time::Instant, data: &DashboardData, show_info: bool, show_launch_lines: bool, audio_alerts: bool) {
    let remaining = AUTO_REFRESH_INTERVAL.as_secs_f32() - last_refresh.elapsed().as_secs_f32();
    let remaining = remaining.max(0.0);

    let info_label = if show_info { " hide info " } else { " info " };
    let launch_label = if show_launch_lines { " hide links " } else { " launchsites " };
    let audio_label = if audio_alerts { " sound on " } else { " sound off " };
    let mut spans = vec![
        Span::styled(" q", Style::default().fg(colors::BORDER_HIGHLIGHT).add_modifier(Modifier::BOLD)),
        Span::styled(" quit ", Style::default().fg(colors::TEXT_DIM)),
        Span::styled("\u{2502} ", Style::default().fg(colors::BORDER_DIM)),
        Span::styled("r", Style::default().fg(colors::BORDER_HIGHLIGHT).add_modifier(Modifier::BOLD)),
        Span::styled(" refresh ", Style::default().fg(colors::TEXT_DIM)),
        Span::styled("\u{2502} ", Style::default().fg(colors::BORDER_DIM)),
        Span::styled("i", Style::default().fg(colors::BORDER_HIGHLIGHT).add_modifier(Modifier::BOLD)),
        Span::styled(info_label, Style::default().fg(colors::TEXT_DIM)),
        Span::styled("\u{2502} ", Style::default().fg(colors::BORDER_DIM)),
        Span::styled("l", Style::default().fg(colors::BORDER_HIGHLIGHT).add_modifier(Modifier::BOLD)),
        Span::styled(launch_label, Style::default().fg(colors::TEXT_DIM)),
        Span::styled("\u{2502} ", Style::default().fg(colors::BORDER_DIM)),
        Span::styled("a", Style::default().fg(colors::BORDER_HIGHLIGHT).add_modifier(Modifier::BOLD)),
        Span::styled(audio_label, Style::default().fg(colors::TEXT_DIM)),
        Span::styled("\u{2502} ", Style::default().fg(colors::BORDER_DIM)),
        Span::styled(
            format!("Next refresh: {:.0}s", remaining),
            Style::default().fg(colors::TEXT_DIM),
        ),
        Span::styled(" \u{2502} ", Style::default().fg(colors::BORDER_DIM)),
        Span::styled(
            format!("Updated {:.0}s ago", last_refresh.elapsed().as_secs_f32()),
            Style::default().fg(colors::TEXT_DIM),
        ),
    ];

    if !data.fetch_errors.is_empty() {
        spans.push(Span::styled(" \u{2502} ", Style::default().fg(colors::BORDER_DIM)));
        spans.push(Span::styled(
            format!("{} fetch error(s)", data.fetch_errors.len()),
            Style::default().fg(Color::Rgb(255, 100, 100)),
        ));
    }

    let footer = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    f.render_widget(footer, area);
}

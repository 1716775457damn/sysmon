use crate::app::{App, ProcSort, HISTORY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, BarChart, Block, Borders, Cell, Chart, Dataset, Gauge,
        GraphType, Paragraph, Row, Sparkline, Table, Tabs,
    },
    Frame,
};

// ── Palette ───────────────────────────────────────────────────────────────────
// Consistent 8-color semantic palette — every widget uses these, never raw Color
// calls scattered through the code.

const C_BORDER:  Color = Color::Rgb(55, 65, 80);          // subtle border
const C_TITLE:   Color = Color::Rgb(148, 163, 184);       // panel titles
const C_DIM:     Color = Color::Rgb(75, 85, 100);         // secondary text
const C_TEXT:    Color = Color::Rgb(203, 213, 225);       // primary text
const C_ACCENT:  Color = Color::Rgb(56, 189, 248);        // sky-400 — active/selected
const C_GREEN:   Color = Color::Rgb(52, 211, 153);        // emerald-400 — good/low
const C_YELLOW:  Color = Color::Rgb(251, 191, 36);        // amber-400 — warn/mid
const C_RED:     Color = Color::Rgb(248, 113, 113);       // red-400 — danger/high
const C_MAGENTA: Color = Color::Rgb(192, 132, 252);       // violet-400 — misc
const C_BLUE:    Color = Color::Rgb(96, 165, 250);        // blue-400 — info

// Shorthand style constructors
#[inline] fn s_dim()    -> Style { Style::default().fg(C_DIM) }
#[inline] fn s_text()   -> Style { Style::default().fg(C_TEXT) }
#[inline] fn s_accent() -> Style { Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD) }
#[inline] fn s_green()  -> Style { Style::default().fg(C_GREEN) }
#[inline] fn s_yellow() -> Style { Style::default().fg(C_YELLOW) }
#[inline] fn s_red()    -> Style { Style::default().fg(C_RED) }

/// Styled block with uniform border color and optional title.
fn panel(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_BORDER))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(C_TITLE).add_modifier(Modifier::BOLD),
        ))
}

/// Panel with a colored border (for alerts / active input).
fn panel_colored(title: &str, border_color: Color) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
        ))
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn draw(f: &mut Frame, app: &App) {
    if app.show_help {
        draw_help(f, f.area());
        return;
    }
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Tab bar — accent on selected, dim on others
    let tab_titles: Vec<Span> = [
        " 总览 [1] ", " CPU [2] ", " 内存 [3] ",
        " 网络 [4] ", " 磁盘 [5] ", " 进程 [6] ", " 扫描 [7] ", " Git [8] ",
    ]
    .iter()
    .enumerate()
    .map(|(i, &t)| {
        if i == app.tab {
            Span::styled(t, Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(t, s_dim())
        }
    })
    .collect();

    let tabs = Tabs::new(tab_titles)
        .select(app.tab)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER))
                .title(Span::styled(
                    format!(" ⚡ SysMon  {}ms  ? 帮助 ", app.tick_rate_ms),
                    Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(s_accent())
        .divider(Span::styled("│", s_dim()));
    f.render_widget(tabs, chunks[0]);

    match app.tab {
        0 => draw_overview(f, app, chunks[1]),
        1 => draw_cpu(f, app, chunks[1]),
        2 => draw_memory(f, app, chunks[1]),
        3 => draw_network(f, app, chunks[1]),
        4 => draw_disk(f, app, chunks[1]),
        5 => draw_processes(f, app, chunks[1]),
        6 => draw_scanner(f, app, chunks[1]),
        7 => draw_git(f, app, chunks[1]),
        _ => {}
    }
}

// ── Overview ──────────────────────────────────────────────────────────────────

fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let cpu_pct = *app.cpu_total_history.back().unwrap_or(&0.0);
    let alert_blink = app.alert_cpu && (app.tick / 2) % 2 == 0;
    let cpu_border = if alert_blink { C_RED } else { C_BORDER };
    let cpu_title  = if app.alert_cpu { "CPU  ⚠ HIGH" } else { "CPU" };
    let cpu_gauge = Gauge::default()
        .block(panel_colored(cpu_title, cpu_border))
        .gauge_style(Style::default().fg(metric_color(cpu_pct, 60.0, 90.0)))
        .percent(cpu_pct as u16)
        .label(Span::styled(
            format!("{:.1}%", cpu_pct),
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(cpu_gauge, top[0]);

    let mem_pct = if app.mem_total > 0 {
        app.mem_used as f64 / app.mem_total as f64 * 100.0
    } else { 0.0 };
    let mem_blink  = app.alert_mem && (app.tick / 2) % 2 == 0;
    let mem_border = if mem_blink { C_RED } else { C_BORDER };
    let mem_title  = format!(
        "{}  {}/{}",
        if app.alert_mem { "内存  ⚠ HIGH" } else { "内存" },
        fmt_bytes(app.mem_used), fmt_bytes(app.mem_total)
    );
    let mem_gauge = Gauge::default()
        .block(panel_colored(&mem_title, mem_border))
        .gauge_style(Style::default().fg(metric_color(mem_pct, 70.0, 90.0)))
        .percent(mem_pct as u16)
        .label(Span::styled(
            format!("{:.1}%", mem_pct),
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(mem_gauge, top[1]);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    let rx_data: Vec<u64> = app.net_rx_history.iter().map(|&v| v as u64).collect();
    let net_title = format!("网络  ↓{}/s  ↑{}/s",
        fmt_bytes(app.net_rx_rate as u64), fmt_bytes(app.net_tx_rate as u64));
    let net_spark = Sparkline::default()
        .block(panel(&net_title))
        .data(&rx_data)
        .style(s_green());
    f.render_widget(net_spark, mid[0]);

    let dr_data: Vec<u64> = app.disk_read_history.iter().map(|&v| v as u64).collect();
    let disk_title = format!("磁盘  R:{}/s  W:{}/s",
        fmt_bytes(app.disk_read_rate as u64), fmt_bytes(app.disk_write_rate as u64));
    let disk_spark = Sparkline::default()
        .block(panel(&disk_title))
        .data(&dr_data)
        .style(s_yellow());
    f.render_widget(disk_spark, mid[1]);

    let bot = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[2]);

    let cpu_data: Vec<u64> = app.cpu_total_history.iter().map(|&v| v as u64).collect();
    let cpu_spark = Sparkline::default()
        .block(panel("CPU 历史"))
        .data(&cpu_data)
        .style(Style::default().fg(C_ACCENT));
    f.render_widget(cpu_spark, bot[0]);

    let header = Row::new(vec!["PID", "进程名", "CPU%", "内存"])
        .style(s_accent());
    let rows_data: Vec<Row> = app.processes.iter().take(10).map(|p| {
        Row::new(vec![
            Cell::from(p.pid.to_string()).style(s_dim()),
            Cell::from(p.name.chars().take(20).collect::<String>()).style(s_text()),
            Cell::from(format!("{:.1}", p.cpu))
                .style(Style::default().fg(metric_color(p.cpu as f64, 60.0, 90.0))),
            Cell::from(format!("{:.0} M", p.mem_mb)).style(s_dim()),
        ])
    }).collect();
    let table = Table::new(rows_data, [
        Constraint::Length(7), Constraint::Min(20),
        Constraint::Length(7), Constraint::Length(8),
    ])
    .header(header)
    .block(panel("Top 进程 (CPU)"));
    f.render_widget(table, bot[1]);
}

// ── CPU ───────────────────────────────────────────────────────────────────────

fn draw_cpu(f: &mut Frame, app: &App, area: Rect) {
    let core_count = app.cpu_history.len();
    let cols = if core_count <= 4 { 1 } else if core_count <= 8 { 2 } else { 4 };
    let rows_count = (core_count + cols - 1) / cols;

    let row_constraints: Vec<Constraint> = (0..rows_count)
        .map(|_| Constraint::Ratio(1, rows_count as u32))
        .collect();
    let row_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);

    let col_constraints: Vec<Constraint> = (0..cols)
        .map(|_| Constraint::Ratio(1, cols as u32))
        .collect();

    let mut data_buf: Vec<u64> = Vec::with_capacity(HISTORY);

    for row in 0..rows_count {
        let col_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(&col_constraints)
            .split(row_chunks[row]);

        for col in 0..cols {
            let idx = row * cols + col;
            if idx >= core_count { break; }
            let usage = *app.cpu_history[idx].back().unwrap_or(&0.0);
            data_buf.clear();
            data_buf.extend(app.cpu_history[idx].iter().map(|&v| v as u64));
            let color = metric_color(usage, 60.0, 90.0);
            let core_title = format!("Core {}  {:.1}%", idx, usage);
            let spark = Sparkline::default()
                .block(panel_colored(&core_title, color))
                .data(&data_buf)
                .max(100)
                .style(Style::default().fg(color));
            f.render_widget(spark, col_chunks[col]);
        }
    }
}

// ── Memory ────────────────────────────────────────────────────────────────────

fn draw_memory(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Length(4), Constraint::Min(0)])
        .split(area);

    let mem_pct = if app.mem_total > 0 {
        app.mem_used as f64 / app.mem_total as f64 * 100.0
    } else { 0.0 };
    let mem_title = format!("RAM  已用 {} / 总计 {}",
        fmt_bytes(app.mem_used), fmt_bytes(app.mem_total));
    let mem_gauge = Gauge::default()
        .block(panel(&mem_title))
        .gauge_style(Style::default().fg(metric_color(mem_pct, 70.0, 90.0)))
        .percent(mem_pct as u16)
        .label(Span::styled(
            format!("{:.1}%  空闲 {}", mem_pct,
                fmt_bytes(app.mem_total.saturating_sub(app.mem_used))),
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(mem_gauge, chunks[0]);

    let swap_pct = if app.swap_total > 0 {
        app.swap_used as f64 / app.swap_total as f64 * 100.0
    } else { 0.0 };
    let swap_title = format!("Swap  已用 {} / 总计 {}",
        fmt_bytes(app.swap_used), fmt_bytes(app.swap_total));
    let swap_gauge = Gauge::default()
        .block(panel(&swap_title))
        .gauge_style(Style::default().fg(C_BLUE))
        .percent(swap_pct as u16)
        .label(Span::styled(
            format!("{:.1}%", swap_pct),
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(swap_gauge, chunks[1]);

    let mem_data: Vec<(f64, f64)> = app.mem_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();
    let dataset = Dataset::default()
        .name("RAM %")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(s_green())
        .data(&mem_data);
    let chart = Chart::new(vec![dataset])
        .block(panel("内存使用历史"))
        .x_axis(
            Axis::default()
                .bounds([0.0, HISTORY as f64])
                .style(s_dim())
                .labels(vec![
                    Span::styled("60s", s_dim()),
                    Span::styled("30s", s_dim()),
                    Span::styled("0s",  s_dim()),
                ]),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, 100.0])
                .style(s_dim())
                .labels(vec![
                    Span::styled("0%",   s_dim()),
                    Span::styled("50%",  s_dim()),
                    Span::styled("100%", s_dim()),
                ]),
        );
    f.render_widget(chart, chunks[2]);
}

// ── Network ───────────────────────────────────────────────────────────────────

fn draw_network(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let rx_data: Vec<(f64, f64)> = app.net_rx_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v / 1024.0))
        .collect();
    let tx_data: Vec<(f64, f64)> = app.net_tx_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v / 1024.0))
        .collect();
    let max_net = app.net_rx_history.iter().chain(app.net_tx_history.iter())
        .cloned().fold(1.0_f64, f64::max) / 1024.0;

    let rx_ds = Dataset::default()
        .name(Span::styled("↓ 下载", s_green()))
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(s_green())
        .data(&rx_data);
    let tx_ds = Dataset::default()
        .name(Span::styled("↑ 上传", s_yellow()))
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(s_yellow())
        .data(&tx_data);

    let net_io_title = format!("网络 IO  ↓{}/s  ↑{}/s",
        fmt_bytes(app.net_rx_rate as u64), fmt_bytes(app.net_tx_rate as u64));
    let net_chart = Chart::new(vec![rx_ds, tx_ds])
        .block(panel(&net_io_title))
        .x_axis(Axis::default().bounds([0.0, HISTORY as f64]).style(s_dim()))
        .y_axis(
            Axis::default()
                .bounds([0.0, max_net * 1.1])
                .style(s_dim())
                .labels(vec![
                    Span::styled("0", s_dim()),
                    Span::styled(fmt_bytes((max_net * 512.0) as u64),  s_dim()),
                    Span::styled(fmt_bytes((max_net * 1024.0) as u64), s_dim()),
                ]),
        );
    f.render_widget(net_chart, chunks[0]);

    let bar_data: Vec<(String, u64)> = app.networks.iter()
        .map(|(name, n)| (name.clone(), (n.received() + n.transmitted()) / 1024))
        .collect();
    let bar_refs: Vec<(&str, u64)> = bar_data.iter()
        .map(|(n, v)| (n.as_str(), *v))
        .collect();
    let bar = BarChart::default()
        .block(panel("接口流量 (KB/s)"))
        .data(&bar_refs)
        .bar_width(9)
        .bar_gap(1)
        .bar_style(Style::default().fg(C_ACCENT))
        .value_style(Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD));
    f.render_widget(bar, chunks[1]);
}

// ── Disk ──────────────────────────────────────────────────────────────────────

fn draw_disk(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let rd_data: Vec<(f64, f64)> = app.disk_read_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v / 1024.0))
        .collect();
    let wr_data: Vec<(f64, f64)> = app.disk_write_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v / 1024.0))
        .collect();
    let max_disk = app.disk_read_history.iter().chain(app.disk_write_history.iter())
        .cloned().fold(1.0_f64, f64::max) / 1024.0;

    let rd_ds = Dataset::default()
        .name(Span::styled("读取", Style::default().fg(C_ACCENT)))
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(C_ACCENT))
        .data(&rd_data);
    let wr_ds = Dataset::default()
        .name(Span::styled("写入", Style::default().fg(C_MAGENTA)))
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(C_MAGENTA))
        .data(&wr_data);

    let disk_io_title = format!("磁盘 IO  读:{}/s  写:{}/s",
        fmt_bytes(app.disk_read_rate as u64), fmt_bytes(app.disk_write_rate as u64));
    let disk_chart = Chart::new(vec![rd_ds, wr_ds])
        .block(panel(&disk_io_title))
        .x_axis(Axis::default().bounds([0.0, HISTORY as f64]).style(s_dim()))
        .y_axis(
            Axis::default()
                .bounds([0.0, max_disk * 1.1])
                .style(s_dim())
                .labels(vec![
                    Span::styled("0", s_dim()),
                    Span::styled(fmt_bytes((max_disk * 512.0) as u64),  s_dim()),
                    Span::styled(fmt_bytes((max_disk * 1024.0) as u64), s_dim()),
                ]),
        );
    f.render_widget(disk_chart, chunks[0]);

    let disk_rows: Vec<Row> = app.disks.iter().map(|d| {
        let total = d.total_space();
        let avail = d.available_space();
        let used  = total.saturating_sub(avail);
        let pct   = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
        let bar_len = (pct / 5.0) as usize;
        let bar = format!(
            "[{}{}] {:.0}%",
            "█".repeat(bar_len),
            "░".repeat(20usize.saturating_sub(bar_len)),
            pct
        );
        let bar_color = metric_color(pct, 70.0, 90.0);
        Row::new(vec![
            Cell::from(d.mount_point().to_string_lossy().to_string()).style(s_text()),
            Cell::from(d.name().to_string_lossy().to_string()).style(s_dim()),
            Cell::from(fmt_bytes(total)).style(s_dim()),
            Cell::from(fmt_bytes(used)).style(s_text()),
            Cell::from(fmt_bytes(avail)).style(s_green()),
            Cell::from(bar).style(Style::default().fg(bar_color)),
        ])
    }).collect();

    let header = Row::new(vec!["挂载点", "设备", "总计", "已用", "可用", "使用率"])
        .style(s_accent());
    let table = Table::new(disk_rows, [
        Constraint::Min(15), Constraint::Min(12),
        Constraint::Length(9), Constraint::Length(9),
        Constraint::Length(9), Constraint::Min(25),
    ])
    .header(header)
    .block(panel("磁盘分区"));
    f.render_widget(table, chunks[1]);
}

// ── Processes ─────────────────────────────────────────────────────────────────

fn draw_processes(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Hint bar — sort keys + filter + position counter
    let filter_display = if app.proc_filter.is_empty() {
        "/ 过滤".to_string()
    } else {
        format!("/ 过滤: {}_", app.proc_filter)
    };

    let hint = Paragraph::new(Line::from(vec![
        Span::styled(" 排序: ", s_dim()),
        sort_span("C CPU",  app.proc_sort == ProcSort::Cpu),
        Span::styled("  ", s_dim()),
        sort_span("M 内存", app.proc_sort == ProcSort::Mem),
        Span::styled("  ", s_dim()),
        sort_span("P PID",  app.proc_sort == ProcSort::Pid),
        Span::styled("  ", s_dim()),
        sort_span("N 名称", app.proc_sort == ProcSort::Name),
        Span::styled("  ↑↓ 导航  ", s_dim()),
        Span::styled(
            format!("{}/{}", app.proc_selected + 1, app.processes.len()),
            s_yellow(),
        ),
        Span::styled("  ", s_dim()),
        Span::styled(
            filter_display,
            if app.proc_filter_active { s_accent() } else { s_dim() },
        ),
        Span::styled("  K 结束  Shift+K 强杀", s_red()),
    ]))
    .block(panel("进程控制"));
    f.render_widget(hint, chunks[0]);

    let header = Row::new(vec!["PID", "进程名", "CPU%", "内存", "状态"])
        .style(s_accent());

    let rows: Vec<Row> = app.processes.iter().enumerate().map(|(i, p)| {
        let is_sel = i == app.proc_selected;
        // Selected row: bright bg + bold; others: alternating subtle tint
        let row_style = if is_sel {
            Style::default()
                .bg(Color::Rgb(30, 41, 59))   // slate-800
                .add_modifier(Modifier::BOLD)
        } else if i % 2 == 0 {
            Style::default().bg(Color::Rgb(15, 18, 24))
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(p.pid.to_string()).style(s_dim()),
            Cell::from(p.name.chars().take(30).collect::<String>()).style(s_text()),
            Cell::from(format!("{:.1}", p.cpu))
                .style(Style::default().fg(metric_color(p.cpu as f64, 60.0, 90.0))),
            Cell::from(format!("{:.1} MB", p.mem_mb)).style(s_dim()),
            Cell::from(p.status.clone()).style(s_dim()),
        ])
        .style(row_style)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(8), Constraint::Min(25),
        Constraint::Length(7), Constraint::Length(10), Constraint::Length(12),
    ])
    .header(header)
    .block(panel("进程列表"));
    f.render_widget(table, chunks[1]);
}

// ── Help overlay ──────────────────────────────────────────────────────────────

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled(" 键盘快捷键", s_accent())),
        Line::from(""),
        Line::from(Span::styled(" 切换页面", s_yellow())),
        Line::from(vec![
            Span::styled(" 1–8 ", s_accent()),
            Span::styled("切换标签页", s_text()),
        ]),
        Line::from(vec![
            Span::styled(" Tab / Shift+Tab ", s_accent()),
            Span::styled("前/后一页", s_text()),
        ]),
        Line::from(""),
        Line::from(Span::styled(" 进程列表", s_yellow())),
        Line::from(vec![Span::styled(" ↑↓ / j k ", s_accent()), Span::styled("导航", s_text())]),
        Line::from(vec![Span::styled(" g / G    ", s_accent()), Span::styled("顶部 / 底部", s_text())]),
        Line::from(vec![Span::styled(" c m p n  ", s_accent()), Span::styled("按 CPU/内存/PID/名称排序", s_text())]),
        Line::from(vec![Span::styled(" /        ", s_accent()), Span::styled("过滤进程名", s_text())]),
        Line::from(vec![Span::styled(" K        ", s_accent()), Span::styled("结束进程 (SIGTERM)", s_text())]),
        Line::from(vec![Span::styled(" Shift+K  ", s_accent()), Span::styled("强杀进程 (SIGKILL)", s_text())]),
        Line::from(""),
        Line::from(Span::styled(" 刷新速度", s_yellow())),
        Line::from(vec![Span::styled(" + / -    ", s_accent()), Span::styled("加快 / 减慢 (200ms–5000ms)", s_text())]),
        Line::from(""),
        Line::from(Span::styled(" 其他", s_yellow())),
        Line::from(vec![Span::styled(" ?        ", s_accent()), Span::styled("显示/隐藏帮助", s_text())]),
        Line::from(vec![Span::styled(" q / C-c  ", s_accent()), Span::styled("退出", s_text())]),
        Line::from(""),
        Line::from(Span::styled(" 按任意键关闭", s_dim())),
    ];

    let popup_area = centered_rect(52, 82, area);
    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(
        Paragraph::new(help_text)
            .block(panel_colored("❓ 帮助", C_ACCENT))
            .style(s_text()),
        popup_area,
    );
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Three-tier color: green → yellow → red based on two thresholds.
fn metric_color(val: f64, warn: f64, crit: f64) -> Color {
    if val >= crit { C_RED } else if val >= warn { C_YELLOW } else { C_GREEN }
}

fn sort_span(label: &str, active: bool) -> Span<'_> {
    if active {
        Span::styled(label, Style::default()
            .fg(C_ACCENT)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
    } else {
        Span::styled(label, s_text())
    }
}

pub fn fmt_bytes(b: u64) -> String {
    if b < 1024 { format!("{} B", b) }
    else if b < 1_048_576   { format!("{:.1} KB", b as f64 / 1024.0) }
    else if b < 1_073_741_824 { format!("{:.1} MB", b as f64 / 1_048_576.0) }
    else { format!("{:.2} GB", b as f64 / 1_073_741_824.0) }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}

// ── Scanner ───────────────────────────────────────────────────────────────────

fn draw_scanner(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // config
            Constraint::Length(3),  // status
            Constraint::Min(0),     // results
        ])
        .split(area);

    // Config: left = inputs, right = presets + options
    let cfg_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    let input_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(cfg_cols[0]);

    // Host input — accent border when active
    let host_active = app.scan_input_active && app.scan_input_field == 0;
    let host_text = if host_active {
        format!("{}_", app.scan_host)
    } else {
        app.scan_host.clone()
    };
    f.render_widget(
        Paragraph::new(Span::styled(host_text, s_text()))
            .block(panel_colored(
                "目标地址  (IP / IP段 / CIDR)",
                if host_active { C_ACCENT } else { C_BORDER },
            )),
        input_rows[0],
    );

    let port_active = app.scan_input_active && app.scan_input_field == 1;
    let port_text = if port_active {
        format!("{}_", app.scan_ports)
    } else {
        app.scan_ports.clone()
    };
    f.render_widget(
        Paragraph::new(Span::styled(port_text, s_text()))
            .block(panel_colored(
                "端口  (80,443,22-25)",
                if port_active { C_ACCENT } else { C_BORDER },
            )),
        input_rows[1],
    );

    // Right panel: presets + options
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(cfg_cols[1]);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 预设: ", s_dim()),
            Span::styled("F1 常用", s_yellow()),
            Span::styled("  ", s_dim()),
            Span::styled("F2 Web", s_yellow()),
            Span::styled("  ", s_dim()),
            Span::styled("F3 数据库", s_yellow()),
            Span::styled("  ", s_dim()),
            Span::styled("F4 远程", s_yellow()),
        ]))
        .block(panel("端口预设")),
        right_rows[0],
    );

    let action_span = if app.scan_running {
        Span::styled("⏹ 停止 [Esc]", s_red())
    } else {
        Span::styled("▶ 扫描 [Enter]", s_green())
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 超时: ", s_dim()),
            Span::styled(format!("{}ms", app.scan_timeout_ms), Style::default().fg(C_ACCENT)),
            Span::styled("  并发: ", s_dim()),
            Span::styled(format!("{}", app.scan_concurrency), Style::default().fg(C_ACCENT)),
            Span::styled("  ", s_dim()),
            Span::styled(
                if app.scan_show_closed { "[✓] 显示关闭" } else { "[ ] 显示关闭" },
                s_dim(),
            ),
            Span::styled("  ", s_dim()),
            action_span,
        ]))
        .block(panel("选项")),
        right_rows[1],
    );

    // Status / progress bar
    let progress_str = if app.scan_total > 0 {
        let pct = app.scan_done as f64 / app.scan_total as f64;
        let bar_w = (chunks[1].width.saturating_sub(4)) as usize;
        let filled = (pct * bar_w as f64) as usize;
        format!(
            " [{}{}] {}/{} 开放:{}",
            "━".repeat(filled),
            " ".repeat(bar_w.saturating_sub(filled)),
            app.scan_done, app.scan_total, app.scan_open
        )
    } else {
        String::new()
    };
    let status_color = if app.scan_running { C_YELLOW } else { C_GREEN };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(&app.scan_status, Style::default().fg(status_color)),
            Span::styled(progress_str, Style::default().fg(C_ACCENT)),
        ]))
        .block(panel("状态")),
        chunks[1],
    );

    // Results table
    let visible: Vec<&crate::scanner::ScanResult> = app.scan_results.iter()
        .filter(|r| app.scan_show_closed || r.open)
        .collect();

    let header = Row::new(vec!["IP 地址", "端口", "状态", "延迟", "服务"])
        .style(s_accent());

    let rows: Vec<Row> = visible.iter().enumerate().map(|(i, r)| {
        let is_sel = i == app.scan_selected;
        let row_style = if is_sel {
            Style::default().bg(Color::Rgb(30, 41, 59)).add_modifier(Modifier::BOLD)
        } else if i % 2 == 0 {
            Style::default().bg(Color::Rgb(15, 18, 24))
        } else {
            Style::default()
        };
        let (status_str, status_color) = if r.open {
            ("● 开放", C_GREEN)
        } else {
            ("○ 关闭", C_DIM)
        };
        let latency = r.latency_ms
            .map(|ms| format!("{:.1} ms", ms))
            .unwrap_or_else(|| "—".to_string());
        let lat_color = r.latency_ms.map_or(C_DIM, |ms| {
            if ms < 10.0 { C_GREEN } else if ms < 100.0 { C_YELLOW } else { C_RED }
        });
        Row::new(vec![
            Cell::from(r.host.clone()).style(s_text()),
            Cell::from(r.port.to_string()).style(s_dim()),
            Cell::from(status_str).style(Style::default().fg(status_color)),
            Cell::from(latency).style(Style::default().fg(lat_color)),
            Cell::from(r.service).style(Style::default().fg(C_MAGENTA)),
        ])
        .style(row_style)
    }).collect();

    let open_count = app.scan_results.iter().filter(|r| r.open).count();
    let title = format!(
        "扫描结果  开放: {}  共: {}  (↑↓ 导航  H 切换显示关闭)",
        open_count, app.scan_results.len()
    );
    let table = Table::new(rows, [
        Constraint::Min(16), Constraint::Length(7),
        Constraint::Length(9), Constraint::Length(11), Constraint::Min(12),
    ])
    .header(header)
    .block(panel(&title));
    f.render_widget(table, chunks[2]);
}

// ── Git Analyzer ──────────────────────────────────────────────────────────────

fn draw_git(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // path input
            Constraint::Length(3),  // sub-tabs + history
            Constraint::Length(3),  // status / progress
            Constraint::Min(0),     // content
        ])
        .split(area);

    // Path input — accent border when active
    let hist_hint = if !app.git_history.is_empty() {
        format!(" ↑↓历史({})  Tab补全  Enter分析  Esc退出", app.git_history.len())
    } else {
        " Tab补全  Enter分析  Esc退出".to_string()
    };
    let path_text = if app.git_input_active {
        format!("{}_", app.git_path)
    } else {
        app.git_path.clone()
    };
    let path_title = if app.git_input_active {
        format!("Git 仓库路径{}", hist_hint)
    } else {
        "Git 仓库路径  (i 或 Enter 开始输入)".to_string()
    };
    f.render_widget(
        Paragraph::new(Span::styled(path_text, s_text()))
            .block(panel_colored(
                &path_title,
                if app.git_input_active { C_ACCENT } else { C_BORDER },
            )),
        chunks[0],
    );

    // Sub-tabs + recent history
    let top2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    let sub_tab_titles: Vec<Span> = [" 概览(o) ", " 作者(a) ", " 热点文件(f) ", " 热力图(h) "]
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            if i == app.git_tab {
                Span::styled(t, s_accent())
            } else {
                Span::styled(t, s_dim())
            }
        })
        .collect();
    let sub_tabs = Tabs::new(sub_tab_titles)
        .select(app.git_tab)
        .block(panel("视图"))
        .highlight_style(s_accent())
        .divider(Span::styled("│", s_dim()));
    f.render_widget(sub_tabs, top2[0]);

    let hist_spans: Vec<Span> = std::iter::once(Span::styled(" 最近: ", s_dim()))
        .chain(app.git_history.iter().take(3).enumerate().map(|(i, p)| {
            let label = std::path::Path::new(p)
                .file_name().and_then(|n| n.to_str()).unwrap_or(p);
            let color = if app.git_history_idx == Some(i) { C_ACCENT } else { C_YELLOW };
            Span::styled(
                format!("[{}]{} ", i + 1, label.chars().take(12).collect::<String>()),
                Style::default().fg(color),
            )
        }))
        .collect();
    f.render_widget(
        Paragraph::new(Line::from(hist_spans)).block(panel("历史路径")),
        top2[1],
    );

    // Status / progress
    let prog_str = if app.git_progress.1 > 0 {
        let pct = app.git_progress.0 as f64 / app.git_progress.1 as f64;
        let w = (chunks[2].width.saturating_sub(4)) as usize;
        let filled = (pct * w as f64) as usize;
        format!(
            " [{}{}] {}/{}",
            "━".repeat(filled),
            " ".repeat(w.saturating_sub(filled)),
            app.git_progress.0, app.git_progress.1
        )
    } else {
        String::new()
    };
    let status_color = if app.git_running {
        C_YELLOW
    } else if app.git_stats.is_some() {
        C_GREEN
    } else {
        C_DIM
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(&app.git_status, Style::default().fg(status_color)),
            Span::styled(prog_str, Style::default().fg(C_ACCENT)),
        ]))
        .block(panel("状态")),
        chunks[2],
    );

    // Content area
    let Some(ref stats) = app.git_stats else {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    " 输入仓库路径后按 Enter 开始分析",
                    s_text(),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    " 支持格式: /path/to/repo  或  C:\\path\\to\\repo",
                    s_dim(),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    " 提示: 切换到此页时自动进入输入模式，Tab 可补全目录",
                    s_dim(),
                )),
            ])
            .block(panel("提示")),
            chunks[3],
        );
        return;
    };

    match app.git_tab {
        0 => draw_git_overview(f, stats, chunks[3]),
        1 => draw_git_authors(f, stats, app.git_selected, chunks[3]),
        2 => draw_git_hotfiles(f, stats, app.git_selected, chunks[3]),
        3 => draw_git_heatmap(f, stats, chunks[3]),
        _ => {}
    }
}

fn draw_git_overview(f: &mut Frame, stats: &crate::gitanalyzer::GitStats, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    let info = vec![
        Line::from(vec![
            Span::styled(" 仓库:    ", s_dim()),
            Span::styled(&stats.repo_path, s_text()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Commits: ", s_dim()),
            Span::styled(stats.total_commits.to_string(),
                Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(" 作者数:  ", s_dim()),
            Span::styled(stats.total_authors.to_string(),
                Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(" 文件数:  ", s_dim()),
            Span::styled(stats.total_files.to_string(),
                Style::default().fg(C_YELLOW).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" 首次提交: ", s_dim()),
            Span::styled(&stats.first_commit, s_text()),
        ]),
        Line::from(vec![
            Span::styled(" 最新提交: ", s_dim()),
            Span::styled(&stats.last_commit, s_text()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(info).block(panel("仓库概要")),
        cols[0],
    );

    let act_data: Vec<u64> = stats.activity.iter().map(|&v| v as u64).collect();
    f.render_widget(
        Sparkline::default()
            .block(panel("过去 52 周提交活跃度"))
            .data(&act_data)
            .style(s_green()),
        cols[1],
    );
}

fn draw_git_authors(
    f: &mut Frame,
    stats: &crate::gitanalyzer::GitStats,
    selected: usize,
    area: Rect,
) {
    let header = Row::new(vec!["作者", "Commits", "新增行", "删除行", "触动文件"])
        .style(s_accent());

    let max_commits = stats.authors.first().map(|a| a.commits).unwrap_or(1);
    let rows: Vec<Row> = stats.authors.iter().enumerate().map(|(i, a)| {
        let bar_w  = 15usize;
        let filled = (a.commits as f64 / max_commits as f64 * bar_w as f64) as usize;
        let bar    = format!("{}{}", "█".repeat(filled), "░".repeat(bar_w - filled));
        let row_style = if i == selected {
            Style::default().bg(Color::Rgb(30, 41, 59)).add_modifier(Modifier::BOLD)
        } else if i % 2 == 0 {
            Style::default().bg(Color::Rgb(15, 18, 24))
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(a.name.chars().take(25).collect::<String>()).style(s_text()),
            Cell::from(format!("{} {}", a.commits, bar))
                .style(Style::default().fg(C_ACCENT)),
            Cell::from(format!("+{}", a.additions)).style(s_green()),
            Cell::from(format!("-{}", a.deletions)).style(s_red()),
            Cell::from(a.files_touched.to_string()).style(s_yellow()),
        ])
        .style(row_style)
    }).collect();

    let authors_title = format!("作者贡献榜 (共 {} 人)", stats.authors.len());
    let table = Table::new(rows, [
        Constraint::Min(20), Constraint::Min(25),
        Constraint::Length(12), Constraint::Length(12), Constraint::Length(10),
    ])
    .header(header)
    .block(panel(&authors_title));
    f.render_widget(table, area);
}

fn draw_git_hotfiles(
    f: &mut Frame,
    stats: &crate::gitanalyzer::GitStats,
    selected: usize,
    area: Rect,
) {
    let header = Row::new(vec!["文件路径", "修改次数", "新增", "删除", "作者数"])
        .style(s_accent());

    let max_changes = stats.hot_files.first().map(|f| f.changes).unwrap_or(1);
    let rows: Vec<Row> = stats.hot_files.iter().enumerate().map(|(i, file)| {
        let heat  = file.changes as f64 / max_changes as f64;
        let color = if heat > 0.7 { C_RED } else if heat > 0.4 { C_YELLOW } else { C_GREEN };
        let row_style = if i == selected {
            Style::default().bg(Color::Rgb(30, 41, 59)).add_modifier(Modifier::BOLD)
        } else if i % 2 == 0 {
            Style::default().bg(Color::Rgb(15, 18, 24))
        } else {
            Style::default()
        };
        let path_display = if file.path.chars().count() > 45 {
            let tail: String = file.path.chars().rev().take(42)
                .collect::<String>().chars().rev().collect();
            format!("…{}", tail)
        } else {
            file.path.clone()
        };
        Row::new(vec![
            Cell::from(path_display).style(s_text()),
            Cell::from(file.changes.to_string()).style(Style::default().fg(color)),
            Cell::from(format!("+{}", file.additions)).style(s_green()),
            Cell::from(format!("-{}", file.deletions)).style(s_red()),
            Cell::from(file.authors.to_string()).style(Style::default().fg(C_MAGENTA)),
        ])
        .style(row_style)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Min(35), Constraint::Length(10),
        Constraint::Length(10), Constraint::Length(10), Constraint::Length(8),
    ])
    .header(header)
    .block(panel("热点文件 (Top 100 修改最频繁)"));
    f.render_widget(table, area);
}

fn draw_git_heatmap(f: &mut Frame, stats: &crate::gitanalyzer::GitStats, area: Rect) {
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let max_val = stats.heatmap.iter()
        .flat_map(|row| row.iter())
        .copied()
        .max()
        .unwrap_or(1);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "     00 01 02 03 04 05 06 07 08 09 10 11 12 13 14 15 16 17 18 19 20 21 22 23",
        s_dim(),
    )));

    for (d, day) in days.iter().enumerate() {
        let mut spans = vec![
            Span::styled(format!(" {} ", day), Style::default().fg(C_ACCENT)),
        ];
        for h in 0..24 {
            let v    = stats.heatmap[d][h];
            let heat = v as f64 / max_val as f64;
            let (ch, color) = if v == 0 {
                ("· ", C_DIM)
            } else if heat < 0.25 {
                ("░ ", Color::Rgb(6, 78, 59))
            } else if heat < 0.5 {
                ("▒ ", Color::Rgb(16, 185, 129))
            } else if heat < 0.75 {
                ("▓ ", C_YELLOW)
            } else {
                ("█ ", C_RED)
            };
            spans.push(Span::styled(ch, Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" 图例: ", s_dim()),
        Span::styled("· 无",   s_dim()),
        Span::styled("  ░ 少", Style::default().fg(Color::Rgb(6, 78, 59))),
        Span::styled("  ▒ 中", Style::default().fg(Color::Rgb(16, 185, 129))),
        Span::styled("  ▓ 多", s_yellow()),
        Span::styled("  █ 最多", s_red()),
    ]));

    f.render_widget(
        Paragraph::new(lines).block(panel("提交时间热力图 (星期 × 小时)")),
        area,
    );
}

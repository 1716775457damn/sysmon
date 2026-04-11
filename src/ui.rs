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

const CYAN:   Color = Color::Cyan;
const GREEN:  Color = Color::Green;
const YELLOW: Color = Color::Yellow;
const RED:    Color = Color::Red;
const BLUE:   Color = Color::Blue;
const MAGENTA:Color = Color::Magenta;
const WHITE:  Color = Color::White;
const GRAY:   Color = Color::DarkGray;

pub fn draw(f: &mut Frame, app: &App) {
    if app.show_help {
        draw_help(f, f.area());
        return;
    }
    let area = f.area();

    // Tab bar at top
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let tab_titles = vec![
        " 总览 [1] ", " CPU [2] ", " 内存 [3] ",
        " 网络 [4] ", " 磁盘 [5] ", " 进程 [6] ", " 扫描 [7] ", " Git [8] ",
    ];
    let tabs = Tabs::new(tab_titles)
        .select(app.tab)
        .block(Block::default().borders(Borders::ALL)
            .title(format!(" ⚡ SysMon  刷新: {}ms  ? 帮助 ", app.tick_rate_ms))
            .title_style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD)))
        .highlight_style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD))
        .divider("|");
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

// ─── Overview ────────────────────────────────────────────────────────────────

fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    // Row 1: CPU + Memory gauges
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let cpu_pct = *app.cpu_total_history.back().unwrap_or(&0.0);
    // Blink alert: alternate color every 2 ticks
    let alert_blink = app.alert_cpu && (app.tick / 2) % 2 == 0;
    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL)
            .title(if app.alert_cpu { " CPU ⚠ HIGH " } else { " CPU " })
            .border_style(if alert_blink { Style::default().fg(RED) } else { Style::default() }))
        .gauge_style(Style::default().fg(cpu_color(cpu_pct)))
        .percent(cpu_pct as u16)
        .label(format!("{:.1}%", cpu_pct));
    f.render_widget(cpu_gauge, top[0]);

    let mem_pct = if app.mem_total > 0 {
        app.mem_used as f64 / app.mem_total as f64 * 100.0
    } else { 0.0 };
    let mem_blink = app.alert_mem && (app.tick / 2) % 2 == 0;
    let mem_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL)
            .title(if app.alert_mem {
                format!(" 内存 ⚠ HIGH  {}/{} ", fmt_bytes(app.mem_used), fmt_bytes(app.mem_total))
            } else {
                format!(" 内存  {}/{} ", fmt_bytes(app.mem_used), fmt_bytes(app.mem_total))
            })
            .border_style(if mem_blink { Style::default().fg(RED) } else { Style::default() }))
        .gauge_style(Style::default().fg(mem_color(mem_pct)))
        .percent(mem_pct as u16)
        .label(format!("{:.1}%", mem_pct));
    f.render_widget(mem_gauge, top[1]);

    // Row 2: Net + Disk sparklines
    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    let rx_data: Vec<u64> = app.net_rx_history.iter().map(|&v| v as u64).collect();
    let net_spark = Sparkline::default()
        .block(Block::default().borders(Borders::ALL)
            .title(format!(" 网络 ↓{}/s ↑{}/s ", fmt_bytes(app.net_rx_rate as u64), fmt_bytes(app.net_tx_rate as u64))))
        .data(&rx_data)
        .style(Style::default().fg(GREEN));
    f.render_widget(net_spark, mid[0]);

    let dr_data: Vec<u64> = app.disk_read_history.iter().map(|&v| v as u64).collect();
    let disk_spark = Sparkline::default()
        .block(Block::default().borders(Borders::ALL)
            .title(format!(" 磁盘 R:{}/s W:{}/s ", fmt_bytes(app.disk_read_rate as u64), fmt_bytes(app.disk_write_rate as u64))))
        .data(&dr_data)
        .style(Style::default().fg(YELLOW));
    f.render_widget(disk_spark, mid[1]);

    // Row 3: CPU history sparkline + top processes
    let bot = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[2]);

    let cpu_data: Vec<u64> = app.cpu_total_history.iter().map(|&v| v as u64).collect();
    let cpu_spark = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(" CPU 历史 "))
        .data(&cpu_data)
        .style(Style::default().fg(CYAN));
    f.render_widget(cpu_spark, bot[0]);

    // Top 10 processes by CPU
    let header = Row::new(vec!["PID", "进程名", "CPU%", "内存"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));
    let rows_data: Vec<Row> = app.processes.iter().take(10).map(|p| {
        Row::new(vec![
            Cell::from(p.pid.to_string()),
            Cell::from(p.name.chars().take(20).collect::<String>()),
            Cell::from(format!("{:.1}", p.cpu)).style(Style::default().fg(cpu_color(p.cpu as f64))),
            Cell::from(format!("{:.0}M", p.mem_mb)),
        ])
    }).collect();
    let table = Table::new(rows_data, [
        Constraint::Length(7), Constraint::Min(20),
        Constraint::Length(7), Constraint::Length(8),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Top 进程 (CPU) "));
    f.render_widget(table, bot[1]);
}

// ─── CPU ─────────────────────────────────────────────────────────────────────

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

    // Build col_constraints once, reuse across rows
    let col_constraints: Vec<Constraint> = (0..cols)
        .map(|_| Constraint::Ratio(1, cols as u32))
        .collect();

    // Reusable buffer to avoid per-core Vec allocation
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
            // Reuse buffer
            data_buf.clear();
            data_buf.extend(app.cpu_history[idx].iter().map(|&v| v as u64));
            let spark = Sparkline::default()
                .block(Block::default().borders(Borders::ALL)
                    .title(format!(" Core {} — {:.1}% ", idx, usage))
                    .title_style(Style::default().fg(cpu_color(usage))))
                .data(&data_buf)
                .max(100)
                .style(Style::default().fg(cpu_color(usage)));
            f.render_widget(spark, col_chunks[col]);
        }
    }
}

// ─── Memory ──────────────────────────────────────────────────────────────────

fn draw_memory(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Length(4), Constraint::Min(0)])
        .split(area);

    let mem_pct = if app.mem_total > 0 {
        app.mem_used as f64 / app.mem_total as f64 * 100.0
    } else { 0.0 };
    let mem_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL)
            .title(format!(" RAM  已用 {} / 总计 {} ", fmt_bytes(app.mem_used), fmt_bytes(app.mem_total))))
        .gauge_style(Style::default().fg(mem_color(mem_pct)))
        .percent(mem_pct as u16)
        .label(format!("{:.1}%  空闲 {}", mem_pct, fmt_bytes(app.mem_total.saturating_sub(app.mem_used))));
    f.render_widget(mem_gauge, chunks[0]);

    let swap_pct = if app.swap_total > 0 {
        app.swap_used as f64 / app.swap_total as f64 * 100.0
    } else { 0.0 };
    let swap_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL)
            .title(format!(" Swap  已用 {} / 总计 {} ", fmt_bytes(app.swap_used), fmt_bytes(app.swap_total))))
        .gauge_style(Style::default().fg(BLUE))
        .percent(swap_pct as u16)
        .label(format!("{:.1}%", swap_pct));
    f.render_widget(swap_gauge, chunks[1]);

    // Memory history chart
    let mem_data: Vec<(f64, f64)> = app.mem_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();
    let dataset = Dataset::default()
        .name("RAM %")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(GREEN))
        .data(&mem_data);
    let chart = Chart::new(vec![dataset])
        .block(Block::default().borders(Borders::ALL).title(" 内存使用历史 "))
        .x_axis(Axis::default().bounds([0.0, HISTORY as f64])
            .labels(vec![Span::raw("60s"), Span::raw("30s"), Span::raw("0s")]))
        .y_axis(Axis::default().bounds([0.0, 100.0])
            .labels(vec![Span::raw("0%"), Span::raw("50%"), Span::raw("100%")]));
    f.render_widget(chart, chunks[2]);
}

// ─── Network ─────────────────────────────────────────────────────────────────

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

    let rx_ds = Dataset::default().name("↓ 下载")
        .marker(symbols::Marker::Braille).graph_type(GraphType::Line)
        .style(Style::default().fg(GREEN)).data(&rx_data);
    let tx_ds = Dataset::default().name("↑ 上传")
        .marker(symbols::Marker::Braille).graph_type(GraphType::Line)
        .style(Style::default().fg(YELLOW)).data(&tx_data);

    let net_chart = Chart::new(vec![rx_ds, tx_ds])
        .block(Block::default().borders(Borders::ALL)
            .title(format!(" 网络 IO  ↓{}/s  ↑{}/s ",
                fmt_bytes(app.net_rx_rate as u64), fmt_bytes(app.net_tx_rate as u64))))
        .x_axis(Axis::default().bounds([0.0, HISTORY as f64]))
        .y_axis(Axis::default().bounds([0.0, max_net * 1.1])
            .labels(vec![Span::raw("0"), Span::raw(fmt_bytes((max_net * 512.0) as u64)),
                         Span::raw(fmt_bytes((max_net * 1024.0) as u64))]));
    f.render_widget(net_chart, chunks[0]);

    // Per-interface bar chart — build bar_data directly, skip intermediate Vec
    let bar_data: Vec<(String, u64)> = app.networks.iter()
        .map(|(name, n)| (name.clone(), (n.received() + n.transmitted()) / 1024))
        .collect();
    let bar_refs: Vec<(&str, u64)> = bar_data.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    let bar = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(" 接口流量 (KB/s) "))
        .data(&bar_refs)
        .bar_width(9)
        .bar_gap(1)
        .bar_style(Style::default().fg(CYAN))
        .value_style(Style::default().fg(WHITE).add_modifier(Modifier::BOLD));
    f.render_widget(bar, chunks[1]);
}

// ─── Disk ────────────────────────────────────────────────────────────────────

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

    let rd_ds = Dataset::default().name("读取")
        .marker(symbols::Marker::Braille).graph_type(GraphType::Line)
        .style(Style::default().fg(CYAN)).data(&rd_data);
    let wr_ds = Dataset::default().name("写入")
        .marker(symbols::Marker::Braille).graph_type(GraphType::Line)
        .style(Style::default().fg(MAGENTA)).data(&wr_data);

    let disk_chart = Chart::new(vec![rd_ds, wr_ds])
        .block(Block::default().borders(Borders::ALL)
            .title(format!(" 磁盘 IO  读:{}/s  写:{}/s ",
                fmt_bytes(app.disk_read_rate as u64), fmt_bytes(app.disk_write_rate as u64))))
        .x_axis(Axis::default().bounds([0.0, HISTORY as f64]))
        .y_axis(Axis::default().bounds([0.0, max_disk * 1.1])
            .labels(vec![Span::raw("0"), Span::raw(fmt_bytes((max_disk * 512.0) as u64)),
                         Span::raw(fmt_bytes((max_disk * 1024.0) as u64))]));
    f.render_widget(disk_chart, chunks[0]);

    // Disk usage per mount
    let disk_rows: Vec<Row> = app.disks.iter().map(|d| {
        let total = d.total_space();
        let avail = d.available_space();
        let used = total.saturating_sub(avail);
        let pct = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
        let bar_len = (pct / 5.0) as usize;
        let bar = format!("[{}{}] {:.0}%",
            "█".repeat(bar_len), "░".repeat(20usize.saturating_sub(bar_len)), pct);
        Row::new(vec![
            Cell::from(d.mount_point().to_string_lossy().to_string()),
            Cell::from(d.name().to_string_lossy().to_string()),
            Cell::from(fmt_bytes(total)),
            Cell::from(fmt_bytes(used)),
            Cell::from(fmt_bytes(avail)),
            Cell::from(bar).style(Style::default().fg(if pct > 90.0 { RED } else if pct > 70.0 { YELLOW } else { GREEN })),
        ])
    }).collect();

    let header = Row::new(vec!["挂载点", "设备", "总计", "已用", "可用", "使用率"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));
    let table = Table::new(disk_rows, [
        Constraint::Min(15), Constraint::Min(12),
        Constraint::Length(9), Constraint::Length(9),
        Constraint::Length(9), Constraint::Min(25),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" 磁盘分区 "));
    f.render_widget(table, chunks[1]);
}

// ─── Processes ───────────────────────────────────────────────────────────────

fn draw_processes(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Sort + filter hint bar
    let filter_display = if app.proc_filter.is_empty() {
        "/ 过滤".to_string()
    } else {
        format!("/ 过滤: {}_", app.proc_filter)
    };
    let sort_hint = Paragraph::new(Line::from(vec![
        Span::raw(" 排序: "),
        sort_span("C CPU", app.proc_sort == ProcSort::Cpu),
        Span::raw("  "),
        sort_span("M 内存", app.proc_sort == ProcSort::Mem),
        Span::raw("  "),
        sort_span("P PID", app.proc_sort == ProcSort::Pid),
        Span::raw("  "),
        sort_span("N 名称", app.proc_sort == ProcSort::Name),
        Span::raw("  "),
        Span::styled("↑↓ 导航", Style::default().fg(GRAY)),
        Span::raw("  "),
        Span::styled(
            format!("{}/{}", app.proc_selected + 1, app.processes.len()),
            Style::default().fg(YELLOW)
        ),
        Span::raw("  "),
        Span::styled(
            filter_display,
            if app.proc_filter_active {
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(GRAY)
            }
        ),
        Span::raw("  "),
        Span::styled("K 结束  Shift+K 强杀", Style::default().fg(RED)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(sort_hint, chunks[0]);

    let header = Row::new(vec!["PID", "进程名", "CPU%", "内存", "状态"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app.processes.iter().enumerate().map(|(i, p)| {
        let style = if i == app.proc_selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(p.pid.to_string()),
            Cell::from(p.name.chars().take(30).collect::<String>()),
            Cell::from(format!("{:.1}", p.cpu))
                .style(Style::default().fg(cpu_color(p.cpu as f64))),
            Cell::from(format!("{:.1} MB", p.mem_mb)),
            Cell::from(p.status.clone()).style(Style::default().fg(GRAY)),
        ]).style(style)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(8), Constraint::Min(25),
        Constraint::Length(7), Constraint::Length(10), Constraint::Length(12),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" 进程列表 "));
    f.render_widget(table, chunks[1]);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn cpu_color(pct: f64) -> Color {
    if pct >= 90.0 { RED } else if pct >= 60.0 { YELLOW } else { GREEN }
}

fn mem_color(pct: f64) -> Color {
    if pct >= 90.0 { RED } else if pct >= 70.0 { YELLOW } else { BLUE }
}

fn sort_span(label: &str, active: bool) -> Span<'_> {
    if active {
        Span::styled(label, Style::default().fg(CYAN).add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
    } else {
        Span::styled(label, Style::default().fg(WHITE))
    }
}

pub fn fmt_bytes(b: u64) -> String {
    if b < 1024 { format!("{} B", b) }
    else if b < 1024 * 1024 { format!("{:.1} KB", b as f64 / 1024.0) }
    else if b < 1024 * 1024 * 1024 { format!("{:.1} MB", b as f64 / 1024.0 / 1024.0) }
    else { format!("{:.2} GB", b as f64 / 1024.0 / 1024.0 / 1024.0) }
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![Span::styled(" 键盘快捷键 ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![Span::styled(" 切换页面 ", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))]),
        Line::from(" 1-6       切换到对应标签页"),
        Line::from(" Tab       切换到下一页"),
        Line::from(" Shift+Tab 切换到上一页"),
        Line::from(""),
        Line::from(vec![Span::styled(" 进程列表 ", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))]),
        Line::from(" ↑ / k     向上移动"),
        Line::from(" ↓ / j     向下移动"),
        Line::from(" g / Home  跳到顶部"),
        Line::from(" G / End   跳到底部"),
        Line::from(" c         按 CPU 排序"),
        Line::from(" m         按内存排序"),
        Line::from(" p         按 PID 排序"),
        Line::from(" n         按名称排序"),
        Line::from(" /         输入过滤关键词"),
        Line::from(" Esc       清除过滤"),
        Line::from(""),
        Line::from(vec![Span::styled(" 刷新速度 ", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))]),
        Line::from(" +         加快刷新 (最快 200ms)"),
        Line::from(" -         减慢刷新 (最慢 5000ms)"),
        Line::from(""),
        Line::from(vec![Span::styled(" 其他 ", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))]),
        Line::from(" ?         显示/隐藏此帮助页"),
        Line::from(" q / Ctrl+C 退出"),
        Line::from(""),
        Line::from(vec![Span::styled(" 按任意键关闭 ", Style::default().fg(GRAY))]),
    ];

    let popup_area = centered_rect(50, 80, area);
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL)
            .title(" ❓ 帮助 ")
            .title_style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD)))
        .style(Style::default().fg(WHITE));
    // Clear background
    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(help, popup_area);
}

// ─── Scanner ───────────────────────────────────────────────────────────────────────────────

fn draw_scanner(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // config panel
            Constraint::Length(3),  // status bar
            Constraint::Min(0),     // results
        ])
        .split(area);

    // ─ Config panel ─
    let cfg_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    // Left: host + ports input
    let input_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(cfg_cols[0]);

    let host_style = if app.scan_input_active && app.scan_input_field == 0 {
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
    } else { Style::default().fg(WHITE) };
    let host_block = Block::default().borders(Borders::ALL)
        .title(" 目标地址 (IP / IP段 / CIDR) ")
        .border_style(host_style);
    let host_text = if app.scan_input_active && app.scan_input_field == 0 {
        format!("{}_", app.scan_host)
    } else {
        app.scan_host.clone()
    };
    f.render_widget(
        Paragraph::new(host_text).block(host_block),
        input_rows[0],
    );

    let port_style = if app.scan_input_active && app.scan_input_field == 1 {
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
    } else { Style::default().fg(WHITE) };
    let port_block = Block::default().borders(Borders::ALL)
        .title(" 端口 (80,443,22-25) ")
        .border_style(port_style);
    let port_text = if app.scan_input_active && app.scan_input_field == 1 {
        format!("{}_", app.scan_ports)
    } else {
        app.scan_ports.clone()
    };
    f.render_widget(
        Paragraph::new(port_text).block(port_block),
        input_rows[1],
    );

    // Right: presets + options
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(cfg_cols[1]);

    let presets = Paragraph::new(Line::from(vec![
        Span::raw(" 预设: "),
        Span::styled("F1 常用", Style::default().fg(YELLOW)),
        Span::raw("  "),
        Span::styled("F2 Web", Style::default().fg(YELLOW)),
        Span::raw("  "),
        Span::styled("F3 数据库", Style::default().fg(YELLOW)),
        Span::raw("  "),
        Span::styled("F4 远程", Style::default().fg(YELLOW)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" 端口预设 "));
    f.render_widget(presets, right_rows[0]);

    let opts = Paragraph::new(Line::from(vec![
        Span::raw(" 超时: "),
        Span::styled(format!("{}ms", app.scan_timeout_ms), Style::default().fg(CYAN)),
        Span::raw("  并发: "),
        Span::styled(format!("{}", app.scan_concurrency), Style::default().fg(CYAN)),
        Span::raw("  "),
        Span::styled(
            if app.scan_show_closed { "[✓] 显示关闭" } else { "[ ] 显示关闭" },
            Style::default().fg(GRAY),
        ),
        Span::raw("  "),
        if app.scan_running {
            Span::styled("⏹ 停止 [Esc]", Style::default().fg(RED).add_modifier(Modifier::BOLD))
        } else {
            Span::styled("▶ 扫描 [Enter]", Style::default().fg(GREEN).add_modifier(Modifier::BOLD))
        },
    ]))
    .block(Block::default().borders(Borders::ALL).title(" 选项 "));
    f.render_widget(opts, right_rows[1]);

    // ─ Status bar ─
    let progress = if app.scan_total > 0 {
        let pct = app.scan_done as f64 / app.scan_total as f64;
        let bar_w = (chunks[1].width.saturating_sub(4)) as usize;
        let filled = (pct * bar_w as f64) as usize;
        format!(
            " [{}>{}] {}/{} 开放:{} ",
            "=".repeat(filled),
            " ".repeat(bar_w.saturating_sub(filled)),
            app.scan_done, app.scan_total, app.scan_open
        )
    } else {
        String::new()
    };
    let status_color = if app.scan_running { YELLOW } else { GREEN };
    let status = Paragraph::new(Line::from(vec![
        Span::styled(&app.scan_status, Style::default().fg(status_color)),
        Span::styled(progress, Style::default().fg(CYAN)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[1]);

    // ─ Results table ─
    let visible: Vec<&crate::scanner::ScanResult> = app.scan_results.iter()
        .filter(|r| app.scan_show_closed || r.open)
        .collect();

    let header = Row::new(vec!["IP地址", "端口", "状态", "延迟", "服务"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = visible.iter().enumerate().map(|(i, r)| {
        let sel_style = if i == app.scan_selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let (status_str, status_color) = if r.open {
            ("✅ 开放", GREEN)
        } else {
            ("❌ 关闭", GRAY)
        };
        let latency = r.latency_ms
            .map(|ms| format!("{:.1}ms", ms))
            .unwrap_or_else(|| "-".to_string());
        Row::new(vec![
            Cell::from(r.host.clone()),
            Cell::from(r.port.to_string()),
            Cell::from(status_str).style(Style::default().fg(status_color)),
            Cell::from(latency).style(Style::default().fg(
                if r.latency_ms.map_or(false, |ms| ms < 10.0) { GREEN }
                else if r.latency_ms.map_or(false, |ms| ms < 100.0) { YELLOW }
                else { RED }
            )),
            Cell::from(r.service).style(Style::default().fg(MAGENTA)),
        ]).style(sel_style)
    }).collect();

    let open_count = app.scan_results.iter().filter(|r| r.open).count();
    let title = format!(
        " 扫描结果  开放: {}  共: {}  (↑↓导航  H 切换显示关闭端口) ",
        open_count, app.scan_results.len()
    );
    let table = Table::new(rows, [
        Constraint::Min(16),
        Constraint::Length(7),
        Constraint::Length(9),
        Constraint::Length(10),
        Constraint::Min(12),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(table, chunks[2]);
}

// ─── Git Analyzer ───────────────────────────────────────────────────────────────────────────

fn draw_git(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // path input (full width)
            Constraint::Length(3),  // sub-tabs + hints
            Constraint::Length(3),  // status
            Constraint::Min(0),
        ])
        .split(area);

    // ─ Full-width path input ─
    let hist_hint = if !app.git_history.is_empty() {
        format!(" ↑↓历史({})  Tab补全  Enter分析  Esc退出输入", app.git_history.len())
    } else {
        " Tab补全  Enter分析  Esc退出输入".to_string()
    };
    let path_text = if app.git_input_active {
        format!("{}_", app.git_path)
    } else {
        app.git_path.clone()
    };
    let path_style = if app.git_input_active {
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(WHITE)
    };
    let path_title = if app.git_input_active {
        format!(" Git 仓库路径{} ", hist_hint)
    } else {
        " Git 仓库路径  (i 或 Enter 开始输入) ".to_string()
    };
    f.render_widget(
        Paragraph::new(path_text)
            .block(Block::default().borders(Borders::ALL)
                .title(path_title)
                .border_style(path_style)),
        chunks[0],
    );

    // ─ Sub-tabs + history list ─
    let top2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    let sub_tabs = Tabs::new(vec![" 概览(o) ", " 作者(a) ", " 热点文件(f) ", " 热力图(h) "])
        .select(app.git_tab)
        .block(Block::default().borders(Borders::ALL).title(" 视图 "))
        .highlight_style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD))
        .divider("|");
    f.render_widget(sub_tabs, top2[0]);

    // Recent history quick-view
    let hist_spans: Vec<Span> = std::iter::once(Span::styled(" 最近: ", Style::default().fg(GRAY)))
        .chain(app.git_history.iter().take(3).enumerate().map(|(i, p)| {
            let label = std::path::Path::new(p)
                .file_name().and_then(|n| n.to_str()).unwrap_or(p);
            let color = if app.git_history_idx == Some(i) { CYAN } else { YELLOW };
            Span::styled(format!("[{}]{} ", i + 1, label.chars().take(12).collect::<String>()), Style::default().fg(color))
        }))
        .collect();
    f.render_widget(
        Paragraph::new(Line::from(hist_spans))
            .block(Block::default().borders(Borders::ALL).title(" 历史路径 ")),
        top2[1],
    );

    // ─ Status / progress ─
    let prog_text = if app.git_progress.1 > 0 {
        let pct = app.git_progress.0 as f64 / app.git_progress.1 as f64;
        let w = (chunks[2].width.saturating_sub(4)) as usize;
        let filled = (pct * w as f64) as usize;
        format!(" [{}>{}] {}/{} ",
            "=".repeat(filled), " ".repeat(w.saturating_sub(filled)),
            app.git_progress.0, app.git_progress.1)
    } else { String::new() };
    let status_color = if app.git_running { YELLOW } else if app.git_stats.is_some() { GREEN } else { GRAY };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(&app.git_status, Style::default().fg(status_color)),
            Span::styled(prog_text, Style::default().fg(CYAN)),
        ])).block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );

    // ─ Content area ─
    let Some(ref stats) = app.git_stats else {
        f.render_widget(
            Paragraph::new(" 输入仓库路径后按 Enter 开始分析\n\n 支持格式: /path/to/repo  或  C:\\path\\to\\repo\n\n 提示: 切换到此页时自动进入输入模式，Tab 可补全目录")
                .block(Block::default().borders(Borders::ALL).title(" 提示 "))
                .style(Style::default().fg(GRAY)),
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

    // Left: summary
    let info = vec![
        Line::from(vec![Span::styled(" 仓库: ", Style::default().fg(GRAY)), Span::styled(&stats.repo_path, Style::default().fg(WHITE))]),
        Line::from(""),
        Line::from(vec![Span::styled(" Commits:  ", Style::default().fg(GRAY)), Span::styled(stats.total_commits.to_string(), Style::default().fg(CYAN).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled(" 作者数:  ", Style::default().fg(GRAY)), Span::styled(stats.total_authors.to_string(), Style::default().fg(GREEN).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled(" 文件数:  ", Style::default().fg(GRAY)), Span::styled(stats.total_files.to_string(), Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![Span::styled(" 首次提交: ", Style::default().fg(GRAY)), Span::raw(&stats.first_commit)]),
        Line::from(vec![Span::styled(" 最新提交: ", Style::default().fg(GRAY)), Span::raw(&stats.last_commit)]),
    ];
    f.render_widget(
        Paragraph::new(info).block(Block::default().borders(Borders::ALL).title(" 仓库概要 ")),
        cols[0],
    );

    // Right: 52-week activity sparkline
    let act_data: Vec<u64> = stats.activity.iter().map(|&v| v as u64).collect();
    let spark = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(" 过去 52 周提交活跃度 "))
        .data(&act_data)
        .style(Style::default().fg(GREEN));
    f.render_widget(spark, cols[1]);
}

fn draw_git_authors(f: &mut Frame, stats: &crate::gitanalyzer::GitStats, selected: usize, area: Rect) {
    let header = Row::new(vec!["作者", "Commits", "新增行", "删除行", "触动文件"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));

    let max_commits = stats.authors.first().map(|a| a.commits).unwrap_or(1);
    let rows: Vec<Row> = stats.authors.iter().enumerate().map(|(i, a)| {
        let bar_w = 15usize;
        let filled = (a.commits as f64 / max_commits as f64 * bar_w as f64) as usize;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(bar_w - filled));
        let sel = if i == selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else { Style::default() };
        Row::new(vec![
            Cell::from(a.name.chars().take(25).collect::<String>()),
            Cell::from(format!("{} {}", a.commits, bar)).style(Style::default().fg(CYAN)),
            Cell::from(format!("+{}", a.additions)).style(Style::default().fg(GREEN)),
            Cell::from(format!("-{}", a.deletions)).style(Style::default().fg(RED)),
            Cell::from(a.files_touched.to_string()).style(Style::default().fg(YELLOW)),
        ]).style(sel)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Min(20), Constraint::Min(25),
        Constraint::Length(12), Constraint::Length(12), Constraint::Length(10),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL)
        .title(format!(" 作者贡献榜 (共 {} 人) ", stats.authors.len())));
    f.render_widget(table, area);
}

fn draw_git_hotfiles(f: &mut Frame, stats: &crate::gitanalyzer::GitStats, selected: usize, area: Rect) {
    let header = Row::new(vec!["文件路径", "修改次数", "新增", "删除", "作者数"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));

    let max_changes = stats.hot_files.first().map(|f| f.changes).unwrap_or(1);
    let rows: Vec<Row> = stats.hot_files.iter().enumerate().map(|(i, file)| {
        let heat = file.changes as f64 / max_changes as f64;
        let color = if heat > 0.7 { RED } else if heat > 0.4 { YELLOW } else { GREEN };
        let sel = if i == selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else { Style::default() };
        // Show tail of path — safe char-boundary truncation for UTF-8
        let path_display = if file.path.chars().count() > 45 {
            let tail: String = file.path.chars().rev().take(42).collect::<String>()
                .chars().rev().collect();
            format!("...{}", tail)
        } else {
            file.path.clone()
        };
        Row::new(vec![
            Cell::from(path_display),
            Cell::from(file.changes.to_string()).style(Style::default().fg(color)),
            Cell::from(format!("+{}", file.additions)).style(Style::default().fg(GREEN)),
            Cell::from(format!("-{}", file.deletions)).style(Style::default().fg(RED)),
            Cell::from(file.authors.to_string()).style(Style::default().fg(MAGENTA)),
        ]).style(sel)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Min(35), Constraint::Length(10),
        Constraint::Length(10), Constraint::Length(10), Constraint::Length(8),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL)
        .title(" 热点文件 (Top 100 修改最频繁) "));
    f.render_widget(table, area);
}

fn draw_git_heatmap(f: &mut Frame, stats: &crate::gitanalyzer::GitStats, area: Rect) {
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let max_val = stats.heatmap.iter().flat_map(|row| row.iter()).copied().max().unwrap_or(1);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "     00 01 02 03 04 05 06 07 08 09 10 11 12 13 14 15 16 17 18 19 20 21 22 23",
        Style::default().fg(GRAY),
    )));

    for (d, day) in days.iter().enumerate() {
        let mut spans = vec![Span::styled(format!(" {} ", day), Style::default().fg(CYAN))];
        for h in 0..24 {
            let v = stats.heatmap[d][h];
            let heat = v as f64 / max_val as f64;
            let (ch, color) = if v == 0 {
                ("· ", GRAY)
            } else if heat < 0.25 {
                ("░ ", Color::Rgb(0, 100, 0))
            } else if heat < 0.5 {
                ("▒ ", Color::Rgb(0, 180, 0))
            } else if heat < 0.75 {
                ("▓ ", YELLOW)
            } else {
                ("█ ", RED)
            };
            spans.push(Span::styled(ch, Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw(" 图例: "),
        Span::styled("· 无", Style::default().fg(GRAY)), Span::raw("  "),
        Span::styled("░ 少", Style::default().fg(Color::Rgb(0, 100, 0))), Span::raw("  "),
        Span::styled("▒ 中", Style::default().fg(Color::Rgb(0, 180, 0))), Span::raw("  "),
        Span::styled("▓ 多", Style::default().fg(YELLOW)), Span::raw("  "),
        Span::styled("█ 最多", Style::default().fg(RED)),
    ]));

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL)
                .title(" 提交时间热力图 (星期 × 小时) ")),
        area,
    );
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

mod app;
mod gitanalyzer;
mod scanner;
mod ui;

use app::{App, ProcSort};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers,
            MouseEvent, MouseEventKind, MouseButton},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::mpsc, time::Instant};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    if let Err(e) = res { eprintln!("Error: {e}"); }
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    let mut last_tick = Instant::now();

    loop {
        // Drain scan + git results before drawing
        let scan_updated = app.drain_scan();
        let git_updated  = app.drain_git();
        let _ = (scan_updated, git_updated);

        terminal.draw(|f| ui::draw(f, &app))?;

        let tick_rate = app.tick_duration();
        let timeout = tick_rate.checked_sub(last_tick.elapsed()).unwrap_or_default();
        if event::poll(timeout)? {
            match event::read()? {
                Event::Mouse(mouse) => handle_mouse(&mut app, mouse),
                Event::Key(key) => {
                // Help overlay: any key closes
                if app.show_help {
                    app.show_help = false;
                    continue;
                }

                // Git input mode
                if app.git_input_active {
                    match key.code {
                        KeyCode::Esc   => { app.git_input_active = false; }
                        KeyCode::Tab   => { app.git_path_complete(); }
                        KeyCode::Up    => { app.git_history_up(); }
                        KeyCode::Down  => { app.git_history_down(); }
                        KeyCode::Enter => {
                            app.git_input_active = false;
                            app.git_history_push();
                            start_git_analysis(&mut app);
                        }
                        KeyCode::Backspace => {
                            app.git_history_idx = None;
                            app.git_path.pop();
                        }
                        KeyCode::Char(c) => {
                            app.git_history_idx = None;
                            app.git_path.push(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                // Scanner input mode
                if app.scan_input_active {
                    match key.code {
                        KeyCode::Esc => { app.scan_input_active = false; }
                        KeyCode::Tab => {
                            app.scan_input_field = 1 - app.scan_input_field;
                        }
                        KeyCode::Enter => {
                            app.scan_input_active = false;
                            start_scan(&mut app);
                        }
                        KeyCode::Backspace => {
                            if app.scan_input_field == 0 { app.scan_host.pop(); }
                            else { app.scan_ports.pop(); }
                        }
                        KeyCode::Char(c) => {
                            if app.scan_input_field == 0 { app.scan_host.push(c); }
                            else { app.scan_ports.push(c); }
                        }
                        _ => {}
                    }
                    continue;
                }

                // Process filter input mode
                if app.proc_filter_active {
                    match key.code {
                        KeyCode::Esc => { app.proc_filter.clear(); app.proc_filter_active = false; }
                        KeyCode::Enter => { app.proc_filter_active = false; }
                        KeyCode::Backspace => { app.proc_filter.pop(); }
                        KeyCode::Char(c) => { app.proc_filter.push(c); }
                        _ => {}
                    }
                    continue;
                }

                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) |
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(()),

                    (KeyCode::Char('?'), _) => app.show_help = !app.show_help,

                    // Tab switching
                    (KeyCode::Char('1'), _) => { app.tab = 0; }
                    (KeyCode::Char('2'), _) => { app.tab = 1; }
                    (KeyCode::Char('3'), _) => { app.tab = 2; }
                    (KeyCode::Char('4'), _) => { app.tab = 3; }
                    (KeyCode::Char('5'), _) => { app.tab = 4; }
                    (KeyCode::Char('6'), _) => { app.tab = 5; }
                    (KeyCode::Char('7'), _) => { app.tab = 6; }
                    (KeyCode::Char('8'), _) => { app.tab = 7; app.git_input_active = true; }
                    (KeyCode::Tab, _) => {
                        app.tab = (app.tab + 1) % 8;
                        if app.tab == 7 { app.git_input_active = true; }
                    }
                    (KeyCode::BackTab, _) => {
                        app.tab = (app.tab + 7) % 8;
                        if app.tab == 7 { app.git_input_active = true; }
                    }

                    // Tick rate
                    (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
                        app.tick_rate_ms = (app.tick_rate_ms.saturating_sub(200)).max(200);
                    }
                    (KeyCode::Char('-'), _) => {
                        app.tick_rate_ms = (app.tick_rate_ms + 500).min(5000);
                    }

                    // Process filter
                    (KeyCode::Char('/'), _) => {
                        app.tab = 5;
                        app.proc_filter_active = true;
                    }
                    (KeyCode::Esc, _) => {
                        app.proc_filter.clear();
                        app.proc_filter_active = false;
                        if app.scan_running {
                            app.scan_running = false;
                            app.scan_rx = None;
                            app.scan_status = "已取消".to_string();
                        }
                    }

                    // Git tab keys
                    (KeyCode::Enter, _) if app.tab == 7 => {
                        if app.git_running {
                            app.git_running = false;
                            app.git_rx = None;
                            app.git_status = "已取消".to_string();
                        } else {
                            app.git_input_active = true;
                        }
                    }
                    (KeyCode::Char('i'), _) if app.tab == 7 => { app.git_input_active = true; }
                    // Git sub-tab switching
                    (KeyCode::Char('a'), _) if app.tab == 7 => app.git_tab = 1,
                    (KeyCode::Char('f'), _) if app.tab == 7 => app.git_tab = 2,
                    (KeyCode::Char('h'), _) if app.tab == 7 => app.git_tab = 3,
                    (KeyCode::Char('o'), _) if app.tab == 7 => app.git_tab = 0,

                    // Scanner tab keys
                    (KeyCode::Enter, _) if app.tab == 6 => {
                        if app.scan_running {
                            app.scan_running = false;
                            app.scan_rx = None;
                            app.scan_status = "已取消".to_string();
                        } else {
                            app.scan_input_active = true;
                            app.scan_input_field = 0;
                        }
                    }
                    (KeyCode::Char('i'), _) if app.tab == 6 => {
                        app.scan_input_active = true;
                        app.scan_input_field = 0;
                    }
                    (KeyCode::Char('h'), _) | (KeyCode::Char('H'), _) if app.tab == 6 => {
                        app.scan_show_closed = !app.scan_show_closed;
                    }
                    // Preset ports via F-keys
                    (KeyCode::F(1), _) if app.tab == 6 => {
                        app.scan_ports = scanner::preset_ports("常用").to_string();
                    }
                    (KeyCode::F(2), _) if app.tab == 6 => {
                        app.scan_ports = scanner::preset_ports("Web").to_string();
                    }
                    (KeyCode::F(3), _) if app.tab == 6 => {
                        app.scan_ports = scanner::preset_ports("数据库").to_string();
                    }
                    (KeyCode::F(4), _) if app.tab == 6 => {
                        app.scan_ports = scanner::preset_ports("远程").to_string();
                    }

                    // Process / scanner / git navigation
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                        if app.tab == 7 {
                            if app.git_selected > 0 { app.git_selected -= 1; }
                        } else if app.tab == 6 {
                            if app.scan_selected > 0 { app.scan_selected -= 1; }
                        } else if app.proc_selected > 0 {
                            app.proc_selected -= 1;
                        }
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                        if app.tab == 7 {
                            let max = match app.git_tab {
                                1 => app.git_stats.as_ref().map(|s| s.authors.len()).unwrap_or(0),
                                2 => app.git_stats.as_ref().map(|s| s.hot_files.len()).unwrap_or(0),
                                _ => 0,
                            };
                            if app.git_selected + 1 < max { app.git_selected += 1; }
                        } else if app.tab == 6 {
                            let max = app.scan_results.iter()
                                .filter(|r| app.scan_show_closed || r.open).count();
                            if app.scan_selected + 1 < max { app.scan_selected += 1; }
                        } else if app.proc_selected + 1 < app.processes.len() {
                            app.proc_selected += 1;
                        }
                    }
                    (KeyCode::Home, _) | (KeyCode::Char('g'), _) => {
                        if app.tab == 6 { app.scan_selected = 0; }
                        else { app.proc_selected = 0; }
                    }
                    (KeyCode::End, _) | (KeyCode::Char('G'), _) => {
                        if app.tab == 6 {
                            let max = app.scan_results.iter()
                                .filter(|r| app.scan_show_closed || r.open).count();
                            app.scan_selected = max.saturating_sub(1);
                        } else {
                            app.proc_selected = app.processes.len().saturating_sub(1);
                        }
                    }

                    // Process sort (only when not on scanner tab)
                    (KeyCode::Char('c'), _) if app.tab != 6 => app.proc_sort = ProcSort::Cpu,
                    (KeyCode::Char('m'), _) if app.tab != 6 => app.proc_sort = ProcSort::Mem,
                    (KeyCode::Char('p'), _) if app.tab != 6 => app.proc_sort = ProcSort::Pid,
                    (KeyCode::Char('n'), _) if app.tab != 6 => app.proc_sort = ProcSort::Name,

                    // Kill process (tab 5 = processes)
                    (KeyCode::Char('K'), _) if app.tab == 5 => {
                        if let Some(err) = app.kill_selected(false) {
                            // Show error in status — reuse scan_status as temp message
                            app.scan_status = format!("Kill 失败: {}", err);
                        }
                    }
                    (KeyCode::Char('K'), KeyModifiers::SHIFT) if app.tab == 5 => {
                        if let Some(err) = app.kill_selected(true) {
                            app.scan_status = format!("SIGKILL 失败: {}", err);
                        }
                    }

                    _ => {}
                }
            } // end Key
                _ => {}
            } // end match event
        }

        if last_tick.elapsed() >= app.tick_duration() {
            app.update();
            last_tick = Instant::now();
        }
    }
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        // Click on tab bar (row 1, inside the 3-row tab block)
        MouseEventKind::Down(MouseButton::Left) if mouse.row <= 2 => {
            // Tab titles are roughly 10 chars each starting at col 1
            // " 总览 [1] " = ~10 chars, estimate tab index from column
            let col = mouse.column as usize;
            // Each tab ~10 wide, first tab starts at ~1
            let idx = col.saturating_sub(1) / 10;
            if idx < 8 { app.tab = idx; }
        }
        // Scroll in process list
        MouseEventKind::ScrollUp => {
            if app.tab == 5 && app.proc_selected > 0 {
                app.proc_selected -= 1;
            } else if app.tab == 6 && app.scan_selected > 0 {
                app.scan_selected -= 1;
            } else if app.tab == 7 && app.git_selected > 0 {
                app.git_selected -= 1;
            }
        }
        MouseEventKind::ScrollDown => {
            if app.tab == 5 && app.proc_selected + 1 < app.processes.len() {
                app.proc_selected += 1;
            } else if app.tab == 6 {
                let max = app.scan_results.iter()
                    .filter(|r| app.scan_show_closed || r.open).count();
                if app.scan_selected + 1 < max { app.scan_selected += 1; }
            } else if app.tab == 7 {
                let max = match app.git_tab {
                    1 => app.git_stats.as_ref().map(|s| s.authors.len()).unwrap_or(0),
                    2 => app.git_stats.as_ref().map(|s| s.hot_files.len()).unwrap_or(0),
                    _ => 0,
                };
                if app.git_selected + 1 < max { app.git_selected += 1; }
            }
        }
        _ => {}
    }
}

fn start_git_analysis(app: &mut App) {
    if app.git_running || app.git_path.is_empty() { return; }
    app.git_stats = None;
    app.git_selected = 0;
    app.git_progress = (0, 0);
    app.git_running = true;
    app.git_status = format!("正在打开仓库: {}", app.git_path);
    let (tx, rx) = mpsc::channel();
    app.git_rx = Some(rx);
    gitanalyzer::analyze(app.git_path.clone(), tx);
}

fn start_scan(app: &mut App) {
    if app.scan_running { return; }
    let hosts = scanner::parse_hosts(&app.scan_host);
    let ports = scanner::parse_ports(&app.scan_ports);
    if hosts.is_empty() || ports.is_empty() {
        app.scan_status = "地址或端口格式错误".to_string();
        return;
    }
    app.scan_results.clear();
    app.scan_selected = 0;
    app.scan_total = hosts.len() * ports.len();
    app.scan_done = 0;
    app.scan_open = 0;
    app.scan_running = true;
    app.scan_status = format!("扫描中… {} 个主机 × {} 个端口 = {} 个任务",
        hosts.len(), ports.len(), app.scan_total);

    let (tx, rx) = mpsc::channel();
    app.scan_rx = Some(rx);
    scanner::start_scan(hosts, ports, app.scan_timeout_ms, app.scan_concurrency, tx);
}

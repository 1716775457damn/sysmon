use sysinfo::{Disks, Networks, System};
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use crate::scanner::{ScanMsg, ScanResult};
use crate::gitanalyzer::{GitMsg, GitStats};

pub const HISTORY: usize = 60;

pub struct App {
    pub sys: System,
    pub disks: Disks,
    pub networks: Networks,

    // CPU
    pub cpu_history: Vec<VecDeque<f64>>,
    pub cpu_total_history: VecDeque<f64>,

    // Memory
    pub mem_total: u64,
    pub mem_used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub mem_history: VecDeque<f64>,

    // Network (bytes/s)
    pub net_rx_history: VecDeque<f64>,
    pub net_tx_history: VecDeque<f64>,
    pub net_rx_rate: f64,
    pub net_tx_rate: f64,
    prev_rx: u64,
    prev_tx: u64,

    // Disk (bytes/s)
    pub disk_read_history: VecDeque<f64>,
    pub disk_write_history: VecDeque<f64>,
    pub disk_read_rate: f64,
    pub disk_write_rate: f64,
    prev_disk_read: u64,
    prev_disk_write: u64,

    // Processes
    pub processes: Vec<ProcInfo>,
    pub proc_sort: ProcSort,
    pub proc_selected: usize,
    pub proc_filter: String,
    pub proc_filter_active: bool,

    // UI state
    pub tab: usize,
    pub tick: u64,
    pub tick_rate_ms: u64,
    pub show_help: bool,
    pub alert_cpu: bool,    // true when CPU > 90% threshold
    pub alert_mem: bool,    // true when mem > 90% threshold
    pub alert_tick: u64,    // tick when alert was last triggered (for blink)

    // Scanner
    pub scan_host: String,
    pub scan_ports: String,
    pub scan_timeout_ms: u64,
    pub scan_concurrency: usize,
    pub scan_results: Vec<ScanResult>,
    pub scan_running: bool,
    pub scan_status: String,
    pub scan_selected: usize,
    pub scan_show_closed: bool,
    pub scan_rx: Option<Receiver<ScanMsg>>,
    pub scan_input_field: usize, // 0=host 1=ports
    pub scan_input_active: bool,
    pub scan_total: usize,
    pub scan_done: usize,
    pub scan_open: usize,

    // Git analyzer
    pub git_path: String,
    pub git_input_active: bool,
    pub git_stats: Option<Box<GitStats>>,
    pub git_running: bool,
    pub git_status: String,
    pub git_progress: (usize, usize),
    pub git_rx: Option<Receiver<GitMsg>>,
    pub git_tab: usize,
    pub git_selected: usize,
}

#[derive(Clone)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_mb: f64,
    pub status: String,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ProcSort { Cpu, Mem, Pid, Name }

fn ring(size: usize) -> VecDeque<f64> {
    let mut d = VecDeque::with_capacity(size);
    for _ in 0..size { d.push_back(0.0); }
    d
}

impl App {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let core_count = sys.cpus().len().max(1);
        let mem_total  = sys.total_memory();
        let swap_total = sys.total_swap();
        let mut cpu_history = Vec::with_capacity(core_count);
        for _ in 0..core_count { cpu_history.push(ring(HISTORY)); }

        App {
            cpu_history,
            cpu_total_history: ring(HISTORY),
            mem_history: ring(HISTORY),
            net_rx_history: ring(HISTORY),
            net_tx_history: ring(HISTORY),
            disk_read_history: ring(HISTORY),
            disk_write_history: ring(HISTORY),
            mem_total,
            mem_used: sys.used_memory(),
            swap_total,
            swap_used: sys.used_swap(),
            net_rx_rate: 0.0, net_tx_rate: 0.0,
            prev_rx: 0, prev_tx: 0,
            disk_read_rate: 0.0, disk_write_rate: 0.0,
            prev_disk_read: 0, prev_disk_write: 0,
            processes: Vec::new(),
            proc_sort: ProcSort::Cpu,
            proc_selected: 0,
            proc_filter: String::new(),
            proc_filter_active: false,
            tab: 0,
            tick: 0,
            tick_rate_ms: 1000,
            show_help: false,
            alert_cpu: false,
            alert_mem: false,
            alert_tick: 0,
            scan_host: "192.168.1.1".to_string(),
            scan_ports: "21,22,80,443,3306,3389,8080".to_string(),
            scan_timeout_ms: 500,
            scan_concurrency: 100,
            scan_results: Vec::new(),
            scan_running: false,
            scan_status: "输入目标地址和端口，按 Enter 开始扫描".to_string(),
            scan_selected: 0,
            scan_show_closed: false,
            scan_rx: None,
            scan_input_field: 0,
            scan_input_active: false,
            scan_total: 0,
            scan_done: 0,
            scan_open: 0,
            git_path: String::new(),
            git_input_active: false,
            git_stats: None,
            git_running: false,
            git_status: "输入 Git 仓库路径，按 Enter 开始分析".to_string(),
            git_progress: (0, 0),
            git_rx: None,
            git_tab: 0,
            git_selected: 0,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            sys,
        }
    }

    /// Drain pending scan results from background thread
    pub fn drain_scan(&mut self) -> bool {
        let mut got = false;
        if let Some(ref rx) = self.scan_rx {
            loop {
                match rx.try_recv() {
                    Ok(ScanMsg::Result(r)) => {
                        self.scan_done += 1;
                        if r.open { self.scan_open += 1; }
                        self.scan_results.push(r);
                        got = true;
                    }
                    Ok(ScanMsg::Done { total, open, elapsed_ms }) => {
                        self.scan_running = false;
                        self.scan_status = format!(
                            "扫描完成  共 {} 个端口  开放 {}  耗时 {:.1}s",
                            total, open, elapsed_ms as f64 / 1000.0
                        );
                        // Sort: open first, then by host+port
                        self.scan_results.sort_by(|a, b| {
                            b.open.cmp(&a.open)
                                .then(a.host.cmp(&b.host))
                                .then(a.port.cmp(&b.port))
                        });
                        got = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
        if !self.scan_running { self.scan_rx = None; }
        got
    }

    pub fn drain_git(&mut self) -> bool {
        let mut got = false;
        if let Some(ref rx) = self.git_rx {
            loop {
                match rx.try_recv() {
                    Ok(GitMsg::Progress { done, total }) => {
                        self.git_progress = (done, total);
                        self.git_status = format!("分析中… {}/{} commits", done, total);
                        got = true;
                    }
                    Ok(GitMsg::Done(stats)) => {
                        self.git_status = format!(
                            "分析完成  {} commits  {} 作者  {} 文件",
                            stats.total_commits, stats.total_authors, stats.total_files
                        );
                        self.git_stats = Some(stats);
                        self.git_running = false;
                        got = true;
                        break;
                    }
                    Ok(GitMsg::Error(e)) => {
                        self.git_status = format!("错误: {}", e);
                        self.git_running = false;
                        got = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
        if !self.git_running { self.git_rx = None; }
        got
    }

    pub fn update(&mut self) {
        // Adaptive refresh: only collect heavy data when on relevant tab
        let need_net_disk = matches!(self.tab, 0 | 3 | 4);
        let need_proc     = matches!(self.tab, 0 | 5);

        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();

        if need_proc {
            self.sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::All,
                true,
                sysinfo::ProcessRefreshKind::nothing()
                    .with_cpu()
                    .with_memory(),
            );
        }
        if need_net_disk {
            self.disks.refresh(true);
            self.networks.refresh(true);
        }
        self.tick += 1;

        let core_count = self.sys.cpus().len();
        let mut total_sum = 0.0f64;
        for (i, cpu) in self.sys.cpus().iter().enumerate() {
            let u = cpu.cpu_usage() as f64;
            total_sum += u;
            if i < self.cpu_history.len() { push(&mut self.cpu_history[i], u); }
        }
        let total = if core_count > 0 { total_sum / core_count as f64 } else { 0.0 };
        push(&mut self.cpu_total_history, total);

        self.mem_used  = self.sys.used_memory();
        self.swap_used = self.sys.used_swap();
        let mem_pct = if self.mem_total > 0 {
            self.mem_used as f64 / self.mem_total as f64 * 100.0
        } else { 0.0 };
        push(&mut self.mem_history, mem_pct);

        // Only update net/disk history when those tabs are active
        if need_net_disk {
            let (rx, tx) = self.networks.iter()
                .fold((0u64, 0u64), |(ar, at), (_, n)| {
                    (ar + n.total_received(), at + n.total_transmitted())
                });
            self.net_rx_rate = rx.saturating_sub(self.prev_rx) as f64;
            self.net_tx_rate = tx.saturating_sub(self.prev_tx) as f64;
            self.prev_rx = rx; self.prev_tx = tx;
            push(&mut self.net_rx_history, self.net_rx_rate);
            push(&mut self.net_tx_history, self.net_tx_rate);

            let (dr, dw) = self.disks.iter()
                .fold((0u64, 0u64), |(ar, aw), d| {
                    (ar + d.usage().total_read_bytes, aw + d.usage().total_written_bytes)
                });
            self.disk_read_rate  = dr.saturating_sub(self.prev_disk_read) as f64;
            self.disk_write_rate = dw.saturating_sub(self.prev_disk_write) as f64;
            self.prev_disk_read = dr; self.prev_disk_write = dw;
            push(&mut self.disk_read_history, self.disk_read_rate);
            push(&mut self.disk_write_history, self.disk_write_rate);
        }

        self.processes.clear();
        let filter_lc = self.proc_filter.to_lowercase();
        for p in self.sys.processes().values() {
            let name = p.name().to_string_lossy().to_string();
            if !filter_lc.is_empty() && !name.to_lowercase().contains(&filter_lc) { continue; }
            self.processes.push(ProcInfo {
                pid: p.pid().as_u32(),
                name,
                cpu: p.cpu_usage(),
                mem_mb: p.memory() as f64 / 1024.0 / 1024.0,
                status: format!("{:?}", p.status()),
            });
        }
        match self.proc_sort {
            ProcSort::Cpu  => self.processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            ProcSort::Mem  => self.processes.sort_by(|a, b| b.mem_mb.total_cmp(&a.mem_mb)),
            ProcSort::Pid  => self.processes.sort_by_key(|p| p.pid),
            ProcSort::Name => self.processes.sort_by(|a, b| a.name.cmp(&b.name)),
        }
        self.proc_selected = self.proc_selected.min(self.processes.len().saturating_sub(1));

        // Threshold alerts
        let cpu_pct = *self.cpu_total_history.back().unwrap_or(&0.0);
        let mem_pct = if self.mem_total > 0 {
            self.mem_used as f64 / self.mem_total as f64 * 100.0
        } else { 0.0 };
        let was_cpu = self.alert_cpu;
        let was_mem = self.alert_mem;
        self.alert_cpu = cpu_pct >= 90.0;
        self.alert_mem = mem_pct >= 90.0;
        // Ring terminal bell on new alert
        if (self.alert_cpu && !was_cpu) || (self.alert_mem && !was_mem) {
            self.alert_tick = self.tick;
            print!("\x07"); // BEL
        }
    }

    /// Send signal to selected process. Returns error string on failure.
    pub fn kill_selected(&mut self, force: bool) -> Option<String> {
        let p = self.processes.get(self.proc_selected)?;
        let pid = p.pid;
        #[cfg(unix)]
        {
            use std::process::Command;
            let sig = if force { "-9" } else { "-15" };
            let out = Command::new("kill").arg(sig).arg(pid.to_string()).output();
            match out {
                Ok(o) if o.status.success() => None,
                Ok(o) => Some(String::from_utf8_lossy(&o.stderr).to_string()),
                Err(e) => Some(e.to_string()),
            }
        }
        #[cfg(windows)]
        {
            use std::process::Command;
            let out = if force {
                Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output()
            } else {
                Command::new("taskkill").args(["/PID", &pid.to_string()]).output()
            };
            match out {
                Ok(o) if o.status.success() => None,
                Ok(o) => Some(String::from_utf8_lossy(&o.stderr).to_string()),
                Err(e) => Some(e.to_string()),
            }
        }
        #[cfg(not(any(unix, windows)))]
        { Some("不支持此平台".to_string()) }
    }

    pub fn tick_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.tick_rate_ms)
    }

    /// Tab-complete git_path: find first matching subdirectory
    pub fn git_path_complete(&mut self) {
        let path = std::path::Path::new(&self.git_path);
        let (dir, prefix) = if self.git_path.ends_with(std::path::MAIN_SEPARATOR)
            || self.git_path.ends_with('/') {
            (path, "")
        } else {
            let parent = path.parent().unwrap_or(std::path::Path::new("."));
            let stem = path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            (parent, stem)
        };
        if let Ok(rd) = std::fs::read_dir(dir) {
            let mut matches: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.to_lowercase().starts_with(&prefix.to_lowercase()))
                .collect();
            matches.sort();
            if let Some(first) = matches.first() {
                let sep = std::path::MAIN_SEPARATOR;
                let base = dir.to_string_lossy();
                self.git_path = if base == "." {
                    format!("{}{}", first, sep)
                } else {
                    format!("{}{}{}{}", base, sep, first, sep)
                };
            }
        }
    }
}

fn push(buf: &mut VecDeque<f64>, val: f64) {
    if buf.len() >= HISTORY { buf.pop_front(); }
    buf.push_back(val);
}

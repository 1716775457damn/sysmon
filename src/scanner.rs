use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Result of scanning one port on one host
#[derive(Clone)]
pub struct ScanResult {
    pub host: String,
    pub port: u16,
    pub open: bool,
    pub latency_ms: Option<f64>,
    pub service: &'static str,
}

/// Message sent from scan threads back to the UI
pub enum ScanMsg {
    Result(ScanResult),
    Done { total: usize, open: usize, elapsed_ms: u128 },
}

/// Well-known port → service name
pub fn service_name(port: u16) -> &'static str {
    match port {
        21   => "FTP",
        22   => "SSH",
        23   => "Telnet",
        25   => "SMTP",
        53   => "DNS",
        80   => "HTTP",
        110  => "POP3",
        143  => "IMAP",
        443  => "HTTPS",
        445  => "SMB",
        3306 => "MySQL",
        3389 => "RDP",
        5432 => "PostgreSQL",
        5900 => "VNC",
        6379 => "Redis",
        8080 => "HTTP-Alt",
        8443 => "HTTPS-Alt",
        27017=> "MongoDB",
        _    => "",
    }
}

/// Parse "192.168.1.1" or "192.168.1.1-10" into a list of IPs
pub fn parse_hosts(input: &str) -> Vec<String> {
    let input = input.trim();
    // Range: 192.168.1.1-20
    if let Some(dash) = input.rfind('-') {
        let base = &input[..dash];
        let end_str = &input[dash + 1..];
        if let Ok(end) = end_str.parse::<u8>() {
            // Find last dot in base
            if let Some(dot) = base.rfind('.') {
                let prefix = &base[..=dot];
                if let Ok(start) = base[dot + 1..].parse::<u8>() {
                    return (start..=end)
                        .map(|i| format!("{}{}", prefix, i))
                        .collect();
                }
            }
        }
    }
    // CIDR: 192.168.1.0/24 (supports any prefix length, not just /24)
    if let Some(slash) = input.find('/') {
        let ip_part = &input[..slash];
        let bits: u32 = input[slash + 1..].parse().unwrap_or(24);
        if bits <= 32 {
            if let Ok(IpAddr::V4(v4)) = ip_part.parse::<IpAddr>() {
                let base = u32::from(v4) & (!0u32 << (32 - bits));
                let broadcast = base | (!(!0u32 << (32 - bits)));
                // Skip network address (base) and broadcast
                return ((base + 1)..broadcast)
                    .map(|n| {
                        let [a, b, c, d] = n.to_be_bytes();
                        format!("{}.{}.{}.{}", a, b, c, d)
                    })
                    .collect();
            }
        }
    }
    // Single IP or hostname
    vec![input.to_string()]
}

/// Parse "80,443,22-25,8080" into sorted unique port list
pub fn parse_ports(input: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if let Some(dash) = part.find('-') {
            let a: u16 = part[..dash].trim().parse().unwrap_or(0);
            let b: u16 = part[dash + 1..].trim().parse().unwrap_or(0);
            if a > 0 && b >= a { for p in a..=b { ports.push(p); } }
        } else if let Ok(p) = part.parse::<u16>() {
            if p > 0 { ports.push(p); }
        }
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// Spawn concurrent scan. Results sent via tx. Returns immediately.
pub fn start_scan(
    hosts: Vec<String>,
    ports: Vec<u16>,
    timeout_ms: u64,
    concurrency: usize,
    tx: Sender<ScanMsg>,
) {
    std::thread::spawn(move || {
        use std::sync::mpsc as task_mpsc;

        let start = Instant::now();
        let total = hosts.len() * ports.len();

        // Use a channel as a lock-free work queue
        let (task_tx, task_rx) = task_mpsc::channel::<(String, u16)>();
        let task_rx = std::sync::Arc::new(std::sync::Mutex::new(task_rx));

        // Feed all tasks upfront
        for h in &hosts {
            for &p in &ports {
                let _ = task_tx.send((h.clone(), p));
            }
        }
        drop(task_tx); // signal end-of-work

        let open_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let timeout = Duration::from_millis(timeout_ms);
        let mut handles = Vec::with_capacity(concurrency);

        for _ in 0..concurrency {
            let task_rx = task_rx.clone();
            let tx = tx.clone();
            let open_count = open_count.clone();

            handles.push(std::thread::spawn(move || {
                loop {
                    let item = { task_rx.lock().unwrap().recv().ok() };
                    let (host, port) = match item { Some(x) => x, None => break };

                    let t0 = Instant::now();
                    // Parse directly as SocketAddr — avoids the LOCALHOST fallback bug
                    let addr: SocketAddr = format!("{}:{}", host, port)
                        .parse()
                        .unwrap_or_else(|_| SocketAddr::new(
                            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port
                        ));
                    let connected = TcpStream::connect_timeout(&addr, timeout).is_ok();
                    let latency = if connected { Some(t0.elapsed().as_secs_f64() * 1000.0) } else { None };

                    if connected {
                        open_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }

                    if tx.send(ScanMsg::Result(ScanResult {
                        host, port, open: connected, latency_ms: latency,
                        service: service_name(port),
                    })).is_err() { break; } // receiver gone
                }
            }));
        }

        for h in handles { let _ = h.join(); }

        let open = open_count.load(std::sync::atomic::Ordering::Relaxed);
        let _ = tx.send(ScanMsg::Done {
            total, open,
            elapsed_ms: start.elapsed().as_millis(),
        });
    });
}

/// Common port presets
pub fn preset_ports(name: &str) -> &'static str {
    match name {
        "常用"   => "21,22,23,25,53,80,110,143,443,445,3306,3389,5432,5900,6379,8080,8443,27017",
        "Web"    => "80,443,8080,8443,8888,9090,9443",
        "数据库" => "1433,1521,3306,5432,6379,27017,9200",
        "远程"   => "22,23,3389,5900,5901",
        _        => "1-1024",
    }
}

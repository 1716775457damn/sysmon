# ⚡ SysMon

> **A blazing-fast terminal system monitor built with Rust — real-time CPU, memory, network, disk, process management, port scanner, and Git repository analyzer, all in one TUI.**

![Rust](https://img.shields.io/badge/built%20with-Rust-orange?logo=rust)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Version](https://img.shields.io/badge/version-1.0.0-brightgreen)

---

## ✨ Why SysMon?

Most system monitors are either too heavy (Electron-based), too simple (basic `top`), or require complex installation. **SysMon** is a single binary that runs in any terminal — no installation, no dependencies, no configuration.

- ⚡ **8 functional panels** — system metrics, port scanner, and Git analyzer in one tool
- 🖥️ **Pure terminal UI** — built with `ratatui`, works over SSH, in tmux, anywhere
- 🔍 **Real-time monitoring** — CPU, memory, network, disk updated every second
- 🛡️ **Process management** — view, filter, sort, and kill processes
- 🌐 **Port scanner** — scan IP ranges with concurrent TCP connections
- 📊 **Git analyzer** — author stats, hot files, commit heatmap for any local repo
- 🖱️ **Mouse support** — click tabs, scroll lists with the mouse wheel
- 📦 **Zero dependencies** — single binary, runs anywhere

---

## 🚀 Features

### System Monitoring (Tabs 1–5)
- **CPU** — per-core sparklines with color-coded load (green → yellow → red)
- **Memory** — RAM + Swap gauges with 60-second history chart
- **Network** — upload/download line chart + per-interface bar chart
- **Disk** — read/write line chart + per-partition usage table with visual bars
- **Overview** — all metrics at a glance + Top 10 processes

### Process Manager (Tab 6)
- Real-time process list with CPU%, memory, PID, status
- Sort by CPU / Memory / PID / Name
- Live filter by process name (`/` key)
- **Kill process** — `K` sends SIGTERM, `Shift+K` sends SIGKILL
- Position counter (e.g. `15/342`)

### Port Scanner (Tab 7)
- Scan single IP, IP range (`192.168.1.1-20`), or CIDR (`192.168.1.0/24`)
- Custom port list: `80,443,22-25,8080`
- Port presets: Common / Web / Database / Remote (`F1`–`F4`)
- Concurrent scanning (100 threads by default)
- Shows open/closed status, latency, and service name
- Toggle display of closed ports (`H`)

### Git Analyzer (Tab 8)
- **Overview** — repo stats + 52-week commit activity sparkline
- **Authors** — commit count, lines added/deleted, files touched per author
- **Hot Files** — Top 100 most-modified files with heat coloring
- **Heatmap** — 7×24 commit time distribution (weekday × hour)
- Tab key path completion when entering repo path

### Interface
- **Mouse support** — click tabs to switch, scroll wheel in lists
- **Keyboard navigation** — `j/k` or arrow keys, `g/G` for top/bottom
- **Adaptive refresh** — only collects data for the active tab (saves CPU)
- **Threshold alerts** — CPU/memory > 90% triggers blinking border + terminal bell
- **Adjustable refresh rate** — `+`/`-` keys (200ms to 5000ms)
- **Help overlay** — press `?` to show all shortcuts

---

## 📸 Screenshot

```
┌─ ⚡ SysMon  刷新: 1000ms  ? 帮助 ──────────────────────────────────────────┐
│ 总览 [1] │ CPU [2] │ 内存 [3] │ 网络 [4] │ 磁盘 [5] │ 进程 [6] │ 扫描 [7] │ Git [8] │
├────────────────────────────────────────────────────────────────────────────┤
│ ┌─ CPU ──────────────┐  ┌─ 内存  6.2GB/16GB ─────────────────────────────┐ │
│ │ ████████░░  42.3%  │  │ ████████████████░░░░░░░░░░░░  38.8%  空闲 9.8GB│ │
│ └────────────────────┘  └────────────────────────────────────────────────┘ │
│ ┌─ 网络 ↓2.1MB/s ↑340KB/s ──┐  ┌─ 磁盘 R:45MB/s W:12MB/s ──────────────┐ │
│ │ ▁▂▃▅▇█▇▅▃▂▁▂▃▄▅▆▇█▇▅▃▂▁  │  │ ▁▁▂▃▅▇█▅▃▂▁▁▂▃▄▅▆▇█▇▅▃▂▁▁▂▃▄▅▆▇█▇▅▃  │ │
│ └────────────────────────────┘  └────────────────────────────────────────┘ │
│ ┌─ CPU 历史 ─────────────────────────────────────────────────────────────┐ │
│ │ ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅ │ │
│ └────────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## ⌨️ Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1`–`8` | Switch to tab |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `↑↓` or `j/k` | Navigate lists |
| `g` / `G` | Jump to top / bottom |
| `c/m/p/n` | Sort processes by CPU / Memory / PID / Name |
| `/` | Filter processes by name |
| `K` | Kill selected process (SIGTERM) |
| `Shift+K` | Force kill selected process (SIGKILL) |
| `+` / `-` | Faster / slower refresh (200ms–5000ms) |
| `?` | Show help overlay |
| `q` or `Ctrl+C` | Quit |

**Port Scanner (Tab 7)**

| Key | Action |
|-----|--------|
| `i` or `Enter` | Enter input mode |
| `Tab` (in input) | Switch between host/port fields |
| `Enter` (in input) | Start scan |
| `F1`–`F4` | Load port presets |
| `H` | Toggle show/hide closed ports |
| `Esc` | Cancel scan |

**Git Analyzer (Tab 8)**

| Key | Action |
|-----|--------|
| `i` or `Enter` | Enter repo path |
| `Tab` (in input) | Path auto-complete |
| `o/a/f/h` | Switch sub-view (overview/authors/files/heatmap) |
| `Esc` | Cancel analysis |

---

## 📥 Download & Run

### Windows
1. Go to [Releases](../../releases)
2. Download `sysmon-windows-x86_64.exe`
3. Open a terminal and run it:
```
sysmon-windows-x86_64.exe
```

### macOS
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/1716775457damn/sysmon.git
cd sysmon
cargo build --release

./target/release/sysmon
```

> ℹ️ On first launch macOS may show a security warning.  
> Go to **System Settings → Privacy & Security** and click **Open Anyway**.

### Linux
```bash
# Download the pre-built binary
# Or build from source:
git clone https://github.com/1716775457damn/sysmon.git
cd sysmon
cargo build --release
./target/release/sysmon
```

---

## 🛠️ Build from Source

Requires [Rust](https://rustup.rs/) (stable toolchain).

```bash
git clone https://github.com/1716775457damn/sysmon.git
cd sysmon
cargo build --release
# Windows: target/release/sysmon.exe
# macOS/Linux: target/release/sysmon
```

> The build uses `vendored-libgit2` and `vendored-openssl` — no system libraries required.

---

## 🏗️ Architecture

```
src/
├── main.rs          # Entry point, event loop, keyboard/mouse handling
├── app.rs           # Application state, data collection, alert logic
├── ui.rs            # All ratatui rendering (8 tabs)
├── scanner.rs       # Concurrent TCP port scanner
└── gitanalyzer.rs   # Git repository analysis via libgit2
```

| Component | Crate | Why |
|-----------|-------|-----|
| Terminal UI | `ratatui` | Feature-rich TUI framework |
| Terminal input | `crossterm` | Cross-platform keyboard/mouse events |
| System metrics | `sysinfo` | CPU, memory, disk, network, processes |
| Git analysis | `git2` | libgit2 bindings (vendored) |

---

## ⚡ Performance

- **Adaptive refresh** — only collects data for the active tab, minimizing CPU overhead
- **Single-pass process collection** — reuses `Vec` allocation each tick
- **Pre-compiled exclude patterns** — `ExcludeSet` built once per scan
- **Virtual scroll** — log panels render only visible rows regardless of entry count
- **Concurrent port scanning** — 100 parallel TCP connections by default

---

## 🗺️ Roadmap

- [ ] Network connection list (active TCP/UDP sockets)
- [ ] Temperature sensors (CPU/GPU)
- [ ] Custom alert thresholds via config file
- [ ] Export scan/git results to file
- [ ] SSH remote monitoring

---

## 📄 License

MIT © 2025

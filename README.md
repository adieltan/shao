<div align="center">

# 🛡️ Shao, 哨兵

**The Ultra-Lightweight, Single-Binary Linux Server Sentinel & Telemetry Dashboard**

[![Rust](https://img.shields.io/badge/Language-Rust%202021-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/Docker-Ready-blue.svg?style=flat-square&logo=docker)](https://github.com/adieltan/shao/pkgs/container/shao)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg?style=flat-square)](LICENSE)
[![Memory Footprint](https://img.shields.io/badge/RAM%20Usage-%3C%205%20MB-emerald.svg?style=flat-square)](#-benchmarks)
[![Binary Size](https://img.shields.io/badge/Binary%20Size-~4%20MB%20(Single%20File)-purple.svg?style=flat-square)](#-features)

*Shao (哨兵 - Sentinel) is a high-performance, zero-bloat server monitoring engine and glassmorphism web dashboard compiled into a single static binary. It provides sub-millisecond hardware telemetry, real-time power analytics, and Docker health tracking using under 5 MB of RAM.*

[Features](#-key-features) • [Quick Start](#-quick-start) • [Run in Docker](#-run-in-docker-shortcuts) • [Configuration](#-configuration) • [Benchmarks](#-benchmarks) • [Architecture](#-architecture)

</div>

---

## ✨ Key Features

- **⚡ Zero Bloat & Single Binary:** Compiled in 100% Rust. Web frontend, SQLite database, and telemetry engine are all baked into one single `~4MB` standalone executable.
- **🔋 Intel RAPL & AMD Powercap Telemetry:** Reads hardware energy microjoule registers directly to provide live power draw (Watts), cumulative energy (Wh / kWh), and real-time financial running costs.
- **🌀 Motherboard Fan & Thermal Dials:** Auto-discovers motherboard tachometers and thermal zones across `/sys/class/hwmon` (e.g. Asus, Lenovo, Dell, Supermicro).
- **🌐 Network Traffic Separation:** Automatically differentiates between **Local Home LAN** (Ethernet/Wi-Fi) and **Remote VPN / Mesh Networks** (Tailscale, WireGuard).
- **🐳 Docker Socket Integration:** Connects directly to `/var/run/docker.sock` to report live container health and status dots with zero overhead.
- **📈 Embedded Time-Series Analytics:** SQLite rolling history tracking 7-day hardware and power metrics with interactive zoom (**15m**, **1h**, **6h**, **24h**, **7d**).
- **🎨 Dark Glassmorphic Dashboard:** Built with Tailwind CSS, animated ApexCharts, circular speedometer dials, and Server-Sent Events (SSE) for smooth sub-second updates.

---

## 🐳 Run in Docker (Shortcuts)

### 1. One-Line Docker Run
```bash
docker run -d \
  --name shao \
  --restart unless-stopped \
  -p 8888:8080 \
  -v /proc:/host/proc:ro \
  -v /sys:/sys:ro \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -v shao_data:/app \
  ghcr.io/adieltan/shao:latest
```

### 2. Docker Compose / Dockge (`docker-compose.yml`)
```yaml
services:
  shao:
    image: ghcr.io/adieltan/shao:latest
    container_name: shao
    restart: unless-stopped
    ports:
      - "8888:8080"
    volumes:
      - /proc:/host/proc:ro
      - /sys:/sys:ro
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./config.toml:/app/config.toml:ro
      - shao_data:/app
    environment:
      - RUST_LOG=info

volumes:
  shao_data:
```

Start the stack:
```bash
docker compose up -d
```

Open **`http://localhost:8888`** to access your dashboard immediately!

---

## 🚀 Native Quick Start (No Docker Required)

### 1. Build from Source
```bash
# Clone the repository
git clone https://github.com/adieltan/shao.git
cd shao

# Build optimized release binary
cargo build --release

# Run Shao
./target/release/shao
```

### 2. Run with Custom Configuration
```bash
cp config.toml.example config.toml
./target/release/shao --config config.toml
```

---

## 📊 Benchmarks vs. Traditional Monitoring

| Tool | RAM Usage | Startup Time | Dependencies | Web UI |
| :--- | :--- | :--- | :--- | :--- |
| **🛡️ Shao (哨兵)** | **~2.1 MB** | **< 2 ms** | **0 (Single Binary)** | **Included (Embedded)** |
| **Glances** | ~35 MB | ~800 ms | Python, psutil | Included |
| **Netdata** | ~65 MB | ~1.5 s | C runtime, plugins | Included |
| **Grafana + Prometheus Stack** | ~140 MB | ~5.0 s | 3-4 Docker Containers | Separate Services |

---

## ⚙️ Configuration (`config.toml`)

```toml
[server]
host = "0.0.0.0"
port = 8080
polling_interval_ms = 1000   # Set to 500 for ultra-fast telemetry

[power]
kwh_cost = 0.15              # Electricity cost per kWh
currency_symbol = "$"

[network]
lan_interfaces = ["enp2s0", "eth0", "wlp3s0", "wlan0"]
vpn_interfaces = ["tailscale0", "wg0"]

[[apps]]
name = "Immich"
url = "http://192.168.1.1:2283"
description = "Photo & Video Server"
icon = "image"
container = "immich_server"
```

---

## 🐧 Run as a Background Service (`systemd`)

Create `/etc/systemd/system/shao.service`:

```ini
[Unit]
Description=Shao (哨兵) Server Monitoring Sentinel
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/shao
ExecStart=/opt/shao/shao --config /opt/shao/config.toml
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now shao.service
```

---

## 🏛️ Architecture

```
                                  +---------------------------------------+
                                  |         Shao (哨兵) Executable        |
                                  |                                       |
+--------------------------+      |  +---------------------------------+  |      +-------------------------+
| Linux Kernel & Hardware  | ---> |  |  Auto-Discovery & Sensor Engine |  | ---> |  Embedded Web UI        |
| - Intel RAPL / Power     |      |  +---------------------------------+  |      |  - Tailwind CSS         |
| - Hwmon Fan & Thermals   |      |                  |                    |      |  - ApexCharts Streaming |
| - /proc/net/dev (LAN/VPN)|      |  +---------------------------------+  |      |  - Speedometer Gauges   |
| - /var/run/docker.sock   |      |  |  Embedded SQLite (7-Day Rolling)|  |      +-------------------------+
+--------------------------+      |  +---------------------------------+  |                   ^
                                  |                  |                    |                   |
                                  |  +---------------------------------+  |                   |
                                  |  |  Axum REST & SSE Stream Engine  |  | ------------------+
                                  |  +---------------------------------+  |
                                  +---------------------------------------+
```

---

## 📄 License

Dual-licensed under either of:
- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT License](http://opensource.org/licenses/MIT)

at your option.

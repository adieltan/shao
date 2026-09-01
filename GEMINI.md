# Shao (哨兵) - Project Guidelines & Persistent Context

## 🖥️ Target Environment & Hardware Specs
- **Production Server:** Asus A456U laptop repurposed as an always-on home server.
- **Operating System:** Ubuntu Linux (`x86_64`).
- **Hostname & User:** Hostname is `a456u`, primary user is `rh` (home directory: `/home/rh/`).
- **Network Configuration:**
  - **Local Home LAN:** Standard ethernet/Wi-Fi (e.g., `192.168.1.17`, `192.168.1.1`, interfaces: `enp2s0`, `wlp3s0`, `eth0`, `wlan0`).
  - **Tailscale Mesh VPN:** Remote connectivity via Tailscale (interface: `tailscale0`, admin network access).
- **Hardware Telemetry Interfaces (Linux-only):**
  - **Intel RAPL Energy:** `/sys/class/powercap/intel-rapl` (calculates real-time Wattage and cumulative kWh power consumption).
  - **Thermal & Fan Tachometers:** `/sys/class/hwmon` (Asus WMI / coretemp / acpitz sensor paths).
  - **Systemd & Power Control:** `/var/run/dbus/system_bus_socket` and `/proc/sysrq-trigger` for safe remote host shutdown and reboot.

---

## 📦 Self-Hosted Server Ecosystem & Services
The `a456u` home server hosts several core services, integrated with or monitored by Shao:

1. **Shao (哨兵 - Server Sentinel):**
   - Single-binary Rust monitoring engine and dark glassmorphic dashboard.
   - Web UI & REST/SSE API exposed on port `8888` (or container internal port `8080`).
   - Direct Docker socket integration via `/var/run/docker.sock`.
   - Embedded SQLite time-series database (`shao.db`) storing 7-day rolling telemetry.

2. **Immich (Photo & Video Server):**
   - High-performance self-hosted photo/video backup suite.
   - Web UI on port `2283` (`http://192.168.1.1:2283` or `http://a456u:2283`).
   - Shao integrates with Immich REST API (`/api/server/statistics`) using `x-api-key` to display live asset counts and storage usage.

3. **Dockge (Stack Manager):**
   - Modern, reactive Docker Compose stack manager.
   - Web UI on port `5001` (`http://192.168.1.1:5001` or `http://a456u:5001`).
   - Compose stacks reside on the server in `/home/rh/stacks`.

4. **Tailscale & Remote SSH Access:**
   - Remote terminal administration is performed via SSH (`ssh rh@a456u` or through Tailscale IP).
   - Network card in Shao dashboard automatically splits and tracks Local LAN vs. Tailscale VPN throughput.

---

## 💻 Local Workstation vs. Remote Deployment Rules
- **Local Machine is macOS (`mac`):**
  - The local development machine is a MacBook running macOS.
  - **NEVER attempt to execute `cargo run` locally expecting production server telemetry.** Local macOS lacks `/proc`, `/sys/class/powercap`, `/sys/class/hwmon`, and the server Docker socket.
  - On the local Mac, only perform:
    - Code checks & testing: `cargo check`, `cargo test`, `cargo clippy`, `cargo build`.
    - Frontend development: Edit HTML/JS/CSS in `frontend/`. Assets are embedded into the binary at compile time via `rust-embed`.
    - Version tagging and Git operations (`bump_version.py`).
- **Deployment to Remote Linux Server:**
  - Automated via GitHub Actions CI (`.github/workflows/docker.yml`) building multi-stage Alpine images pushed to `ghcr.io/adieltan/shao:latest`.
  - Deployed on `a456u` using Docker Compose (`docker compose up -d`) with mounted `/proc`, `/sys`, `/var/run/docker.sock`, and dbus sockets.

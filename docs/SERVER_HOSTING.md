# 🖥️ a456u Server Hosting & Infrastructure Guide

This document is the comprehensive reference for the self-hosted Linux home server infrastructure running on **`a456u`**.

---

## 1. Hardware Architecture & System Specifications

| Component | Specification | Operational Details |
| :--- | :--- | :--- |
| **Model** | Asus A456U Laptop | Repurposed as an always-on, low-power headless server with built-in battery acting as an uninterruptible power supply (UPS). |
| **CPU** | Intel Core i5-7200U @ 2.50GHz | 2 Cores / 4 Threads (Kaby Lake, Turbo up to 3.10GHz). Supports AVX2 and FMA3 vector extensions. |
| **Memory** | 12 GB DDR4 | ~2.5 GB allocated to active services; **~8.6 GB available headroom** for lightweight AI models and caching. |
| **Storage** | 439 GB SATA (`/dev/sda2`) | ~188 GB available for Docker volumes, database WAL, and photos. |
| **Operating System** | Ubuntu Linux (`x86_64`) | Hostname: `a456u`, User: `rh` (`/home/rh`). |
| **Lid & Power Policy** | Systemd `logind.conf` | `HandleLidSwitch=ignore` configured so the laptop runs 24/7 with the lid closed without suspending. |

---

## 2. Network Topology & Access Architecture

```
                 📱 Mobile / 💻 Remote Mac
                            │
               ┌────────────┴────────────┐
               ▼                         ▼
         Home Wi-Fi/LAN           Tailscale Mesh VPN
       (192.168.1.0/24)             (tailscale0)
               │                         │
               └────────────┬────────────┘
                            │
                            ▼
              ┌───────────────────────────┐
              │     a456u Linux Host      │
              │  • UFW allows LAN + VPN   │
              │  • Zero Router Port Forwards
              └───────────────────────────┘
```

### Network Interfaces
1. **Local Home LAN (`enp2s0`, `wlp3s0`):**
   * Static/Reserved DHCP IP on home router (e.g., `192.168.1.17`).
   * Provides gigabit local throughput for photo backup and fast file sync.
2. **Tailscale Mesh VPN (`tailscale0`):**
   * Encrypted WireGuard-based mesh network allowing seamless, secure connection from phone or Mac anywhere in the world.
   * Access via MagicDNS hostname: `http://a456u:<port>` or `ssh rh@a456u`.
   * **Security Benefit:** Zero ports are forwarded on the public home router. The server is completely invisible to public internet scanners.

---

## 3. Hosted Services & Port Directory

All user-facing services are accessible via `http://192.168.1.17:<port>` (Local LAN) or `http://a456u:<port>` (Tailscale):

| Service | Port | Process / Container | Directory / Stack | Description |
| :--- | :--- | :--- | :--- | :--- |
| **Shao (哨兵)** | `8888` | `shao` (Binary / Systemd) | `/opt/shao` | Single-binary server sentinel dashboard, SQLite rolling telemetry, RAPL watt meter, and remote power control. |
| **Immich Web & API** | `2283` | `immich_server` | `/home/rh/stacks/immich` | High-performance self-hosted photo & video backup suite. |
| **Dockge** | `5001` | `dockge` | `/home/rh/stacks` | Reactive web manager for all Docker Compose stacks. |
| **Glacier AI Server** | `8899` | `glacier_server` | `/home/rh/stacks/glacier-ai` | Rust Axum receipt parser microservice with Glacier SQLite schema synthesis. |
| **Ollama Engine** | `11434` | `glacier_ollama` | `/home/rh/stacks/glacier-ai` | Local LLM engine serving `qwen2.5:1.5b` for zero-shot transaction categorization. |
| **PostgreSQL** | `5432` | `immich_postgres` | Immich Stack | PostgreSQL 14 with `pgvector` for photo vector embeddings. |
| **Redis** | `6379` | `immich_redis` | Immich Stack | In-memory cache and background job queue for Immich. |
| **Immich ML** | Internal | `immich_machine_learning` | Immich Stack | On-device CLIP visual search and facial recognition model container. |

---

## 4. Hardware Telemetry & Kernel Interfaces (Linux)

Shao directly reads low-level Linux kernel interfaces to provide telemetry without heavy daemon agents:

1. **Intel RAPL (Running Average Power Limit):**
   * Path: `/sys/class/powercap/intel-rapl/intel-rapl:0/energy_uj`
   * Computes real-time wattage consumption (typically 6W–18W) and cumulative kWh electricity usage.
2. **Thermal Sensors & Fan Tachometer:**
   * Path: `/sys/class/hwmon` (Asus WMI `asus-nb-wmi`, Coretemp `coretemp`, ACPI `acpitz`).
   * Measures motherboard fan RPM and individual CPU core temperatures.
3. **Remote System Power Control:**
   * Path: `/var/run/dbus/system_bus_socket` (Systemd DBus login API) and `/proc/sysrq-trigger` (Kernel emergency fallback).
   * Enables safe, authenticated host shutdown or reboot directly from the Shao UI or API.

---

## 5. Administration & Common Runbooks

### Managing Stacks with Dockge or CLI
Compose stacks are centralized in `/home/rh/stacks/`:
```bash
# SSH into the server
ssh rh@a456u

# Inspect all running containers
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# Restart or update a specific stack
cd /home/rh/stacks/<stack-name>
docker compose pull && docker compose up -d
```

### Managing the Shao Sentinel Service
Shao runs natively as a managed systemd service:
```bash
# Service status & logs
systemctl status shao
journalctl -u shao -f

# Restart daemon
sudo systemctl restart shao
```

### Host Power & Maintenance
```bash
# Safe host reboot
sudo reboot

# Safe host shutdown
sudo poweroff
```

---

## 6. Security Hygiene & Private Repositories

Because companion repositories like **Glacier** are private:
1. **No Hardcoded Secrets:** Never commit `.env` files, Tailscale auth keys, or Immich API tokens to Git. Always copy `.env.example` templates.
2. **Access Boundary:** Keep all management endpoints (`8888`, `5001`, `11434`, `8899`) protected behind the local subnet and Tailscale. Do not expose them to the public internet via NAT port forwarding.
3. **SSH Keys:** Terminal administration relies strictly on SSH public key authentication (`~/.ssh/authorized_keys`). Password authentication over SSH should remain disabled.

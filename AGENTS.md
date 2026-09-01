# Shao (哨兵) - Project Guidelines & Persistent Context

## 🖥️ Target Environment & Architecture
- **Target Deployment Platform:** `shao` is an ultra-lightweight server sentinel designed to run on a **Linux Server** (bare-metal Linux or Docker container on Linux).
- **Host vs. Deployment Environment:**
  - The local development machine is **macOS** (`mac`).
  - **DO NOT** attempt to run `cargo run` locally expecting production Linux server telemetry or attempting to access Linux-specific kernel interfaces (`/proc`, `/sys/class/powercap/intel-rapl`, `/sys/class/hwmon`, systemd dbus, etc.).
  - On the local Mac development machine, only perform compilation checks, unit testing, and static asset editing (`cargo check`, `cargo test`, `cargo build`, editing `frontend/`).
  - Real runtime execution and telemetry verification occur on the remote Linux server or via Docker (`docker compose up` on the Linux host).

## 📦 Project Structure & Tech Stack
- **Language & Runtime:** Rust 2021 edition (Axum 0.7, Tokio, Rusqlite with `bundled` SQLite, pure-Rust `rustls`).
- **Frontend:** Vanilla JavaScript, Tailwind CSS, ApexCharts, Lucide Icons. Embedded directly into the compiled binary via `rust-embed` (`frontend/` directory).
- **Configuration:** TOML-based (`config.toml`, reference `config.toml.example`).
- **Containerization & CI:**
  - `Dockerfile`: Multi-stage Alpine build (`rust:alpine` -> `alpine:latest`).
  - GitHub Actions (`.github/workflows/docker.yml`): Automatically builds and publishes multi-arch container image to `ghcr.io/adieltan/shao:latest` on push to `main` and version tags.
- **Docker Compose:** Configured in `docker-compose.yml` mounting `/proc`, `/sys`, `/var/run/docker.sock`, and dbus socket.

## 🛠️ Development & Coding Guidelines
- **Zero External Runtime Dependencies:** Keep binary standalone and minimal (< 5 MB RAM, ~4 MB binary size).
- **Embedded Frontend:** When modifying UI/UX in `frontend/`, rebuild with `cargo build` to update embedded assets.
- **Linux Compatibility:** When adding hardware sensors or metrics, always ensure fallback gracefully when running in non-Linux or containerized environments without root privileges.
- **Version Updates:** Keep `Cargo.toml` and release tags synchronized.

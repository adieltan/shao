#[allow(unused_imports)]
use std::process::Command;
#[allow(unused_imports)]
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Shutdown,
    Reboot,
}

impl PowerAction {
    pub fn name(&self) -> &'static str {
        match self {
            PowerAction::Shutdown => "shutdown",
            PowerAction::Reboot => "reboot",
        }
    }
}

/// Initiates a host power action (shutdown or reboot) asynchronously after a short grace delay.
pub fn execute_power_action(action: PowerAction) {
    info!("⚙️ Scheduling system {} sequence...", action.name());

    tokio::spawn(async move {
        // Grace period of 500ms allows the HTTP response to be cleanly returned to the client
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        info!("🔌 Executing system {} now...", action.name());

        #[cfg(target_os = "linux")]
        {
            let success = match action {
                PowerAction::Shutdown => {
                    trigger_linux_sysrq(PowerAction::Shutdown)
                        || run_cmd("busctl", &["--address=unix:path=/var/run/dbus/system_bus_socket", "call", "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "PowerOff", "b", "true"])
                        || run_cmd("busctl", &["--address=unix:path=/host/run/dbus/system_bus_socket", "call", "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "PowerOff", "b", "true"])
                        || run_cmd("nsenter", &["-t", "1", "-m", "-u", "-i", "-n", "-p", "systemctl", "poweroff"])
                        || run_cmd("nsenter", &["-t", "1", "-m", "-u", "-i", "-n", "-p", "/sbin/shutdown", "-h", "now"])
                        || run_cmd("systemctl", &["poweroff"])
                        || run_cmd("/sbin/shutdown", &["-h", "now"])
                        || run_cmd("shutdown", &["-h", "now"])
                        || run_cmd("poweroff", &[])
                        || run_cmd("docker", &["run", "--rm", "--privileged", "--pid=host", "alpine", "poweroff"])
                }
                PowerAction::Reboot => {
                    trigger_linux_sysrq(PowerAction::Reboot)
                        || run_cmd("busctl", &["--address=unix:path=/var/run/dbus/system_bus_socket", "call", "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "Reboot", "b", "true"])
                        || run_cmd("busctl", &["--address=unix:path=/host/run/dbus/system_bus_socket", "call", "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "Reboot", "b", "true"])
                        || run_cmd("nsenter", &["-t", "1", "-m", "-u", "-i", "-n", "-p", "systemctl", "reboot"])
                        || run_cmd("nsenter", &["-t", "1", "-m", "-u", "-i", "-n", "-p", "/sbin/shutdown", "-r", "now"])
                        || run_cmd("systemctl", &["reboot"])
                        || run_cmd("/sbin/shutdown", &["-r", "now"])
                        || run_cmd("shutdown", &["-r", "now"])
                        || run_cmd("reboot", &[])
                        || run_cmd("docker", &["run", "--rm", "--privileged", "--pid=host", "alpine", "reboot"])
                }
            };

            if !success {
                error!("❌ All system {} commands failed on Linux host", action.name());
            }
        }

        #[cfg(target_os = "macos")]
        {
            info!("💻 [DEV MODE] Simulated {} command on macOS development machine. Real power actions are executed on your Ubuntu Linux server (Asus A456U).", action.name());
        }

        #[cfg(target_os = "windows")]
        {
            let flag = match action {
                PowerAction::Shutdown => "/s",
                PowerAction::Reboot => "/r",
            };
            let success = run_cmd("shutdown", &[flag, "/t", "0"]);
            if !success {
                error!("❌ Failed to trigger Windows {} command", action.name());
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            warn!("⚠️ System power actions not supported on this OS");
        }
    });
}

#[allow(dead_code)]
fn run_cmd(program: &str, args: &[&str]) -> bool {
    match Command::new(program).args(args).status() {
        Ok(status) => {
            if status.success() {
                info!("✓ Successfully executed: {} {:?}", program, args);
                true
            } else {
                warn!("⚠️ Command returned non-zero exit code: {} {:?}", program, args);
                false
            }
        }
        Err(e) => {
            warn!("⚠️ Could not run {}: {}", program, e);
            false
        }
    }
}

#[cfg(target_os = "linux")]
fn trigger_linux_sysrq(action: PowerAction) -> bool {
    use std::fs::OpenOptions;
    use std::io::Write;

    let sysrq_paths = ["/proc/sysrq-trigger", "/host/proc/sysrq-trigger"];
    for path in sysrq_paths {
        if let Ok(mut file) = OpenOptions::new().write(true).open(path) {
            let cmd_byte = match action {
                PowerAction::Shutdown => b"o",
                PowerAction::Reboot => b"b",
            };
            if file.write_all(cmd_byte).is_ok() {
                info!("✓ Successfully triggered SysRq {:?} via {}", action, path);
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_action_names() {
        assert_eq!(PowerAction::Shutdown.name(), "shutdown");
        assert_eq!(PowerAction::Reboot.name(), "reboot");
    }
}

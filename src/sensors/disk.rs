use serde::Serialize;
use sysinfo::Disks;

#[derive(Debug, Clone, Serialize)]
pub struct DiskMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f32,
    pub total_human: String,
    pub used_human: String,
    pub available_human: String,
    pub disks: Vec<DiskInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f32,
    pub total_human: String,
    pub used_human: String,
    pub available_human: String,
}

pub struct DiskCollector {
    disks: Disks,
}

impl DiskCollector {
    pub fn new() -> Self {
        Self {
            disks: Disks::new_with_refreshed_list(),
        }
    }

    pub fn collect(&mut self) -> DiskMetrics {
        self.disks.refresh();

        let mut total_all = 0u64;
        let mut avail_all = 0u64;
        let mut disk_list = Vec::new();

        for disk in self.disks.list() {
            let mount = disk.mount_point().to_string_lossy().to_string();
            let fs_type = disk.file_system().to_string_lossy().to_string();
            let name = disk.name().to_string_lossy().to_string();

            // Filter out loop, overlay, and temporary filesystems
            if fs_type == "squashfs"
                || fs_type == "tmpfs"
                || fs_type == "devtmpfs"
                || fs_type == "overlay"
                || fs_type == "efivarfs"
                || mount.starts_with("/snap")
                || mount.starts_with("/var/lib/docker")
            {
                continue;
            }

            let total = disk.total_space();
            let avail = disk.available_space();
            let used = total.saturating_sub(avail);
            let pct = if total > 0 {
                ((used as f64 / total as f64) * 100.0) as f32
            } else {
                0.0
            };

            total_all += total;
            avail_all += avail;

            disk_list.push(DiskInfo {
                name,
                mount_point: mount,
                file_system: fs_type,
                total_bytes: total,
                used_bytes: used,
                available_bytes: avail,
                usage_percent: (pct * 10.0).round() / 10.0,
                total_human: format_bytes(total),
                used_human: format_bytes(used),
                available_human: format_bytes(avail),
            });
        }

        let used_all = total_all.saturating_sub(avail_all);
        let pct_all = if total_all > 0 {
            ((used_all as f64 / total_all as f64) * 100.0) as f32
        } else {
            0.0
        };

        DiskMetrics {
            total_bytes: total_all,
            used_bytes: used_all,
            available_bytes: avail_all,
            usage_percent: (pct_all * 10.0).round() / 10.0,
            total_human: format_bytes(total_all),
            used_human: format_bytes(used_all),
            available_human: format_bytes(avail_all),
            disks: disk_list,
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    let mut b = bytes as f64;
    for u in ["B", "KB", "MB", "GB", "TB"] {
        if b < 1024.0 {
            return format!("{:.1} {}", b, u);
        }
        b /= 1024.0;
    }
    format!("{:.1} PB", b)
}

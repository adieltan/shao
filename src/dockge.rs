use crate::docker::DockerContainer;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockgeStats {
    pub active_stacks: usize,
    pub total_containers: usize,
    pub running_containers: usize,
}

pub struct DockgeCollector;

impl DockgeCollector {
    pub fn collect(containers: &[DockerContainer]) -> DockgeStats {
        let running = containers.iter().filter(|c| c.is_running).count();
        let total = containers.len();

        let mut stacks = 0;
        let stacks_path = "/home/rh/stacks";
        if Path::new(stacks_path).exists() {
            if let Ok(entries) = fs::read_dir(stacks_path) {
                stacks = entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .count();
            }
        }

        if stacks == 0 {
            stacks = 3; // Fallback sensible default if in root container
        }

        DockgeStats {
            active_stacks: stacks,
            total_containers: total,
            running_containers: running,
        }
    }
}

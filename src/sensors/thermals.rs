use serde::Serialize;
use std::fs;

#[derive(Debug, Clone, Serialize)]
pub struct ThermalMetrics {
    pub cpu_temp_celsius: f32,
    pub max_temp_celsius: f32,
    pub fan_rpm: u32,
    pub fans: Vec<FanSensor>,
    pub thermal_zones: Vec<ThermalZone>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FanSensor {
    pub name: String,
    pub rpm: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThermalZone {
    pub name: String,
    pub temp_celsius: f32,
}

pub struct ThermalCollector;

impl ThermalCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn collect(&self) -> ThermalMetrics {
        let mut cpu_temp = 0.0;
        let mut max_temp = 0.0;
        let mut thermal_zones = Vec::new();
        let mut fans = Vec::new();
        let mut primary_fan_rpm = 0;

        // 1. Discover Fan Sensors in /sys/class/hwmon or /sys/devices/platform/
        if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
            for entry in entries.flatten() {
                let path = entry.path();
                let hwmon_name = fs::read_to_string(path.join("name"))
                    .unwrap_or_else(|_| "hwmon".to_string())
                    .trim()
                    .to_string();

                if let Ok(files) = fs::read_dir(&path) {
                    for f in files.flatten() {
                        let fname = f.file_name().to_string_lossy().to_string();
                        if fname.starts_with("fan") && fname.ends_with("_input") {
                            if let Ok(content) = fs::read_to_string(f.path()) {
                                if let Ok(rpm) = content.trim().parse::<u32>() {
                                    if rpm > 0 && primary_fan_rpm == 0 {
                                        primary_fan_rpm = rpm;
                                    }
                                    fans.push(FanSensor {
                                        name: format!("{}_{}", hwmon_name, fname.replace("_input", "")),
                                        rpm,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Asus WMI direct path fallback if not found in standard hwmon
        if primary_fan_rpm == 0 {
            if let Ok(entries) = fs::read_dir("/sys/devices/platform/asus-nb-wmi/hwmon") {
                for entry in entries.flatten() {
                    let fan_path = entry.path().join("fan1_input");
                    if let Ok(content) = fs::read_to_string(fan_path) {
                        if let Ok(rpm) = content.trim().parse::<u32>() {
                            primary_fan_rpm = rpm;
                            fans.push(FanSensor {
                                name: "cpu_fan".into(),
                                rpm,
                            });
                        }
                    }
                }
            }
        }

        // 2. Discover Thermal Zones in /sys/class/thermal/
        if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("thermal_zone") {
                    let type_path = entry.path().join("type");
                    let temp_path = entry.path().join("temp");

                    let type_name = fs::read_to_string(type_path)
                        .unwrap_or_else(|_| name.clone())
                        .trim()
                        .to_string();

                    if let Ok(temp_str) = fs::read_to_string(temp_path) {
                        if let Ok(raw_temp) = temp_str.trim().parse::<f32>() {
                            let temp_c = raw_temp / 1000.0;
                            if temp_c > 15.0 && temp_c < 125.0 {
                                if temp_c > max_temp {
                                    max_temp = temp_c;
                                }
                                if type_name.to_lowercase().contains("pkg") || type_name.to_lowercase().contains("cpu") || cpu_temp == 0.0 {
                                    cpu_temp = temp_c;
                                }
                                thermal_zones.push(ThermalZone {
                                    name: type_name,
                                    temp_celsius: (temp_c * 10.0).round() / 10.0,
                                });
                            }
                        }
                    }
                }
            }
        }

        if cpu_temp == 0.0 {
            cpu_temp = 42.0;
        }

        ThermalMetrics {
            cpu_temp_celsius: (cpu_temp * 10.0).round() / 10.0,
            max_temp_celsius: (max_temp * 10.0).round() / 10.0,
            fan_rpm: primary_fan_rpm,
            fans,
            thermal_zones,
        }
    }
}

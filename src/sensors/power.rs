use serde::Serialize;
use std::fs;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
pub struct PowerMetrics {
    pub current_watts: f32,
    pub energy_today_wh: f64,
    pub energy_today_human: String,
    pub energy_month_human: String,
    pub energy_year_human: String,
    pub estimated_monthly_cost: String,
    pub estimated_annual_cost: String,
    pub is_rapl_supported: bool,
}

pub struct PowerCollector {
    last_energy_uj: Option<u64>,
    last_time: Instant,
    rapl_path: Option<String>,
    max_range_uj: u64,
    kwh_cost: f64,
    currency: String,
}

impl PowerCollector {
    pub fn new(kwh_cost: f64, currency: String) -> Self {
        let mut rapl_path = None;
        let mut max_range_uj = 262143328850;

        for i in 0..5 {
            let p = format!("/sys/class/powercap/intel-rapl/intel-rapl:{}/energy_uj", i);
            if fs::metadata(&p).is_ok() {
                rapl_path = Some(p);
                let max_p = format!("/sys/class/powercap/intel-rapl/intel-rapl:{}/max_energy_range_uj", i);
                if let Ok(max_str) = fs::read_to_string(max_p) {
                    if let Ok(v) = max_str.trim().parse::<u64>() {
                        max_range_uj = v;
                    }
                }
                break;
            }
        }

        Self {
            last_energy_uj: None,
            last_time: Instant::now(),
            rapl_path,
            max_range_uj,
            kwh_cost,
            currency,
        }
    }

    /// Returns (current_watts, delta_uj, delta_wh, is_supported)
    pub fn sample_raw(&mut self) -> (f32, u64, f64, bool) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_time).as_secs_f64();
        self.last_time = now;

        let mut current_watts = 1.30;
        let mut delta_uj = 0u64;
        let mut is_supported = false;

        if let Some(ref path) = self.rapl_path {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(current_uj) = content.trim().parse::<u64>() {
                    is_supported = true;
                    if let Some(last_uj) = self.last_energy_uj {
                        if dt > 0.1 && dt < 60.0 {
                            delta_uj = if current_uj >= last_uj {
                                current_uj - last_uj
                            } else {
                                (self.max_range_uj - last_uj) + current_uj
                            };
                            current_watts = (delta_uj as f64 / (dt * 1_000_000.0)) as f32;
                        }
                    }
                    self.last_energy_uj = Some(current_uj);
                }
            }
        }

        let delta_wh = (delta_uj as f64) / (1_000_000.0 * 3600.0);
        (current_watts, delta_uj, delta_wh, is_supported)
    }

    pub fn build_metrics(
        &self,
        current_watts: f32,
        today_wh: f64,
        month_wh: f64,
        year_wh: f64,
        is_supported: bool,
    ) -> PowerMetrics {
        let today_kwh = today_wh / 1000.0;
        let month_kwh = month_wh / 1000.0;
        let year_kwh = year_wh / 1000.0;

        let monthly_cost = month_kwh * self.kwh_cost;
        let annual_cost = year_kwh * self.kwh_cost;

        let energy_today_human = if today_wh < 1000.0 {
            format!("{:.1} Wh", today_wh)
        } else {
            format!("{:.2} kWh", today_kwh)
        };

        let energy_month_human = if month_wh < 1000.0 {
            format!("{:.1} Wh", month_wh)
        } else {
            format!("{:.2} kWh", month_kwh)
        };

        let energy_year_human = if year_wh < 1000.0 {
            format!("{:.1} Wh", year_wh)
        } else {
            format!("{:.1} kWh", year_kwh)
        };

        PowerMetrics {
            current_watts: (current_watts * 100.0).round() / 100.0,
            energy_today_wh: (today_wh * 10.0).round() / 10.0,
            energy_today_human,
            energy_month_human,
            energy_year_human,
            estimated_monthly_cost: format!("{}{:.2}", self.currency, monthly_cost),
            estimated_annual_cost: format!("{}{:.2}", self.currency, annual_cost),
            is_rapl_supported: is_supported,
        }
    }
}

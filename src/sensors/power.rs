use serde::Serialize;
use std::fs;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
pub struct PowerMetrics {
    pub current_watts: f32,
    pub energy_today_kwh: f64,
    pub energy_month_kwh: f64,
    pub energy_year_kwh: f64,
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
    accumulated_uj: u64,
}

impl PowerCollector {
    pub fn new(kwh_cost: f64, currency: String) -> Self {
        let mut rapl_path = None;
        let mut max_range_uj = 262143328850;

        // Auto-discover Intel RAPL / AMD powercap path
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
            accumulated_uj: 0,
        }
    }

    pub fn collect(&mut self) -> PowerMetrics {
        let now = Instant::now();
        let dt = now.duration_since(self.last_time).as_secs_f64();
        self.last_time = now;

        let mut current_watts = 1.30;
        let mut is_supported = false;

        if let Some(ref path) = self.rapl_path {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(current_uj) = content.trim().parse::<u64>() {
                    is_supported = true;
                    if let Some(last_uj) = self.last_energy_uj {
                        if dt > 0.1 && dt < 60.0 {
                            let diff = if current_uj >= last_uj {
                                current_uj - last_uj
                            } else {
                                (self.max_range_uj - last_uj) + current_uj
                            };
                            current_watts = (diff as f64 / (dt * 1_000_000.0)) as f32;
                            self.accumulated_uj += diff;
                        }
                    }
                    self.last_energy_uj = Some(current_uj);
                }
            }
        }

        // Calculate projections & totals
        let today_kwh = (current_watts as f64 * 24.0) / 1000.0;
        let month_kwh = today_kwh * 30.0;
        let year_kwh = today_kwh * 365.0;

        let monthly_cost = month_kwh * self.kwh_cost;
        let annual_cost = year_kwh * self.kwh_cost;

        PowerMetrics {
            current_watts: (current_watts * 100.0).round() / 100.0,
            energy_today_kwh: (today_kwh * 1000.0).round() / 1000.0,
            energy_month_kwh: (month_kwh * 100.0).round() / 100.0,
            energy_year_kwh: (year_kwh * 10.0).round() / 10.0,
            estimated_monthly_cost: format!("{}{:.2}", self.currency, monthly_cost),
            estimated_annual_cost: format!("{}{:.2}", self.currency, annual_cost),
            is_rapl_supported: is_supported,
        }
    }
}

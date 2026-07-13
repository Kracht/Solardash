use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Parse a NOAA/Kyoto timestamp, tolerating the formats these APIs use:
/// "2026-06-24T00:00:00" (T-separated), "2026-06-24 00:00:00" (space-separated),
/// and optional fractional seconds / trailing "Z".
fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim().trim_end_matches('Z');
    const FORMATS: [&str; 4] = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
    ];
    FORMATS
        .iter()
        .find_map(|f| chrono::NaiveDateTime::parse_from_str(s, f).ok())
        .map(|dt| dt.and_utc())
}

/// Extract an f64 from a JSON value that may be a number, a numeric string, or null.
fn json_f64(v: Option<&serde_json::Value>) -> Option<f64> {
    v.and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
}

// ============ NOAA Scales (R, S, G) ============

/// NOAA Space Weather Scales for Radio Blackouts (R), Solar Radiation Storms (S),
/// and Geomagnetic Storms (G)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoaaScales {
    #[serde(rename = "R")]
    pub radio_blackout: ScaleLevel,
    #[serde(rename = "S")]
    pub solar_radiation: ScaleLevel,
    #[serde(rename = "G")]
    pub geomagnetic_storm: ScaleLevel,

    // 3-day forecast (days 1, 2, 3)
    pub forecast_day1: NoaaScalesDay,
    pub forecast_day2: NoaaScalesDay,
    pub forecast_day3: NoaaScalesDay,
}

/// NOAA scales for a single forecast day
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoaaScalesDay {
    pub radio_blackout: ScaleLevel,
    pub solar_radiation: ScaleLevel,
    pub geomagnetic_storm: ScaleLevel,
}

/// Scale level with current value and text description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleLevel {
    #[serde(rename = "Scale")]
    pub scale: String, // e.g., "R1", "S3", "G5"
    #[serde(rename = "Text")]
    pub text: String, // e.g., "Minor", "Strong", "Extreme"
}

impl Default for NoaaScales {
    fn default() -> Self {
        Self {
            radio_blackout: ScaleLevel {
                scale: "R0".to_string(),
                text: "None".to_string(),
            },
            solar_radiation: ScaleLevel {
                scale: "S0".to_string(),
                text: "None".to_string(),
            },
            geomagnetic_storm: ScaleLevel {
                scale: "G0".to_string(),
                text: "None".to_string(),
            },
            forecast_day1: NoaaScalesDay::default(),
            forecast_day2: NoaaScalesDay::default(),
            forecast_day3: NoaaScalesDay::default(),
        }
    }
}

impl Default for NoaaScalesDay {
    fn default() -> Self {
        Self {
            radio_blackout: ScaleLevel {
                scale: "R0".to_string(),
                text: "None".to_string(),
            },
            solar_radiation: ScaleLevel {
                scale: "S0".to_string(),
                text: "None".to_string(),
            },
            geomagnetic_storm: ScaleLevel {
                scale: "G0".to_string(),
                text: "None".to_string(),
            },
        }
    }
}

impl NoaaScales {
    /// Parse NOAA scales from API JSON response
    /// The API returns an object with date-based keys ("-1", "0", "1", "2", "3")
    /// "0" represents the current day, "1", "2", "3" represent forecast days
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        // Parse current day (key "0")
        let current_day = value
            .get("0")
            .context("Missing '0' key in NOAA scales response")?;

        let r_scale = current_day
            .get("R")
            .and_then(|r| r.get("Scale"))
            .and_then(|s| s.as_str())
            .unwrap_or("0");
        let r_text = current_day
            .get("R")
            .and_then(|r| r.get("Text"))
            .and_then(|t| t.as_str())
            .unwrap_or("none");

        let s_scale = current_day
            .get("S")
            .and_then(|s| s.get("Scale"))
            .and_then(|s| s.as_str())
            .unwrap_or("0");
        let s_text = current_day
            .get("S")
            .and_then(|s| s.get("Text"))
            .and_then(|t| t.as_str())
            .unwrap_or("none");

        let g_scale = current_day
            .get("G")
            .and_then(|g| g.get("Scale"))
            .and_then(|s| s.as_str())
            .unwrap_or("0");
        let g_text = current_day
            .get("G")
            .and_then(|g| g.get("Text"))
            .and_then(|t| t.as_str())
            .unwrap_or("none");

        // Parse forecast days (keys "1", "2", "3")
        let day1 = Self::parse_forecast_day(value, "1");
        let day2 = Self::parse_forecast_day(value, "2");
        let day3 = Self::parse_forecast_day(value, "3");

        Ok(Self {
            radio_blackout: ScaleLevel {
                scale: format!("R{}", r_scale),
                text: r_text.to_string(),
            },
            solar_radiation: ScaleLevel {
                scale: format!("S{}", s_scale),
                text: s_text.to_string(),
            },
            geomagnetic_storm: ScaleLevel {
                scale: format!("G{}", g_scale),
                text: g_text.to_string(),
            },
            forecast_day1: day1,
            forecast_day2: day2,
            forecast_day3: day3,
        })
    }

    /// Parse a single forecast day from the NOAA scales JSON
    fn parse_forecast_day(value: &serde_json::Value, day_key: &str) -> NoaaScalesDay {
        let day = match value.get(day_key) {
            Some(d) => d,
            None => return NoaaScalesDay::default(),
        };

        let r_scale = day
            .get("R")
            .and_then(|r| r.get("Scale"))
            .and_then(|s| s.as_str())
            .unwrap_or("0");
        let r_text = day
            .get("R")
            .and_then(|r| r.get("Text"))
            .and_then(|t| t.as_str())
            .unwrap_or("none");

        let s_scale = day
            .get("S")
            .and_then(|s| s.get("Scale"))
            .and_then(|s| s.as_str())
            .unwrap_or("0");
        let s_text = day
            .get("S")
            .and_then(|s| s.get("Text"))
            .and_then(|t| t.as_str())
            .unwrap_or("none");

        let g_scale = day
            .get("G")
            .and_then(|g| g.get("Scale"))
            .and_then(|s| s.as_str())
            .unwrap_or("0");
        let g_text = day
            .get("G")
            .and_then(|g| g.get("Text"))
            .and_then(|t| t.as_str())
            .unwrap_or("none");

        NoaaScalesDay {
            radio_blackout: ScaleLevel {
                scale: format!("R{}", r_scale),
                text: r_text.to_string(),
            },
            solar_radiation: ScaleLevel {
                scale: format!("S{}", s_scale),
                text: s_text.to_string(),
            },
            geomagnetic_storm: ScaleLevel {
                scale: format!("G{}", g_scale),
                text: g_text.to_string(),
            },
        }
    }
}

// ============ Solar Wind Data ============

/// Solar wind magnetic field measurement
#[derive(Debug, Clone)]
pub struct SolarWindMag {
    pub time: DateTime<Utc>,
    pub bx_gsm: f64,  // X component in GSM coordinates (nT)
    pub by_gsm: f64,  // Y component in GSM coordinates (nT)
    pub bz_gsm: f64,  // Z component (southward) in GSM coordinates (nT)
    pub bt: f64,      // Total magnetic field strength (nT)
    pub lon_gsm: f64, // Longitude in GSM coordinates (degrees)
    pub lat_gsm: f64, // Latitude in GSM coordinates (degrees)
}

/// Solar wind plasma measurement
#[derive(Debug, Clone)]
pub struct SolarWindPlasma {
    pub time: DateTime<Utc>,
    pub density: f64,     // Proton density (particles/cm³)
    pub speed: f64,       // Solar wind speed (km/s)
    pub temperature: f64, // Temperature (K)
}

/// Combined solar wind data for the left panel
#[derive(Debug, Clone, Default)]
pub struct SolarWindData {
    pub magnetic: Vec<SolarWindMag>,
    pub plasma: Vec<SolarWindPlasma>,
}

impl SolarWindData {
    /// Get the last N hours of data (for 6-hour window).
    /// Anchored to the most recent available sample rather than the wall clock,
    /// so the graph still renders when the upstream feed is lagging behind now.
    pub fn get_last_hours(&self, hours: usize) -> Self {
        let latest = self
            .magnetic
            .last()
            .map(|m| m.time)
            .into_iter()
            .chain(self.plasma.last().map(|p| p.time))
            .max()
            .unwrap_or_else(Utc::now);
        let cutoff_time = latest - chrono::Duration::hours(hours as i64);

        Self {
            magnetic: self
                .magnetic
                .iter()
                .filter(|m| m.time >= cutoff_time)
                .cloned()
                .collect(),
            plasma: self
                .plasma
                .iter()
                .filter(|p| p.time >= cutoff_time)
                .cloned()
                .collect(),
        }
    }

    /// Get current values (most recent measurement)
    pub fn get_current_magnetic(&self) -> Option<&SolarWindMag> {
        self.magnetic.last()
    }

    pub fn get_current_plasma(&self) -> Option<&SolarWindPlasma> {
        self.plasma.last()
    }

    /// Parse solar wind magnetic field data from the RTSW API JSON array.
    /// Format: array of objects {time_tag, bt, bx_gsm, by_gsm, bz_gsm, phi_gsm, theta_gsm, ...}.
    /// The feed is newest-first; result is sorted oldest-first so `.last()` is current.
    pub fn parse_magnetic(data: Vec<serde_json::Value>) -> Result<Vec<SolarWindMag>> {
        let mut measurements = Vec::new();

        for row in &data {
            let time = match row.get("time_tag").and_then(|v| v.as_str()).and_then(parse_timestamp) {
                Some(t) => t,
                None => continue,
            };
            // Skip rows with a missing total field (null or -9999 sentinel).
            let bt = match json_f64(row.get("bt")) {
                Some(v) if v > -9990.0 => v,
                _ => continue,
            };

            measurements.push(SolarWindMag {
                time,
                bx_gsm: json_f64(row.get("bx_gsm")).unwrap_or(0.0),
                by_gsm: json_f64(row.get("by_gsm")).unwrap_or(0.0),
                bz_gsm: json_f64(row.get("bz_gsm")).unwrap_or(0.0),
                bt,
                lon_gsm: json_f64(row.get("phi_gsm")).unwrap_or(0.0),
                lat_gsm: json_f64(row.get("theta_gsm")).unwrap_or(0.0),
            });
        }

        measurements.sort_by_key(|m| m.time);
        Ok(measurements)
    }

    /// Parse solar wind plasma data from the RTSW API JSON array.
    /// Format: array of objects {time_tag, proton_speed, proton_density, proton_temperature, ...}.
    /// The feed is newest-first; result is sorted oldest-first so `.last()` is current.
    pub fn parse_plasma(data: Vec<serde_json::Value>) -> Result<Vec<SolarWindPlasma>> {
        let mut measurements = Vec::new();

        for row in &data {
            let time = match row.get("time_tag").and_then(|v| v.as_str()).and_then(parse_timestamp) {
                Some(t) => t,
                None => continue,
            };
            // Skip rows with a missing speed (null or -9999 sentinel).
            let speed = match json_f64(row.get("proton_speed")) {
                Some(v) if v > -9990.0 => v,
                _ => continue,
            };

            measurements.push(SolarWindPlasma {
                time,
                density: json_f64(row.get("proton_density")).unwrap_or(0.0),
                speed,
                temperature: json_f64(row.get("proton_temperature")).unwrap_or(0.0),
            });
        }

        measurements.sort_by_key(|m| m.time);
        Ok(measurements)
    }
}

// ============ X-Ray Flares ============

/// X-ray flare event from GOES satellite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XRayFlare {
    #[serde(rename = "begin_time")]
    pub begin_time: String, // ISO 8601 timestamp

    #[serde(rename = "end_time")]
    pub end_time: Option<String>, // ISO 8601 timestamp or null if ongoing

    #[serde(rename = "max_time")]
    pub peak_time: Option<String>, // ISO 8601 timestamp (called max_time in API)

    #[serde(rename = "max_class")]
    pub class_type: String, // e.g., "M4.8", "X2.1" (using max_class from API)

    #[serde(rename = "begin_class")]
    pub begin_class: Option<String>, // Beginning class

    #[serde(rename = "end_class")]
    pub end_class: Option<String>, // Ending class

    #[serde(default)]
    pub source_location: Option<String>, // e.g., "N12E34" (may not be in API)

    #[serde(default)]
    pub active_region: Option<i32>, // NOAA active region number (may not be in API)
}

impl XRayFlare {
    /// Check if the flare is still ongoing
    pub fn is_ongoing(&self) -> bool {
        self.end_time.is_none()
    }

    /// Get flare class letter (A, B, C, M, X)
    pub fn class_letter(&self) -> char {
        self.class_type.chars().next().unwrap_or('A')
    }

    /// Get flare class magnitude
    pub fn class_magnitude(&self) -> f64 {
        self.class_type[1..].parse().unwrap_or(0.0)
    }
}

/// Latest flare events for the right panel
#[derive(Debug, Clone, Default)]
pub struct FlareData {
    pub flares: Vec<XRayFlare>,
}

impl FlareData {
    /// Get the most recent flare event
    pub fn get_latest(&self) -> Option<&XRayFlare> {
        self.flares.first()
    }

    /// Parse X-ray flare data from API JSON array
    /// Expected format: array of flare event objects.
    /// Parsed field-by-field so that in-progress flares (which have
    /// `max_class: null` and `max_time`/`end_time: "Unk"` until they peak/end)
    /// are handled gracefully instead of being dropped.
    pub fn from_json(data: Vec<serde_json::Value>) -> Result<Self> {
        let mut flares = Vec::new();

        // Treat placeholder timestamps like "Unk" as absent.
        let opt_time = |v: Option<&serde_json::Value>| -> Option<String> {
            v.and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("Unk"))
                .map(|s| s.to_string())
        };
        let opt_str = |v: Option<&serde_json::Value>| -> Option<String> {
            v.and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        };

        for value in &data {
            // Skip entries without a begin_time; everything else is optional.
            let begin_time = match opt_time(value.get("begin_time")) {
                Some(t) => t,
                None => continue,
            };

            // Prefer the peak (max) class; before a flare peaks this is null,
            // so fall back to the current class for a sensible display value.
            let class_type = opt_str(value.get("max_class"))
                .or_else(|| opt_str(value.get("current_class")))
                .unwrap_or_else(|| "A0.0".to_string());

            flares.push(XRayFlare {
                begin_time,
                end_time: opt_time(value.get("end_time")),
                peak_time: opt_time(value.get("max_time")),
                class_type,
                begin_class: opt_str(value.get("begin_class")),
                end_class: opt_str(value.get("end_class")),
                source_location: opt_str(value.get("sourceLocation"))
                    .or_else(|| opt_str(value.get("source_location"))),
                active_region: value
                    .get("activeRegion")
                    .or_else(|| value.get("active_region"))
                    .and_then(|v| v.as_i64().map(|n| n as i32)),
            });
        }

        Ok(Self { flares })
    }
}

// ============ Kp Index ============

/// Planetary K-index measurement
#[derive(Debug, Clone)]
pub struct KpIndexMeasurement {
    pub time: DateTime<Utc>,
    pub kp: f64,                    // Kp value (0-9 scale)
    pub a_running: Option<f64>,     // Running A-index
    pub station_count: Option<i32>, // Number of stations reporting
}

/// Kp-index data for geomagnetic storm monitoring
#[derive(Debug, Clone, Default)]
pub struct KpIndexData {
    pub measurements: Vec<KpIndexMeasurement>,
}

impl KpIndexData {
    /// Get the current (most recent) Kp value
    pub fn get_current(&self) -> Option<&KpIndexMeasurement> {
        self.measurements.last()
    }

    /// Get the current Kp value as a float, or 0.0 if not available
    pub fn get_current_value(&self) -> f64 {
        self.get_current().map(|m| m.kp).unwrap_or(0.0)
    }

    /// Parse Kp index data from API JSON array
    /// Expected format: array of objects
    /// [{"time_tag": "2026-06-24T00:00:00", "Kp": 2.33, "a_running": 9, "station_count": 8}, ...]
    pub fn from_json(data: Vec<serde_json::Value>) -> Result<Self> {
        let mut measurements = Vec::new();

        for row in &data {
            // Parse time_tag - format is "YYYY-MM-DDTHH:MM:SS"
            let time_str = match row.get("time_tag").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue, // Skip malformed rows
            };
            let time = match parse_timestamp(time_str) {
                Some(t) => t,
                None => continue,
            };

            // Parse Kp value (required) - handle both string and number
            let kp = match row
                .get("Kp")
                .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            {
                Some(k) => k,
                None => continue,
            };

            let a_running = row
                .get("a_running")
                .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())));

            let station_count = row.get("station_count").and_then(|v| {
                v.as_i64()
                    .map(|n| n as i32)
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            });

            measurements.push(KpIndexMeasurement {
                time,
                kp,
                a_running,
                station_count,
            });
        }

        Ok(Self { measurements })
    }
}

// ============ Three-Day Forecast ============

/// Three-day forecast for R, S, G scales
#[derive(Debug, Clone)]
pub struct ThreeDayForecast {
    pub day1: DayForecast,
    pub day2: DayForecast,
    pub day3: DayForecast,
}

/// Forecast for a single day
#[derive(Debug, Clone)]
pub struct DayForecast {
    pub date: String,              // Date string
    pub radio_blackout: String,    // R scale prediction (e.g., "R1-R2")
    pub solar_radiation: String,   // S scale prediction
    pub geomagnetic_storm: String, // G scale prediction
}

impl Default for ThreeDayForecast {
    fn default() -> Self {
        Self {
            day1: DayForecast {
                date: "N/A".to_string(),
                radio_blackout: "R0".to_string(),
                solar_radiation: "S0".to_string(),
                geomagnetic_storm: "G0".to_string(),
            },
            day2: DayForecast {
                date: "N/A".to_string(),
                radio_blackout: "R0".to_string(),
                solar_radiation: "S0".to_string(),
                geomagnetic_storm: "G0".to_string(),
            },
            day3: DayForecast {
                date: "N/A".to_string(),
                radio_blackout: "R0".to_string(),
                solar_radiation: "S0".to_string(),
                geomagnetic_storm: "G0".to_string(),
            },
        }
    }
}

impl ThreeDayForecast {
    /// Parse 3-day forecast from plain text format
    /// The text format is complex and contains multiple sections
    /// We'll extract the NOAA Geomagnetic Activity Probabilities table
    pub fn from_text(text: &str) -> Result<Self> {
        // Initialize with defaults
        let mut forecast = Self::default();

        // Look for the forecast table section
        // Format typically contains lines like:
        // "NOAA Geomagnetic Activity Probabilities"
        // followed by date rows with R, S, G predictions

        let lines: Vec<&str> = text.lines().collect();

        // Find the probabilities section
        let mut in_forecast_section = false;
        let mut day_index = 0;

        for line in lines {
            // Check if we're entering the forecast section
            if line.contains("NOAA Geomagnetic Activity Observation and Forecast")
                || line.contains("Three Day Forecast")
            {
                in_forecast_section = true;
                continue;
            }

            if !in_forecast_section {
                continue;
            }

            // Look for date lines in format: "Jan 15"
            // These are followed by the R, S, G scale predictions
            let trimmed = line.trim();

            // Simple parsing: look for lines with scale indicators
            if trimmed.contains("None")
                || trimmed.contains("Minor")
                || trimmed.contains("Moderate")
                || trimmed.contains("Strong")
            {
                // Extract scale values from the line
                // This is a simplified parser - in production you'd want more robust parsing
                if day_index < 3 {
                    let forecast_day = match day_index {
                        0 => &mut forecast.day1,
                        1 => &mut forecast.day2,
                        2 => &mut forecast.day3,
                        _ => break,
                    };

                    // Try to extract date from earlier in the line
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        forecast_day.date = format!("{} {}", parts[0], parts[1]);
                    }

                    day_index += 1;
                }
            }
        }

        Ok(forecast)
    }
}

// ============ Aurora Boundary ============

/// Where the aurora boundary came from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuroraSource {
    /// Simplified circle derived from the Kp index (fallback)
    #[default]
    KpModel,
    /// NOAA OVATION Prime nowcast grid
    Ovation,
}

/// Minimum OVATION probability (percent) treated as inside the auroral oval
pub const OVATION_THRESHOLD: f64 = 10.0;

/// Aurora boundary data for world map visualization
#[derive(Debug, Clone, Default)]
pub struct AuroraBoundary {
    pub north_boundary: Vec<(f64, f64)>, // Vector of (latitude, longitude) pairs
    pub south_boundary: Vec<(f64, f64)>, // Vector of (latitude, longitude) pairs
    pub source: AuroraSource,
    /// OVATION probability grid (percent 0-100), 360 lon x 181 lat,
    /// indexed `(lat + 90) * 360 + lon`; None for the Kp fallback
    pub power_grid: Option<Vec<u8>>,
    /// Nowcast validity time from the OVATION feed
    pub forecast_time: Option<DateTime<Utc>>,
}

impl AuroraBoundary {
    /// Calculate aurora boundary based on Kp index
    /// Higher Kp = aurora extends to lower latitudes
    pub fn from_kp_index(kp: f64) -> Self {
        // Simplified model: Kp 0 = ~67° geomagnetic latitude
        // Each Kp level pushes aurora ~2-3° equatorward
        let base_latitude = 67.0;
        let kp_factor = 2.5;
        let north_lat = base_latitude - (kp * kp_factor);
        let south_lat = -(base_latitude - (kp * kp_factor));

        // Generate a circle of points at the calculated latitude
        let mut north_boundary = Vec::new();
        let mut south_boundary = Vec::new();

        for lon in (-180..=180).step_by(5) {
            north_boundary.push((north_lat, lon as f64));
            south_boundary.push((south_lat, lon as f64));
        }

        Self {
            north_boundary,
            south_boundary,
            source: AuroraSource::KpModel,
            power_grid: None,
            forecast_time: None,
        }
    }

    /// Build the boundary from the NOAA OVATION nowcast grid
    /// (`json/ovation_aurora_latest.json`, `coordinates: [[lon, lat, power], ...]`
    /// with lon 0-359, lat -90..90, power 0-100).
    ///
    /// For each longitude the boundary point is the equatorward-most latitude
    /// (poleward of 45°) whose probability reaches `OVATION_THRESHOLD`.
    /// Returns None if the grid is missing or covers too little of the oval
    /// to be usable, so the caller can fall back to the Kp model.
    pub fn from_ovation(json: &serde_json::Value) -> Option<Self> {
        let coords = json.get("coordinates")?.as_array()?;

        // Per-degree equatorward boundary latitude, indexed by lon 0-359
        let mut north: Vec<Option<f64>> = vec![None; 360];
        let mut south: Vec<Option<f64>> = vec![None; 360];
        let mut grid = vec![0u8; 360 * 181];

        for point in coords {
            let p = match point.as_array() {
                Some(p) if p.len() >= 3 => p,
                _ => continue,
            };
            let (lon, lat, power) = match (p[0].as_f64(), p[1].as_f64(), p[2].as_f64()) {
                (Some(lon), Some(lat), Some(power)) => (lon, lat, power),
                _ => continue,
            };
            let li = (lon as i64).rem_euclid(360) as usize;
            let lat_i = ((lat.round() as i64).clamp(-90, 90) + 90) as usize;
            grid[lat_i * 360 + li] = power.clamp(0.0, 100.0) as u8;
            if power < OVATION_THRESHOLD {
                continue;
            }
            if lat >= 45.0 {
                if north[li].is_none_or(|cur| lat < cur) {
                    north[li] = Some(lat);
                }
            } else if lat <= -45.0 {
                if south[li].is_none_or(|cur| lat > cur) {
                    south[li] = Some(lat);
                }
            }
        }

        // Convert to (lat, lon) pairs sorted by lon in -180..180
        let collect = |grid: &[Option<f64>]| -> Vec<(f64, f64)> {
            (180..540)
                .filter_map(|i| {
                    let li = i % 360;
                    grid[li].map(|lat| (lat, (li as f64 + 180.0) % 360.0 - 180.0))
                })
                .collect()
        };
        let north_boundary = collect(&north);
        let south_boundary = collect(&south);

        // Reject nowcasts too sparse to trace an oval (quiet dayside gaps
        // are normal; near-empty grids are not)
        if north_boundary.len() < 30 || south_boundary.len() < 30 {
            return None;
        }

        Some(Self {
            north_boundary,
            south_boundary,
            source: AuroraSource::Ovation,
            power_grid: Some(grid),
            forecast_time: json
                .get("Forecast Time")
                .and_then(|v| v.as_str())
                .and_then(parse_timestamp),
        })
    }

    /// OVATION probability (percent) at the given coordinates,
    /// nearest-degree lookup; 0 when no grid is loaded (Kp fallback)
    pub fn power_at(&self, lat: f64, lon: f64) -> f64 {
        let Some(grid) = &self.power_grid else {
            return 0.0;
        };
        let li = (lon.round() as i64).rem_euclid(360) as usize;
        let lat_i = ((lat.round() as i64).clamp(-90, 90) + 90) as usize;
        grid[lat_i * 360 + li] as f64
    }

    /// Boundary latitude at the given longitude for the northern oval,
    /// linearly interpolated (with wraparound) across dayside gaps
    pub fn north_lat_at(&self, lon: f64) -> Option<f64> {
        interpolate_boundary_lat(&self.north_boundary, lon)
    }

    /// Boundary latitude at the given longitude for the southern oval
    pub fn south_lat_at(&self, lon: f64) -> Option<f64> {
        interpolate_boundary_lat(&self.south_boundary, lon)
    }
}

/// Interpolate a boundary latitude at `lon` from (lat, lon) points sorted by
/// longitude ascending in -180..180, wrapping around the antimeridian
fn interpolate_boundary_lat(points: &[(f64, f64)], lon: f64) -> Option<f64> {
    match points {
        [] => None,
        [(lat, _)] => Some(*lat),
        _ => {
            let (last_lat, last_lon) = *points.last().unwrap();
            let (mut prev_lat, mut prev_lon) = (last_lat, last_lon - 360.0);
            for &(lat, plon) in points {
                if plon >= lon {
                    let span = plon - prev_lon;
                    let t = if span > 0.0 { (lon - prev_lon) / span } else { 0.0 };
                    return Some(prev_lat + (lat - prev_lat) * t);
                }
                prev_lat = lat;
                prev_lon = plon;
            }
            // lon is past the last point: wrap to the first
            let (first_lat, first_lon) = points[0];
            let span = first_lon + 360.0 - prev_lon;
            let t = if span > 0.0 { (lon - prev_lon) / span } else { 0.0 };
            Some(prev_lat + (first_lat - prev_lat) * t)
        }
    }
}

// ============ Dst (Disturbance Storm Time) Index ============

/// Dst geomagnetic index measurement
#[derive(Debug, Clone)]
pub struct DstMeasurement {
    pub time: DateTime<Utc>,
    pub dst: f64, // Dst value in nT
}

/// Dst index data for geomagnetic storm monitoring
#[derive(Debug, Clone, Default)]
pub struct DstData {
    pub measurements: Vec<DstMeasurement>,
}

impl DstData {
    /// Get the current (most recent) Dst value
    pub fn get_current_value(&self) -> f64 {
        self.measurements.last().map(|m| m.dst).unwrap_or(0.0)
    }

    /// Get the last N hours of data, anchored to the most recent sample so the
    /// graph still renders when the upstream feed is lagging behind now.
    pub fn get_last_hours(&self, hours: usize) -> Vec<DstMeasurement> {
        let latest = self
            .measurements
            .last()
            .map(|m| m.time)
            .unwrap_or_else(Utc::now);
        let cutoff_time = latest - chrono::Duration::hours(hours as i64);
        self.measurements
            .iter()
            .filter(|m| m.time >= cutoff_time)
            .cloned()
            .collect()
    }

    /// Parse Dst data from API JSON array
    /// Expected format: array of objects
    /// [{"time_tag": "2026-06-24T14:00:00", "dst": -1}, ...]
    pub fn from_json(data: Vec<serde_json::Value>) -> Result<Self> {
        let mut measurements = Vec::new();

        for row in &data {
            let time_str = match row.get("time_tag").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue,
            };
            let time = match parse_timestamp(time_str) {
                Some(t) => t,
                None => continue,
            };

            let dst = row
                .get("dst")
                .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                .unwrap_or(0.0);

            measurements.push(DstMeasurement { time, dst });
        }

        Ok(Self { measurements })
    }
}

// ============ Band Conditions ============

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandQuality {
    Good,
    Fair,
    Poor,
}

impl BandQuality {
    pub fn label(&self) -> &'static str {
        match self {
            BandQuality::Good => "GOOD",
            BandQuality::Fair => "FAIR",
            BandQuality::Poor => "POOR",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BandCondition {
    pub band: &'static str,
    pub quality: BandQuality,
}

/// Estimate HF band conditions from Solar Flux Index and Kp
pub fn estimate_band_conditions(sfi: f64, kp: f64) -> Vec<BandCondition> {
    let bands: &[(&str, f64)] = &[
        ("160m", 1.8), ("80m", 3.5), ("60m", 5.3), ("40m", 7.0),
        ("30m", 10.0), ("20m", 14.0), ("17m", 18.0), ("15m", 21.0),
        ("12m", 24.0), ("11m", 27.0), ("10m", 28.0), ("6m", 50.0),
        ("2m", 144.0),
    ];

    bands.iter().map(|(name, freq)| {
        BandCondition {
            band: name,
            quality: estimate_single_band(*freq, sfi, kp),
        }
    }).collect()
}

fn estimate_single_band(freq_mhz: f64, sfi: f64, kp: f64) -> BandQuality {
    // VHF (2m) - tropo/sporadic E only
    if freq_mhz >= 144.0 {
        if sfi > 250.0 && kp < 3.0 { return BandQuality::Fair; }
        return BandQuality::Poor;
    }

    // 6m - sporadic E, rare F2
    if freq_mhz >= 50.0 {
        if sfi > 200.0 && kp < 3.0 { return BandQuality::Good; }
        if sfi > 150.0 && kp < 4.0 { return BandQuality::Fair; }
        return BandQuality::Poor;
    }

    // 12m, 11m, 10m - need high SFI
    if freq_mhz >= 24.0 {
        if kp >= 7.0 { return BandQuality::Poor; }
        if sfi > 150.0 && kp < 4.0 { return BandQuality::Good; }
        if sfi > 100.0 && kp < 5.0 { return BandQuality::Fair; }
        return BandQuality::Poor;
    }

    // 20m, 17m, 15m - need moderate SFI
    if freq_mhz >= 14.0 {
        if kp >= 7.0 { return BandQuality::Poor; }
        if sfi > 100.0 && kp < 4.0 { return BandQuality::Good; }
        if sfi > 80.0 && kp < 5.0 { return BandQuality::Fair; }
        return BandQuality::Fair;
    }

    // 60m, 40m, 30m - reliable mid bands
    if freq_mhz >= 5.0 {
        if kp >= 7.0 { return BandQuality::Poor; }
        if kp < 4.0 { return BandQuality::Good; }
        if kp < 6.0 { return BandQuality::Fair; }
        return BandQuality::Poor;
    }

    // 160m, 80m - low bands, heavily affected by absorption and Kp
    if kp < 3.0 { return BandQuality::Fair; }
    BandQuality::Poor
}

// ============ Upcoming Launch ============

#[derive(Debug, Clone)]
pub struct UpcomingLaunch {
    pub net: DateTime<Utc>,
    pub window_start: DateTime<Utc>,
    pub vehicle: String,
    pub mission: String,
    pub orbit: String,
    pub site: String,
}

impl UpcomingLaunch {
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        let net_str = value
            .get("net")
            .and_then(|v| v.as_str())
            .context("Missing 'net' field")?;
        let net = DateTime::parse_from_rfc3339(net_str)
            .context("Failed to parse 'net' as RFC3339")?
            .with_timezone(&Utc);

        let window_start_str = value
            .get("window_start")
            .and_then(|v| v.as_str())
            .context("Missing 'window_start' field")?;
        let window_start = DateTime::parse_from_rfc3339(window_start_str)
            .context("Failed to parse 'window_start' as RFC3339")?
            .with_timezone(&Utc);

        let vehicle = value
            .pointer("/rocket/configuration/name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let mission = value
            .pointer("/mission/name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Mission")
            .to_string();

        let orbit = value
            .pointer("/mission/orbit/abbrev")
            .and_then(|v| v.as_str())
            .unwrap_or("--")
            .to_string();

        let pad_location = value
            .pointer("/pad/location/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let site = map_site_name(pad_location);

        Ok(Self {
            net,
            window_start,
            vehicle,
            mission,
            orbit,
            site,
        })
    }
}

pub fn map_site_name(ll2_name: &str) -> String {
    if ll2_name.contains("Vandenberg") {
        "Vandenberg SFB".to_string()
    } else if ll2_name.contains("Starbase") {
        "Starbase".to_string()
    } else if ll2_name.contains("Cape Canaveral") || ll2_name.contains("Kennedy") {
        "Cape Canaveral".to_string()
    } else if ll2_name.contains("Guiana") {
        "Guiana Space Centre".to_string()
    } else if ll2_name.contains("Baikonur") {
        "Baikonur Cosmodrome".to_string()
    } else if ll2_name.contains("Jiuquan") {
        "Jiuquan Launch Ctr".to_string()
    } else if ll2_name.contains("Tanegashima") {
        "Tanegashima SC".to_string()
    } else {
        String::new()
    }
}

// ============ Aggregate Dashboard Data ============

/// All data needed for the dashboard
#[derive(Debug, Clone, Default)]
pub struct DashboardData {
    pub noaa_scales: NoaaScales,
    pub solar_wind: SolarWindData,
    pub flares: FlareData,
    pub kp_index: KpIndexData,
    pub dst: DstData,
    pub three_day_forecast: ThreeDayForecast,
    pub aurora_boundary: AuroraBoundary,
    pub solar_flux: f64,
    pub upcoming_launch: Option<UpcomingLaunch>,
    pub last_update: Option<DateTime<Utc>>,
    pub fetch_errors: Vec<String>,
}

impl DashboardData {
    /// Create a new empty dashboard data structure
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the last refresh time to now
    pub fn mark_updated(&mut self) {
        self.last_update = Some(Utc::now());
    }

    /// Update aurora boundary based on current Kp index
    pub fn update_aurora_boundary(&mut self) {
        let kp = self.kp_index.get_current_value();
        self.aurora_boundary = AuroraBoundary::from_kp_index(kp);
    }

    /// Estimate HF band conditions from SFI and Kp
    pub fn get_band_conditions(&self) -> Vec<BandCondition> {
        let sfi = if self.solar_flux > 0.0 {
            self.solar_flux
        } else {
            // Fallback: estimate SFI from flare class
            self.flares.get_latest().map(|f| match f.class_letter() {
                'X' => 200.0,
                'M' => 160.0,
                'C' => 120.0,
                'B' => 90.0,
                _ => 75.0,
            }).unwrap_or(90.0)
        };
        let kp = self.kp_index.get_current_value();
        estimate_band_conditions(sfi, kp)
    }
}

// ============ Utility Functions ============

/// Parse scale level from string (e.g., "R1" -> 1, "G5" -> 5)
pub fn parse_scale_level(scale: &str) -> i32 {
    scale
        .chars()
        .skip(1)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// Get color/severity for scale level (0-5)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleSeverity {
    None,     // 0 - Green
    Minor,    // 1 - Green/Yellow
    Moderate, // 2-3 - Yellow/Orange
    Strong,   // 4 - Orange/Red
    Severe,   // 5 - Red/Dark Red
}

impl ScaleSeverity {
    pub fn from_level(level: i32) -> Self {
        match level {
            0 => Self::None,
            1 => Self::Minor,
            2 | 3 => Self::Moderate,
            4 => Self::Strong,
            5 => Self::Severe,
            _ => Self::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scale_level() {
        assert_eq!(parse_scale_level("R1"), 1);
        assert_eq!(parse_scale_level("G5"), 5);
        assert_eq!(parse_scale_level("S0"), 0);
    }

    #[test]
    fn test_scale_severity() {
        assert_eq!(ScaleSeverity::from_level(0), ScaleSeverity::None);
        assert_eq!(ScaleSeverity::from_level(1), ScaleSeverity::Minor);
        assert_eq!(ScaleSeverity::from_level(3), ScaleSeverity::Moderate);
        assert_eq!(ScaleSeverity::from_level(5), ScaleSeverity::Severe);
    }

    #[test]
    fn test_xray_flare_parsing() {
        let flare = XRayFlare {
            begin_time: "2025-01-01T12:00:00Z".to_string(),
            end_time: Some("2025-01-01T13:00:00Z".to_string()),
            peak_time: Some("2025-01-01T12:30:00Z".to_string()),
            class_type: "M4.8".to_string(),
            begin_class: Some("M2.0".to_string()),
            end_class: Some("M3.5".to_string()),
            source_location: Some("N12E34".to_string()),
            active_region: Some(3514),
        };

        assert_eq!(flare.class_letter(), 'M');
        assert_eq!(flare.class_magnitude(), 4.8);
        assert!(!flare.is_ongoing());
    }

    #[test]
    fn test_aurora_boundary_calculation() {
        let boundary = AuroraBoundary::from_kp_index(5.0);
        assert!(!boundary.north_boundary.is_empty());
        assert!(!boundary.south_boundary.is_empty());

        // At Kp=5, aurora should be around 54.5° latitude
        let expected_lat = 67.0 - (5.0 * 2.5);
        assert_eq!(boundary.north_boundary[0].0, expected_lat);
        assert_eq!(boundary.source, AuroraSource::KpModel);
        // Kp circle is flat, so interpolation returns the same latitude everywhere
        assert_eq!(boundary.north_lat_at(37.3), Some(expected_lat));
    }

    #[test]
    fn test_aurora_boundary_from_ovation() {
        // Synthetic OVATION grid: oval at 65°N / 65°S everywhere,
        // dipping to 60° at lon 100-160, below-threshold noise elsewhere
        let mut coords = Vec::new();
        for lon in 0..360 {
            let lat = if (100..160).contains(&lon) { 60 } else { 65 };
            coords.push(serde_json::json!([lon, lat, 50]));
            coords.push(serde_json::json!([lon, -lat, 50]));
            // Poleward cells also above threshold — boundary must pick the
            // equatorward-most latitude
            coords.push(serde_json::json!([lon, 80, 50]));
            // Sub-threshold power must be ignored
            coords.push(serde_json::json!([lon, 50, 5]));
        }
        let json = serde_json::json!({ "coordinates": coords });

        let boundary = AuroraBoundary::from_ovation(&json).unwrap();
        assert_eq!(boundary.source, AuroraSource::Ovation);
        assert_eq!(boundary.north_lat_at(0.0), Some(65.0));
        assert_eq!(boundary.north_lat_at(120.0), Some(60.0));
        assert_eq!(boundary.south_lat_at(120.0), Some(-60.0));
        // Longitudes are sorted ascending for interpolation
        assert!(boundary
            .north_boundary
            .windows(2)
            .all(|w| w[0].1 < w[1].1));

        // Near-empty grid falls back to None
        let sparse = serde_json::json!({ "coordinates": [[0, 65, 50]] });
        assert!(AuroraBoundary::from_ovation(&sparse).is_none());
    }

    #[test]
    fn test_dashboard_data_creation() {
        let mut data = DashboardData::new();
        assert!(data.last_update.is_none());

        data.mark_updated();
        assert!(data.last_update.is_some());
    }

    #[test]
    fn test_noaa_scales_parsing() {
        let json_str = r#"{
            "0": {
                "DateStamp": "2026-02-07",
                "TimeStamp": "12:00:00",
                "R": {"Scale": "1", "Text": "minor"},
                "S": {"Scale": "0", "Text": "none"},
                "G": {"Scale": "2", "Text": "moderate"}
            },
            "1": {
                "DateStamp": "2026-02-08",
                "R": {"Scale": "2", "Text": "moderate"},
                "S": {"Scale": "1", "Text": "minor"},
                "G": {"Scale": "1", "Text": "minor"}
            },
            "2": {
                "DateStamp": "2026-02-09",
                "R": {"Scale": "0", "Text": "none"},
                "S": {"Scale": "0", "Text": "none"},
                "G": {"Scale": "3", "Text": "strong"}
            },
            "3": {
                "DateStamp": "2026-02-10",
                "R": {"Scale": "1", "Text": "minor"},
                "S": {"Scale": "0", "Text": "none"},
                "G": {"Scale": "0", "Text": "none"}
            }
        }"#;
        let json: serde_json::Value = serde_json::from_str(json_str).unwrap();
        let scales = NoaaScales::from_json(&json).unwrap();

        // Test current day
        assert_eq!(scales.radio_blackout.scale, "R1");
        assert_eq!(scales.radio_blackout.text, "minor");
        assert_eq!(scales.solar_radiation.scale, "S0");
        assert_eq!(scales.geomagnetic_storm.scale, "G2");

        // Test forecast day 1
        assert_eq!(scales.forecast_day1.radio_blackout.scale, "R2");
        assert_eq!(scales.forecast_day1.solar_radiation.scale, "S1");
        assert_eq!(scales.forecast_day1.geomagnetic_storm.scale, "G1");

        // Test forecast day 2
        assert_eq!(scales.forecast_day2.geomagnetic_storm.scale, "G3");

        // Test forecast day 3
        assert_eq!(scales.forecast_day3.radio_blackout.scale, "R1");
    }

    #[test]
    fn test_solar_wind_mag_parsing() {
        // Newest-first, as the RTSW feed delivers; parser must sort oldest-first.
        let json_data = vec![
            serde_json::json!({
                "time_tag": "2025-01-15T12:01:00", "bt": 6.0,
                "bx_gsm": 2.0, "by_gsm": -1.0, "bz_gsm": -3.0,
                "phi_gsm": 45.0, "theta_gsm": 30.0
            }),
            serde_json::json!({
                "time_tag": "2025-01-15T12:00:00", "bt": 5.1,
                "bx_gsm": 1.5, "by_gsm": -2.3, "bz_gsm": 4.2,
                "phi_gsm": 45.0, "theta_gsm": 30.0
            }),
        ];

        let result = SolarWindData::parse_magnetic(json_data).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].bx_gsm, 1.5); // oldest first after sort
        assert_eq!(result[0].bz_gsm, 4.2);
        assert_eq!(result.last().unwrap().bt, 6.0); // newest last
    }

    #[test]
    fn test_solar_wind_plasma_parsing() {
        let json_data = vec![
            serde_json::json!({
                "time_tag": "2025-01-15T12:00:00",
                "proton_density": 5.2, "proton_speed": 420.5, "proton_temperature": 100000.0
            }),
        ];

        let result = SolarWindData::parse_plasma(json_data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].density, 5.2);
        assert_eq!(result[0].speed, 420.5);
        assert_eq!(result[0].temperature, 100000.0);
    }

    #[test]
    fn test_flare_data_parsing() {
        let json_data = vec![serde_json::json!({
            "begin_time": "2025-01-15T10:30:00Z",
            "end_time": "2025-01-15T11:45:00Z",
            "max_time": "2025-01-15T11:00:00Z",
            "max_class": "M2.4",
            "begin_class": "M1.8",
            "end_class": "M2.0",
            "sourceLocation": "N15E20",
            "activeRegion": 3514
        })];

        let result = FlareData::from_json(json_data).unwrap();
        assert_eq!(result.flares.len(), 1);
        assert_eq!(result.flares[0].class_type, "M2.4");
        assert_eq!(result.flares[0].class_letter(), 'M');
        assert_eq!(result.flares[0].class_magnitude(), 2.4);
    }

    #[test]
    fn test_kp_index_parsing() {
        let json_data = vec![
            serde_json::json!({
                "time_tag": "2026-06-24T00:00:00",
                "Kp": 2.33,
                "a_running": 9,
                "station_count": 8
            }),
            serde_json::json!({
                "time_tag": "2026-06-24T03:00:00",
                "Kp": 3.0,
                "a_running": 15,
                "station_count": 8
            }),
        ];

        let result = KpIndexData::from_json(json_data).unwrap();
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.get_current_value(), 3.0);
        assert_eq!(result.get_current().unwrap().station_count, Some(8));
    }

    #[test]
    fn test_dst_parsing() {
        let json_data = vec![
            serde_json::json!({"time_tag": "2026-06-24T14:00:00", "dst": -1}),
            serde_json::json!({"time_tag": "2026-06-24T15:00:00", "dst": 12}),
        ];

        let result = DstData::from_json(json_data).unwrap();
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.get_current_value(), 12.0);
    }

    #[test]
    fn test_three_day_forecast_parsing() {
        let forecast_text = r#"
        :Product: 3-Day Forecast
        NOAA Geomagnetic Activity Observation and Forecast
        Three Day Forecast
        Jan 15  None  Minor  Moderate
        Jan 16  Minor  None  Strong
        Jan 17  None  None  Minor
        "#;

        let result = ThreeDayForecast::from_text(forecast_text);
        assert!(result.is_ok());
        // Basic test - actual parsing is simplified
        let forecast = result.unwrap();
        assert!(!forecast.day1.date.is_empty());
    }
}

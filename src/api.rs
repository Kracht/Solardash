use crate::data::*;
use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

// NOAA SWPC API endpoint constants
const NOAA_SCALES_URL: &str = "https://services.swpc.noaa.gov/products/noaa-scales.json";
const SOLAR_WIND_MAG_URL: &str =
    "https://services.swpc.noaa.gov/json/rtsw/rtsw_mag_1m.json";
const SOLAR_WIND_PLASMA_URL: &str =
    "https://services.swpc.noaa.gov/json/rtsw/rtsw_wind_1m.json";
const XRAY_FLARES_URL: &str =
    "https://services.swpc.noaa.gov/json/goes/primary/xray-flares-latest.json";
const KP_INDEX_URL: &str = "https://services.swpc.noaa.gov/products/noaa-planetary-k-index.json";
const THREE_DAY_FORECAST_URL: &str = "https://services.swpc.noaa.gov/text/3-day-forecast.txt";
const SOLAR_FLUX_URL: &str = "https://services.swpc.noaa.gov/products/summary/10cm-flux.json";
const DST_URL: &str = "https://services.swpc.noaa.gov/products/kyoto-dst.json";
const OVATION_AURORA_URL: &str =
    "https://services.swpc.noaa.gov/json/ovation_aurora_latest.json";
const LL2_UPCOMING_URL: &str = "https://lldev.thespacedevs.com/2.3.0/launches/upcoming/?limit=20&mode=normal";

/// HTTP client for fetching data from NOAA SWPC APIs
pub struct NoaaClient {
    client: Client,
}

impl NoaaClient {
    /// Creates a new NOAA API client with appropriate timeout settings
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("solardash/0.1.0")
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self { client })
    }

    /// Fetches raw text from a given URL
    pub async fn fetch_text(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to send request")?;

        let text = response
            .text()
            .await
            .context("Failed to read response body")?;

        Ok(text)
    }

    /// Fetches and parses JSON from a given URL
    pub async fn fetch_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to send request")?;

        let data = response
            .json::<T>()
            .await
            .context("Failed to parse JSON response")?;

        Ok(data)
    }

    // ============ NOAA SWPC Specific Endpoints ============

    /// Fetches NOAA scales (R, S, G storm scales) with current values
    /// Returns raw JSON value for flexible parsing
    pub async fn fetch_noaa_scales(&self) -> Result<serde_json::Value> {
        self.fetch_json(NOAA_SCALES_URL)
            .await
            .context("Failed to fetch NOAA scales")
    }

    /// Fetches real-time solar wind magnetic field data (RTSW, 1-minute cadence)
    /// Returns array of measurement objects: {time_tag, bt, bx_gsm, by_gsm, bz_gsm, ...}
    pub async fn fetch_solar_wind_mag(&self) -> Result<Vec<serde_json::Value>> {
        self.fetch_json(SOLAR_WIND_MAG_URL)
            .await
            .context("Failed to fetch solar wind magnetic data")
    }

    /// Fetches real-time solar wind plasma data (RTSW, 1-minute cadence)
    /// Returns array of measurement objects: {time_tag, proton_density, proton_speed, ...}
    pub async fn fetch_solar_wind_plasma(&self) -> Result<Vec<serde_json::Value>> {
        self.fetch_json(SOLAR_WIND_PLASMA_URL)
            .await
            .context("Failed to fetch solar wind plasma data")
    }

    /// Fetches latest X-ray flare events from GOES primary satellite
    /// Returns array of flare event objects
    pub async fn fetch_xray_flares(&self) -> Result<Vec<serde_json::Value>> {
        self.fetch_json(XRAY_FLARES_URL)
            .await
            .context("Failed to fetch X-ray flares")
    }

    /// Fetches NOAA planetary K-index data
    /// Returns array of Kp measurement objects: {time_tag, Kp, a_running, station_count}
    pub async fn fetch_kp_index(&self) -> Result<Vec<serde_json::Value>> {
        self.fetch_json(KP_INDEX_URL)
            .await
            .context("Failed to fetch Kp index")
    }

    /// Fetches 3-day space weather forecast as plain text
    /// Returns the full forecast text for parsing
    pub async fn fetch_three_day_forecast(&self) -> Result<String> {
        self.fetch_text(THREE_DAY_FORECAST_URL)
            .await
            .context("Failed to fetch 3-day forecast")
    }

    /// Fetches Kyoto Dst index data
    /// Returns array of measurement objects: {time_tag, dst}
    pub async fn fetch_dst(&self) -> Result<Vec<serde_json::Value>> {
        self.fetch_json(DST_URL)
            .await
            .context("Failed to fetch Dst index")
    }

    /// Fetches the NOAA OVATION Prime aurora nowcast grid
    /// Returns {"coordinates": [[lon, lat, power], ...], "Forecast Time": ...}
    pub async fn fetch_ovation_aurora(&self) -> Result<serde_json::Value> {
        self.fetch_json(OVATION_AURORA_URL)
            .await
            .context("Failed to fetch OVATION aurora nowcast")
    }

    /// Fetches and parses the OVATION aurora nowcast into a boundary
    pub async fn get_ovation_aurora(&self) -> Result<AuroraBoundary> {
        let json = self.fetch_ovation_aurora().await?;
        AuroraBoundary::from_ovation(&json)
            .context("OVATION nowcast grid too sparse or malformed")
    }

    /// Fetches upcoming launches from Launch Library 2
    pub async fn fetch_upcoming_launch(&self) -> Result<serde_json::Value> {
        self.fetch_json(LL2_UPCOMING_URL)
            .await
            .context("Failed to fetch upcoming launches from LL2")
    }

    /// Fetches and parses the next upcoming launch from one of our tracked sites
    pub async fn get_upcoming_launch(&self) -> Result<Option<UpcomingLaunch>> {
        let json = self.fetch_upcoming_launch().await?;
        let results = json
            .get("results")
            .and_then(|v| v.as_array())
            .context("Missing 'results' array in LL2 response")?;

        let now = chrono::Utc::now();
        for entry in results {
            if let Ok(launch) = UpcomingLaunch::from_json(entry) {
                if !launch.site.is_empty() && launch.net > now {
                    return Ok(Some(launch));
                }
            }
        }
        Ok(None)
    }

    /// Fetches the 10.7cm solar radio flux (SFI)
    pub async fn fetch_solar_flux(&self) -> Result<f64> {
        let json: serde_json::Value = self.fetch_json(SOLAR_FLUX_URL)
            .await
            .context("Failed to fetch solar flux")?;
        // The endpoint returns an array of measurements, e.g.
        // [{"flux":203,"time_tag":"2026-06-30T20:00:00"}].
        // Older responses used a top-level object with a "Flux" field, so
        // accept both the array form and the legacy object form.
        let obj = json
            .as_array()
            .and_then(|a| a.last())
            .unwrap_or(&json);
        let flux = obj
            .get("flux")
            .or_else(|| obj.get("Flux"))
            .and_then(|f| f.as_f64().or_else(|| f.as_str().and_then(|s| s.parse().ok())))
            .unwrap_or(0.0);
        Ok(flux)
    }

    // ============ High-Level Parsed Data Methods ============

    /// Fetches and parses NOAA scales into structured data
    pub async fn get_noaa_scales(&self) -> Result<NoaaScales> {
        let json = self.fetch_noaa_scales().await?;
        NoaaScales::from_json(&json)
    }

    /// Fetches and parses solar wind magnetic field data
    pub async fn get_solar_wind_mag(&self) -> Result<Vec<SolarWindMag>> {
        let raw_data = self.fetch_solar_wind_mag().await?;
        SolarWindData::parse_magnetic(raw_data)
    }

    /// Fetches and parses solar wind plasma data
    pub async fn get_solar_wind_plasma(&self) -> Result<Vec<SolarWindPlasma>> {
        let raw_data = self.fetch_solar_wind_plasma().await?;
        SolarWindData::parse_plasma(raw_data)
    }

    /// Fetches and parses X-ray flare events
    pub async fn get_xray_flares(&self) -> Result<FlareData> {
        let raw_data = self.fetch_xray_flares().await?;
        FlareData::from_json(raw_data)
    }

    /// Fetches and parses Kp index data
    pub async fn get_kp_index(&self) -> Result<KpIndexData> {
        let raw_data = self.fetch_kp_index().await?;
        KpIndexData::from_json(raw_data)
    }

    /// Fetches and parses Dst index data
    pub async fn get_dst(&self) -> Result<DstData> {
        let raw_data = self.fetch_dst().await?;
        DstData::from_json(raw_data)
    }

    /// Fetches and parses 3-day forecast
    pub async fn get_three_day_forecast(&self) -> Result<ThreeDayForecast> {
        let text = self.fetch_three_day_forecast().await?;
        ThreeDayForecast::from_text(&text)
    }

    /// Fetches all dashboard data in one go
    pub async fn fetch_all_data(&self) -> Result<DashboardData> {
        let mut data = DashboardData::new();

        // Fetch all data concurrently using tokio::join
        let (
            noaa_scales_result,
            mag_result,
            plasma_result,
            flares_result,
            kp_result,
            dst_result,
            forecast_result,
            flux_result,
            aurora_result,
        ) = tokio::join!(
            self.get_noaa_scales(),
            self.get_solar_wind_mag(),
            self.get_solar_wind_plasma(),
            self.get_xray_flares(),
            self.get_kp_index(),
            self.get_dst(),
            self.get_three_day_forecast(),
            self.fetch_solar_flux(),
            self.get_ovation_aurora(),
        );

        // Update data structure with results, tracking errors for UI display
        let mut errors = Vec::new();

        match noaa_scales_result {
            Ok(scales) => data.noaa_scales = scales,
            Err(e) => errors.push(format!("NOAA scales: {e:#}")),
        }

        match mag_result {
            Ok(mag) => data.solar_wind.magnetic = mag,
            Err(e) => errors.push(format!("Solar wind mag: {e:#}")),
        }

        match plasma_result {
            Ok(plasma) => data.solar_wind.plasma = plasma,
            Err(e) => errors.push(format!("Solar wind plasma: {e:#}")),
        }

        match flares_result {
            Ok(flares) => data.flares = flares,
            Err(e) => errors.push(format!("X-ray flares: {e:#}")),
        }

        match kp_result {
            Ok(kp) => data.kp_index = kp,
            Err(e) => errors.push(format!("Kp index: {e:#}")),
        }

        match dst_result {
            Ok(dst) => data.dst = dst,
            Err(e) => errors.push(format!("Dst index: {e:#}")),
        }

        match forecast_result {
            Ok(forecast) => data.three_day_forecast = forecast,
            Err(e) => errors.push(format!("3-day forecast: {e:#}")),
        }

        match flux_result {
            Ok(flux) => data.solar_flux = flux,
            Err(e) => errors.push(format!("Solar flux: {e:#}")),
        }

        data.fetch_errors = errors;

        // Prefer the OVATION nowcast oval; fall back to the Kp-derived
        // circle if the feed is down or unusable (no error banner — the
        // fallback keeps the map fully functional)
        match aurora_result {
            Ok(boundary) => data.aurora_boundary = boundary,
            Err(_) => data.update_aurora_boundary(),
        }

        // Mark the update time
        data.mark_updated();

        Ok(data)
    }
}

impl Default for NoaaClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default NOAA client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = NoaaClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_text() {
        let client = NoaaClient::new().unwrap();
        // Test with a simple endpoint
        let result = client
            .fetch_text("https://services.swpc.noaa.gov/text/3-day-forecast.txt")
            .await;

        // We don't assert success here because network might be unavailable,
        // but we verify the function signature works
        let _ = result;
    }

    #[tokio::test]
    async fn test_fetch_noaa_scales() {
        let client = NoaaClient::new().unwrap();
        let result = client.fetch_noaa_scales().await;
        // Network test - verify function signature
        let _ = result;
    }

    #[tokio::test]
    async fn test_fetch_solar_wind_mag() {
        let client = NoaaClient::new().unwrap();
        let result = client.fetch_solar_wind_mag().await;
        let _ = result;
    }

    #[tokio::test]
    async fn test_fetch_solar_wind_plasma() {
        let client = NoaaClient::new().unwrap();
        let result = client.fetch_solar_wind_plasma().await;
        let _ = result;
    }

    #[tokio::test]
    async fn test_fetch_xray_flares() {
        let client = NoaaClient::new().unwrap();
        let result = client.fetch_xray_flares().await;
        let _ = result;
    }

    #[tokio::test]
    async fn test_fetch_kp_index() {
        let client = NoaaClient::new().unwrap();
        let result = client.fetch_kp_index().await;
        let _ = result;
    }

    #[tokio::test]
    async fn test_fetch_three_day_forecast() {
        let client = NoaaClient::new().unwrap();
        let result = client.fetch_three_day_forecast().await;
        let _ = result;
    }
}

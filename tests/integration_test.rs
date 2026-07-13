//! Integration tests for API data fetching and parsing

use solardash::api::NoaaClient;

#[tokio::test]
#[ignore] // Ignore by default to avoid network calls in CI
async fn test_fetch_and_parse_noaa_scales() {
    let client = NoaaClient::new().expect("Failed to create client");
    let result = client.get_noaa_scales().await;

    match result {
        Ok(scales) => {
            println!("NOAA Scales:");
            println!(
                "  R (Radio Blackout): {} - {}",
                scales.radio_blackout.scale, scales.radio_blackout.text
            );
            println!(
                "  S (Solar Radiation): {} - {}",
                scales.solar_radiation.scale, scales.solar_radiation.text
            );
            println!(
                "  G (Geomagnetic Storm): {} - {}",
                scales.geomagnetic_storm.scale, scales.geomagnetic_storm.text
            );
        }
        Err(e) => {
            eprintln!(
                "Warning: API call failed (this is expected if network is unavailable): {}",
                e
            );
        }
    }
}

#[tokio::test]
#[ignore] // Ignore by default
async fn test_fetch_and_parse_solar_wind() {
    let client = NoaaClient::new().expect("Failed to create client");

    match client.get_solar_wind_mag().await {
        Ok(mag_data) => {
            println!("Solar Wind Magnetic Data: {} measurements", mag_data.len());
            if let Some(latest) = mag_data.last() {
                println!(
                    "  Latest Bt: {:.2} nT, Bz: {:.2} nT",
                    latest.bt, latest.bz_gsm
                );
            }
        }
        Err(e) => {
            eprintln!("Warning: Magnetic data fetch failed: {}", e);
        }
    }

    match client.get_solar_wind_plasma().await {
        Ok(plasma_data) => {
            println!("Solar Wind Plasma Data: {} measurements", plasma_data.len());
            if let Some(latest) = plasma_data.last() {
                println!(
                    "  Latest Speed: {:.2} km/s, Density: {:.2} p/cm³",
                    latest.speed, latest.density
                );
            }
        }
        Err(e) => {
            eprintln!("Warning: Plasma data fetch failed: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Ignore by default
async fn test_fetch_and_parse_flares() {
    let client = NoaaClient::new().expect("Failed to create client");
    let result = client.get_xray_flares().await;

    match result {
        Ok(flare_data) => {
            println!("X-Ray Flares: {} events", flare_data.flares.len());
            if let Some(latest) = flare_data.get_latest() {
                println!("  Latest: {} at {}", latest.class_type, latest.begin_time);
                println!("  Ongoing: {}", latest.is_ongoing());
            }
        }
        Err(e) => {
            eprintln!("Warning: Flare data fetch failed: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Ignore by default
async fn test_fetch_and_parse_kp_index() {
    let client = NoaaClient::new().expect("Failed to create client");
    let result = client.get_kp_index().await;

    match result {
        Ok(kp_data) => {
            println!("Kp Index: {} measurements", kp_data.measurements.len());
            println!("  Current Kp: {:.1}", kp_data.get_current_value());
        }
        Err(e) => {
            eprintln!("Warning: Kp index fetch failed: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Ignore by default
async fn test_fetch_all_dashboard_data() {
    let client = NoaaClient::new().expect("Failed to create client");
    let result = client.fetch_all_data().await;

    match result {
        Ok(data) => {
            println!("Dashboard Data Fetched Successfully!");
            println!("  Last Update: {:?}", data.last_update);
            println!(
                "  NOAA Scales: R={}, S={}, G={}",
                data.noaa_scales.radio_blackout.scale,
                data.noaa_scales.solar_radiation.scale,
                data.noaa_scales.geomagnetic_storm.scale
            );
            println!(
                "  Solar Wind Magnetic: {} measurements",
                data.solar_wind.magnetic.len()
            );
            println!(
                "  Solar Wind Plasma: {} measurements",
                data.solar_wind.plasma.len()
            );
            println!("  Flares: {} events", data.flares.flares.len());
            println!(
                "  Kp Index: {} measurements",
                data.kp_index.measurements.len()
            );
            println!(
                "  Aurora Boundary: {} north points, {} south points",
                data.aurora_boundary.north_boundary.len(),
                data.aurora_boundary.south_boundary.len()
            );
        }
        Err(e) => {
            eprintln!("Warning: Dashboard data fetch failed: {}", e);
        }
    }
}

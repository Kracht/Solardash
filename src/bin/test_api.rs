//! Standalone test binary for verifying NOAA API data fetching
//! Run with: cargo run --bin test_api

use anyhow::Result;
use solardash::api::NoaaClient;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== NOAA SWPC API Data Fetching Test ===\n");

    let client = NoaaClient::new()?;

    // Test 1: NOAA Scales
    println!("1. Testing NOAA Scales (R, S, G)...");
    match client.get_noaa_scales().await {
        Ok(scales) => {
            println!("   ✓ SUCCESS");
            println!(
                "   Radio Blackout (R): {} - {}",
                scales.radio_blackout.scale, scales.radio_blackout.text
            );
            println!(
                "   Solar Radiation (S): {} - {}",
                scales.solar_radiation.scale, scales.solar_radiation.text
            );
            println!(
                "   Geomagnetic Storm (G): {} - {}",
                scales.geomagnetic_storm.scale, scales.geomagnetic_storm.text
            );
        }
        Err(e) => {
            println!("   ✗ FAILED: {}", e);
            println!("   Trying raw fetch...");
            match client.fetch_noaa_scales().await {
                Ok(raw) => println!("   Raw JSON: {}", serde_json::to_string_pretty(&raw)?),
                Err(e) => println!("   Raw fetch also failed: {}", e),
            }
        }
    }
    println!();

    // Test 2: Solar Wind Magnetic Data
    println!("2. Testing Solar Wind Magnetic Field Data...");
    match client.get_solar_wind_mag().await {
        Ok(mag_data) => {
            println!("   ✓ SUCCESS - {} measurements", mag_data.len());
            if let Some(latest) = mag_data.last() {
                println!("   Latest measurement:");
                println!("     Time: {}", latest.time);
                println!("     Bt (total): {:.2} nT", latest.bt);
                println!("     Bz (GSM): {:.2} nT", latest.bz_gsm);
                println!("     Bx (GSM): {:.2} nT", latest.bx_gsm);
                println!("     By (GSM): {:.2} nT", latest.by_gsm);
            } else {
                println!("   ⚠ No measurements found");
            }
        }
        Err(e) => {
            println!("   ✗ FAILED: {}", e);
            println!("   Trying raw fetch...");
            match client.fetch_solar_wind_mag().await {
                Ok(raw) => {
                    println!("   Raw data: {} rows", raw.len());
                    if raw.len() > 1 {
                        println!("   Sample row: {:?}", raw.get(1));
                    }
                }
                Err(e) => println!("   Raw fetch also failed: {}", e),
            }
        }
    }
    println!();

    // Test 3: Solar Wind Plasma Data
    println!("3. Testing Solar Wind Plasma Data...");
    match client.get_solar_wind_plasma().await {
        Ok(plasma_data) => {
            println!("   ✓ SUCCESS - {} measurements", plasma_data.len());
            if let Some(latest) = plasma_data.last() {
                println!("   Latest measurement:");
                println!("     Time: {}", latest.time);
                println!("     Speed: {:.2} km/s", latest.speed);
                println!("     Density: {:.2} p/cm³", latest.density);
                println!("     Temperature: {:.0} K", latest.temperature);
            } else {
                println!("   ⚠ No measurements found");
            }
        }
        Err(e) => {
            println!("   ✗ FAILED: {}", e);
            println!("   Trying raw fetch...");
            match client.fetch_solar_wind_plasma().await {
                Ok(raw) => {
                    println!("   Raw data: {} rows", raw.len());
                    if raw.len() > 1 {
                        println!("   Sample row: {:?}", raw.get(1));
                    }
                }
                Err(e) => println!("   Raw fetch also failed: {}", e),
            }
        }
    }
    println!();

    // Test 4: X-Ray Flares
    println!("4. Testing X-Ray Flare Events...");
    match client.get_xray_flares().await {
        Ok(flare_data) => {
            println!("   ✓ SUCCESS - {} flare events", flare_data.flares.len());
            if let Some(latest) = flare_data.get_latest() {
                println!("   Latest flare:");
                println!("     Class: {}", latest.class_type);
                println!("     Begin: {}", latest.begin_time);
                println!("     Peak: {:?}", latest.peak_time);
                println!("     End: {:?}", latest.end_time);
                println!("     Ongoing: {}", latest.is_ongoing());
                if let Some(loc) = &latest.source_location {
                    println!("     Location: {}", loc);
                }
                if let Some(ar) = latest.active_region {
                    println!("     Active Region: {}", ar);
                }
            } else {
                println!("   ⚠ No flare events found");
            }
        }
        Err(e) => {
            println!("   ✗ FAILED: {}", e);
            println!("   Trying raw fetch...");
            match client.fetch_xray_flares().await {
                Ok(raw) => {
                    println!("   Raw data: {} events", raw.len());
                    if !raw.is_empty() {
                        println!(
                            "   Sample event: {}",
                            serde_json::to_string_pretty(&raw[0])?
                        );
                    }
                }
                Err(e) => println!("   Raw fetch also failed: {}", e),
            }
        }
    }
    println!();

    // Test 5: Kp Index
    println!("5. Testing Kp Index Data...");
    match client.get_kp_index().await {
        Ok(kp_data) => {
            println!("   ✓ SUCCESS - {} measurements", kp_data.measurements.len());
            if let Some(latest) = kp_data.get_current() {
                println!("   Latest measurement:");
                println!("     Time: {}", latest.time);
                println!("     Kp: {:.1}", latest.kp);
                if let Some(a_running) = latest.a_running {
                    println!("     A-index: {:.1}", a_running);
                }
                if let Some(count) = latest.station_count {
                    println!("     Station count: {}", count);
                }
            } else {
                println!("   ⚠ No measurements found");
            }
        }
        Err(e) => {
            println!("   ✗ FAILED: {}", e);
            println!("   Trying raw fetch...");
            match client.fetch_kp_index().await {
                Ok(raw) => {
                    println!("   Raw data: {} rows", raw.len());
                    if raw.len() > 1 {
                        println!("   Sample row: {:?}", raw.get(1));
                    }
                }
                Err(e) => println!("   Raw fetch also failed: {}", e),
            }
        }
    }
    println!();

    // Test 6: Three-Day Forecast
    println!("6. Testing Three-Day Forecast...");
    match client.get_three_day_forecast().await {
        Ok(forecast) => {
            println!("   ✓ SUCCESS");
            println!(
                "   Day 1 ({}): R={}, S={}, G={}",
                forecast.day1.date,
                forecast.day1.radio_blackout,
                forecast.day1.solar_radiation,
                forecast.day1.geomagnetic_storm
            );
            println!(
                "   Day 2 ({}): R={}, S={}, G={}",
                forecast.day2.date,
                forecast.day2.radio_blackout,
                forecast.day2.solar_radiation,
                forecast.day2.geomagnetic_storm
            );
            println!(
                "   Day 3 ({}): R={}, S={}, G={}",
                forecast.day3.date,
                forecast.day3.radio_blackout,
                forecast.day3.solar_radiation,
                forecast.day3.geomagnetic_storm
            );
        }
        Err(e) => {
            println!("   ✗ FAILED: {}", e);
            println!("   Trying raw fetch...");
            match client.fetch_three_day_forecast().await {
                Ok(raw) => {
                    println!("   Raw text ({} chars):", raw.len());
                    println!("   --- First 500 chars ---");
                    println!("{}", &raw[..raw.len().min(500)]);
                }
                Err(e) => println!("   Raw fetch also failed: {}", e),
            }
        }
    }
    println!();

    // Test 7: Fetch All Data
    println!("7. Testing Fetch All Dashboard Data (concurrent)...");
    match client.fetch_all_data().await {
        Ok(data) => {
            println!("   ✓ SUCCESS");
            println!("   Last update: {:?}", data.last_update);
            println!("   Components fetched:");
            println!(
                "     - NOAA Scales: R={}, S={}, G={}",
                data.noaa_scales.radio_blackout.scale,
                data.noaa_scales.solar_radiation.scale,
                data.noaa_scales.geomagnetic_storm.scale
            );
            println!(
                "     - Solar Wind Magnetic: {} measurements",
                data.solar_wind.magnetic.len()
            );
            println!(
                "     - Solar Wind Plasma: {} measurements",
                data.solar_wind.plasma.len()
            );
            println!("     - Flares: {} events", data.flares.flares.len());
            println!(
                "     - Kp Index: {} measurements (current: {:.1})",
                data.kp_index.measurements.len(),
                data.kp_index.get_current_value()
            );
            println!(
                "     - Aurora Boundary: {} north, {} south points",
                data.aurora_boundary.north_boundary.len(),
                data.aurora_boundary.south_boundary.len()
            );
        }
        Err(e) => {
            println!("   ✗ FAILED: {}", e);
        }
    }
    println!();

    println!("=== Test Complete ===");
    Ok(())
}

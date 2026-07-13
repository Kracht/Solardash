use crate::colors;
use crate::data::{AuroraBoundary, AuroraSource, OVATION_THRESHOLD};
use crate::world_data::WORLD_COASTLINE;
use chrono::{Datelike, Timelike, Utc};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// === Solar Position Functions ===

fn solar_declination(day_of_year: u32) -> f64 {
    let angle = 2.0 * std::f64::consts::PI * (day_of_year as f64 - 81.0) / 365.0;
    23.44 * angle.sin()
}

fn solar_subsolar_longitude(utc_hour: u32, utc_minute: u32) -> f64 {
    let decimal_hours = utc_hour as f64 + (utc_minute as f64 / 60.0);
    let longitude = (12.0 - decimal_hours) * 15.0;
    if longitude > 180.0 {
        longitude - 360.0
    } else if longitude < -180.0 {
        longitude + 360.0
    } else {
        longitude
    }
}

fn is_in_daylight(lat: f64, lon: f64, solar_lat: f64, solar_lon: f64) -> bool {
    let lat1 = lat.to_radians();
    let lon1 = lon.to_radians();
    let lat2 = solar_lat.to_radians();
    let lon2 = solar_lon.to_radians();
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    c.to_degrees() < 90.0
}

fn is_near_terminator(lat: f64, lon: f64, solar_lat: f64, solar_lon: f64) -> bool {
    let lat1 = lat.to_radians();
    let lon1 = lon.to_radians();
    let lat2 = solar_lat.to_radians();
    let lon2 = solar_lon.to_radians();
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    (c.to_degrees() - 90.0).abs() < 6.0
}

// === Coastline Data ===
// Uses Natural Earth high-resolution data (5125 points) from world_data module.
// Data format: (longitude, latitude) pairs organized as polygons separated by large jumps.

/// Parse WORLD_COASTLINE data into polygon segments for polyline drawing.
/// Segments are split where consecutive points jump more than 10 degrees apart.
fn coastline_segments() -> Vec<Vec<(f64, f64)>> {
    let data = &WORLD_COASTLINE;
    let mut segments: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();

    for &(lon, lat) in data.iter() {
        if let Some(&(prev_lat, prev_lon)) = current.last() {
            let dlat = (lat - prev_lat).abs();
            let dlon = (lon - prev_lon).abs();
            // Large jump = new polygon
            if dlat > 10.0 || dlon > 10.0 {
                if current.len() >= 2 {
                    segments.push(current);
                }
                current = Vec::new();
            }
        }
        // Store as (lat, lon) for our projection functions
        current.push((lat, lon));
    }
    if current.len() >= 2 {
        segments.push(current);
    }
    segments
}

/// Major country borders (simplified polylines in (lat, lon) format)
fn border_data() -> Vec<Vec<(f64, f64)>> {
    vec![
        // US-Canada border (49th parallel)
        vec![
            (49.0, -124.0), (49.0, -115.0), (49.0, -105.0), (49.0, -95.0),
        ],
        // US-Canada eastern border
        vec![
            (49.0, -95.0), (48.0, -90.0), (47.0, -85.0), (46.0, -84.0),
            (45.0, -82.0), (42.5, -83.0), (43.0, -79.0), (44.0, -76.0),
            (45.0, -72.0), (47.0, -68.0),
        ],
        // US-Mexico border
        vec![
            (32.5, -117.0), (32.0, -113.0), (31.5, -110.0), (31.5, -108.0),
            (30.0, -105.0), (29.5, -103.0), (28.0, -100.0), (27.0, -97.0),
        ],
        // China-Mongolia border
        vec![
            (50.0, 87.0), (48.0, 90.0), (46.0, 91.0), (44.0, 96.0),
            (42.0, 105.0), (42.0, 110.0), (44.0, 112.0), (46.0, 116.0),
            (47.5, 120.0),
        ],
        // India-Pakistan border
        vec![
            (35.0, 77.0), (33.0, 74.0), (30.5, 71.0), (28.0, 70.0),
            (25.0, 69.0), (24.0, 68.5),
        ],
        // India-China border (Himalayas)
        vec![
            (35.0, 77.0), (32.0, 79.0), (30.0, 81.0), (28.5, 84.0),
            (28.0, 86.0), (27.5, 89.0), (28.0, 92.0), (28.0, 97.0),
        ],
        // Russia-Kazakhstan border
        vec![
            (54.0, 55.0), (52.0, 59.0), (51.0, 62.0), (51.0, 67.0),
            (50.0, 73.0), (51.0, 77.0), (50.0, 80.0), (49.0, 87.0),
        ],
        // Russia European western border
        vec![
            (70.0, 30.0), (68.0, 30.0), (65.0, 30.0), (62.0, 32.0),
            (60.0, 30.0), (58.0, 28.0), (56.0, 28.0), (54.0, 26.0),
            (52.0, 24.0), (50.0, 24.0), (48.0, 25.0), (47.0, 30.0),
            (46.0, 33.0), (45.0, 36.0), (44.0, 40.0), (43.0, 42.0),
        ],
        // Egypt-Libya border
        vec![(31.0, 25.0), (25.0, 25.0), (22.0, 25.0)],
        // Egypt-Sudan border
        vec![(22.0, 25.0), (22.0, 31.0), (22.0, 37.0)],
        // Algeria southern border
        vec![
            (27.0, -8.5), (24.0, -1.0), (22.0, 2.0), (20.0, 4.0),
            (19.0, 6.0), (18.0, 8.0), (16.0, 12.0), (15.0, 16.0),
        ],
        // Chad-Sudan border
        vec![
            (23.0, 24.0), (20.0, 24.0), (15.0, 24.0), (13.0, 22.0),
            (10.0, 24.0), (8.0, 25.0),
        ],
        // Norway-Sweden border
        vec![
            (69.0, 18.0), (67.0, 16.0), (65.0, 14.0), (63.0, 12.0),
            (61.0, 12.0), (59.0, 11.5), (58.5, 11.0),
        ],
        // Afghanistan-Pakistan border
        vec![
            (37.0, 71.0), (36.0, 71.5), (35.0, 71.5), (34.0, 71.0),
            (33.0, 70.0), (32.0, 69.0), (31.0, 67.0), (30.0, 66.0),
            (29.0, 64.0),
        ],
        // Iran-Iraq border
        vec![
            (37.0, 42.0), (35.0, 46.0), (33.5, 46.0), (31.5, 47.5),
            (30.5, 48.0),
        ],
        // Turkey-Syria border
        vec![
            (37.0, 36.0), (37.0, 38.0), (37.0, 40.0), (37.0, 42.0),
        ],
        // China-North Korea border
        vec![
            (42.5, 130.0), (41.5, 128.0), (40.5, 125.0), (40.0, 124.0),
        ],
        // North-South Korea DMZ
        vec![(38.0, 125.0), (38.0, 127.0), (38.5, 128.5)],
        // === EUROPEAN BORDERS ===
        // France-Spain border (Pyrenees)
        vec![
            (43.3, -1.8), (42.8, -0.5), (42.5, 0.7), (42.5, 1.5),
            (42.5, 3.1),
        ],
        // France-Germany border (Rhine)
        vec![
            (49.5, 6.0), (48.5, 7.6), (47.6, 7.6),
        ],
        // France-Italy border (Alps)
        vec![
            (43.8, 7.5), (44.5, 6.8), (45.3, 7.0), (45.8, 7.0),
            (46.2, 7.0), (46.5, 8.0), (47.0, 9.5),
        ],
        // Germany-Denmark border
        vec![
            (54.8, 8.6), (54.8, 9.4), (54.6, 10.0),
        ],
        // Germany-Poland border
        vec![
            (54.0, 14.2), (52.5, 14.5), (51.5, 14.9), (51.0, 15.0),
            (50.4, 12.1),
        ],
        // Germany-Austria-Switzerland
        vec![
            (47.5, 7.6), (47.5, 10.0), (47.3, 11.0), (47.3, 13.0),
            (47.0, 15.0),
        ],
        // Poland-Czech-Slovakia
        vec![
            (50.4, 12.1), (49.5, 14.4), (49.0, 17.0), (49.4, 19.0),
            (49.0, 22.0),
        ],
        // Spain-Portugal border
        vec![
            (42.0, -8.6), (41.0, -7.4), (39.5, -7.0), (38.5, -7.0),
            (37.0, -7.5),
        ],
        // Norway-Sweden border
        vec![
            (69.0, 18.0), (67.0, 16.0), (65.0, 14.0), (63.0, 12.0),
            (61.0, 12.0), (59.0, 11.5), (58.5, 11.0),
        ],
        // Sweden-Finland border
        vec![
            (69.0, 20.5), (67.0, 24.0), (65.5, 24.0), (63.5, 22.0),
            (61.0, 22.0), (60.3, 21.5),
        ],
    ]
}

// === Braille Rendering Engine ===

/// Encode a 2x4 dot pattern into a Unicode braille character
fn dots_to_braille(dots: [[bool; 4]; 2]) -> char {
    let mut code: u32 = 0x2800;
    // Left column (index 0)
    if dots[0][0] { code |= 0x01; }
    if dots[0][1] { code |= 0x02; }
    if dots[0][2] { code |= 0x04; }
    if dots[0][3] { code |= 0x40; }
    // Right column (index 1)
    if dots[1][0] { code |= 0x08; }
    if dots[1][1] { code |= 0x10; }
    if dots[1][2] { code |= 0x20; }
    if dots[1][3] { code |= 0x80; }
    char::from_u32(code).unwrap_or(' ')
}

/// Canvas for braille-based map rendering
struct MapCanvas {
    coast_pixels: Vec<bool>,
    grid_pixels: Vec<bool>,
    aurora_pixels: Vec<bool>,
    aurora_fill_pixels: Vec<bool>,
    border_pixels: Vec<bool>,
    terminator_pixels: Vec<bool>,
    land_fill_pixels: Vec<bool>,
    pixel_width: usize,
    pixel_height: usize,
}

impl MapCanvas {
    fn new(char_width: usize, char_height: usize) -> Self {
        let pw = char_width * 2;
        let ph = char_height * 4;
        Self {
            coast_pixels: vec![false; pw * ph],
            grid_pixels: vec![false; pw * ph],
            aurora_pixels: vec![false; pw * ph],
            aurora_fill_pixels: vec![false; pw * ph],
            border_pixels: vec![false; pw * ph],
            terminator_pixels: vec![false; pw * ph],
            land_fill_pixels: vec![false; pw * ph],
            pixel_width: pw,
            pixel_height: ph,
        }
    }

    fn set_coast(&mut self, x: usize, y: usize) {
        if x < self.pixel_width && y < self.pixel_height {
            self.coast_pixels[y * self.pixel_width + x] = true;
        }
    }

    fn set_grid(&mut self, x: usize, y: usize) {
        if x < self.pixel_width && y < self.pixel_height {
            self.grid_pixels[y * self.pixel_width + x] = true;
        }
    }

    fn set_aurora(&mut self, x: usize, y: usize) {
        if x < self.pixel_width && y < self.pixel_height {
            self.aurora_pixels[y * self.pixel_width + x] = true;
        }
    }

    fn set_aurora_fill(&mut self, x: usize, y: usize) {
        if x < self.pixel_width && y < self.pixel_height {
            self.aurora_fill_pixels[y * self.pixel_width + x] = true;
        }
    }

    fn get_aurora_fill(&self, x: usize, y: usize) -> bool {
        if x < self.pixel_width && y < self.pixel_height {
            self.aurora_fill_pixels[y * self.pixel_width + x]
        } else {
            false
        }
    }

    fn set_border(&mut self, x: usize, y: usize) {
        if x < self.pixel_width && y < self.pixel_height {
            self.border_pixels[y * self.pixel_width + x] = true;
        }
    }

    fn get_coast(&self, x: usize, y: usize) -> bool {
        if x < self.pixel_width && y < self.pixel_height {
            self.coast_pixels[y * self.pixel_width + x]
        } else {
            false
        }
    }

    fn get_grid(&self, x: usize, y: usize) -> bool {
        if x < self.pixel_width && y < self.pixel_height {
            self.grid_pixels[y * self.pixel_width + x]
        } else {
            false
        }
    }

    fn get_aurora(&self, x: usize, y: usize) -> bool {
        if x < self.pixel_width && y < self.pixel_height {
            self.aurora_pixels[y * self.pixel_width + x]
        } else {
            false
        }
    }

    fn get_border(&self, x: usize, y: usize) -> bool {
        if x < self.pixel_width && y < self.pixel_height {
            self.border_pixels[y * self.pixel_width + x]
        } else {
            false
        }
    }

    fn set_terminator(&mut self, x: usize, y: usize) {
        if x < self.pixel_width && y < self.pixel_height {
            self.terminator_pixels[y * self.pixel_width + x] = true;
        }
    }

    fn get_terminator(&self, x: usize, y: usize) -> bool {
        if x < self.pixel_width && y < self.pixel_height {
            self.terminator_pixels[y * self.pixel_width + x]
        } else {
            false
        }
    }

    #[allow(dead_code)]
    fn set_land_fill(&mut self, x: usize, y: usize) {
        if x < self.pixel_width && y < self.pixel_height {
            self.land_fill_pixels[y * self.pixel_width + x] = true;
        }
    }

    fn get_land_fill(&self, x: usize, y: usize) -> bool {
        if x < self.pixel_width && y < self.pixel_height {
            self.land_fill_pixels[y * self.pixel_width + x]
        } else {
            false
        }
    }

    /// Bresenham's line algorithm
    fn draw_line_on(&mut self, layer: u8, x0: i32, y0: i32, x1: i32, y1: i32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut cx = x0;
        let mut cy = y0;

        loop {
            if cx >= 0 && cy >= 0 && (cx as usize) < self.pixel_width && (cy as usize) < self.pixel_height {
                match layer {
                    0 => self.set_coast(cx as usize, cy as usize),
                    1 => self.set_grid(cx as usize, cy as usize),
                    2 => self.set_aurora(cx as usize, cy as usize),
                    3 => self.set_border(cx as usize, cy as usize),
                    _ => {}
                }
            }
            if cx == x1 && cy == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy {
                if cx == x1 { break; }
                err += dy;
                cx += sx;
            }
            if e2 <= dx {
                if cy == y1 { break; }
                err += dx;
                cy += sy;
            }
        }
    }

    /// Project lat/lon to pixel coordinates (equirectangular)
    fn project(&self, lat: f64, lon: f64) -> (i32, i32) {
        let x = ((lon + 180.0) / 360.0 * self.pixel_width as f64) as i32;
        let y = ((90.0 - lat) / 180.0 * self.pixel_height as f64) as i32;
        (x, y)
    }

    fn pixel_to_lat_lon(&self, px: usize, py: usize) -> (f64, f64) {
        let lat = 90.0 - (py as f64 / self.pixel_height as f64) * 180.0;
        let lon = (px as f64 / self.pixel_width as f64) * 360.0 - 180.0;
        (lat, lon)
    }

    fn draw_coastlines(&mut self) {
        for segment in coastline_segments() {
            for i in 0..segment.len().saturating_sub(1) {
                // Skip wrap-around segments across date line
                if (segment[i].1 - segment[i + 1].1).abs() > 170.0 {
                    continue;
                }
                let (x0, y0) = self.project(segment[i].0, segment[i].1);
                let (x1, y1) = self.project(segment[i + 1].0, segment[i + 1].1);
                self.draw_line_on(0, x0, y0, x1, y1);
            }
        }
    }

    fn draw_borders(&mut self) {
        for segment in border_data() {
            for i in 0..segment.len().saturating_sub(1) {
                if (segment[i].1 - segment[i + 1].1).abs() > 170.0 {
                    continue;
                }
                let (x0, y0) = self.project(segment[i].0, segment[i].1);
                let (x1, y1) = self.project(segment[i + 1].0, segment[i + 1].1);
                self.draw_line_on(3, x0, y0, x1, y1);
            }
        }
    }

    fn draw_grid(&mut self) {
        // Compute alignment anchors: equator row and Greenwich column
        let (greenwich_x, equator_y) = self.project(0.0, 0.0);
        let gx = greenwich_x.max(0) as usize;
        let ey = equator_y.max(0) as usize;
        let spacing = 4usize;

        // Latitude lines every 10 degrees
        for lat_deg in (-80..=80).step_by(10) {
            let (_, y) = self.project(lat_deg as f64, 0.0);
            if y >= 0 && (y as usize) < self.pixel_height {
                // Dotted, aligned to Greenwich meridian column
                let x_offset = gx % spacing;
                for x in (x_offset..self.pixel_width).step_by(spacing) {
                    self.set_grid(x, y as usize);
                }
            }
        }
        // Longitude lines every 15 degrees (1 hour), aligned to equator row
        let y_offset = ey % spacing;
        for lon_deg in (-180..=165).step_by(15) {
            let (x, _) = self.project(0.0, lon_deg as f64);
            if x >= 0 && (x as usize) < self.pixel_width {
                for y in (y_offset..self.pixel_height).step_by(spacing) {
                    self.set_grid(x as usize, y);
                }
            }
        }
    }

    fn fill_landmasses(&mut self) {
        // Flood fill from edges to find ocean, then mark everything else as land
        let w = self.pixel_width;
        let h = self.pixel_height;
        let mut ocean = vec![false; w * h];
        let mut queue = std::collections::VecDeque::new();

        // Seed from all edge pixels that aren't coastline
        for x in 0..w {
            if !self.coast_pixels[x] {
                ocean[x] = true;
                queue.push_back((x, 0usize));
            }
            let bot = (h - 1) * w + x;
            if !self.coast_pixels[bot] {
                ocean[bot] = true;
                queue.push_back((x, h - 1));
            }
        }
        for y in 0..h {
            let left = y * w;
            if !self.coast_pixels[left] {
                ocean[left] = true;
                queue.push_back((0, y));
            }
            let right = y * w + w - 1;
            if !self.coast_pixels[right] {
                ocean[right] = true;
                queue.push_back((w - 1, y));
            }
        }

        // Seed inland seas/water bodies that edge flood can't reach
        let inland_seas: &[(f64, f64)] = &[
            (35.0, 18.0),    // Mediterranean (central)
            (38.0, 5.0),     // Mediterranean (west)
            (34.0, 25.0),    // Mediterranean (east)
            (33.0, 30.0),    // Eastern Mediterranean
            (43.0, 16.0),    // Adriatic Sea
            (43.0, 34.0),    // Black Sea
            (42.0, 50.0),    // Caspian Sea
            (22.0, 38.0),    // Red Sea
            (27.0, 51.0),    // Persian Gulf
            (60.0, -85.0),   // Hudson Bay
            (40.0, 135.0),   // Sea of Japan
            (12.0, 44.0),    // Gulf of Aden
            (0.0, 108.0),    // South China Sea
            (58.0, 10.0),    // North Sea
            (58.0, 20.0),    // Baltic Sea
        ];
        for &(lat, lon) in inland_seas {
            let (px, py) = self.project(lat, lon);
            if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                let idx = py as usize * w + px as usize;
                if !self.coast_pixels[idx] && !ocean[idx] {
                    ocean[idx] = true;
                    queue.push_back((px as usize, py as usize));
                }
            }
        }

        // BFS flood fill
        while let Some((x, y)) = queue.pop_front() {
            let neighbors: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
            for (dx, dy) in neighbors {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                    let nx = nx as usize;
                    let ny = ny as usize;
                    let idx = ny * w + nx;
                    if !ocean[idx] && !self.coast_pixels[idx] {
                        ocean[idx] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }

        // Mark land fill pixels with staggered pattern (every 3 pixels)
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if !ocean[idx] && !self.coast_pixels[idx] {
                    let row_offset = if (y / 3) % 2 == 0 { 0 } else { 1 };
                    if x % 3 == row_offset && y % 3 == 0 {
                        self.land_fill_pixels[idx] = true;
                    }
                }
            }
        }
    }

    fn draw_aurora_boundary(&mut self, aurora: &AuroraBoundary) {
        // Dashed equatorward boundary, one sample per pixel column so the
        // OVATION oval renders as a smooth curve (Kp fallback stays a line)
        for x in (0..self.pixel_width).step_by(2) {
            let lon = (x as f64 / self.pixel_width as f64) * 360.0 - 180.0;
            for lat in [aurora.north_lat_at(lon), aurora.south_lat_at(lon)]
                .into_iter()
                .flatten()
            {
                let (_, y) = self.project(lat, lon);
                if y >= 0 && (y as usize) < self.pixel_height {
                    self.set_aurora(x, y as usize);
                }
            }
        }
    }

    /// Sparse dot fill inside the auroral oval (OVATION grid only), so the
    /// activity band is visible over oceans and colored by intensity
    fn draw_aurora_fill(&mut self, aurora: &AuroraBoundary) {
        if aurora.power_grid.is_none() {
            return;
        }
        for y in (0..self.pixel_height).step_by(2) {
            let stagger = if (y / 2) % 2 == 0 { 0 } else { 1 };
            for x in 0..self.pixel_width {
                if x % 3 != stagger {
                    continue;
                }
                let (lat, lon) = self.pixel_to_lat_lon(x, y);
                if aurora.power_at(lat, lon) >= OVATION_THRESHOLD {
                    self.set_aurora_fill(x, y);
                }
            }
        }
    }

    fn draw_terminator_line(&mut self, solar_lat: f64, solar_lon: f64) {
        // Draw the day/night terminator on its own layer
        // The terminator is the set of points where angular distance from subsolar = 90°
        for px in 0..self.pixel_width {
            let lon = (px as f64 / self.pixel_width as f64) * 360.0 - 180.0;
            let sl = solar_lat.to_radians();
            let dl = (lon - solar_lon).to_radians();
            let lat = if sl.sin().abs() > 0.001 {
                (-(dl.cos() * sl.cos()) / sl.sin()).atan().to_degrees()
            } else {
                0.0
            };
            let (_, py) = self.project(lat, lon);
            if py >= 0 && (py as usize) < self.pixel_height {
                // Draw every pixel for a solid, visible terminator line
                self.set_terminator(px, py as usize);
            }
        }
    }

    fn render(
        &self,
        aurora: Option<&AuroraBoundary>,
        solar_lat: f64,
        solar_lon: f64,
    ) -> Vec<Line<'static>> {
        let char_width = self.pixel_width / 2;
        let char_height = self.pixel_height / 4;
        let mut lines = Vec::with_capacity(char_height);

        for cy in 0..char_height {
            let mut spans = Vec::with_capacity(char_width);
            for cx in 0..char_width {
                let px_base = cx * 2;
                let py_base = cy * 4;

                let mut has_coast = false;
                let mut has_grid = false;
                let mut has_aurora = false;
                let mut has_aurora_fill = false;
                let mut has_border = false;
                let mut has_terminator = false;
                let mut has_land_fill = false;
                let mut dots = [[false; 4]; 2];

                for dx in 0..2usize {
                    for dy in 0..4usize {
                        let px = px_base + dx;
                        let py = py_base + dy;
                        if self.get_coast(px, py) {
                            has_coast = true;
                            dots[dx][dy] = true;
                        }
                        if self.get_aurora(px, py) {
                            has_aurora = true;
                            dots[dx][dy] = true;
                        }
                        if self.get_terminator(px, py) && !self.get_coast(px, py) {
                            has_terminator = true;
                            dots[dx][dy] = true;
                        }
                        if self.get_border(px, py) && !self.get_coast(px, py) {
                            has_border = true;
                            dots[dx][dy] = true;
                        }
                        if self.get_land_fill(px, py) && !self.get_coast(px, py) && !self.get_border(px, py) && !self.get_terminator(px, py) {
                            has_land_fill = true;
                            dots[dx][dy] = true;
                        }
                        if self.get_aurora_fill(px, py) && !self.get_coast(px, py) && !self.get_border(px, py) && !self.get_terminator(px, py) && !self.get_land_fill(px, py) {
                            has_aurora_fill = true;
                            dots[dx][dy] = true;
                        }
                        if self.get_grid(px, py) && !self.get_coast(px, py) && !self.get_border(px, py) && !self.get_terminator(px, py) && !self.get_land_fill(px, py) {
                            has_grid = true;
                            dots[dx][dy] = true;
                        }
                    }
                }

                let ch = dots_to_braille(dots);

                // Empty cell = space
                if ch == '\u{2800}' {
                    spans.push(Span::raw(" "));
                    continue;
                }

                // Determine geographic center of this cell
                let center_px = px_base + 1;
                let center_py = py_base + 2;
                let (lat, lon) = self.pixel_to_lat_lon(center_px, center_py);

                let daylight = is_in_daylight(lat, lon, solar_lat, solar_lon);
                let terminator = is_near_terminator(lat, lon, solar_lat, solar_lon);

                // Inside the oval? OVATION: cell probability >= threshold,
                // colored green→yellow→red by intensity. Kp fallback: flat
                // poleward-of-boundary zone in the classic green.
                let (in_aurora_zone, aurora_color) = if let Some(boundary) = aurora {
                    match boundary.source {
                        AuroraSource::Ovation => {
                            let power = boundary.power_at(lat, lon);
                            (power >= OVATION_THRESHOLD, aurora_gradient(power))
                        }
                        AuroraSource::KpModel => {
                            let nlat = boundary.north_lat_at(lon).unwrap_or(67.0);
                            let slat = boundary.south_lat_at(lon).unwrap_or(-67.0);
                            let in_zone =
                                (lat >= nlat && lat > 0.0) || (lat <= slat && lat < 0.0);
                            (in_zone, Color::Rgb(0, 220, 90))
                        }
                    }
                } else {
                    (false, Color::Rgb(0, 220, 90))
                };

                let style = if has_aurora {
                    // Aurora boundary line - always bright
                    Style::default()
                        .fg(colors::AURORA_BRIGHT)
                        .add_modifier(Modifier::BOLD)
                } else if has_coast {
                    if in_aurora_zone && !daylight {
                        Style::default()
                            .fg(aurora_color)
                            .add_modifier(Modifier::BOLD)
                    } else if in_aurora_zone {
                        Style::default().fg(scale_color(aurora_color, 0.7))
                    } else if terminator {
                        Style::default().fg(Color::Rgb(180, 160, 60))
                    } else if daylight {
                        Style::default().fg(colors::MAP_LAND)
                    } else {
                        Style::default().fg(colors::MAP_LAND_NIGHT)
                    }
                } else if has_terminator {
                    // Terminator line - warm amber/orange tone
                    if in_aurora_zone {
                        Style::default().fg(scale_color(aurora_color, 0.8))
                    } else {
                        Style::default().fg(colors::MAP_TERMINATOR_LINE)
                    }
                } else if has_border {
                    if in_aurora_zone && !daylight {
                        Style::default().fg(scale_color(aurora_color, 0.75))
                    } else if in_aurora_zone {
                        Style::default().fg(scale_color(aurora_color, 0.55))
                    } else if terminator {
                        Style::default().fg(colors::MAP_BORDER_TERMINATOR)
                    } else if daylight {
                        Style::default().fg(colors::MAP_BORDER_DAY)
                    } else {
                        Style::default().fg(colors::MAP_BORDER_NIGHT)
                    }
                } else if has_land_fill {
                    // Land fill: dimmer than coastlines, day/night aware
                    if in_aurora_zone && !daylight {
                        Style::default().fg(scale_color(aurora_color, 0.30))
                    } else if in_aurora_zone {
                        Style::default().fg(scale_color(aurora_color, 0.22))
                    } else if daylight {
                        Style::default().fg(colors::MAP_LAND_FILL)
                    } else {
                        Style::default().fg(colors::MAP_LAND_FILL_NIGHT)
                    }
                } else if has_aurora_fill {
                    // Oval interior over ocean: intensity-colored glow
                    if daylight {
                        Style::default().fg(scale_color(aurora_color, 0.5))
                    } else {
                        Style::default().fg(scale_color(aurora_color, 0.85))
                    }
                } else if has_grid {
                    if in_aurora_zone && !daylight {
                        Style::default().fg(scale_color(aurora_color, 0.3))
                    } else if daylight {
                        Style::default().fg(colors::MAP_GRID)
                    } else {
                        Style::default().fg(colors::MAP_GRID_NIGHT)
                    }
                } else {
                    Style::default().fg(colors::TEXT_DIM)
                };

                spans.push(Span::styled(ch.to_string(), style));
            }
            lines.push(Line::from(spans));
        }
        lines
    }
}

/// Map an OVATION probability (percent) to a green→yellow→red gradient,
/// matching the NOAA aurora forecast product's color ramp
fn aurora_gradient(power: f64) -> Color {
    let t = ((power - OVATION_THRESHOLD) / (90.0 - OVATION_THRESHOLD)).clamp(0.0, 1.0);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let (r, g, b) = if t < 0.5 {
        // green → yellow
        let u = t * 2.0;
        (lerp(0.0, 240.0, u), lerp(220.0, 210.0, u), lerp(90.0, 0.0, u))
    } else {
        // yellow → red
        let u = (t - 0.5) * 2.0;
        (lerp(240.0, 255.0, u), lerp(210.0, 50.0, u), lerp(0.0, 40.0, u))
    };
    Color::Rgb(r as u8, g as u8, b as u8)
}

/// Dim an RGB color by a brightness factor (non-RGB colors pass through)
fn scale_color(c: Color, factor: f64) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f64 * factor) as u8,
            (g as f64 * factor) as u8,
            (b as f64 * factor) as u8,
        ),
        other => other,
    }
}

// === Public Interface ===

pub struct WorldMap;

impl WorldMap {
    pub fn new() -> Self {
        Self
    }

    /// Render the world map to fit the given character dimensions
    pub fn render_to_size(
        &self,
        width: u16,
        height: u16,
        aurora: Option<&AuroraBoundary>,
    ) -> Vec<Line<'static>> {
        let now = Utc::now();
        let solar_lat = solar_declination(now.ordinal());
        let solar_lon = solar_subsolar_longitude(now.hour(), now.minute());

        let w = width as usize;
        let h = height as usize;
        if w < 4 || h < 2 {
            return vec![];
        }

        let mut canvas = MapCanvas::new(w, h);
        canvas.draw_coastlines();
        canvas.fill_landmasses();
        canvas.draw_borders();
        canvas.draw_grid();
        canvas.draw_terminator_line(solar_lat, solar_lon);
        if let Some(boundary) = aurora {
            canvas.draw_aurora_boundary(boundary);
            canvas.draw_aurora_fill(boundary);
        }
        canvas.render(aurora, solar_lat, solar_lon)
    }

    /// Backward-compatible render (fixed size)
    pub fn render_with_aurora(&self, aurora: Option<&AuroraBoundary>) -> Vec<Line<'static>> {
        self.render_to_size(100, 25, aurora)
    }

    pub fn render(&self) -> Vec<Line<'static>> {
        self.render_with_aurora(None)
    }

    pub fn width(&self) -> usize {
        100
    }

    pub fn height(&self) -> usize {
        25
    }
}

impl Default for WorldMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_map_creation() {
        let map = WorldMap::new();
        assert!(map.height() > 0);
        assert!(map.width() > 0);
    }

    #[test]
    fn test_world_map_render() {
        let map = WorldMap::new();
        let lines = map.render();
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_world_map_render_with_aurora() {
        let map = WorldMap::new();
        let aurora = crate::data::AuroraBoundary::from_kp_index(5.0);
        let lines = map.render_with_aurora(Some(&aurora));
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_to_size() {
        let map = WorldMap::new();
        let lines = map.render_to_size(80, 20, None);
        assert_eq!(lines.len(), 20);
    }

    #[test]
    fn test_braille_encoding() {
        // Empty pattern
        let empty = dots_to_braille([[false; 4]; 2]);
        assert_eq!(empty, '\u{2800}');

        // Top-left dot only
        let mut dots = [[false; 4]; 2];
        dots[0][0] = true;
        let ch = dots_to_braille(dots);
        assert_eq!(ch, '\u{2801}');

        // All dots
        let all = dots_to_braille([[true; 4]; 2]);
        assert_eq!(all, '\u{28FF}');
    }

    #[test]
    fn test_solar_declination() {
        let dec_equinox = solar_declination(80);
        assert!(dec_equinox.abs() < 1.0);
        let dec_summer = solar_declination(172);
        assert!((dec_summer - 23.44).abs() < 1.0);
    }

    #[test]
    fn test_solar_subsolar_longitude() {
        let lon_noon = solar_subsolar_longitude(12, 0);
        assert_eq!(lon_noon, 0.0);
        let lon_midnight = solar_subsolar_longitude(0, 0);
        assert_eq!(lon_midnight, 180.0);
    }
}

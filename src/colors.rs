/// Color scheme: futuristic, scientific, cyber-teal theme
use ratatui::style::Color;

// Primary theme colors
pub const BACKGROUND: Color = Color::Black;
pub const BORDER: Color = Color::Rgb(0, 140, 180);
pub const BORDER_HIGHLIGHT: Color = Color::Rgb(0, 200, 240);
pub const BORDER_DIM: Color = Color::Rgb(0, 60, 80);
pub const TEXT_PRIMARY: Color = Color::Rgb(210, 225, 240);
pub const TEXT_SECONDARY: Color = Color::Rgb(120, 145, 170);
pub const TEXT_DIM: Color = Color::Rgb(50, 65, 80);

// Header colors
pub const TITLE: Color = Color::Rgb(0, 210, 255);
pub const UTC_TIME: Color = Color::Rgb(0, 240, 200);
pub const LOCAL_TIME: Color = Color::Rgb(255, 180, 0);
pub const LAST_UPDATE: Color = Color::Rgb(120, 145, 170);

// Scale severity colors (NOAA-inspired, enhanced contrast)
pub const SEVERITY_NONE: Color = Color::Rgb(0, 200, 80);
pub const SEVERITY_MINOR: Color = Color::Rgb(240, 220, 0);
pub const SEVERITY_MODERATE: Color = Color::Rgb(255, 150, 0);
pub const SEVERITY_STRONG: Color = Color::Rgb(255, 60, 30);
pub const SEVERITY_SEVERE: Color = Color::Rgb(255, 0, 0);

// Solar wind parameter colors
pub const SOLAR_WIND_BT: Color = Color::Rgb(0, 190, 255);
pub const SOLAR_WIND_BZ_POSITIVE: Color = Color::Rgb(0, 220, 100);
pub const SOLAR_WIND_BZ_NEGATIVE: Color = Color::Rgb(255, 50, 50);
pub const SOLAR_WIND_SPEED: Color = Color::Rgb(255, 200, 0);
pub const SOLAR_WIND_DENSITY: Color = Color::Rgb(190, 90, 255);

// Aurora and map colors
pub const AURORA: Color = Color::Rgb(0, 170, 70);
pub const AURORA_BRIGHT: Color = Color::Rgb(0, 255, 100);
pub const MAP_LAND: Color = Color::Rgb(90, 150, 200);
pub const MAP_LAND_NIGHT: Color = Color::Rgb(35, 55, 80);
pub const MAP_WATER: Color = Color::Rgb(15, 30, 50);
pub const MAP_GRID: Color = Color::Rgb(18, 33, 48);
pub const MAP_GRID_NIGHT: Color = Color::Rgb(10, 20, 30);

// Country border colors (dimmer than coastlines for visual separation)
pub const MAP_BORDER_DAY: Color = Color::Rgb(55, 100, 150);
pub const MAP_BORDER_NIGHT: Color = Color::Rgb(18, 32, 50);
pub const MAP_BORDER_TERMINATOR: Color = Color::Rgb(35, 65, 100);
pub const MAP_BORDER_AURORA_NIGHT: Color = Color::Rgb(0, 80, 45);
pub const MAP_BORDER_AURORA_DAY: Color = Color::Rgb(0, 120, 65);

// X-ray flare class colors
pub const FLARE_X: Color = Color::Rgb(255, 0, 0);
pub const FLARE_M: Color = Color::Rgb(255, 90, 40);
pub const FLARE_C: Color = Color::Rgb(255, 200, 0);
pub const FLARE_B: Color = Color::Rgb(0, 200, 80);
pub const FLARE_A: Color = Color::Rgb(70, 90, 110);

// Kp index level colors
pub const KP_LOW: Color = Color::Rgb(0, 200, 80);
pub const KP_MEDIUM: Color = Color::Rgb(255, 200, 0);
pub const KP_HIGH: Color = Color::Rgb(255, 60, 30);
pub const KP_EXTREME: Color = Color::Rgb(255, 0, 0);

// Forecast colors
pub const FORECAST: Color = Color::Rgb(0, 190, 240);
pub const FORECAST_SECONDARY: Color = Color::Rgb(180, 80, 255);

// Graph/chart elements
pub const GRAPH_AXIS: Color = Color::Rgb(50, 70, 90);
pub const GRAPH_LABEL: Color = Color::Rgb(90, 115, 140);

// Current value highlight
pub const VALUE_HIGHLIGHT: Color = Color::Rgb(255, 255, 255);

// RSG scale block colors
pub const RSG_BLOCK_ACTIVE: Color = Color::Rgb(0, 255, 180);
pub const RSG_BORDER: Color = Color::Rgb(0, 100, 130);

// Band conditions panel
pub const BAND_GOOD: Color = Color::Rgb(0, 200, 80);
pub const BAND_FAIR: Color = Color::Rgb(255, 200, 0);
pub const BAND_POOR: Color = Color::Rgb(255, 60, 30);
pub const BAND_LABEL: Color = Color::Rgb(180, 195, 210);

// Terminator line
pub const MAP_TERMINATOR_LINE: Color = Color::Rgb(140, 120, 50);

// Land fill (interior dots, dimmer than coastlines but visible enough to distinguish from ocean)
pub const MAP_LAND_FILL: Color = Color::Rgb(50, 85, 120);
pub const MAP_LAND_FILL_NIGHT: Color = Color::Rgb(15, 25, 40);

// Map axis/scale labels
pub const MAP_AXIS_LABEL: Color = Color::Rgb(60, 85, 110);

// Status indicators
pub const STATUS_ONLINE: Color = Color::Rgb(0, 255, 100);
pub const STATUS_STALE: Color = Color::Rgb(255, 180, 0);

// Launch site markers
pub const LAUNCH_SITE: Color = Color::Rgb(255, 40, 40);

// Dst index
pub const DST_INDEX: Color = Color::Rgb(255, 140, 80);

// Launch site clock panel
pub const LAUNCH_NAME: Color = Color::Rgb(0, 165, 210);      // Group A: prominent cyan, below TITLE
pub const LAUNCH_TIME: Color = Color::Rgb(0, 240, 200);      // Group A: brightest teal (= UTC_TIME)
pub const LAUNCH_LOCATION: Color = Color::Rgb(55, 78, 100);  // Group B: muted blue-gray
pub const LAUNCH_TIMEZONE: Color = Color::Rgb(38, 52, 68);   // Group B: subtle, near-background

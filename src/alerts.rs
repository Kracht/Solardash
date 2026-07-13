use crate::data::{parse_scale_level, DashboardData};
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::path::PathBuf;

pub struct AlertState {
    prev_g: i32,
    prev_s: i32,
    prev_r: i32,
    prev_x_flare: bool,
    first_fetch: bool,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    audio_dir: PathBuf,
}

impl AlertState {
    pub fn new() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let audio_dir = std::env::current_dir().ok()?.join("audio");
        Some(Self {
            prev_g: 0,
            prev_s: 0,
            prev_r: 0,
            prev_x_flare: false,
            first_fetch: true,
            _stream: stream,
            stream_handle: handle,
            audio_dir,
        })
    }

    /// Track scale transitions on every refresh; only audible when `enabled`.
    /// Tracking while muted avoids a burst of stale alerts when the user
    /// toggles audio on later.
    pub fn check_and_play(&mut self, data: &DashboardData, enabled: bool) {
        let cur_g = parse_scale_level(&data.noaa_scales.geomagnetic_storm.scale);
        let cur_s = parse_scale_level(&data.noaa_scales.solar_radiation.scale);
        let cur_r = parse_scale_level(&data.noaa_scales.radio_blackout.scale);
        let cur_x = data
            .flares
            .get_latest()
            .map(|f| f.class_letter() == 'X')
            .unwrap_or(false);

        if self.first_fetch {
            self.prev_g = cur_g;
            self.prev_s = cur_s;
            self.prev_r = cur_r;
            self.prev_x_flare = cur_x;
            self.first_fetch = false;
            return;
        }

        if enabled {
            if cur_g > self.prev_g {
                self.play_wav(&format!("G{}.wav", cur_g.min(3)));
            }
            if cur_s > self.prev_s {
                self.play_wav(&format!("S{}.wav", cur_s.min(3)));
            }
            if cur_r > self.prev_r {
                self.play_wav(&format!("R{}.wav", cur_r.min(3)));
            }
            if cur_x && !self.prev_x_flare {
                self.play_wav("XClass.wav");
            }
        }

        self.prev_g = cur_g;
        self.prev_s = cur_s;
        self.prev_r = cur_r;
        self.prev_x_flare = cur_x;
    }

    fn play_wav(&self, filename: &str) {
        let path = self.audio_dir.join(filename);
        let handle = self.stream_handle.clone();
        std::thread::spawn(move || {
            let file = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Warning: could not open {:?}: {}", path, e);
                    return;
                }
            };
            let reader = std::io::BufReader::new(file);
            let source = match rodio::Decoder::new(reader) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Warning: could not decode {:?}: {}", path, e);
                    return;
                }
            };
            let sink = match Sink::try_new(&handle) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Warning: could not create sink: {}", e);
                    return;
                }
            };
            sink.append(source);
            sink.sleep_until_end();
        });
    }
}

//! COMTRADE (IEEE C37.111) oscillogram reader — the open interchange format the
//! original's `OscillHelper::ComtradeConverter` produces from the native `.sto`.
//!
//! Supports the 1991 and 1999 revisions (and the common 2013 layout) with ASCII
//! and BINARY (int16) / BINARY32 / FLOAT32 data files. A single-file `.cff`
//! container is not parsed; pass the `.cfg`/`.dat` pair.

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ComtradeError {
    #[error("io: {0}")]
    Io(String),
    #[error("malformed .cfg at line {0}: {1}")]
    Cfg(usize, String),
    #[error("malformed .dat: {0}")]
    Dat(String),
}

/// One analog channel definition and its samples (already scaled to primary
/// engineering units via `a*raw + b`).
#[derive(Debug, Clone)]
pub struct AnalogChannel {
    pub index: u32,
    pub id: String,
    pub phase: String,
    pub unit: String,
    pub a: f64,
    pub b: f64,
    pub primary: f64,
    pub secondary: f64,
    /// `true` if the stored channel is already a primary value (`PS` = 'P').
    pub is_primary: bool,
    pub samples: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct DigitalChannel {
    pub index: u32,
    pub id: String,
    pub phase: String,
    pub samples: Vec<bool>,
}

#[derive(Debug, Clone)]
pub struct Oscillogram {
    pub station: String,
    pub device: String,
    pub rev_year: i32,
    pub line_freq: f64,
    /// Per-sample time in seconds (from the sample-rate sections, scaled by the
    /// configured time multiplier).
    pub time: Vec<f64>,
    pub analog: Vec<AnalogChannel>,
    pub digital: Vec<DigitalChannel>,
    /// Nominal samples-per-second of the first rate section (for DFT windowing).
    pub sample_rate: f64,
}

impl Oscillogram {
    pub fn sample_count(&self) -> usize {
        self.time.len()
    }

    /// Estimate samples-per-fundamental-period for DFT windows.
    pub fn samples_per_cycle(&self) -> usize {
        if self.sample_rate > 0.0 && self.line_freq > 0.0 {
            (self.sample_rate / self.line_freq).round() as usize
        } else if self.time.len() > 1 {
            let dt = self.time[1] - self.time[0];
            if dt > 0.0 {
                (1.0 / self.line_freq / dt).round() as usize
            } else {
                20
            }
        } else {
            20
        }
    }

    /// Find an analog channel by (case-insensitive) id or phase label.
    pub fn analog_by_id(&self, needle: &str) -> Option<&AnalogChannel> {
        let n = needle.to_lowercase();
        self.analog
            .iter()
            .find(|c| c.id.to_lowercase() == n || c.phase.to_lowercase() == n)
    }
}

/// Load a COMTRADE record given the path to either the `.cfg` or `.dat` file
/// (the sibling is located by extension, case-insensitively).
pub fn load(path: impl AsRef<Path>) -> Result<Oscillogram, ComtradeError> {
    let path = path.as_ref();
    let (cfg_path, dat_path) = sibling_paths(path)?;
    let cfg_bytes = std::fs::read(&cfg_path).map_err(|e| ComtradeError::Io(e.to_string()))?;
    let cfg_text = decode_text(&cfg_bytes);
    let (mut rec, fmt) = parse_cfg(&cfg_text)?;
    let dat = std::fs::read(&dat_path).map_err(|e| ComtradeError::Io(e.to_string()))?;
    parse_dat(&mut rec, fmt, &dat)?;
    Ok(rec)
}

/// Parse from in-memory cfg text + dat bytes (used by tests and the `.cff` path).
pub fn parse(cfg_text: &str, dat: &[u8]) -> Result<Oscillogram, ComtradeError> {
    let (mut rec, fmt) = parse_cfg(cfg_text)?;
    parse_dat(&mut rec, fmt, dat)?;
    Ok(rec)
}

fn sibling_paths(path: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf), ComtradeError> {
    let stem = path
        .file_stem()
        .ok_or_else(|| ComtradeError::Io("no file stem".into()))?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    // Try common extension cases.
    let find = |exts: &[&str]| -> Option<std::path::PathBuf> {
        for e in exts {
            let p = dir.join(format!("{}.{e}", stem.to_string_lossy()));
            if p.exists() {
                return Some(p);
            }
        }
        None
    };
    let cfg = find(&["cfg", "CFG", "Cfg"])
        .ok_or_else(|| ComtradeError::Io("no .cfg sibling found".into()))?;
    let dat = find(&["dat", "DAT", "Dat"])
        .ok_or_else(|| ComtradeError::Io("no .dat sibling found".into()))?;
    Ok((cfg, dat))
}

/// Decode cfg bytes: UTF-8/ASCII, or CP1251 fall-back for Cyrillic channel ids.
fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| cp1251_to_char(b)).collect(),
    }
}

fn cp1251_to_char(b: u8) -> char {
    if b < 0x80 {
        b as char
    } else {
        // CP1251 upper half maps onto Cyrillic U+0410.. with a well-known table;
        // the regular А-я run is contiguous from 0xC0.
        match b {
            0xC0..=0xFF => char::from_u32(0x0410 + (b as u32 - 0xC0)).unwrap_or('?'),
            0xA8 => 'Ё',
            0xB8 => 'ё',
            _ => '?',
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DataFormat {
    Ascii,
    Int16,
    Int32,
    Float32,
}

struct RateSection {
    rate: f64,
    end_sample: usize,
}

fn parse_cfg(text: &str) -> Result<(Oscillogram, DataFormat), ComtradeError> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches(['\r'])).collect();
    let mut li = 0;
    let next = |li: &mut usize| -> Result<&str, ComtradeError> {
        while *li < lines.len() && lines[*li].trim().is_empty() {
            *li += 1;
        }
        if *li >= lines.len() {
            return Err(ComtradeError::Cfg(*li, "unexpected end of file".into()));
        }
        let l = lines[*li];
        *li += 1;
        Ok(l)
    };

    // Line 1: station, device, [rev_year]
    let l1 = next(&mut li)?;
    let f1: Vec<&str> = l1.split(',').collect();
    let station = f1.first().unwrap_or(&"").trim().to_string();
    let device = f1.get(1).unwrap_or(&"").trim().to_string();
    let rev_year = f1
        .get(2)
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(1991);

    // Line 2: TT, ##A, ##D
    let l2 = next(&mut li)?;
    let f2: Vec<&str> = l2.split(',').collect();
    let n_analog = f2
        .get(1)
        .and_then(|s| s.trim().trim_end_matches(['A', 'a']).parse::<usize>().ok())
        .ok_or_else(|| ComtradeError::Cfg(li, "analog count".into()))?;
    let n_digital = f2
        .get(2)
        .and_then(|s| s.trim().trim_end_matches(['D', 'd']).parse::<usize>().ok())
        .unwrap_or(0);

    let mut analog = Vec::with_capacity(n_analog);
    for _ in 0..n_analog {
        let l = next(&mut li)?;
        let f: Vec<&str> = l.split(',').collect();
        let g = |i: usize| f.get(i).map(|s| s.trim()).unwrap_or("");
        analog.push(AnalogChannel {
            index: g(0).parse().unwrap_or(0),
            id: g(1).to_string(),
            phase: g(2).to_string(),
            unit: g(4).to_string(),
            a: g(5).parse().unwrap_or(1.0),
            b: g(6).parse().unwrap_or(0.0),
            primary: g(9).parse().unwrap_or(1.0),
            secondary: g(10).parse().unwrap_or(1.0),
            is_primary: g(11).eq_ignore_ascii_case("p"),
            samples: Vec::new(),
        });
    }

    let mut digital = Vec::with_capacity(n_digital);
    for _ in 0..n_digital {
        let l = next(&mut li)?;
        let f: Vec<&str> = l.split(',').collect();
        let g = |i: usize| f.get(i).map(|s| s.trim()).unwrap_or("");
        digital.push(DigitalChannel {
            index: g(0).parse().unwrap_or(0),
            id: g(1).to_string(),
            phase: g(2).to_string(),
            samples: Vec::new(),
        });
    }

    // line frequency
    let lf = next(&mut li)?;
    let line_freq = lf.trim().parse::<f64>().unwrap_or(50.0);

    // nrates then that many "samp,endsamp"
    let nrates = next(&mut li)?.trim().parse::<usize>().unwrap_or(1).max(1);
    let mut rates = Vec::new();
    for _ in 0..nrates {
        let l = next(&mut li)?;
        let f: Vec<&str> = l.split(',').collect();
        let rate = f.first().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
        let end = f.get(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        rates.push(RateSection { rate, end_sample: end });
    }

    // two timestamp lines (first sample, trigger) — kept only loosely
    let _first_ts = next(&mut li).ok();
    let _trigger_ts = next(&mut li).ok();

    // file type
    let ft = next(&mut li).map(|s| s.trim().to_lowercase()).unwrap_or_default();

    // 1999+: timemult (optional)
    let timemult = next(&mut li)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(1.0);

    // Build the per-sample time base from the rate sections.
    let total = rates.last().map(|r| r.end_sample).unwrap_or(0);
    let mut time = Vec::with_capacity(total);
    let mut t = 0.0;
    let mut prev_end = 0usize;
    for r in &rates {
        let dt = if r.rate > 0.0 { 1.0 / r.rate } else { 0.0 } * timemult;
        for _ in prev_end..r.end_sample {
            time.push(t);
            t += dt;
        }
        prev_end = r.end_sample;
    }
    let sample_rate = rates.first().map(|r| r.rate).unwrap_or(0.0);

    let fmt = match ft.as_str() {
        "ascii" => DataFormat::Ascii,
        "binary" => DataFormat::Int16,
        "binary32" => DataFormat::Int32,
        "float32" => DataFormat::Float32,
        _ => DataFormat::Ascii,
    };
    let _ = timemult; // already folded into `time`
    let rec = Oscillogram {
        station,
        device,
        rev_year,
        line_freq,
        time,
        analog,
        digital,
        sample_rate,
    };
    Ok((rec, fmt))
}

fn parse_dat(rec: &mut Oscillogram, fmt: DataFormat, dat: &[u8]) -> Result<(), ComtradeError> {
    let n = rec.time.len();
    let na = rec.analog.len();
    let nd = rec.digital.len();
    for ch in rec.analog.iter_mut() {
        ch.samples = Vec::with_capacity(n);
    }
    for ch in rec.digital.iter_mut() {
        ch.samples = Vec::with_capacity(n);
    }

    match fmt {
        DataFormat::Ascii => {
            let text = decode_text(dat);
            let mut rows = 0usize;
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let f: Vec<&str> = line.split(',').collect();
                // columns: sample#, timestamp, A1..Ana, D1..Dnd
                for (ai, ch) in rec.analog.iter_mut().enumerate() {
                    let raw: f64 = f.get(2 + ai).and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
                    ch.samples.push(ch.a * raw + ch.b);
                }
                for (di, ch) in rec.digital.iter_mut().enumerate() {
                    let v: i64 = f.get(2 + na + di).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                    ch.samples.push(v != 0);
                }
                rows += 1;
            }
            // If the cfg under-counted samples, extend the time base.
            if rows > rec.time.len() {
                let dt = rec.time.get(1).copied().unwrap_or(0.0) - rec.time.first().copied().unwrap_or(0.0);
                let mut t = rec.time.last().copied().unwrap_or(0.0);
                while rec.time.len() < rows {
                    t += dt;
                    rec.time.push(t);
                }
            }
        }
        DataFormat::Int16 | DataFormat::Int32 | DataFormat::Float32 => {
            let (abytes, dwords) = match fmt {
                DataFormat::Int16 => (2usize, (nd + 15) / 16),
                DataFormat::Int32 | DataFormat::Float32 => (4usize, (nd + 15) / 16),
                _ => unreachable!(),
            };
            // record = 4 (sample#) + 4 (timestamp) + na*abytes + dwords*2
            let rec_len = 8 + na * abytes + dwords * 2;
            if rec_len == 0 {
                return Err(ComtradeError::Dat("zero record length".into()));
            }
            let count = dat.len() / rec_len;
            for r in 0..count {
                let base = r * rec_len;
                let mut off = base + 8; // skip sample# + timestamp
                for ch in rec.analog.iter_mut() {
                    let raw = match fmt {
                        DataFormat::Int16 => {
                            let v = i16::from_le_bytes([dat[off], dat[off + 1]]);
                            v as f64
                        }
                        DataFormat::Int32 => {
                            let v = i32::from_le_bytes([dat[off], dat[off + 1], dat[off + 2], dat[off + 3]]);
                            v as f64
                        }
                        DataFormat::Float32 => {
                            let v = f32::from_le_bytes([dat[off], dat[off + 1], dat[off + 2], dat[off + 3]]);
                            v as f64
                        }
                        _ => 0.0,
                    };
                    ch.samples.push(ch.a * raw + ch.b);
                    off += abytes;
                }
                for w in 0..dwords {
                    let word = u16::from_le_bytes([dat[off], dat[off + 1]]);
                    off += 2;
                    for bit in 0..16 {
                        let di = w * 16 + bit;
                        if di < nd {
                            rec.digital[di].samples.push((word >> bit) & 1 == 1);
                        }
                    }
                }
            }
            if count > rec.time.len() {
                let dt = rec.time.get(1).copied().unwrap_or(0.0) - rec.time.first().copied().unwrap_or(0.0);
                let mut t = rec.time.last().copied().unwrap_or(0.0);
                while rec.time.len() < count {
                    t += dt;
                    rec.time.push(t);
                }
            }
        }
    }
    // Truncate the time base to the number of rows actually read.
    let rows = rec.analog.first().map(|c| c.samples.len()).unwrap_or(rec.time.len());
    if rec.time.len() > rows {
        rec.time.truncate(rows);
    }
    let _ = na;
    Ok(())
}

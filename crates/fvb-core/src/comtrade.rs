//! COMTRADE (IEEE C37.111) oscillogram reader — the open interchange format the
//! original's `OscillHelper::ComtradeConverter` produces from the native `.sto`.
//!
//! Supports the 1991 and 1999 revisions (and the common 2013 layout) with the
//! data-file types the original accepts — exactly `ASCII`, `BINARY` (int16) and
//! `binary32` — plus the single-file `.cff` container (ledger L3,
//! `notes/FORMATS_SPEC.md`).

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ComtradeError {
    #[error("io: {0}")]
    Io(String),
    #[error("malformed .cfg at line {0}: {1}")]
    Cfg(usize, String),
    #[error("malformed .dat: {0}")]
    Dat(String),
    /// String 330 — the original's "invalid file format" (unknown data file
    /// type, a `.cff` without CFG or DAT section, a bad `.osc`).
    #[error("Неправильный формат файла")]
    Format,
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
    /// Gain applied to the fundamental by `fourie:` (`rec+0x278`): 1 for
    /// COMTRADE, set by the OSC2 version (1.0128 / 1.00259 / 1.014065).
    pub harmonic1_gain: f64,
    /// The record's time stamps as the `.cfg` writes them (the loader
    /// `0x450e70` keeps the four tokens: `doc+0x24c` start date, `+0x250`
    /// start time, `+0x254` trigger date, `+0x258` trigger time).
    pub stamps: Stamps,
}

/// The two `.cfg` time-stamp lines, token by token, and the COMTRADE 2013
/// `time_code,local_code` line (`local_code` = `doc+0x264`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stamps {
    pub start_date: String,
    pub start_time: String,
    pub trigger_date: String,
    pub trigger_time: String,
    pub time_code: String,
    pub local_code: String,
}

impl Oscillogram {
    pub fn sample_count(&self) -> usize {
        self.time.len()
    }

    /// Samples per nominal period for DFT windows (the original's
    /// `rate / trunc(f)`; estimated from the time axis if the rate is unknown).
    pub fn samples_per_cycle(&self) -> usize {
        if let Some(n) = crate::signal::samples_per_period(self.sample_rate, self.line_freq) {
            n
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
    load_with(path, Encoding::Auto)
}

/// Load a COMTRADE record, decoding the `.cfg` with a specific text encoding
/// (the original's program-settings dialog lets the user choose it).
pub fn load_with(path: impl AsRef<Path>, enc: Encoding) -> Result<Oscillogram, ComtradeError> {
    let path = path.as_ref();
    // 0x44ee70: the last four characters, lowercased, choose the reader.
    let name = path.to_string_lossy();
    let ext: String = name.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<String>().to_lowercase();
    if ext == ".osc" {
        let bytes = std::fs::read(path).map_err(|e| ComtradeError::Io(e.to_string()))?;
        return parse_osc(&bytes, enc);
    }
    if ext == ".cff" {
        let bytes = std::fs::read(path).map_err(|e| ComtradeError::Io(e.to_string()))?;
        return parse_cff(&bytes, enc).map(|(rec, _)| rec);
    }
    let (cfg_path, dat_path) = sibling_paths(path)?;
    let cfg_bytes = std::fs::read(&cfg_path).map_err(|e| ComtradeError::Io(e.to_string()))?;
    let cfg_text = decode_text_enc(&cfg_bytes, enc);
    let (mut rec, fmt) = parse_cfg(&cfg_text)?;
    let dat = std::fs::read(&dat_path).map_err(|e| ComtradeError::Io(e.to_string()))?;
    parse_dat(&mut rec, fmt, &dat)?;
    Ok(rec)
}

/// Serialise an oscillogram to a COMTRADE 1999 ASCII `(cfg, dat)` pair
/// (analog + digital channels; computed channels are not COMTRADE channels).
pub fn write(osc: &Oscillogram) -> (String, String) {
    let na = osc.analog.len();
    let nd = osc.digital.len();
    let n = osc.sample_count();
    let rate = if osc.sample_rate > 0.0 {
        osc.sample_rate
    } else if n > 1 && osc.time[1] > osc.time[0] {
        1.0 / (osc.time[1] - osc.time[0])
    } else {
        0.0
    };

    let mut cfg = String::new();
    cfg.push_str(&format!("{},{},1999\n", osc.station, osc.device));
    cfg.push_str(&format!("{},{}A,{}D\n", na + nd, na, nd));
    for (i, ch) in osc.analog.iter().enumerate() {
        cfg.push_str(&format!(
            "{},{},{},,{},{},{},0,-32768,32767,{},{},{}\n",
            i + 1, ch.id, ch.phase, ch.unit, ch.a, ch.b, ch.primary, ch.secondary,
            if ch.is_primary { "P" } else { "S" },
        ));
    }
    for (i, ch) in osc.digital.iter().enumerate() {
        cfg.push_str(&format!("{},{},{},,0\n", i + 1, ch.id, ch.phase));
    }
    cfg.push_str(&format!("{}\n", osc.line_freq));
    cfg.push_str("1\n");
    cfg.push_str(&format!("{},{}\n", rate, n));
    // The record's own time stamps (a record without them gets 01/01/2000).
    let st = &osc.stamps;
    for (d, t) in [(&st.start_date, &st.start_time), (&st.trigger_date, &st.trigger_time)] {
        if d.is_empty() {
            cfg.push_str("01/01/2000,00:00:00.000000\n");
        } else {
            cfg.push_str(&format!("{d},{t}\n"));
        }
    }
    cfg.push_str("ASCII\n1\n");
    if osc.rev_year == 2013 {
        cfg.push_str(&format!("{},{}\n", st.time_code, st.local_code));
    }

    let mut dat = String::new();
    for k in 0..n {
        let ts = (osc.time.get(k).copied().unwrap_or(0.0) * 1e6).round() as i64;
        dat.push_str(&format!("{},{}", k + 1, ts));
        for ch in &osc.analog {
            let v = ch.samples.get(k).copied().unwrap_or(0.0);
            let raw = if ch.a.abs() > 1e-12 { (v - ch.b) / ch.a } else { v };
            dat.push_str(&format!(",{}", raw.round() as i64));
        }
        for ch in &osc.digital {
            dat.push_str(&format!(",{}", if ch.samples.get(k).copied().unwrap_or(false) { 1 } else { 0 }));
        }
        dat.push('\n');
    }
    (cfg, dat)
}

/// Parse from in-memory cfg text + dat bytes (used by tests and the `.cff` path).
pub fn parse(cfg_text: &str, dat: &[u8]) -> Result<Oscillogram, ComtradeError> {
    let (mut rec, fmt) = parse_cfg(cfg_text)?;
    parse_dat(&mut rec, fmt, dat)?;
    Ok(rec)
}

/// The non-CFG/DAT text sections of a `.cff` (re-emitted when saving).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CffSections {
    pub inf: Option<String>,
    pub hdr: Option<String>,
    pub set: Option<String>,
}

/// Find `needle` in `hay` at or after `from` (MFC `CString::Find`).
fn find_from(hay: &str, needle: &str, from: usize) -> Option<usize> {
    hay.get(from..).and_then(|h| h.find(needle)).map(|i| i + from)
}

/// One text section of a `.cff` as the original extracts it (`0x44fe30`):
/// content starts two characters after the first CRLF following the marker
/// (index 1 if there is none) and runs to the next `--- file type:`; with no
/// later marker it is empty. `None` if the marker is absent.
fn cff_section(text: &str, kind: &str) -> Option<String> {
    let p = text.find(&format!("--- file type: {kind}"))?;
    let start = find_from(text, "\r\n", p).map_or(1, |q| q + 2);
    Some(match find_from(text, "--- file type:", start) {
        Some(e) => text[start..e].to_string(),
        None => String::new(),
    })
}

/// Read a single-file COMTRADE (`.cff`) the way the original does
/// (`0x44fe30`, ledger L3): text sections from the file decoded up to its
/// first NUL byte; the DAT payload located on the raw bytes — the byte count
/// is the first run of digits on the first line containing
/// `--- file type: DAT`, and the payload starts **one byte after** that
/// line's terminator (the LF of a CRLF pair: a one-byte shift, harmless for
/// ASCII data and reproduced as is). Missing CFG or DAT → error 330.
pub fn parse_cff(bytes: &[u8], enc: Encoding) -> Result<(Oscillogram, CffSections), ComtradeError> {
    let nul = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let text = decode_text_enc(&bytes[..nul], enc);
    let cfg = cff_section(&text, "CFG").ok_or(ComtradeError::Format)?;
    let sections = CffSections {
        inf: cff_section(&text, "INF"),
        hdr: cff_section(&text, "HDR"),
        set: cff_section(&text, "SET"),
    };
    // DAT: scan lines of the raw bytes (leading tab/LF/CR/space skipped, a
    // line ends at CR or LF).
    let marker = b"--- file type: DAT";
    let mut pos = 0;
    let mut dat: Option<&[u8]> = None;
    while pos < bytes.len() {
        while pos < bytes.len() && matches!(bytes[pos], b'\t' | b'\n' | b'\r' | b' ') {
            pos += 1;
        }
        let start = pos;
        while pos < bytes.len() && !matches!(bytes[pos], b'\r' | b'\n') {
            pos += 1;
        }
        let line = &bytes[start..pos];
        if line.windows(marker.len()).any(|w| w == marker) {
            let digits: Vec<u8> = line
                .iter()
                .skip_while(|b| !b.is_ascii_digit())
                .take_while(|b| b.is_ascii_digit())
                .copied()
                .collect();
            let n: usize = std::str::from_utf8(&digits).ok().and_then(|d| d.parse().ok()).unwrap_or(0);
            let from = (pos + 1).min(bytes.len());
            dat = Some(&bytes[from..(from + n).min(bytes.len())]);
            break;
        }
    }
    let dat = dat.ok_or(ComtradeError::Format)?;
    let (mut rec, fmt) = parse_cfg(&cfg)?;
    parse_dat(&mut rec, fmt, dat)?;
    Ok((rec, sections))
}

/// Encode text as CP1251 (the original writes every text section in it).
fn to_cp1251(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| {
            if (c as u32) < 0x80 {
                c as u8
            } else {
                (0x80..=0xFFu8).find(|b| cp1251_to_char(*b) == c).unwrap_or(b'?')
            }
        })
        .collect()
}

/// Error 187 of the `.cff` writer.
#[derive(Debug, thiserror::Error)]
#[error("В формате .cff можно сохранять только осциллограммы стандарта comtrade'13 и новее")]
pub struct CffRevisionError;

/// Write a single-file COMTRADE (`0x44d0f0`): only records of revision ≥ 2013;
/// `--- file type: CFG ---` + the cfg (CRLF lines), then the HDR, INF, SET
/// sections that came with the record, then `--- file type: DAT BINARY: n---`
/// and the data bytes. Text in CP1251.
pub fn write_cff(osc: &Oscillogram, sections: &CffSections) -> Result<Vec<u8>, CffRevisionError> {
    if osc.rev_year < 2013 {
        return Err(CffRevisionError);
    }
    let (cfg, dat) = write(osc);
    let cfg = cfg.replacen(",1999\n", &format!(",{}\n", osc.rev_year), 1).replace('\n', "\r\n");
    let mut out = to_cp1251(&format!("--- file type: CFG ---\r\n{cfg}"));
    for (kind, body) in [("HDR", &sections.hdr), ("INF", &sections.inf), ("SET", &sections.set)] {
        if let Some(b) = body {
            out.extend(to_cp1251(&format!("--- file type: {kind} ---\r\n{b}")));
        }
    }
    out.extend(to_cp1251(&format!("--- file type: DAT BINARY: {}---\r\n", dat.len())));
    out.extend(dat.as_bytes());
    Ok(out)
}

/// Big-endian readers over a block.
fn be_i16(b: &[u8], o: usize) -> i16 {
    i16::from_be_bytes([b[o], b[o + 1]])
}
fn be_u16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}
fn be_f32(b: &[u8], o: usize) -> f64 {
    f64::from(f32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]))
}

/// The original's rounding (`FUN_00411bf0`): floor unless at least half-way.
fn round_fv(x: f64) -> f64 {
    let f = x.floor();
    if (f - x).abs() >= 0.5 {
        x.ceil()
    } else {
        f
    }
}

/// Unit prefix multiplier (`0x422b10`; micro and milli verified, the rest of
/// its table is not determined).
fn unit_multiplier(unit: &str) -> f64 {
    let u = unit.trim();
    for p in ["мк", "mk", "µ", "u"] {
        if u.len() > p.len() && u.starts_with(p) {
            return 1e-6;
        }
    }
    for p in ["м", "m"] {
        if u.len() > p.len() && u.starts_with(p) {
            return 1e-3;
        }
    }
    1.0
}

/// The first block's sample rate snapped to the standard rates (`0x453db0`).
fn snap_rate(fs: f64, fnom: f64) -> f64 {
    let bands: &[(f64, f64, f64)] = if (fnom - 60.0).abs() < 0.5 {
        &[
            (17280.0, 26000.0, 23040.0), (9600.0, 17280.0, 11520.0), (6720.0, 9600.0, 7680.0),
            (4550.0, 6720.0, 5760.0), (3110.0, 4550.0, 3340.0), (2400.0, 3110.0, 2880.0),
            (1680.0, 2400.0, 1920.0), (1200.0, 1680.0, 1440.0), (840.0, 1200.0, 960.0),
            (600.0, 840.0, 720.0), (420.0, 600.0, 480.0), (300.0, 420.0, 360.0), (200.0, 300.0, 240.0),
        ]
    } else {
        &[
            (14400.0, 22000.0, 19200.0), (8000.0, 14400.0, 9600.0), (5600.0, 8000.0, 6400.0),
            (4000.0, 5600.0, 4800.0), (2800.0, 4000.0, 3200.0), (2000.0, 2800.0, 2400.0),
            (1400.0, 2000.0, 1600.0), (1000.0, 1400.0, 1200.0), (700.0, 1000.0, 800.0),
            (500.0, 700.0, 600.0), (350.0, 500.0, 400.0), (250.0, 350.0, 300.0), (160.0, 250.0, 200.0),
        ]
    };
    bands
        .iter()
        .find(|(lo, hi, _)| *lo <= fs && fs <= *hi)
        .map(|b| b.2)
        .unwrap_or_else(|| round_fv(fs / fnom) * round_fv(fnom))
}

/// Read an OSC2 oscillogram (`.osc`, `0x4524b0`; layout in
/// `notes/FORMATS_SPEC.md`): big-endian 128-byte blocks — header, calibration,
/// channel-table text, info, then fixed-size data blocks with a per-block
/// sample-rate code.
pub fn parse_osc(bytes: &[u8], enc: Encoding) -> Result<Oscillogram, ComtradeError> {
    const B: usize = 128;
    let block = |i: usize| bytes.get(i * B..(i + 1) * B).ok_or(ComtradeError::Format);
    let b0 = block(0)?;
    let (major, minor, h) = (b0[0], b0[1], b0[2] as usize);
    let k = if b0[0x7D] != 0 { 2 } else { 1 };
    if h < k + 1 {
        return Err(ComtradeError::Format);
    }
    let gain = match (major, minor) {
        (2, 3 | 4 | 5 | 8) => 1.0128,
        (2, 6 | 7) => 1.00259,
        (2, 9) => 1.014065,
        _ => 1.0,
    };
    // Channel table text.
    let mut text = String::new();
    for i in k + 1..h {
        let raw: Vec<u8> = block(i)?.iter().copied().filter(|b| *b != 0).collect();
        text.push_str(&decode_text_enc(&raw, enc));
        text.push('\n');
    }
    let lines: Vec<&str> = text.split(['\r', '\n']).filter(|l| !l.trim().is_empty()).collect();
    let (mut analog_defs, mut digital_defs, mut n_per_block) = (Vec::new(), Vec::new(), None);
    let mut li = 0;
    while li < lines.len() {
        let line = lines[li];
        let tag: String = line.chars().take(3).collect();
        let count: usize = line.chars().skip(4).take(3).collect::<String>().trim().parse().unwrap_or(0);
        li += 1;
        match tag.chars().nth(2) {
            Some('A') | Some('D') => {
                for _ in 0..count {
                    let Some(l) = lines.get(li) else { break };
                    li += 1;
                    let f: Vec<String> = l.split(',').map(|x| x.trim().to_string()).collect();
                    let name = f.get(1).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| "Резерв.".into());
                    let phase = f.get(2).cloned().unwrap_or_default();
                    if tag.ends_with('A') {
                        let unit = if f.len() >= 5 { f[4].clone() } else { f.get(3).cloned().unwrap_or_default() };
                        analog_defs.push((name, phase, unit));
                    } else {
                        digital_defs.push((name, phase));
                    }
                }
            }
            Some('F') => {
                n_per_block = line.split(',').nth(1).and_then(|n| n.trim().parse::<usize>().ok());
            }
            _ => return Err(ComtradeError::Format),
        }
    }
    let n = n_per_block.filter(|n| *n > 0).ok_or(ComtradeError::Format)?;
    let (na, nd) = (analog_defs.len(), digital_defs.len());
    // Calibration (a, b) per analog channel.
    let mut cal = vec![(1.0, 0.0); na];
    for (i, c) in cal.iter_mut().enumerate() {
        if i / 8 >= k {
            break;
        }
        let blk = block(1 + i / 8)?;
        let j = i % 8;
        let (f1, s1, f2, s2) = (be_f32(blk, 4 * j), be_i16(blk, 0x20 + 2 * j), be_f32(blk, 0x30 + 4 * j), be_i16(blk, 0x50 + 2 * j));
        let a = if f2 == 0.0 { 1.0 } else { f1 / (f2 * 3276.8) };
        let b = -(f64::from(s1) + f64::from(s2)) * a;
        *c = if a.is_finite() && b.is_finite() { (a, b) } else { (1.0, 0.0) };
    }
    // Info block.
    let info = block(h)?;
    let max_blocks = be_i16(info, 2).max(0) as usize;
    let ratio = |i: usize| {
        let r = if 0x40 + 4 * i + 4 <= B { be_f32(info, 0x40 + 4 * i) } else { 1.0 };
        if r > 0.1 && r < 100000.0 {
            r
        } else {
            1.0
        }
    };
    let mut analog: Vec<AnalogChannel> = analog_defs
        .iter()
        .enumerate()
        .map(|(i, (name, phase, unit))| AnalogChannel {
            index: i as u32 + 1,
            id: name.clone(),
            phase: phase.clone(),
            unit: unit.clone(),
            a: cal[i].0,
            b: cal[i].1,
            primary: ratio(i),
            secondary: 1.0,
            is_primary: false,
            samples: Vec::new(),
        })
        .collect();
    let mut digital: Vec<DigitalChannel> = digital_defs
        .iter()
        .enumerate()
        .map(|(i, (name, phase))| DigitalChannel { index: i as u32 + 1, id: name.clone(), phase: phase.clone(), samples: Vec::new() })
        .collect();
    let m = analog_defs.iter().filter(|(_, _, u)| matches!(u.trim(), "V" | "В" | "A" | "А" | "")).count() as f64;
    let words = nd / 16;
    let rec_len = (na + words) * 2;
    let mul: Vec<f64> = analog_defs.iter().map(|(_, _, u)| unit_multiplier(u)).collect();
    let fnom = 50.0;
    let (mut rate, mut first_rate) = (2400.0, None);
    let mut time = Vec::new();
    let mut t_us: i64 = 0;
    let mut bi = 0;
    while (max_blocks == 0 || bi < max_blocks) && (h + 1 + bi + 1) * B <= bytes.len() {
        let blk = block(h + 1 + bi)?;
        let code = f64::from(be_i16(blk, 0x7C));
        let fs = round_fv(1e6 / (code * 0.03125 * m));
        if fs.is_finite() && fs >= 1.0 {
            rate = if first_rate.is_none() { snap_rate(fs, fnom) } else { fs };
        }
        first_rate.get_or_insert(rate);
        // N records from the block start; nothing checks that they end
        // before the rate code at +0x7C or the block end.
        let base = (h + 1 + bi) * B;
        for s in 0..n {
            let off = base + s * rec_len;
            if off + rec_len > bytes.len() {
                break;
            }
            for (ci, ch) in analog.iter_mut().enumerate() {
                let raw = f64::from(be_i16(bytes, off + 2 * ci));
                ch.samples.push(raw * mul[ci] * ch.a + ch.b);
            }
            for w in 0..words {
                let word = be_u16(bytes, off + 2 * (na + w));
                for bit in 0..16 {
                    if let Some(d) = digital.get_mut(16 * w + bit) {
                        d.samples.push((word >> bit) & 1 == 1);
                    }
                }
            }
            time.push(t_us as f64 / 1e6);
            t_us += (1e6 / rate).floor() as i64;
        }
        bi += 1;
    }
    for d in digital.iter_mut().skip(16 * words) {
        d.samples = vec![false; time.len()];
    }
    Ok(Oscillogram {
        station: String::new(),
        device: format!("OSC Осциллограф. Версия {major}.{minor}"),
        rev_year: 0,
        line_freq: fnom,
        time,
        analog,
        digital,
        sample_rate: first_rate.unwrap_or(rate),
        harmonic1_gain: gain,
        stamps: Stamps::default(),
    })
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

/// Text encoding for the `.cfg` (the original's program-settings dialog lets the
/// user pick among these — `DIALOG 510`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Encoding {
    /// UTF-8 if valid, else CP1251 — the previous behaviour.
    #[default]
    Auto,
    Utf8,
    Cp1251,
    Cp866,
    Koi8r,
}

/// Decode cfg bytes with the given encoding (Cyrillic single-byte code pages, or
/// UTF-8 / auto-detect).
fn decode_text_enc(bytes: &[u8], enc: Encoding) -> String {
    match enc {
        Encoding::Auto => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => bytes.iter().map(|&b| cp1251_to_char(b)).collect(),
        },
        Encoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        Encoding::Cp1251 => bytes.iter().map(|&b| cp1251_to_char(b)).collect(),
        Encoding::Cp866 => bytes.iter().map(|&b| cp866_to_char(b)).collect(),
        Encoding::Koi8r => bytes.iter().map(|&b| koi8r_to_char(b)).collect(),
    }
}

/// Decode cfg bytes: UTF-8/ASCII, or CP1251 fall-back for Cyrillic channel ids.
fn decode_text(bytes: &[u8]) -> String {
    decode_text_enc(bytes, Encoding::Auto)
}

/// Decode bytes with an encoding (the public face of the loader's decoder).
pub fn decode_bytes(bytes: &[u8], enc: Encoding) -> String {
    decode_text_enc(bytes, enc)
}

/// Encode text back to the bytes `enc` decodes it from (`Auto` = UTF-8);
/// characters the code page lacks become `?`.
pub fn encode_text(text: &str, enc: Encoding) -> Vec<u8> {
    let single = |f: fn(u8) -> char| -> Vec<u8> {
        text.chars()
            .map(|c| if (c as u32) < 0x80 { c as u8 } else { (0x80..=0xFFu8).find(|b| f(*b) == c).unwrap_or(b'?') })
            .collect()
    };
    match enc {
        Encoding::Auto | Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Cp1251 => single(cp1251_to_char),
        Encoding::Cp866 => single(cp866_to_char),
        Encoding::Koi8r => single(koi8r_to_char),
    }
}

/// Re-read text decoded with `from` as if its bytes were in `to` — the
/// code-page switches of the original (file parameters `0x4543e0`, the
/// comment dialog `0x43e410`) re-decode the record's raw bytes.
pub fn recode(text: &str, from: Encoding, to: Encoding) -> String {
    decode_text_enc(&encode_text(text, from), to)
}

/// The record code page guessed from the raw channel-name bytes
/// (`0x459e60`, run by the settings loader `0x44f5b0` at every open): count
/// the bytes that are Cyrillic letters read as Windows-1251 (0xC0..0xFF) and
/// read as IBM866 (А–п 0x80..0xAF, р–я 0xE0..0xEF); 1251 wins only with
/// strictly more letters, otherwise 866.
pub fn detect_codepage(raw: &[u8]) -> Encoding {
    let n1251 = raw.iter().filter(|b| **b >= 0xC0).count();
    let n866 = raw.iter().filter(|b| matches!(**b, 0x80..=0xAF | 0xE0..=0xEF)).count();
    if n866 < n1251 {
        Encoding::Cp1251
    } else {
        Encoding::Cp866
    }
}

/// Windows-1251 upper half (0x98 is unassigned; kept as U+0098 so the table
/// is a bijection).
const CP1251_HI: [u16; 64] = [
    0x0402, 0x0403, 0x201A, 0x0453, 0x201E, 0x2026, 0x2020, 0x2021, 0x20AC, 0x2030, 0x0409, 0x2039, 0x040A, 0x040C, 0x040B, 0x040F,
    0x0452, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x0098, 0x2122, 0x0459, 0x203A, 0x045A, 0x045C, 0x045B, 0x045F,
    0x00A0, 0x040E, 0x045E, 0x0408, 0x00A4, 0x0490, 0x00A6, 0x00A7, 0x0401, 0x00A9, 0x0404, 0x00AB, 0x00AC, 0x00AD, 0x00AE, 0x0407,
    0x00B0, 0x00B1, 0x0406, 0x0456, 0x0491, 0x00B5, 0x00B6, 0x00B7, 0x0451, 0x2116, 0x0454, 0x00BB, 0x0458, 0x0405, 0x0455, 0x0457,
];

fn cp1251_to_char(b: u8) -> char {
    match b {
        0x00..=0x7F => b as char,
        0x80..=0xBF => char::from_u32(u32::from(CP1251_HI[(b - 0x80) as usize])).unwrap_or('?'),
        // А..я are contiguous from 0xC0.
        _ => char::from_u32(0x0410 + (b as u32 - 0xC0)).unwrap_or('?'),
    }
}

/// IBM866 pseudo-graphics 0xB0..0xDF and the tail 0xF0..0xFF.
const CP866_BOX: [u16; 48] = [
    0x2591, 0x2592, 0x2593, 0x2502, 0x2524, 0x2561, 0x2562, 0x2556, 0x2555, 0x2563, 0x2551, 0x2557, 0x255D, 0x255C, 0x255B, 0x2510,
    0x2514, 0x2534, 0x252C, 0x251C, 0x2500, 0x253C, 0x255E, 0x255F, 0x255A, 0x2554, 0x2569, 0x2566, 0x2560, 0x2550, 0x256C, 0x2567,
    0x2568, 0x2564, 0x2565, 0x2559, 0x2558, 0x2552, 0x2553, 0x256B, 0x256A, 0x2518, 0x250C, 0x2588, 0x2584, 0x258C, 0x2590, 0x2580,
];
const CP866_TAIL: [u16; 16] = [
    0x0401, 0x0451, 0x0404, 0x0454, 0x0407, 0x0457, 0x040E, 0x045E, 0x00B0, 0x2219, 0x00B7, 0x221A, 0x2116, 0x00A4, 0x25A0, 0x00A0,
];

fn cp866_to_char(b: u8) -> char {
    let u = match b {
        0x00..=0x7F => return b as char,
        0x80..=0x9F => 0x0410 + (b as u32 - 0x80), // А..Я
        0xA0..=0xAF => 0x0430 + (b as u32 - 0xA0), // а..п
        0xB0..=0xDF => u32::from(CP866_BOX[(b - 0xB0) as usize]),
        0xE0..=0xEF => 0x0440 + (b as u32 - 0xE0), // р..я
        _ => u32::from(CP866_TAIL[(b - 0xF0) as usize]),
    };
    char::from_u32(u).unwrap_or('?')
}

fn koi8r_to_char(b: u8) -> char {
    const UPPER: &str = "юабцдефгхийклмнопярстужвьызшэщчъ\
ЮАБЦДЕФГХИЙКЛМНОПЯРСТУЖВЬЫЗШЭЩЧЪ";
    match b {
        0x00..=0x7F => b as char,
        0xA3 => 'ё',
        0xB3 => 'Ё',
        0xC0..=0xFF => UPPER.chars().nth((b - 0xC0) as usize).unwrap_or('?'),
        _ => '?',
    }
}

/// The leading integers of `s` in the shape of `swscanf` with `%d` fields
/// separated by the literal characters `seps` (whitespace before a number is
/// skipped, a field may carry a sign); returns the values read until the
/// first mismatch.
fn scan_ints(s: &str, seps: &[char]) -> Vec<i64> {
    let mut out = Vec::new();
    let mut it = s.chars().peekable();
    for k in 0..=seps.len() {
        while it.peek().is_some_and(|c| c.is_whitespace()) {
            it.next();
        }
        let mut t = String::new();
        if let Some(&c) = it.peek() {
            if c == '-' || c == '+' {
                t.push(c);
                it.next();
            }
        }
        while let Some(&c) = it.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            t.push(c);
            it.next();
        }
        match t.parse::<i64>() {
            Ok(v) => out.push(v),
            Err(_) => break,
        }
        if k < seps.len() {
            if it.peek() != Some(&seps[k]) {
                break;
            }
            it.next();
        }
    }
    out
}

/// `_wtoi`: the leading (signed) integer of `s`, 0 if none.
fn wtoi(s: &str) -> i64 {
    scan_ints(s, &[]).first().copied().unwrap_or(0)
}

/// A `d/m/y` date field as the original orders it (`0x455800`, `0x456e90`):
/// the first number is the month unless the revision is exactly 2013 (then
/// the day); a "month" above 12 swaps the two; a year below 100 gets +2000.
/// Returns (year, month, day).
fn stamp_date(date: &str, rev_year: i32) -> Option<(i64, i64, i64)> {
    let v = scan_ints(date, &['/', '/']);
    if v.len() != 3 {
        return None;
    }
    let (mut month, mut day) = (v[0], v[1]);
    if rev_year == 2013 {
        std::mem::swap(&mut month, &mut day);
    }
    if month > 12 {
        std::mem::swap(&mut month, &mut day);
    }
    let year = if v[2] < 100 { v[2] + 2000 } else { v[2] };
    Some((year, month, day))
}

/// Days from 1970-01-01 of a proleptic Gregorian date (month/day may be out
/// of range and are normalised, as `mktime` does).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let (y, m) = (y + (m - 1).div_euclid(12), (m - 1).rem_euclid(12) + 1);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The raw form of a time stamp (`0x4495e0` / `0x449690`): the date
/// re-ordered as `dd/mm/yyyy` (empty when it does not parse) + ", " + the
/// time field as written.
pub fn raw_stamp(date: &str, time: &str, rev_year: i32) -> String {
    let d = stamp_date(date, rev_year).map(|(y, m, d)| format!("{d:02}/{m:02}/{y:04}")).unwrap_or_default();
    format!("{d}, {time}")
}

/// A record time stamp as the parameters bar shows it (`0x456e90`).
///
/// Without a COMTRADE 2013 `local_code` (`doc+0x264`, trimmed) the raw form
/// is shown. Otherwise `d/m/y` and `h:m:s.frac` are parsed (failure → raw
/// form) and printed `dd.mm.YYYY, HH:MM:SS.` + the fraction as an integer
/// (`_itow`: its leading zeros are lost, as in the original) + « (UTC)» or
/// « (Местное время)». Local time adds the `local_code` offset — `+h`,
/// `-h`, `+hhmm` written `±<h>h<m>` — to the stamp, unless the code is "0"
/// or has no single sign.
pub fn format_stamp(date: &str, time: &str, rev_year: i32, local_code: &str, utc: bool) -> String {
    let code = local_code.trim();
    if code.is_empty() {
        return raw_stamp(date, time, rev_year);
    }
    let Some((y, mo, d)) = stamp_date(date, rev_year) else {
        return raw_stamp(date, time, rev_year);
    };
    let t = scan_ints(time, &[':', ':', '.']);
    if t.len() != 4 {
        return raw_stamp(date, time, rev_year);
    }
    let mut secs = days_from_civil(y, mo, d) * 86400 + t[0] * 3600 + t[1] * 60 + t[2];
    if !utc && code != "0" {
        let (plus, minus) = (code.starts_with('+'), code.starts_with('-'));
        if plus != minus {
            let rest = &code[1..];
            let (h, m) = match rest.find('h') {
                Some(i) => (wtoi(&rest[..i]), wtoi(&rest[i + 1..])),
                None => (wtoi(rest), 0),
            };
            let off = (h * 60 + m) * 60;
            secs += if plus { off } else { -off };
        }
    }
    let (yy, mm, dd) = civil_from_days(secs.div_euclid(86400));
    let s = secs.rem_euclid(86400);
    let suffix = if utc { " (UTC)" } else { " (Местное время)" };
    format!("{dd:02}.{mm:02}.{yy:04}, {:02}:{:02}:{:02}.{}{suffix}", s / 3600, s / 60 % 60, s % 60, t[3])
}

impl Oscillogram {
    /// Re-time the record at a new sample rate (file parameters, `0x449f40`):
    /// a uniform grid from the first sample.
    pub fn retime(&mut self, rate: f64) {
        if rate <= 0.0 || !rate.is_finite() {
            return;
        }
        let t0 = self.time.first().copied().unwrap_or(0.0);
        for (k, t) in self.time.iter_mut().enumerate() {
            *t = t0 + k as f64 / rate;
        }
        self.sample_rate = rate;
    }

    /// The record duration (`0x44c0b0`): last minus first sample time, s.
    pub fn duration(&self) -> f64 {
        self.time.last().copied().unwrap_or(0.0) - self.time.first().copied().unwrap_or(0.0)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DataFormat {
    Ascii,
    Int16,
    Int32,
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

    // Two time-stamp lines (first sample, trigger): `date,time` tokens kept
    // as text, as the loader `0x450e70` stores them.
    let mut stamps = Stamps::default();
    let split2 = |l: Option<&str>| -> (String, String) {
        let l = l.unwrap_or("");
        let (a, b) = l.split_once(',').unwrap_or((l, ""));
        (a.trim().to_string(), b.trim().to_string())
    };
    (stamps.start_date, stamps.start_time) = split2(next(&mut li).ok());
    (stamps.trigger_date, stamps.trigger_time) = split2(next(&mut li).ok());

    // file type
    let ft = next(&mut li).map(|s| s.trim().to_string()).unwrap_or_default();

    // 1999+: timemult (optional)
    let timemult = next(&mut li)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(1.0);
    // 2013: `time_code,local_code` (only read for revision "2013").
    if rev_year == 2013 {
        (stamps.time_code, stamps.local_code) = split2(next(&mut li).ok());
    }

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
        // 0x45158a: exactly these three spellings; anything else (FLOAT32,
        // BINARY32, lowercase ascii…) is error 330.
        "ASCII" => DataFormat::Ascii,
        "BINARY" => DataFormat::Int16,
        "binary32" => DataFormat::Int32,
        _ => return Err(ComtradeError::Format),
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
        harmonic1_gain: 1.0,
        stamps,
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
        DataFormat::Int16 | DataFormat::Int32 => {
            let (abytes, dwords) = match fmt {
                DataFormat::Int16 => (2usize, nd.div_ceil(16)),
                DataFormat::Int32 => (4usize, nd.div_ceil(16)),
                DataFormat::Ascii => unreachable!(),
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
                        DataFormat::Ascii => 0.0,
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

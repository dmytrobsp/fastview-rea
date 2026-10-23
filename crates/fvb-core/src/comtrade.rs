//! COMTRADE (IEEE C37.111) oscillogram reader — the open interchange format the
//! original's `OscillHelper::ComtradeConverter` produces from the native `.sto`.
//!
//! Supports the 1991 and 1999 revisions (and the common 2013 layout) with the
//! data-file types the original accepts — exactly `ASCII`, `BINARY` (int16) and
//! `binary32` — plus the single-file `.cff` container (ledger L3,
//! `notes/FORMATS_SPEC.md`).

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

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
    /// The COMTRADE `ccbm` field (`ch+0x74`).
    pub ccbm: String,
    /// The canonical unit (`ch+0x7c`, [`crate::units::normalize`]): "A" for
    /// a file's «кА»; what every unit test and readout uses.
    pub unit: String,
    /// The unit as written in the file (`ch+0x78`), saved back as is.
    pub raw_unit: String,
    /// The unit's SI-prefix multiplier, folded into `samples` (`ch+0x98`).
    pub unit_mult: f64,
    /// The unit ended in «/с» or "/s" (`ch+0x154`): shown with «/с».
    pub per_second: bool,
    pub a: f64,
    pub b: f64,
    pub primary: f64,
    pub secondary: f64,
    /// `true` if the stored channel is already a primary value (`PS` = 'P').
    pub is_primary: bool,
    pub samples: Vec<f64>,
    /// The channel's own frequency per sample (ch+0x50): empty for a
    /// file's channel; a pasted one's `net_frequency`, a merged one's
    /// resampled (`0x47e4b5`) — what the ОМП's tracked phasor reads.
    pub own_freq: Vec<f64>,
}

impl AnalogChannel {
    /// The unit as the readouts show it (`0x42bc50`).
    pub fn shown_unit(&self) -> crate::units::Shown {
        crate::units::Shown::file(&self.unit, self.per_second)
    }

    /// The unit to write to a file: as read, else the canonical one.
    pub fn file_unit(&self) -> &str {
        if self.raw_unit.is_empty() {
            &self.unit
        } else {
            &self.raw_unit
        }
    }
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
    /// The sample-rate segments (`rec+0x1f0`): the rate and `end`, the
    /// 0-based index of the next run's first sample (COMTRADE's endsamp,
    /// OSC's block × samples per block).
    pub rate_segs: Vec<(f64, usize)>,
    /// The record's integer rate (`rec+0x228`, `0x453db0`).
    pub rate_r: f64,
    /// The `.cfg`'s time multiplier (`rec+0x230`; 1 elsewhere).
    pub time_mult: f64,
    /// The per-sample nominal frequency (`rec+0x1fc`, built by
    /// [`nominal_track`]): what `frequency:` and the clipboard's
    /// `net_frequency` read.
    pub freq_track: Vec<f64>,
    /// The trigger's sample (`doc+0x214`): pre-trigger blocks × samples per
    /// block for `.osc` (`0x452d89`), 0 for COMTRADE and after a `.cfg` /
    /// `.cff` Save-As (`0x44d6c5`, `0x44daaa`).
    pub trigger_sample: usize,
    /// «Частота для коррекции расчётных значений» (`rec+0x874`): 0 none,
    /// k the k-th entry of [`crate::freqcorr::candidate_names`].
    pub freq_sel: i32,
    /// Frequency candidates that are not file channels (formula and pasted
    /// channels), in vector order. Kept by whoever holds them.
    pub extra_freq: Vec<ExtraFreq>,
}

/// A frequency candidate outside the file's channels: a formula channel
/// (inserted after the analog channels, `0x42ea36`) or a pasted one
/// (before the first selected analog channel, else after them, `0x45ad77`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtraFreq {
    pub name: String,
    /// Its stored samples (read at the corrected channel's index).
    pub samples: Vec<f64>,
    /// Its place among the file's analog channels: in front of analog
    /// channel `pos` (the analog count = after them).
    pub pos: usize,
    /// The holder's own key for it (ours: the computed channel's index).
    pub id: usize,
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

/// Loading progress shared with the «Загрузка» dialog (DIALOG 492): the
/// document's position `+0x82c`, total `+0x830` and cancel flag `+0x838`,
/// plain 32-bit counters as in the original (ledger L9.31).
#[derive(Debug, Default)]
pub struct Progress {
    pub pos: AtomicU32,
    pub total: AtomicU32,
    pub cancel: AtomicBool,
}

impl Progress {
    /// What the dialog's 500 ms timer shows (`0x471870`):
    /// `((pos·100) mod 2³²) / total`, 0 with no total, clamped to 100 by the
    /// progress control.
    pub fn percent(&self) -> u32 {
        let t = self.total.load(Ordering::Relaxed);
        if t == 0 {
            return 0;
        }
        (self.pos.load(Ordering::Relaxed).wrapping_mul(100) / t).min(100)
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn set_total(&self, bytes: usize) {
        self.total.store((bytes as u64 * 5) as u32, Ordering::Relaxed);
    }

    /// `T − remaining` (BINARY / binary32 records, `.osc` blocks).
    fn set_remaining(&self, remaining: usize) {
        self.pos.store(self.total.load(Ordering::Relaxed).wrapping_sub(remaining as u32), Ordering::Relaxed);
    }

    /// The recompute phase (`0x428490`, `0x428809`): `step` added per sample
    /// of every analog channel, cancel polled per sample. `step` =
    /// ⌊T / (N·C)⌋ (`0x450c17`; `.osc` ×5, `0x4525d3`), N the first
    /// channel's samples, C all analog + digital channels.
    fn recompute_phase(&self, rec: &Oscillogram, osc: bool) {
        let n = rec.analog.first().map(|c| c.samples.len()).or_else(|| rec.digital.first().map(|c| c.samples.len())).unwrap_or(0);
        let c = rec.analog.len() + rec.digital.len();
        let nc = (n as u64 * c as u64) as u32;
        if nc == 0 {
            return;
        }
        let mut step = self.total.load(Ordering::Relaxed) / nc;
        if osc {
            step = step.wrapping_mul(5);
        }
        for ch in &rec.analog {
            for chunk in ch.samples.chunks(1024) {
                if self.cancelled() {
                    return;
                }
                self.pos.fetch_add(step.wrapping_mul(chunk.len() as u32), Ordering::Relaxed);
            }
        }
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
    load_progress(path, enc, None)
}

/// [`load_with`] reporting to the «Загрузка» dialog; a cancel stops the
/// readers at their next poll (the result is then to be discarded).
pub fn load_progress(path: impl AsRef<Path>, enc: Encoding, prog: Option<&Progress>) -> Result<Oscillogram, ComtradeError> {
    let path = path.as_ref();
    // 0x44ee70: the last four characters, lowercased, choose the reader.
    let name = path.to_string_lossy();
    let ext: String = name.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<String>().to_lowercase();
    if ext == ".osc" {
        let bytes = std::fs::read(path).map_err(|e| ComtradeError::Io(e.to_string()))?;
        return parse_osc_progress(&bytes, enc, prog);
    }
    if ext == ".cff" {
        let bytes = std::fs::read(path).map_err(|e| ComtradeError::Io(e.to_string()))?;
        return parse_cff_progress(&bytes, enc, prog).map(|(rec, _)| rec);
    }
    let (cfg_path, dat_path) = sibling_paths(path)?;
    let cfg_bytes = std::fs::read(&cfg_path).map_err(|e| ComtradeError::Io(e.to_string()))?;
    let cfg_text = decode_text_enc(&cfg_bytes, enc);
    let (mut rec, fmt) = parse_cfg(&cfg_text)?;
    let dat = std::fs::read(&dat_path).map_err(|e| ComtradeError::Io(e.to_string()))?;
    parse_dat(&mut rec, fmt, &dat, prog)?;
    if let Some(p) = prog {
        p.recompute_phase(&rec, false);
    }
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
            i + 1, ch.id, ch.phase, ch.file_unit(), ch.a, ch.b, ch.primary, ch.secondary,
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
            // The inverse of the reader's mult·a·raw + b (`0x422250`).
            let k = ch.a * ch.unit_mult;
            let raw = if k.abs() > 1e-12 { (v - ch.b) / k } else { v };
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
    parse_progress(cfg_text, dat, None)
}

fn parse_progress(cfg_text: &str, dat: &[u8], prog: Option<&Progress>) -> Result<Oscillogram, ComtradeError> {
    let (mut rec, fmt) = parse_cfg(cfg_text)?;
    parse_dat(&mut rec, fmt, dat, prog)?;
    if let Some(p) = prog {
        p.recompute_phase(&rec, false);
    }
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
    parse_cff_progress(bytes, enc, None)
}

/// [`parse_cff`] reporting progress: none while the container is split
/// (`0x44fe30`), then the inner `.cfg` rule over the DAT part.
pub fn parse_cff_progress(bytes: &[u8], enc: Encoding, prog: Option<&Progress>) -> Result<(Oscillogram, CffSections), ComtradeError> {
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
    parse_dat(&mut rec, fmt, dat, prog)?;
    if let Some(p) = prog {
        p.recompute_phase(&rec, false);
    }
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
pub(crate) fn round_fv(x: f64) -> f64 {
    let f = x.floor();
    if (f - x).abs() >= 0.5 {
        x.ceil()
    } else {
        f
    }
}

/// `0x453db0` without the snap: round(R/f)·round(f), both truncated to
/// integers.
pub fn int_rate(fs: f64, fnom: f64) -> f64 {
    ((round_fv(fs / fnom) as i32).wrapping_mul(round_fv(fnom) as i32)) as f64
}

/// The record's nominal-frequency track (`0x454960`): `old` resized to `n`
/// (old values kept, new ones 0); then from sample 0 each sample gets its
/// segment's rate ÷ X · f, X = [`int_rate`](R, f), moving to the next
/// segment after the sample whose index is the segment's `end` and
/// stopping when the segments run out (the rest keep what they had). A
/// record with no samples keeps `old`.
pub fn nominal_track(segs: &[(f64, usize)], n: usize, r: f64, f: f64, old: &[f64]) -> Vec<f64> {
    let mut v = old.to_vec();
    if n == 0 {
        return v;
    }
    v.resize(n, 0.0);
    let x = int_rate(r, f);
    let mut k = 0;
    for (i, slot) in v.iter_mut().enumerate() {
        let Some(&(rate, end)) = segs.get(k) else { break };
        *slot = rate / x * f;
        if end == i {
            k += 1;
            if k == segs.len() {
                break;
            }
        }
    }
    v
}

impl Oscillogram {
    /// The number of rate segments (`rec+0x1f0`).
    pub fn segments(&self) -> usize {
        self.rate_segs.len()
    }

    /// Rebuild [`Oscillogram::freq_track`] over the current samples
    /// (`0x454960`; after a load, a rate change in «Параметры
    /// осциллограммы» `0x42e540`, «Объединить»).
    pub fn rebuild_freq_track(&mut self) {
        self.freq_track = nominal_track(&self.rate_segs, self.time.len(), self.rate_r, self.line_freq, &self.freq_track);
    }
}

/// The first block's sample rate snapped to the standard rates (`0x453db0`
/// with its flag set: the tables only for a network frequency of exactly
/// 50 or 60).
fn snap_rate(fs: f64, fnom: f64) -> f64 {
    if fnom != 50.0 && fnom != 60.0 {
        return int_rate(fs, fnom);
    }
    let bands: &[(f64, f64, f64)] = if fnom == 60.0 {
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
        .unwrap_or_else(|| int_rate(fs, fnom))
}

/// Read an OSC2 oscillogram (`.osc`, `0x4524b0`; layout in
/// `notes/FORMATS_SPEC.md`): big-endian 128-byte blocks — header, calibration,
/// channel-table text, info, then fixed-size data blocks with a per-block
/// sample-rate code.
pub fn parse_osc(bytes: &[u8], enc: Encoding) -> Result<Oscillogram, ComtradeError> {
    parse_osc_progress(bytes, enc, None)
}

/// [`parse_osc`] reporting progress: total 5 × the bytes after the header,
/// position `T − remaining` after each data block (`0x453080`, `0x45342d`;
/// no cancel poll in the block loop), then the recompute phase with the
/// ×5 step.
pub fn parse_osc_progress(bytes: &[u8], enc: Encoding, prog: Option<&Progress>) -> Result<Oscillogram, ComtradeError> {
    let rec = parse_osc_blocks(bytes, enc, prog)?;
    if let Some(p) = prog {
        p.recompute_phase(&rec, true);
    }
    Ok(rec)
}

fn parse_osc_blocks(bytes: &[u8], enc: Encoding, prog: Option<&Progress>) -> Result<Oscillogram, ComtradeError> {
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
            ccbm: String::new(),
            unit: crate::units::normalize(unit).unit,
            raw_unit: unit.clone(),
            unit_mult: crate::units::normalize(unit).mult,
            per_second: crate::units::normalize(unit).per_second,
            a: cal[i].0,
            b: cal[i].1,
            primary: ratio(i),
            secondary: 1.0,
            is_primary: false,
            samples: Vec::new(),
            own_freq: Vec::new(),
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
    let mul: Vec<f64> = analog.iter().map(|c| c.unit_mult).collect();
    let fnom = 50.0;
    let (mut rate, mut first_rate) = (2400.0, None);
    // Segments (`0x453010`): block 0 sets R (snapped) as the current rate;
    // a later block whose rate differs pushes {current, its first sample}.
    let (mut rate_segs, mut cur) = (Vec::new(), f64::NAN);
    let mut time = Vec::new();
    let mut t_us: i64 = 0;
    let mut bi = 0;
    if let Some(p) = prog {
        p.set_total(bytes.len().saturating_sub((h + 1) * B));
        p.pos.store(0, Ordering::Relaxed);
    }
    while (max_blocks == 0 || bi < max_blocks) && (h + 1 + bi + 1) * B <= bytes.len() {
        let blk = block(h + 1 + bi)?;
        let code = f64::from(be_i16(blk, 0x7C));
        let fs = round_fv(1e6 / (code * 0.03125 * m));
        if fs.is_finite() && fs >= 1.0 {
            rate = if first_rate.is_none() { snap_rate(fs, fnom) } else { fs };
        }
        first_rate.get_or_insert(rate);
        if bi == 0 {
            cur = rate;
        } else if rate != cur {
            rate_segs.push((cur, bi * n));
            cur = rate;
        }
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
        if let Some(p) = prog {
            p.set_remaining(bytes.len().saturating_sub((h + 1 + bi) * B));
        }
    }
    for d in digital.iter_mut().skip(16 * words) {
        d.samples = vec![false; time.len()];
    }
    // Then {R, total} unless the last run already is R up to the end — the
    // final run always carries R (`0x453bdb`–`0x453d13`).
    let r = first_rate.unwrap_or(rate);
    if !(cur == r && rate_segs.last().is_some_and(|s: &(f64, usize)| s.1 == time.len())) {
        rate_segs.push((r, time.len()));
    }
    let mut rec = Oscillogram {
        station: String::new(),
        device: format!("OSC Осциллограф. Версия {major}.{minor}"),
        rev_year: 0,
        line_freq: fnom,
        time,
        analog,
        digital,
        sample_rate: first_rate.unwrap_or(rate),
        harmonic1_gain: gain,
        stamps: osc_stamps(info, n, first_rate.unwrap_or(rate)),
        rate_segs,
        rate_r: r,
        time_mult: 1.0,
        freq_track: Vec::new(),
        trigger_sample: (be_i16(info, 0xE).max(0) as usize) * n,
        freq_sel: 0,
        extra_freq: Vec::new(),
    };
    rec.rebuild_freq_track();
    Ok(rec)
}

/// A `.cfg`'s `rec+0x228` (`0x45146a`): `0x453db0` of the mean of the
/// rates, snapped only with several rate lines.
fn comtrade_rate(rates: &[RateSection], f: f64) -> f64 {
    let mean = rates.iter().map(|r| r.rate).sum::<f64>() / rates.len().max(1) as f64;
    if rates.len() > 1 {
        snap_rate(mean, f)
    } else {
        int_rate(mean, f)
    }
}

/// A BCD byte (`0x44a570`).
fn bcd(b: u8) -> u32 {
    u32::from(b >> 4) * 10 + u32::from(b & 15)
}

/// The OSC info block's trigger moment, BCD-decoded (`0x44a570`,
/// `0x44a690`): year = byte 0x0B + 2000, month 0x0A, day 9, hour 8,
/// minute 7, second 6, milliseconds 10 × byte 5.
pub fn osc_trigger(info: &[u8]) -> Option<[u32; 7]> {
    let b = |i: usize| info.get(i).copied().map(bcd);
    Some([2000 + b(0xB)?, b(0xA)?, b(9)?, b(8)?, b(7)?, b(6)?, 10 * b(5)?])
}

/// The `.osc` record's stamps (`0x452d42`–`0x452e57`): the trigger from
/// the info block; the start (pretrig × N − 1) sample periods of
/// ⌊10⁷ / R⌋ × 100 ns before it (pretrig the i16 at 0x0E — so with no
/// pre-trigger blocks the start is a period after the trigger); dates as
/// `%i/%02i/%4i` month first, times `%i:%02i:%02i.%03i`.
fn osc_stamps(info: &[u8], n: usize, rate: f64) -> Stamps {
    let Some([y, mo, d, h, mi, s, ms]) = osc_trigger(info) else { return Stamps::default() };
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return Stamps::default();
    }
    let ticks = |y: u32, mo: u32, d: u32, h: u32, mi: u32, s: u32, ms: u32| {
        days_from_civil(i64::from(y), i64::from(mo), i64::from(d)) * 864_000_000_000 + ((i64::from(h) * 3600 + i64::from(mi) * 60 + i64::from(s)) * 1000 + i64::from(ms)) * 10_000
    };
    let trig = ticks(y, mo, d, h, mi, s, ms);
    let period = if rate.round() >= 1.0 { 10_000_000 / rate.round() as i64 } else { 0 };
    let pretrig = i64::from(be_i16(info, 0xE));
    let start = trig - (pretrig * n as i64 - 1) * period;
    let fmt = |t: i64| {
        let (days, rem) = (t.div_euclid(864_000_000_000), t.rem_euclid(864_000_000_000));
        let (y, m, d) = civil_from_days(days);
        let ms = rem / 10_000;
        (format!("{m}/{d:02}/{y:4}"), format!("{}:{:02}:{:02}.{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000))
    };
    let ((sd, st), (td, tt)) = (fmt(start), fmt(trig));
    Stamps { start_date: sd, start_time: st, trigger_date: td, trigger_time: tt, ..Stamps::default() }
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

/// The record start as seconds since 1970-01-01 (the stamp as written, no
/// time-zone shift), for synchronising two records by their start times.
pub fn stamp_seconds(date: &str, time: &str, rev_year: i32) -> Option<f64> {
    let (y, m, d) = stamp_date(date, rev_year)?;
    let mut hms = time.split(':');
    let (h, mi) = (hms.next()?.trim().parse::<i64>().ok()?, hms.next()?.trim().parse::<i64>().ok()?);
    let sec: f64 = hms.next()?.trim().parse().ok()?;
    // Days from the civil date (proleptic Gregorian).
    let (yy, mm) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * mm + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days as f64 * 86_400.0 + (h * 3600 + mi * 60) as f64 + sec)
}

/// The raw form of a time stamp (`0x4495e0` / `0x449690`): the date
/// re-ordered as `dd/mm/yyyy` (empty when it does not parse) + ", " + the
/// time field as written.
pub fn raw_stamp(date: &str, time: &str, rev_year: i32) -> String {
    format!("{}, {time}", stamp_date_text(date, rev_year).unwrap_or_default())
}

/// A stamp's date as `dd/mm/yyyy` (the absolute-time ruler `0x464350` and
/// the raw stamp), `None` when it does not parse.
pub fn stamp_date_text(date: &str, rev_year: i32) -> Option<String> {
    stamp_date(date, rev_year).map(|(y, m, d)| format!("{d:02}/{m:02}/{y:04}"))
}

/// The «Время:» value of a moment comment (`0x47a4e0`), `dt_us` after the
/// first sample. The start date (as [`stamp_date_text`], "" when it does
/// not parse) + " " + the raw start time is cut at its first `.`; the left
/// part is parsed as an OLE date (`VarDateFromStr`, user locale — taken as
/// ru-RU / uk-UA: day first) and the first three characters after the `.`
/// are `_wtoi`-ed and **added to the µs offset** (the original's slip: the
/// start's milliseconds count as microseconds). Whole seconds of that sum
/// move the date; it is printed as `VarBstrFromDate` does (`dd.MM.yyyy
/// H:mm:ss`, the date alone at midnight, the time alone on day 0) + "." +
/// `%03d` of the sum's milliseconds (truncated). `None` when the date does
/// not parse (the info text is then empty).
pub fn moment_time_text(stamps: &Stamps, rev_year: i32, dt_us: i64) -> Option<String> {
    let s = format!("{} {}", stamp_date_text(&stamps.start_date, rev_year).unwrap_or_default(), stamps.start_time);
    let (left, right) = s.split_once('.').unwrap_or((&s, ""));
    let (mut day, tod) = ole_date(left)?;
    let total = wtoi(&right.chars().take(3).collect::<String>()) + dt_us;
    let mut secs = tod + total / 1_000_000;
    day += secs.div_euclid(86400);
    secs = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(day + days_from_civil(1899, 12, 30));
    let time = format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    let shown = match (day, secs) {
        (_, 0) => format!("{d:02}.{m:02}.{y:04}"),
        (0, _) => time,
        _ => format!("{d:02}.{m:02}.{y:04} {time}"),
    };
    Some(format!("{shown}.{:03}", total % 1_000_000 / 1000))
}

/// The day number from 1899-12-30 and the second of the day of a
/// `d/m/y h:m:s` string, either part optional (`VarDateFromStr` for the
/// shapes [`moment_time_text`] builds; other OLE spellings not modelled).
fn ole_date(s: &str) -> Option<(i64, i64)> {
    let (mut day, mut tod, mut any) = (0, 0, false);
    for tok in s.split_whitespace() {
        if tok.contains('/') {
            let v = scan_ints(tok, &['/', '/']);
            let [d, m, y] = v[..] else { return None };
            let dim = days_from_civil(y + m / 12, m % 12 + 1, 1) - days_from_civil(y, m, 1);
            if !(1..=12).contains(&m) || d < 1 || d > dim {
                return None;
            }
            day = days_from_civil(y, m, d) - days_from_civil(1899, 12, 30);
        } else if tok.contains(':') {
            let v = scan_ints(tok, &[':', ':']);
            let (h, mi, se) = (*v.first()?, *v.get(1)?, v.get(2).copied().unwrap_or(0));
            if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..60).contains(&se) {
                return None;
            }
            tod = h * 3600 + mi * 60 + se;
        } else {
            return None;
        }
        any = true;
    }
    any.then_some((day, tod))
}

/// A stamp's time of day in seconds as the absolute-time labels read it
/// (`0x464350`, `0x466170`): `_wtoi` of the hours and of the minutes after
/// the first `:`, `_wtof` of the seconds after the second; a missing field
/// counts 0.
pub fn time_of_day(time: &str) -> f64 {
    let mut f = time.splitn(3, ':');
    let h = wtoi(f.next().unwrap_or(""));
    let m = f.next().map_or(0, wtoi);
    let s = f.next().map_or(0.0, wtof);
    (h * 3600 + m * 60) as f64 + s
}

/// `_wtof`: the longest leading decimal number of `s`, 0 if none.
fn wtof(s: &str) -> f64 {
    let s = s.trim_start();
    let end = s
        .char_indices()
        .take_while(|&(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+')))
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    (0..=end).rev().find_map(|e| s[..e].parse().ok()).unwrap_or(0.0)
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
        // `rec+0x228`, then the track over the old segments (`0x42e540`).
        self.rate_r = rate;
        self.rebuild_freq_track();
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
        // The unit token is not trimmed (`0x477050` cuts at TAB, CR, LF or
        // a comma only).
        let raw_unit = f.get(4).copied().unwrap_or("").to_string();
        let norm = crate::units::normalize(&raw_unit);
        analog.push(AnalogChannel {
            index: g(0).parse().unwrap_or(0),
            id: g(1).to_string(),
            phase: g(2).to_string(),
            ccbm: g(3).to_string(),
            unit: norm.unit,
            raw_unit,
            unit_mult: norm.mult,
            per_second: norm.per_second,
            a: g(5).parse().unwrap_or(1.0),
            b: g(6).parse().unwrap_or(0.0),
            // An,ch_id,ph,ccbm,uu,a,b,skew,min,max,primary,secondary,PS
            // (fields 11–13 of the 1999 line; absent in 1991 → 1, 1, S).
            primary: g(10).parse().unwrap_or(1.0),
            secondary: g(11).parse().unwrap_or(1.0),
            is_primary: g(12).eq_ignore_ascii_case("p"),
            samples: Vec::new(),
            own_freq: Vec::new(),
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
        rate_segs: rates.iter().map(|r| (r.rate, r.end_sample)).collect(),
        rate_r: comtrade_rate(&rates, line_freq),
        time_mult: timemult,
        freq_track: Vec::new(),
        trigger_sample: 0,
        freq_sel: 0,
        extra_freq: Vec::new(),
    };
    // The loader rebuilds the frequency candidates (`0x45127d`), which
    // selects the first one.
    let mut rec = rec;
    crate::freqcorr::refresh_selection(&mut rec);
    Ok((rec, fmt))
}

/// The data file. Progress (`0x451800` / `0x451be0` / `0x4520f0`): ASCII
/// total 5·S, position = bytes consumed after each line; BINARY total 5·S,
/// position `T − remaining` after each record; binary32 never sets the
/// total (the bar stays at 0). Cancel is polled before each line / record.
fn parse_dat(rec: &mut Oscillogram, fmt: DataFormat, dat: &[u8], prog: Option<&Progress>) -> Result<(), ComtradeError> {
    if let Some(p) = prog {
        if fmt != DataFormat::Int32 {
            p.set_total(dat.len());
        }
    }
    let cancelled = || prog.is_some_and(Progress::cancelled);
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
            let mut stamps_us: Vec<f64> = Vec::new();
            let mut consumed = 0usize;
            for line in text.split_inclusive('\n') {
                if cancelled() {
                    break;
                }
                consumed += line.len();
                if let Some(p) = prog {
                    p.pos.store(consumed.min(dat.len()) as u32, Ordering::Relaxed);
                }
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let f: Vec<&str> = line.split(',').collect();
                // Nothing after the sample number and the timestamp — the
                // reader is at the buffer's end (`0x45189b`): no row (a
                // trailing ^Z line).
                if f.len() < 3 {
                    break;
                }
                // columns: sample#, timestamp, A1..Ana, D1..Dnd; the
                // timestamp read as an integer (`0x477130`).
                stamps_us.push(crate::clipdata::wtoi(f.get(1).copied().unwrap_or("")) as f64);
                // value = mult·a·raw + b (`0x451904`).
                for (ai, ch) in rec.analog.iter_mut().enumerate() {
                    let raw: f64 = f.get(2 + ai).and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
                    ch.samples.push(ch.unit_mult * ch.a * raw + ch.b);
                }
                for (di, ch) in rec.digital.iter_mut().enumerate() {
                    let v: i64 = f.get(2 + na + di).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                    ch.samples.push(v != 0);
                }
                rows += 1;
            }
            // Every row's time is its timestamp × the multiplier, rounded
            // to whole µs (`0x4518a7`–`0x4518cd`) — whatever the rates say.
            let _ = rows;
            rec.time = stamps_us.iter().map(|t| round_fv(t * rec.time_mult) / 1e6).collect();
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
            // Each record's time (`0x451d4d`–`0x451dad`): k · 1e6 / R × the
            // multiplier with an integer rate R (rec+0x228), else its
            // timestamp × the multiplier; whole µs.
            let mut times = Vec::with_capacity(count);
            for r in 0..count {
                let base = r * rec_len;
                let us = if rec.rate_r != 0.0 {
                    r as f64 * 1e6 / rec.rate_r * rec.time_mult
                } else {
                    f64::from(i32::from_le_bytes([dat[base + 4], dat[base + 5], dat[base + 6], dat[base + 7]])) * rec.time_mult
                };
                times.push(round_fv(us) / 1e6);
            }
            for r in 0..count {
                if cancelled() {
                    break;
                }
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
                    // BINARY folds in the unit multiplier (`0x451de0`); the
                    // binary32 reader does not read it (`0x4520f0`; whether a
                    // helper applies it is NOT DETERMINED).
                    let mult = if fmt == DataFormat::Int16 { ch.unit_mult } else { 1.0 };
                    ch.samples.push(mult * ch.a * raw + ch.b);
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
                if let Some(p) = prog {
                    p.set_remaining(dat.len() - (base + rec_len));
                }
            }
            rec.time = times;
        }
    }
    // Truncate the time base to the number of rows actually read.
    let rows = rec.analog.first().map(|c| c.samples.len()).unwrap_or(rec.time.len());
    if rec.time.len() > rows {
        rec.time.truncate(rows);
    }
    let _ = na;
    // A rate of 0 takes R from the second sample's time (`0x451b49`); then
    // the nominal track (`0x450c36`).
    if rec.rate_r == 0.0 && rec.time.get(1).is_some_and(|&t| t > 0.0) {
        rec.rate_r = int_rate(1.0 / rec.time[1], rec.line_freq);
    }
    rec.rebuild_freq_track();
    Ok(())
}

#[cfg(test)]
mod stamp_seconds_tests {
    #[test]
    fn epoch_seconds() {
        assert_eq!(super::stamp_seconds("01/01/1970", "00:00:00.0", 1999), Some(0.0));
        assert_eq!(super::stamp_seconds("02/01/1970", "00:00:01.5", 2013), Some(86_401.5), "2013: day first");
        assert_eq!(super::stamp_seconds("03/01/2000", "00:00:00", 1999), Some(951_868_800.0), "1999: month first");
    }

    #[test]
    fn moment_time() {
        let st = |date: &str, time: &str| super::Stamps { start_date: date.into(), start_time: time.into(), ..Default::default() };
        let t = |s: &super::Stamps, us| super::moment_time_text(s, 1999, us);
        let s = st("06/27/2023", "13:29:11.933000");
        // The start's 933 ms are added as 933 µs: they vanish.
        assert_eq!(t(&s, 0).as_deref(), Some("27.06.2023 13:29:11.000"));
        assert_eq!(t(&s, 200_500).as_deref(), Some("27.06.2023 13:29:11.201"));
        assert_eq!(t(&s, 49_000_000).as_deref(), Some("27.06.2023 13:30:00.000"));
        // Midnight prints the date alone; no date prints the time alone.
        assert_eq!(t(&st("01/01/2024", "00:00:00.000000"), 30_000).as_deref(), Some("01.01.2024.030"));
        assert_eq!(t(&st("", "09:05:03.5"), 0).as_deref(), Some("9:05:03.000"));
        assert_eq!(t(&st("06/27/2023", "25:00:00.0"), 0), None);
        assert_eq!(t(&st("", ""), 0), None);
    }
}

#[cfg(test)]
mod osc_stamp_tests {
    use super::*;

    /// `0x452d42`–`0x452e57`: the trigger from the BCD info block, the start
    /// (pretrig × N − 1) periods before it; month-first, unpadded hour.
    #[test]
    fn osc_stamps_from_the_info_block() {
        let mut info = vec![0u8; 128];
        info[5..12].copy_from_slice(&[0x45, 0x30, 0x20, 0x09, 0x07, 0x03, 0x24]);
        info[0xE..0x10].copy_from_slice(&2i16.to_be_bytes());
        let st = osc_stamps(&info, 8, 1000.0);
        assert_eq!((st.trigger_date.as_str(), st.trigger_time.as_str()), ("3/07/2024", "9:20:30.450"));
        assert_eq!((st.start_date.as_str(), st.start_time.as_str()), ("3/07/2024", "9:20:30.435"), "15 periods of 1 ms before");
        // No pre-trigger blocks: the start a period after the trigger.
        info[0xE..0x10].copy_from_slice(&0i16.to_be_bytes());
        assert_eq!(osc_stamps(&info, 8, 1000.0).start_time, "9:20:30.451");
        // Not a date: no stamps.
        info[10] = 0x13;
        assert_eq!(osc_stamps(&info, 8, 1000.0).trigger_date, "");
    }
}

#[cfg(test)]
mod freq_track_tests {
    use super::*;

    /// `0x454960` on the shapes it meets (`0x453db0` for X).
    #[test]
    fn nominal_track_as_read() {
        // One segment at a rate that is not a multiple of 50: X = 25·50.
        let t = nominal_track(&[(1234.0, 10)], 10, int_rate(1234.0, 50.0), 50.0, &[]);
        assert!(t.iter().all(|&v| (v - 1234.0 / 1250.0 * 50.0).abs() < 1e-12), "{t:?}");
        // Two .cfg rate lines: R = the mean snapped (2500 → 2400); the
        // sample at a segment's `end` still has that segment's value.
        let segs = [(4000.0, 1000), (1000.0, 3000)];
        let r = comtrade_rate(&[RateSection { rate: 4000.0, end_sample: 1000 }, RateSection { rate: 1000.0, end_sample: 3000 }], 50.0);
        assert_eq!(r, 2400.0);
        let t = nominal_track(&segs, 3000, r, 50.0, &[]);
        assert!((t[1000] - 4000.0 / 2400.0 * 50.0).abs() < 1e-12 && (t[1001] - 1000.0 / 2400.0 * 50.0).abs() < 1e-12);
        // A longer grid over the same segments: the old values stay below
        // the old length, the new samples past the last `end` are 0.
        let old = vec![50.0; 100];
        let t = nominal_track(&[(1000.0, 100)], 150, 1000.0, 50.0, &old);
        assert_eq!((t[100], t[101], t[149]), (50.0, 0.0, 0.0));
        // No samples: nothing changes.
        assert_eq!(nominal_track(&[(1000.0, 1)], 0, 1000.0, 50.0, &old), old);
        // The snap tables only at exactly 50 / 60 Hz.
        assert_eq!(snap_rate(2500.0, 50.0), 2400.0);
        assert_eq!(snap_rate(2500.0, 55.0), int_rate(2500.0, 55.0));
    }

    /// The readers' times (`0x451800`, `0x451be0`): ASCII rows take their
    /// own timestamps × the multiplier; BINARY records k·1e6 / R × the
    /// multiplier with the integer rate — a two-rate file is timed
    /// uniformly at the snapped mean.
    #[test]
    fn record_times_as_the_readers_build_them() {
        let head = |rates: &str, ft: &str, mult: &str| format!("S,D,1999\n1,1A,0D\n1,A,,,V,1,0,0,-32768,32767,1,1,S\n50\n{rates}01/01/2024,00:00:00.000000\n01/01/2024,00:00:00.000000\n{ft}\n{mult}\n");
        let cfg = head("1\n1000,3\n", "ASCII", "2");
        let rec = parse(&cfg, b"1,0,5\r\n2,1500,6\r\n3,4000,7\r\n\x1a\r\n").unwrap();
        assert_eq!(rec.time, vec![0.0, 0.003, 0.008], "the timestamps × 2, not the rate; a trailing ^Z line is no row");
        assert_eq!(rec.analog[0].samples.len(), 3);
        let cfg = head("2\n1000,2\n2000,4\n", "BINARY", "1");
        let mut dat = Vec::new();
        for k in 0..4i32 {
            dat.extend((k + 1).to_le_bytes());
            dat.extend((k * 777).to_le_bytes());
            dat.extend(100i16.to_le_bytes());
        }
        let rec = parse(&cfg, &dat).unwrap();
        assert_eq!(rec.rate_r, 1600.0, "snap(1500)");
        assert_eq!(rec.time, vec![0.0, 0.000625, 0.00125, 0.001875], "k·1e6/1600 µs");
    }
}

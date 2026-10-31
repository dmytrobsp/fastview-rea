//! Record-level dialogs and the parameters bar: «Параметры осциллограммы»
//! (32861, DIALOG 392, `FileInfoDlg`), «Установка настроек программы» (129,
//! DIALOG 510, `CProgSettingdDlg`), the record comment editor (32805, DIALOG
//! 332 in record mode, `CCommentDlg`) and the parameters bar (142,
//! `CParamOscBar`, id 810). Also the per-channel primary/secondary switch
//! (32867 / 32868) and the full recompute those commands trigger.

use crate::app::{App, Doc};
use eframe::egui::{self, Rect};
use fvb_core::comtrade::{self, Encoding};

/// State of the dialogs and bars of this module and `chandlg`.
#[derive(Debug, Clone)]
pub struct DialogState {
    /// The parameters bar is shown (visible at start, `0x474dbe`).
    pub param_bar: bool,
    /// `Settings Parameters/Dst folder` (default "").
    pub dst_folder: String,
    /// `Settings Parameters/UTC for Comtrade2013` (default on).
    pub utc_2013: bool,
    pub file_params: Option<FileParamsDlg>,
    pub settings: Option<SettingsDlg>,
    pub hdr_edit: Option<HdrEdit>,
    pub channels: Option<crate::chandlg::ChannelsDlg>,
    pub coef: Option<crate::chandlg::CoefDlg>,
    /// Message boxes (the original's AfxMessageBox, MB_OK) waiting for OK,
    /// shown one after another as the original's modal boxes and posted
    /// exceptions come.
    pub alerts: std::collections::VecDeque<(String, crate::dialogs::MsgIcon)>,
    /// Frames the front box has been up (a box takes keys from its second
    /// frame: the key that raised it is not also its answer).
    pub alert_frames: u32,
}

impl DialogState {
    /// Queue an MB_OK box with the exclamation icon (MFC's default for a
    /// box with no icon).
    pub fn push_alert(&mut self, msg: impl Into<String>) {
        self.push_alert_icon(msg, crate::dialogs::MsgIcon::Exclamation);
    }

    pub fn push_alert_icon(&mut self, msg: impl Into<String>, icon: crate::dialogs::MsgIcon) {
        self.alerts.push_back((msg.into(), icon));
    }

    /// The box shown now.
    #[cfg(test)]
    pub fn alert(&self) -> Option<&str> {
        self.alerts.front().map(|(m, _)| m.as_str())
    }
}

impl Default for DialogState {
    fn default() -> Self {
        DialogState {
            param_bar: true,
            dst_folder: String::new(),
            utc_2013: true,
            file_params: None,
            settings: None,
            hdr_edit: None,
            channels: None,
            coef: None,
            alerts: Default::default(),
            alert_frames: 0,
        }
    }
}

/// DIALOG 392 edits: the sample-rate combo text and the correction choice.
#[derive(Debug, Clone)]
pub struct FileParamsDlg {
    pub rate_text: String,
    pub correction: usize,
    /// A refused rate: the field takes the focus again (CDataExchange::Fail).
    pub fail: bool,
}

/// DIALOG 510 edits (applied on OK).
#[derive(Debug, Clone)]
pub struct SettingsDlg {
    pub dst_folder: String,
    pub utc: bool,
    pub encoding: Encoding,
}

/// DIALOG 332 in record mode: the `.hdr` text and its code page.
#[derive(Debug, Clone)]
pub struct HdrEdit {
    pub text: String,
}

/// `Display Encoding` codes of DIALOG 510 (radios 4005–4008, `0x422300`):
/// 0 UTF-8 (65001), 1 Windows-1251, 2 IBM866, 3 KOI8-R (20866). No auto.
pub const ENCODINGS: [(Encoding, &str); 4] =
    [(Encoding::Utf8, "UTF-8"), (Encoding::Cp1251, "Windows-1251"), (Encoding::Cp866, "IBM866"), (Encoding::Koi8r, "KOI8-R")];

pub fn encoding_code(e: Encoding) -> u8 {
    ENCODINGS.iter().position(|(x, _)| *x == e).unwrap_or(1) as u8
}

pub fn encoding_of(code: u8) -> Encoding {
    ENCODINGS.get(code as usize).map_or(Encoding::Cp1251, |(e, _)| *e)
}

/// The fixed sample-rate lists of the file parameters combo (`0x4499e0`).
pub fn rate_list(line_freq: f64) -> &'static [u32] {
    const F50: [u32; 27] = [
        160000, 80000, 40000, 32000, 20000, 19200, 16000, 10000, 9600, 8000, 6400, 5000, 4800, 4000, 3200, 2500, 2400,
        2000, 1600, 1200, 1000, 800, 600, 500, 400, 300, 200,
    ];
    const F60: [u32; 27] = [
        192000, 96000, 48000, 38400, 24000, 23040, 19200, 11520, 9600, 7680, 6000, 5760, 4800, 3840, 3340, 3000, 2880,
        2400, 1920, 1440, 1200, 960, 720, 600, 480, 360, 240,
    ];
    if line_freq == 50.0 {
        &F50
    } else if line_freq == 60.0 {
        &F60
    } else {
        &[]
    }
}

/// «Длительность»: `"%g с."` of the record duration (`0x44c0b0`).
pub fn duration_text(osc: &comtrade::Oscillogram) -> String {
    format!("{} с.", fvb_core::views::fmt_g(osc.duration()))
}

/// The bar's / dialog's start and trigger texts (`0x456e90` with 0 / 1).
pub fn stamp_texts(osc: &comtrade::Oscillogram, utc: bool) -> (String, String) {
    let s = &osc.stamps;
    (
        comtrade::format_stamp(&s.start_date, &s.start_time, osc.rev_year, &s.local_code, utc),
        comtrade::format_stamp(&s.trigger_date, &s.trigger_time, osc.rev_year, &s.local_code, utc),
    )
}

/// Code-page combo of DIALOG 392 / 332 (DLGINIT: 0 «866 MS-DOS», 1 «Windows-1251»).
const CODEPAGES: [(Encoding, &str); 2] = [(Encoding::Cp866, "866 MS-DOS"), (Encoding::Cp1251, "Windows-1251")];

/// DIALOG 392's «Кодировка» entries for a record read in `read`: the
/// original's two (the combo index is `doc+0x6e8`, `0x44a112` / `0x44a330`)
/// and, ours, the encoding of a record read in UTF-8 or KOI8-R as a third,
/// so it shows as current and a switch away can come back (the names
/// re-decoded from their bytes as read).
pub fn codepage_items(read: Encoding) -> Vec<(Encoding, &'static str)> {
    let mut v = CODEPAGES.to_vec();
    if let Some(e) = ENCODINGS.iter().find(|(e, _)| *e == read && !matches!(read, Encoding::Cp1251 | Encoding::Cp866)) {
        v.push(*e);
    }
    v
}

/// A name's bytes as read (in `read`) shown in the record code page `cp`
/// ([`Doc::set_codepage`]). Read in 1251 / 866: the bytes decoded in `cp`,
/// the pseudo-graphics best-fitted (`0x4543e0`). Read in UTF-8 / KOI8-R
/// (ours keeps the encoding, `Doc::init_record_text`): the original's
/// strings over the names as read (VM) — 1251 its ANSI, the text narrowed
/// to 1251 (`0x422300`: Windows' best fit, «√» → «v», then «?» for a
/// character with none), 866 its OEM, those
/// bytes read as 866 —, and the record's own encoding the names as read,
/// nothing best-fitted (ours: the original has no such entry).
pub fn decode_name(bytes: &[u8], read: Encoding, cp: Encoding) -> String {
    let fit = |s: String| s.chars().map(comtrade::best_fit_1251).collect::<String>();
    if matches!(read, Encoding::Cp1251 | Encoding::Cp866) {
        return fit(comtrade::decode_bytes(bytes, cp));
    }
    let as_read = comtrade::decode_bytes(bytes, read);
    let ansi = || comtrade::recode(&fit(as_read.clone()), Encoding::Cp1251, Encoding::Cp1251);
    match cp {
        _ if cp == read => as_read.clone(),
        Encoding::Cp1251 => ansi(),
        Encoding::Cp866 => fit(comtrade::recode(&ansi(), Encoding::Cp1251, Encoding::Cp866)),
        _ => fit(comtrade::decode_bytes(bytes, cp)),
    }
}

/// The bytes as read of a name shown in `from` when none are kept (a new
/// channel, a file name): its text in `from`'s code page for a record read
/// in 1251 / 866; for one read in UTF-8 / KOI8-R its text taken back to the
/// names as read ([`decode_name`]'s 866 undone to 1251) in `read`.
pub fn name_bytes_of(name: &str, read: Encoding, from: Encoding) -> Vec<u8> {
    if matches!(read, Encoding::Cp1251 | Encoding::Cp866) {
        return comtrade::encode_text(name, from);
    }
    let as_read = if from == Encoding::Cp866 { comtrade::decode_bytes(&comtrade::encode_shown(name, Encoding::Cp866), Encoding::Cp1251) } else { name.to_string() };
    comtrade::encode_text(&as_read, read)
}

/// The raw bytes of a name taken as shown now in `shown` (a pasted lane,
/// #7): for a record read in 1251 / 866 the shown text's bytes in that
/// code page, a best-fitted pseudo-graphic as the 866 byte read so
/// ([`comtrade::encode_shown`]); for one read in UTF-8 / KOI8-R the bytes
/// as read ([`name_bytes_of`]).
pub fn shown_bytes_of(name: &str, read: Encoding, shown: Encoding) -> Vec<u8> {
    if matches!(read, Encoding::Cp1251 | Encoding::Cp866) {
        return comtrade::encode_shown(name, shown);
    }
    name_bytes_of(name, read, shown)
}

/// The kept name of one added to a record (a pasted lane, a merged channel):
/// `bytes` is the name taken to the record's bytes. A name that does not
/// come back from them is kept as an alias, so it survives «Кодировка» as
/// typed (ours, #81; the original keeps the bytes only, so such letters turn
/// into «?»). What "come back" means depends on the record: read in UTF-8 /
/// KOI8-R, the bytes are the record's own encoding, so the name must decode
/// from them in that encoding (KOI8-R has no і ї є ґ; UTF-8 holds all);
/// read in 1251 / 866, the bytes are in the code page shown now, so the
/// name must decode from them there (shown in 866: no і; a name it holds
/// stays a plain name and reads back as the original's bytes do).
pub fn added_name(name: &str, bytes: Vec<u8>, read: Encoding, shown: Encoding) -> crate::app::RawName {
    let alias = if matches!(read, Encoding::Cp1251 | Encoding::Cp866) {
        decode_name(&bytes, read, shown) != name
    } else {
        // Shown in 866 the names are the OEM view of the bytes as read
        // (`decode_name`); elsewhere the name is the bytes as read.
        if shown == Encoding::Cp866 { decode_name(&bytes, read, shown) != name } else { comtrade::decode_bytes(&bytes, read) != name }
    };
    crate::app::RawName { bytes, seen: name.to_string(), alias }
}

impl Doc {
    /// 32867/32868 CMDUI (`0x42ffb0`): the radio follows channel 0.
    pub fn primary_values(&self) -> bool {
        self.chans.first().is_some_and(|c| c.primary)
    }

    /// 32867 / 32868's check (`0x42ffb0`): from the first analog channel;
    /// `None` (neither checked) with no analog channel.
    pub fn primary_shown(&self) -> Option<bool> {
        self.chans.first().map(|c| c.primary)
    }

    /// The raw bytes the code-page guess reads (`0x44f74d`: station and
    /// recorder via `0x4573d0`, then every channel name appended).
    fn name_bytes(&self) -> Vec<u8> {
        let names = [self.osc.station.as_str(), self.osc.device.as_str()]
            .into_iter()
            .chain(self.osc.analog.iter().map(|c| c.id.as_str()))
            .chain(self.osc.digital.iter().map(|c| c.id.as_str()));
        names.flat_map(|n| comtrade::encode_text(n, self.codepage)).collect()
    }

    /// After a file is read: the names are decoded with the program encoding
    /// (or the UTF-8 a `.cfg` fixes itself, `comtrade::cfg_utf8`). The
    /// original's record code page is OEM / ANSI (`doc+0x6e8`, 0 / 1: «866
    /// MS-DOS» / «Windows-1251»; ANSI the names as read, OEM their ANSI
    /// bytes read as 866), guessed from the names at every open (`0x44f5b0`:
    /// `0x459e60` at `0x44f7eb`, applied by `0x4543e0`), the `.set`'s
    /// `Oscillogram/Codepage` winning (`0x44f808`). Ours guesses for a
    /// record read in 1251 or 866; one read in UTF-8 or KOI8-R keeps that
    /// encoding — its real one, which «Кодировка» shows and can pick back
    /// (`codepage_items`; the original's guess would say ANSI, the names as
    /// read, for Cyrillic names). The `.hdr` text comes from the `.cff`
    /// section or the sibling `.hdr` file.
    pub fn init_record_text(&mut self, program: Encoding) {
        let enc = self.osc.text_enc.unwrap_or(program);
        self.codepage = enc;
        self.read_enc = enc;
        if matches!(enc, Encoding::Cp1251 | Encoding::Cp866) {
            let cp = comtrade::detect_codepage(&self.name_bytes());
            self.set_codepage(cp);
        }
        let raw: Option<Vec<u8>> = match &self.cff.hdr {
            Some(t) => Some(comtrade::encode_text(t, enc)),
            None => self.path.as_ref().and_then(|p| {
                ["hdr", "HDR", "Hdr"].iter().find_map(|e| std::fs::read(p.with_extension(e)).ok())
            }),
        };
        if let Some(b) = raw {
            let guess = if matches!(program, Encoding::Cp1251 | Encoding::Cp866) { comtrade::detect_codepage(&b) } else { program };
            // A UTF-8 record's `.hdr` (a 2013 export's header text) is read
            // as UTF-8 when it is valid UTF-8 — kept byte for byte on save.
            let utf8 = self.osc.text_enc == Some(Encoding::Utf8) && std::str::from_utf8(&b).is_ok();
            self.hdr_cp = if utf8 { Encoding::Utf8 } else { guess };
            self.hdr = Some(comtrade::decode_bytes(&b, self.hdr_cp));
            // The reader keeps the `.hdr` bytes (`doc+0x1e0`, written back
            // on every save) and stores their guessed code page as
            // `Comments/Codepage` (`0x450d1f`…`0x450d63`) — the guess
            // under the program encoding, as the original's.
            self.set.tree.section_mut("Comments").set_int("Codepage", crate::setfile::cp_code(guess));
        }
    }

    /// «Кодировка» of DIALOG 392 (`0x4543e0`): re-decode the station, the
    /// recorder and every channel name in the new code page — from the
    /// bytes as read, so 1251 → 866 → 1251 gives the names back (VM) —
    /// into the original's ANSI strings (`comtrade::best_fit_1251`: a
    /// Cyrillic word read as 866 shows box-drawing characters, narrowed to ASCII). A renamed channel keeps its name: the
    /// shown name is the alias while there is one, else the file's name
    /// decoded (`0x425960`). Ours alone: whatever named a channel before
    /// names it after (formulas, quick-analysis picks, levels and areas,
    /// pairs); the names changed are returned (old, new).
    pub fn set_codepage(&mut self, cp: Encoding) -> Vec<(String, String)> {
        use crate::app::ChanKey;
        let from = self.codepage;
        if from == cp {
            return Vec::new();
        }
        let read = self.read_enc;
        let recs: Vec<_> = self.comments.iter().map(|c| self.comment_rec(c)).collect();
        // Every channel's name before (the formulas' resolver order: analog,
        // computed, then digital, `OscChannels`; then every row for the rest).
        let formula_names = |d: &Doc| -> Vec<String> {
            let trim = |s: &String| s.trim_end_matches(' ').to_string();
            d.osc.analog.iter().map(|c| trim(&c.id)).chain(d.computed.iter().map(|c| c.name.clone())).chain(d.osc.digital.iter().map(|c| trim(&c.id))).collect()
        };
        let row_names = |d: &Doc| -> Vec<String> { crate::chandlg::rows(d).into_iter().map(|k| crate::chandlg::row_name(d, k)).collect() };
        let (old_f, old_rows) = (formula_names(self), row_names(self));
        // The file's names in `set.keys` are 1251 text: they stand for the
        // bytes only while the names were read in a single-byte code page —
        // or always for a record read in UTF-8 / KOI8-R, whose keys are its
        // bytes as read (1251 maps every byte) and whose file names
        // `decode_name` gives in any code page.
        let single = matches!(from, Encoding::Cp1251 | Encoding::Cp866) || !matches!(self.read_enc, Encoding::Cp1251 | Encoding::Cp866);
        let na = self.osc.analog.len();
        let file: Vec<Option<String>> = [None, None]
            .into_iter()
            .chain((0..na).map(|i| self.original_name(ChanKey::Analog(i)).filter(|_| single)))
            .chain((0..self.osc.digital.len()).map(|k| self.original_name(ChanKey::Digital(k)).filter(|_| single)))
            .collect();
        let mut names: Vec<String> = [self.osc.station.clone(), self.osc.device.clone()]
            .into_iter()
            .chain(self.osc.analog.iter().map(|c| c.id.clone()))
            .chain(self.osc.digital.iter().map(|c| c.id.clone()))
            .collect();
        // Each name as read, in step with the channels (`move_appended_keys`,
        // `retain_digital_keys`, undo): an alias is kept as typed, with its
        // bytes (`0x425960`); a name still the text last decoded from its
        // bytes is decoded again from them (a name changed otherwise is
        // kept as well). Out of step, a name takes the first entry with its
        // text not yet taken, in order (two names can decode alike: «В» and
        // «С» read as 866 both show «T»).
        let raw = std::mem::take(&mut self.name_raw);
        let aligned = raw.len() == names.len();
        let mut used = vec![false; raw.len()];
        // (bytes, decode again, alias)
        let mut out: Vec<Option<(Vec<u8>, bool, bool)>> = vec![None; names.len()];
        for (p, n) in names.iter().enumerate() {
            if aligned {
                let r = &raw[p];
                let redo = !r.alias && r.seen == *n;
                out[p] = Some((r.bytes.clone(), redo, !redo));
            }
        }
        for (p, n) in names.iter().enumerate() {
            if out[p].is_some() {
                continue;
            }
            out[p] = Some(if let Some(j) = (0..raw.len()).find(|&j| !used[j] && !raw[j].alias && raw[j].seen == *n) {
                used[j] = true;
                (raw[j].bytes.clone(), true, false)
            } else if let Some(f) = &file[p] {
                // No bytes kept: the keys were taken from names read
                // losslessly, and a name not the file's is an alias. A
                // record read in UTF-8 / KOI8-R has its key's bytes exact
                // (the file name as shown may be best-fitted).
                let bytes = if matches!(read, Encoding::Cp1251 | Encoding::Cp866) { name_bytes_of(f, read, from) } else { comtrade::encode_text(&self.file_key(p - 2), Encoding::Cp1251) };
                (bytes, false, true)
            } else {
                // As every site that makes a raw name from a shown one: a
                // best-fitted pseudo-graphic is the 866 byte it was read as.
                let r = added_name(n, shown_bytes_of(n, read, from), read, from);
                (r.bytes, !r.alias, r.alias)
            });
        }
        let decode = |b: &[u8]| -> String { decode_name(b, read, cp) };
        let mut kept = Vec::with_capacity(names.len());
        for (n, o) in names.iter_mut().zip(out) {
            let (bytes, redecode, alias) = o.unwrap_or_default();
            let seen = decode(&bytes);
            if redecode {
                n.clone_from(&seen);
            }
            kept.push(crate::app::RawName { bytes, seen, alias });
        }
        // The ОМП's flags are computed channels (bit 13): their shown name is
        // the stored one, never re-decoded (`0x425960`, #66).
        let flags: Vec<(usize, String)> = (0..self.osc.digital.len()).filter(|&k| self.dig(k).omp).map(|k| (k, self.osc.digital[k].id.clone())).collect();
        let mut it = names.into_iter();
        self.osc.station = it.next().unwrap_or_default();
        self.osc.device = it.next().unwrap_or_default();
        for c in &mut self.osc.analog {
            c.id = it.next().unwrap_or_default();
        }
        for c in &mut self.osc.digital {
            c.id = it.next().unwrap_or_default();
        }
        for (k, name) in flags {
            let p = 2 + na + k;
            if let Some(r) = kept.get_mut(p) {
                *r = crate::app::RawName { bytes: name_bytes_of(&name, read, cp), seen: name.clone(), alias: true };
            }
            self.osc.digital[k].id = name;
        }
        self.name_raw = kept;
        // A pasted lane (bit 14) is re-decoded too — `0x425960` spares only
        // the computed ones (bit 13): VM, a pasted lane's name read as
        // 866 lists its best-fit form with the suffix re-decoded. Its bytes go with the channel; a
        // renamed one keeps its name.
        let pasted: Vec<usize> = (0..self.computed.len()).filter(|&j| self.pending_of(j).is_some()).collect();
        for j in pasted {
            let c = &mut self.computed[j];
            let (bytes, redecode) = match c.name_raw.take() {
                Some(r) => {
                    let redo = !r.alias && r.seen == c.name;
                    (r.bytes, redo)
                }
                None => {
                    let r = added_name(&c.name, shown_bytes_of(&c.name, read, from), read, from);
                    (r.bytes, !r.alias)
                }
            };
            let seen = decode(&bytes);
            if redecode {
                c.name.clone_from(&seen);
            }
            c.name_raw = Some(crate::app::RawName { bytes, seen, alias: !redecode });
            let new = c.name.clone();
            if let Some(p) = self.pending_of_mut(j) {
                match &mut p.pasted.channel {
                    Ok(a) => a.id = new,
                    Err(d) => d.id = new,
                }
            }
        }
        self.codepage = cp;
        // Ours alone (#7, the maintainer's call): what named a channel
        // before the switch names it after — the formulas (the quick
        // analysis's among them), the quick-analysis pages' picks, the
        // levels and areas, the diagram pairs. The original keeps the old
        // names, so a formula on them fails («Имя канала задано
        // неправильно.», the whole batch with it) and the levels no longer
        // draw (L9.227).
        let (new_f, new_rows) = (formula_names(self), row_names(self));
        // A name maps only while it stands for one channel before and one
        // after: names that decode alike («В» and «С» read as 866 both show
        // «T») are left as they were, so a reference fails as in the
        // original rather than landing on the wrong channel.
        let once = |v: &[String], n: &str| v.iter().filter(|x| *x == n).count() == 1;
        let renamed: Vec<(String, String)> =
            old_rows.iter().zip(&new_rows).filter(|(o, n)| o != n && once(&old_rows, o) && once(&new_rows, n)).map(|(o, n)| (o.clone(), n.clone())).collect();
        let by_row = |name: &str| renamed.iter().find(|(o, _)| o == name).map(|(_, n)| n.clone());
        if old_f.len() == new_f.len() {
            // The channel the formula resolved (`OscChannels::resolve`:
            // exact, else the first case-insensitive match), written as a
            // name that resolves to it alone.
            let by_ref = |name: &str| {
                let i = old_f.iter().position(|o| o == name).or_else(|| old_f.iter().position(|o| o.to_lowercase() == name.to_lowercase()))?;
                let unique = new_f.iter().filter(|n| n.to_lowercase() == new_f[i].to_lowercase()).count() == 1;
                (old_f[i] != new_f[i] && unique).then(|| new_f[i].clone())
            };
            for c in self.computed.iter_mut().filter(|c| crate::setfile::is_formula(c)) {
                c.formula = fvb_core::expr::rename_channel_refs(&c.formula, &by_ref);
            }
        }
        for p in self.dif_pairs.iter_mut().chain(self.rx_pairs.iter_mut()) {
            for n in [&mut p.a, &mut p.b] {
                if let Some(new) = by_row(n) {
                    *n = new;
                }
            }
        }
        for st in &mut self.omp.params.stations {
            for n in [&mut st.ia, &mut st.ib, &mut st.ic, &mut st.uab, &mut st.ubc, &mut st.uca, &mut st.ua, &mut st.ub, &mut st.uc] {
                if let Some(new) = by_row(n) {
                    *n = new;
                }
            }
        }
        // The pages' channel picks only: a combo's `channelName` (536–538,
        // 543) and an arm's `IA` / `IB` / `IC` (539's `Leg_%d`).
        for k in &crate::quick_ui::PAGE_KEYS[..5] {
            let Some(sec) = self.set.tree.sections.get_mut(*k) else { continue };
            for (name, node) in &mut sec.children {
                let keys: &[&str] = if name.starts_with("Leg_") { &["IA", "IB", "IC"] } else { &["channelName"] };
                for key in keys {
                    if let Some(new) = node.attrs.get(*key).and_then(|v| by_row(v)) {
                        node.attrs.insert((*key).to_string(), new);
                    }
                }
            }
        }
        // A level or area stays bound to its channel; one kept unbound
        // (from a `.set`, or a rename) is tried again with the new names.
        let comments = std::mem::take(&mut self.comments);
        self.comments = comments
            .into_iter()
            .zip(recs)
            .map(|(c, mut r)| {
                if !matches!(c.kind, crate::comments::CommentKind::Kept(_)) {
                    return c;
                }
                use fvb_core::recset::CommentKindRec as K;
                match &mut r.kind {
                    K::Level { channel, .. } => *channel = by_row(channel).unwrap_or(channel.clone()),
                    K::Area { channel_from, channel_to, .. } => {
                        *channel_from = by_row(channel_from).unwrap_or(channel_from.clone());
                        *channel_to = by_row(channel_to).unwrap_or(channel_to.clone());
                    }
                    K::Moment { .. } => {}
                }
                self.comment_of(&r)
            })
            .collect();
        renamed
    }

    /// The `.cff` text sections to write, with the edited record comment.
    pub fn cff_sections(&self) -> comtrade::CffSections {
        let mut s = self.cff.clone();
        if self.hdr.is_some() {
            // Without a `.hdr`'s BOM: it is not part of the section's text.
            s.hdr = Some(self.hdr_text());
        }
        s
    }

    /// The record comment as text for others (the clipboard's `hdrFile`, a
    /// merged header): a UTF-8 `.hdr`'s BOM, kept for the save, left out.
    pub fn hdr_text(&self) -> String {
        self.hdr.as_deref().unwrap_or_default().trim_start_matches('\u{FEFF}').to_string()
    }

    /// The `.hdr` file to write beside a saved `.cfg` (CRLF lines, in the
    /// comment's code page), if the record has a comment.
    pub fn hdr_file_bytes(&self) -> Option<Vec<u8>> {
        let t = self.hdr.as_ref().filter(|t| !t.is_empty())?;
        Some(comtrade::encode_text(&t.replace("\r\n", "\n").replace('\n', "\r\n"), self.hdr_cp))
    }

    /// Recompute every derived series after a coefficient, primary/secondary
    /// or sample-rate change (`0x42e540`): phasors (keeping the channel
    /// roles), one-period RMS and DC, and the formula channels.
    /// A channel was added or removed: the original rebuilds its frequency
    /// candidates (`0x42ebd0`), which turns «Не корректировать» back into the
    /// first candidate whenever there is one (L9.65).
    pub fn channels_changed(&mut self) {
        self.recolor();
        // Formula channels (after the analog ones) and pasted analog ones
        // (at their insertion point) are candidates too, by the same test
        // (`0x42aee0`: ph / ccbm «system» or a frequency unit — no bit-14
        // test); they keep their vector places.
        let n_analog = self.osc.analog.len();
        let mut extra = Vec::new();
        for (j, c) in self.computed.iter().enumerate() {
            let pos = match c.before {
                Some(crate::app::ChanKey::Analog(a)) => a,
                _ => n_analog,
            };
            if c.formula == crate::merge_ui::PENDING {
                if let Some(Ok(a)) = self.pending_of(j).map(|p| &p.pasted.channel) {
                    if a.phase == "system" || a.ccbm == "system" || fvb_core::freqcorr::is_freq_unit(&a.unit) {
                        extra.push(fvb_core::comtrade::ExtraFreq { name: c.name.clone(), samples: a.samples.clone(), pos, id: j });
                    }
                }
            } else if fvb_core::freqcorr::is_freq_unit(&c.unit) {
                extra.push(fvb_core::comtrade::ExtraFreq { name: c.name.clone(), samples: c.samples.clone(), pos: n_analog, id: j });
            }
        }
        self.osc.extra_freq = extra;
        // The rebuild (`0x42ebd0`) refreshes no cache (L9.103): a changed
        // selection reaches the statistics panel and the frequency diagram
        // at once (they read it live) but the cached series — phasors, RMS
        // lanes, formula channels — only at the next full recompute.
        fvb_core::freqcorr::refresh_selection(&mut self.osc);
    }

    /// The correction for computed channel `j`: none for the frequency
    /// channel itself (the gate compares the channel, `0x4262b6`).
    pub fn computed_corr(&self, j: usize) -> Option<fvb_core::freqcorr::FreqCorr<'_>> {
        fvb_core::freqcorr::of(&self.osc).filter(|_| fvb_core::freqcorr::selected_extra(&self.osc) != Some(j)).map(|c| c.at_rate(self.computed.get(j).map_or(0.0, |c| c.own_rate)))
    }

    /// Formula channel `j`'s window (`0x428490`): its own rate ÷ 50 when it has one.
    pub fn computed_period(&self, j: usize) -> usize {
        self.osc.period_of(self.computed.get(j).map_or(0.0, |c| c.own_rate)).max(2)
    }

    pub fn recompute_signals(&mut self) {
        let roles = (self.analysis.u_abc, self.analysis.i_abc, self.analysis.calc_ib);
        let primary = self.primary_values();
        self.recompute(primary);
        (self.analysis.u_abc, self.analysis.i_abc, self.analysis.calc_ib) = roles;
    }
}

#[cfg(test)]
thread_local! {
    /// Full recomputes run on this thread (tests count them).
    pub static RECOMPUTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl App {
    /// A full recompute of document `di`; ОМП is re-run silently when its
    /// result is on this document (`doc+0x690`).
    pub fn recompute_doc(&mut self, di: usize) {
        #[cfg(test)]
        RECOMPUTES.with(|c| c.set(c.get() + 1));
        let Some(d) = self.docs.get_mut(di) else { return };
        d.recompute_signals();
        if self.docs[di].omp.shown {
            let (active, status) = (self.active, self.status.clone());
            self.active = Some(di);
            self.run_omp();
            self.active = active;
            self.status = status;
        }
    }

    /// 32867 / 32868 (`0x42fdb0`): `ch+0x89` := 1 / 0 on every file channel,
    /// then the full recompute.
    pub fn set_primary_values(&mut self, primary: bool) {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else { return };
        for c in self.docs[ai].chans.iter_mut() {
            c.primary = primary;
        }
        self.recompute_doc(ai);
    }

    /// `Settings Parameters`, read at start-up (`0x448c00`).
    pub fn load_program_settings(&mut self) {
        let s = &self.store.settings;
        if let Some(v) = s.get("Dst folder") {
            self.dlg.dst_folder = v.clone();
        }
        if let Some(v) = s.get("UTC for Comtrade2013") {
            self.dlg.utc_2013 = v != "0";
        }
        if let Some(c) = s.get("Display Encoding").and_then(|v| v.parse::<u8>().ok()) {
            self.encoding = encoding_of(c);
        }
    }

    /// Write `Settings Parameters` (the original at exit, `0x448da0`; we on OK).
    fn save_program_settings(&mut self) {
        if !self.persist {
            return;
        }
        let s = &mut self.store.settings;
        s.insert("Dst folder".into(), self.dlg.dst_folder.clone());
        s.insert("UTC for Comtrade2013".into(), u8::from(self.dlg.utc_2013).to_string());
        s.insert("Display Encoding".into(), encoding_code(self.encoding).to_string());
        self.store.save();
    }

    /// The record comment editor is enabled for `.cfg` / `.cff` records only
    /// (CMDUI `0x44c5e0`).
    pub fn hdr_comment_enabled(&self) -> bool {
        self.active_doc().and_then(|d| d.path.as_ref()).and_then(|p| p.extension()).and_then(|e| e.to_str()).is_some_and(|e| {
            e.eq_ignore_ascii_case("cfg") || e.eq_ignore_ascii_case("cff")
        })
    }

    /// 57603 «Сохранить» (`0x475750`): greyed for a `.sto` record (its path
    /// lowercased ends in ".sto") and with no document.
    pub fn can_save(&self) -> bool {
        self.active_doc().is_some_and(|d| !d.path.as_ref().is_some_and(|p| p.to_string_lossy().to_lowercase().ends_with(".sto")))
    }

    /// 32800 / 32801 / 32802 / 32803's states (`0x45c8c0`): YAnalogSize
    /// below 500 / above 70, `f` below 1.0 / above the fit scale; all
    /// greyed with no document.
    pub fn zoom_enabled(&self) -> [bool; 4] {
        let Some(d) = self.active_doc() else { return [false; 4] };
        let y = self.y_analog.round() as i32;
        let [grow_x, shrink_x] = d.zoom_x_enabled(self.view_width);
        [y < 500, y > 70, grow_x, shrink_x]
    }

    /// A dialog's default button and Cancel (IsDialogMessage): Enter = OK,
    /// Esc = Cancel — not while a message box is up (it takes them).
    pub fn dialog_keys(&self, ctx: &egui::Context) -> Option<bool> {
        if !self.dlg.alerts.is_empty() {
            return None;
        }
        ctx.input(|i| if i.key_pressed(egui::Key::Enter) { Some(true) } else if i.key_pressed(egui::Key::Escape) { Some(false) } else { None })
    }

    pub fn open_hdr_comment(&mut self) {
        if !self.hdr_comment_enabled() {
            return;
        }
        // 32805 (`0x44c2e0`): the `.set`'s `Comments/UserData` — not the
        // `.hdr` text —, the node made if missing (kept on Cancel too).
        if let Some(d) = self.active_doc_mut() {
            let text = d.set.tree.section_mut("Comments").get_str("UserData", "");
            self.dlg.hdr_edit = Some(HdrEdit { text });
        }
    }

    /// Everything of this module and `chandlg`, called from `dialogs_ui`.
    pub fn record_dialogs_ui(&mut self, ctx: &egui::Context) {
        self.file_params_window(ctx);
        self.settings_window(ctx);
        self.hdr_comment_window(ctx);
        self.channels_window(ctx);
        self.coef_window(ctx);
        self.alert_window(ctx);
    }

    fn alert_window(&mut self, ctx: &egui::Context) {
        let Some((msg, icon)) = self.dlg.alerts.front().cloned() else { return };
        let mut ok = false;
        crate::dialogs::modal_backdrop(ctx, "fv_alert_block", egui::Order::Middle);
        crate::dialogs::dlg(ctx, crate::app::APP_TITLE)
            .id(egui::Id::new("fv_alert"))
            .margin(0.0)
                .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ok = crate::dialogs::message_box(ui, icon, &msg, &["OK"]).is_some();
            });
        // The box takes Enter / Esc: consumed, so the dialog under it does
        // not see them too.
        let key = self.dlg.alert_frames > 0 && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) | i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if ok || key {
            self.dlg.alerts.pop_front();
            self.dlg.alert_frames = 0;
        } else {
            self.dlg.alert_frames += 1;
        }
    }

    /// DIALOG 392 «Параметры осциллограммы»: «Закрыть» is IDOK and applies
    /// the sample rate and the correction choice (`0x449f40`); Esc or the
    /// close box cancel them. The code page applies at once (`0x44a330`).
    fn file_params_window(&mut self, ctx: &egui::Context) {
        if !self.show_file_params {
            return;
        }
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else {
            self.show_file_params = false;
            return;
        };
        let utc = self.dlg.utc_2013;
        let mut st = self.dlg.file_params.take().unwrap_or_else(|| FileParamsDlg {
            rate_text: format!("{}", self.docs[ai].osc.rate_r),
            // CB_SETCURSEL `rec+0x874` (0 with no candidate, `0x449efd`).
            correction: if fvb_core::freqcorr::candidate_names(&self.docs[ai].osc).is_empty() { 0 } else { self.docs[ai].osc.freq_sel.max(0) as usize },
            fail: false,
        });
        let mut open = true;
        let mut apply = false;
        let mut cp_pick: Option<Encoding> = None;
        crate::dialogs::dlg(ctx, "Параметры осциллограммы")
            .margin(0.0)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                use crate::dialogs::{dlu, win_label};
                let d = &self.docs[ai];
                let o = &d.osc;
                let (start, trig) = stamp_texts(o, utc);
                // The 347 × 277 DLU template (521 × 450 px): right-aligned
                // labels at x 12–13, the values as static text at 120–121.
                let (client, _) = ui.allocate_exact_size(crate::dialogs::dlu_size(347, 277), egui::Sense::hover());
                let rows: [(i32, i32, &str, Option<String>); 10] = [
                    (12, 12, "Число каналов:", Some((o.analog.len() + o.digital.len()).to_string())),
                    (12, 30, "Число аналоговых каналов:", Some(o.analog.len().to_string())),
                    (12, 48, "Число дискретных каналов:", Some(o.digital.len().to_string())),
                    (12, 66, "Число измерений:", Some(o.sample_count().to_string())),
                    // DDX_Text of a double shows `%.*g` at 15 digits.
                    (12, 84, "Частота сети:", Some(fvb_core::views::fmt_gp(o.line_freq, 15))),
                    (13, 145, "Станция:", Some(o.station.clone())),
                    (13, 162, "Номер регистратора:", Some(o.device.clone())),
                    (13, 181, "Начало записи:", Some(start)),
                    (13, 199, "Момент запуска:", Some(trig)),
                    (13, 217, "Длительность:", Some(duration_text(o))),
                ];
                // The controls in the template's order, the tab order:
                // «Закрыть» first (the initial focus), the rate combo no tab
                // stop, then «Кодировка» and the correction combo (VM).
                apply = crate::dialogs::win_button(ui, dlu(client, 258, 256, 80, 14), "Закрыть", true).clicked();
                for (x, y, label, value) in rows {
                    win_label(ui, dlu(client, x, y, 96, 9), label, true, true);
                    if let Some(v) = value {
                        win_label(ui, dlu(client, if x == 12 { 120 } else { 121 }, y, 216, 9), &v, false, true);
                    }
                }
                win_label(ui, dlu(client, 13, 102, 96, 9), "Частота дискретизации:", true, true);
                let focus = st.fail && self.dlg.alerts.is_empty();
                if focus {
                    st.fail = false;
                }
                let rates: Vec<String> = rate_list(o.line_freq).iter().map(|r| r.to_string()).collect();
                crate::dialogs::no_tab_stop(ui, |ui| crate::dialogs::win_combo_edit(ui, dlu(client, 120, 102, 80, 12), egui::Id::new("fv_rate"), &mut st.rate_text, &rates, focus));
                win_label(ui, dlu(client, 12, 234, 96, 9), "Кодировка:", true, true);
                let items = codepage_items(d.read_enc);
                let cps: Vec<String> = items.iter().map(|(_, l)| l.to_string()).collect();
                // A stored value the combo has no entry for shows empty
                // (`SetCurSel` out of range, `0x44a112`).
                let cur = if d.cp_raw.is_some() { items.len() } else { items.iter().position(|(e, _)| *e == d.codepage).unwrap_or(0) };
                if let Some(k) = crate::dialogs::win_combo_list(ui, dlu(client, 121, 233, 80, 70), egui::Id::new("fv_codepage"), &cps, cur, true) {
                    cp_pick = Some(items[k].0);
                }
                win_label(ui, dlu(client, 12, 123, 96, 17), "Частота для коррекции расчётных значений:", true, true);
                // «Не корректировать», then each frequency candidate by
                // name (`0x4499e0`); disabled when there is none.
                let items: Vec<String> = std::iter::once("Не корректировать".to_string())
                    .chain(fvb_core::freqcorr::candidate_names(o))
                    .collect();
                if let Some(k) = crate::dialogs::win_combo_list(ui, dlu(client, 119, 125, 80, 70), egui::Id::new("fv_corr"), &items, st.correction, items.len() > 1) {
                    st.correction = k;
                }
            });
        if let Some(cp) = cp_pick {
            self.docs[ai].cp_raw = None;
            // The vector diagram's angle reference follows its channel too.
            for (old, new) in self.docs[ai].set_codepage(cp) {
                if self.vec_angle_ref.as_deref() == Some(old.as_str()) {
                    self.vec_angle_ref = Some(new);
                }
            }
        }
        // «Закрыть» is IDOK: Enter applies, Esc cancels.
        let keys = self.dialog_keys(ctx);
        let esc = keys == Some(false);
        let apply = apply || keys == Some(true);
        if apply {
            if let Err(fail) = self.apply_file_rate(ai, &st.rate_text) {
                st.fail = fail;
                self.dlg.file_params = Some(st);
                return;
            }
            // `0x44a2c9`: a new correction choice is stored, the document
            // marked modified and every channel recomputed.
            let d = &mut self.docs[ai];
            if st.correction as i32 != d.osc.freq_sel {
                d.osc.freq_sel = st.correction as i32;
                d.set.modified = true;
                d.recompute_signals();
            }
            // «Закрыть» (`0x449f40`) ends in the refresh-all (`0x44a294`, `0x44a2ed`).
            self.refresh_blocks();
            self.show_file_params = false;
        } else if !open || esc {
            self.show_file_params = false;
        } else {
            self.dlg.file_params = Some(st);
        }
    }

    /// 392's sample rate on «Закрыть». 0x415 goes through DDX_Text of an
    /// int (mfc #3761, `0x44a230`: scanf "%d", L9.148) with no DDV, so 0
    /// and negatives are taken. A new R (`0x44a24a`): the undo snapshot, R
    /// stored and the nominal track (`0x42e540`); then each channel's
    /// recompute, which throws the format error when R div f is 0
    /// (`0x4285b9`) — after the snapshot and the store, before anything
    /// else: no recompute, ОМП or modified mark. Otherwise the full
    /// recompute, the silent ОМП run (always) and the modified mark.
    /// `Err(refocus)`: the dialog stays (UpdateData failed); `true` puts
    /// the caret back in the field (the int parse).
    pub fn apply_file_rate(&mut self, ai: usize, text: &str) -> Result<(), bool> {
        let Some(rate) = crate::dialogs::ddx_int(text) else {
            // AFX_IDP_PARSE_INT.
            self.dlg.push_alert("Введите целое число.");
            return Err(true);
        };
        let rate = f64::from(rate);
        if rate == self.docs[ai].osc.rate_r {
            return Ok(());
        }
        let d = &mut self.docs[ai];
        let snap = App::snapshot_of_doc(d);
        App::push_undo(d, snap);
        d.osc.set_rate_r(rate);
        if fvb_core::signal::samples_per_period(rate, d.osc.line_freq) == Some(0) {
            // The exception's text with an empty detail: no file name.
            self.dlg.push_alert(fvb_core::comtrade::ComtradeError::Format.to_string());
            return Err(false);
        }
        d.recompute_signals();
        d.set.modified = true;
        self.run_omp_silent(ai);
        Ok(())
    }

    /// DIALOG 510 «Установка настроек программы»: OK stores and persists; the
    /// encoding applies to the next file opened (nothing is re-read).
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut st = self.dlg.settings.take().unwrap_or_else(|| SettingsDlg {
            dst_folder: self.dlg.dst_folder.clone(),
            utc: self.dlg.utc_2013,
            encoding: self.encoding,
        });
        let mut open = true;
        let mut result: Option<bool> = None;
        crate::dialogs::dlg(ctx, "Установка настроек программы")
            .margin(0.0)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                use crate::dialogs::{dlu, win_button, win_group, win_label, win_radio};
                // The 419 × 259 DLU template (629 × 421 px).
                let (client, _) = ui.allocate_exact_size(crate::dialogs::dlu_size(419, 259), egui::Sense::hover());
                // The template's order is the tab order: OK, Cancel, the
                // link, «...», then each group's checked radio (an auto radio
                // button takes WS_TABSTOP while checked; VM: Tab after «...»
                // lands on «Отображать в UTC»).
                if win_button(ui, dlu(client, 301, 238, 50, 14), "OK", true).clicked() {
                    result = Some(true);
                }
                if win_button(ui, dlu(client, 362, 238, 50, 14), "Cancel", false).clicked() {
                    result = Some(false);
                }
                win_label(ui, dlu(client, 7, 16, 405, 8), "Директория хранения файлов описания осциллограмм (*.dst):", false, true);
                win_group(ui, dlu(client, 7, 28, 405, 30), "");
                // The folder as an MFC link (`0x47f840`): its text and URL,
                // disabled — and empty — while no folder is set; a click
                // opens the folder.
                let link = dlu(client, 16, 38, 386, 15);
                if !st.dst_folder.is_empty() {
                    let resp = ui.interact(link, ui.id().with("fv_dst_link"), egui::Sense::click());
                    let focused = crate::dialogs::tab_stop(ui, resp.id, crate::dialogs::StopKind::Button);
                    let t = ui.painter().with_clip_rect(link).text(link.left_center(), egui::Align2::LEFT_CENTER, &st.dst_folder, crate::dialogs::dlg_font(), egui::Color32::from_rgb(0, 0, 255));
                    if focused && crate::dialogs::cues(ui) {
                        crate::dialogs::focus_rect(ui.painter(), t.expand(1.0));
                    }
                    if resp.hovered() {
                        ui.painter().hline(t.x_range(), t.bottom() - 0.5, egui::Stroke::new(1.0f32, egui::Color32::from_rgb(0, 0, 255)));
                    }
                    if resp.clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(format!("file:///{}", st.dst_folder.replace('\\', "/"))));
                    }
                }
                // «...» (`0x47f950`): the folder browser starts at the current one.
                if win_button(ui, dlu(client, 6, 60, 50, 14), "...", false).clicked() {
                    let mut fd = crate::dialogs::file_dialog().set_title("Директория хранения файлов описания осциллограмм");
                    if !st.dst_folder.is_empty() {
                        fd = fd.set_directory(&st.dst_folder);
                    }
                    if let Some(p) = fd.pick_folder() {
                        st.dst_folder = p.display().to_string();
                    }
                }
                win_group(ui, dlu(client, 7, 86, 405, 51), "Настройки отображения времени для Comtrade 2013");
                if win_radio(ui, dlu(client, 23, 104, 80, 10), "Отображать в UTC", st.utc) {
                    st.utc = true;
                }
                if win_radio(ui, dlu(client, 23, 120, 110, 10), "Отображать местное время", !st.utc) {
                    st.utc = false;
                }
                win_group(ui, dlu(client, 7, 148, 405, 83), "Отображать согласно кодировке:");
                for ((e, l), (y, w)) in ENCODINGS.into_iter().zip([(164, 35), (180, 63), (197, 40), (213, 39)]) {
                    if win_radio(ui, dlu(client, 23, y, w, 10), l, st.encoding == e) {
                        st.encoding = e;
                    }
                }
                win_label(ui, dlu(client, 135, 176, 211, 22), "Внимание! При смене кодировки изменения вступают в силу после переоткрытия файла осциллограммы!", false, true);
            });
        let result = result.or_else(|| self.dialog_keys(ctx));
        match result {
            Some(true) => {
                self.dlg.dst_folder = st.dst_folder;
                self.dlg.utc_2013 = st.utc;
                self.encoding = st.encoding;
                self.save_program_settings();
                self.show_settings = false;
            }
            Some(false) => self.show_settings = false,
            None if open => self.dlg.settings = Some(st),
            None => self.show_settings = false,
        }
    }

    /// DIALOG 332 in record mode (32805, `0x44c2e0`): the code-page combo
    /// re-encodes the text being edited (`0x43e410`); OK stores text and code
    /// page (modified only when the code page changed).
    fn hdr_comment_window(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.dlg.hdr_edit.take() else { return };
        let mut open = true;
        let mut result: Option<bool> = None;
        crate::dialogs::dlg(ctx, "Комментарии к диаграмме")
            .id(egui::Id::new("fv_hdr_comment"))
            .margin(0.0)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                // The code-page combo stays hidden: the dialog's mode
                // (`+0x9c`) is always 0 (`0x43e110`, `0x43e380`).
                result = crate::dialogs::comment_dialog_body(ui, egui::Id::new("fv_hdr_comment_text"), &mut st.text);
            });
        match result {
            // OK: CRs removed, `UserData` written, modified every time.
            Some(true) => {
                if let Some(d) = self.active_doc_mut() {
                    d.set.tree.section_mut("Comments").set_str("UserData", &st.text.replace('\r', ""));
                    d.set.modified = true;
                }
            }
            Some(false) => {}
            None if open => self.dlg.hdr_edit = Some(st),
            None => {}
        }
    }

    /// The parameters bar (`CParamOscBar::OnDrawText` `0x47ed30`): labels
    /// right-aligned, values left-aligned, in three rows of the 75 px strip;
    /// only the labels without a document.
    pub fn param_bar_ui(&mut self, ui: &mut egui::Ui) {
        let pal = self.skin.palette();
        let rect = ui.max_rect();
        ui.allocate_rect(rect, egui::Sense::hover());
        // The bar's font is the system's 9 pt one: at > 100 % its rows (the
        // text height + 5) and columns (the labels' widths) grow, the bar
        // keeps its 67 px and clips the third row (VM, 150 %).
        let font = egui::FontId::proportional(crate::dpi::font(12.0));
        let width = |s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), pal.text).size());
        let lab = ["Станция: ", "Номер регистратора: ", "Начало записи: ", "Момент запуска:", "Длительность: "];
        let sz = width(lab[1]);
        let (w, h) = (sz.x, sz.y);
        let w2 = width(lab[3]).x;
        // The rows from the bar's top (VM: the first centred 10 px down).
        // `0x47ed30` in the bar window's coordinates, whose top is 4 px
        // above the box: row k from y 5 + k(h + 5), h + 5 high.
        let row = |k: usize| (rect.top() + 1.0 + k as f32 * (h + 5.0), rect.top() + 6.0 + (k + 1) as f32 * h + k as f32 * 5.0);
        let x0 = rect.left();
        let painter = ui.painter().clone();
        let label = |text: &str, x1: f32, k: usize| {
            let (y0, y1) = row(k);
            // Drawn right-aligned with their trailing spaces (`0x47ed30`), so
            // «Номер регистратора: » ends a space before its column's edge.
            painter.text(egui::pos2(x0 + x1, (y0 + y1) / 2.0), egui::Align2::RIGHT_CENTER, text, font.clone(), pal.text);
        };
        label(lab[0], w + 15.0, 0);
        label(lab[1], w + 15.0, 1);
        label(lab[2], w + w2 + 465.0, 0);
        label(lab[3], w + w2 + 465.0, 1);
        label(lab[4], w + w2 + 465.0, 2);
        // Created in MFC's message-bar mode (`0x474dc5`: Create(…, 810, −1,
        // TRUE)): its close button hides the bar, as 142 does (`0x47ecf0`).
        if crate::dialogs::caption_close(ui, rect, ui.id().with("fv_param_close")) {
            self.dlg.param_bar = false;
            return;
        }
        let Some(d) = self.active_doc() else { return };
        let (start, trig) = stamp_texts(&d.osc, self.dlg.utc_2013);
        let value = |text: String, xa: f32, xb: f32, k: usize, ellipsis: bool| {
            let (y0, y1) = row(k);
            let r = Rect::from_min_max(egui::pos2(x0 + xa, y0), egui::pos2(x0 + xb, y1));
            let mut job = egui::text::LayoutJob::simple_singleline(text, font.clone(), pal.text);
            if ellipsis {
                job.wrap = egui::text::TextWrapping::truncate_at_width(r.width());
            }
            let g = ui.fonts(|f| f.layout_job(job));
            let pos = egui::pos2(r.left(), r.center().y - g.size().y / 2.0);
            painter.with_clip_rect(r.intersect(rect)).galley(pos, g, pal.text);
        };
        value(d.osc.station.clone(), w + 30.0, w + 420.0, 0, true);
        value(d.osc.device.clone(), w + 30.0, w + 470.0, 1, false);
        value(start, w + w2 + 480.0, w + w2 + 870.0, 0, false);
        value(trig, w + w2 + 480.0, w + w2 + 870.0, 1, false);
        value(duration_text(&d.osc), w + w2 + 480.0, w + w2 + 770.0, 2, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Command, DEMO_CFG, DEMO_DAT};
    use eframe::egui::Vec2;

    fn demo_app() -> App {
        let mut app = App::new();
        let osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        app.add_doc(Doc::new(osc, "demo".into()));
        app
    }

    #[test]
    fn a_new_rate_changes_the_windows_not_the_time_axis() {
        let mut app = demo_app();
        let d = &app.docs[0];
        let (time, n, r) = (d.osc.time.clone(), d.osc.samples_per_cycle(), d.osc.rate_r);
        let half = (r / 2.0) as i32;
        assert_eq!(app.apply_file_rate(0, &half.to_string()), Ok(()));
        let d = &app.docs[0];
        assert_eq!(d.osc.time, time, "the ruler stays");
        assert_eq!(d.osc.samples_per_cycle(), n / 2);
        assert!(d.set.modified);
        assert_eq!(d.undo.len(), 1);
        // The same value again: nothing.
        app.docs[0].set.modified = false;
        assert_eq!(app.apply_file_rate(0, &format!("{half} Гц")), Ok(()));
        assert!(!app.docs[0].set.modified);
        assert_eq!(app.docs[0].undo.len(), 1);
    }

    #[test]
    fn a_rate_under_the_line_frequency_is_a_format_error_after_the_store() {
        let mut app = demo_app();
        assert_eq!(app.apply_file_rate(0, "0"), Err(false));
        assert_eq!(app.dlg.alert(), Some("Неправильный формат файла"));
        let d = &app.docs[0];
        assert_eq!((d.osc.rate_r, d.undo.len(), d.set.modified), (0.0, 1, false), "stored and undoable, not modified");
        assert!(d.osc.freq_track.iter().all(|f| f.is_infinite()));
        // «Закрыть» again: equal to the stored R, it closes.
        app.dlg.alerts.clear();
        assert_eq!(app.apply_file_rate(0, "0"), Ok(()));
        assert!(app.dlg.alerts.is_empty());
        assert_eq!(app.apply_file_rate(0, "x"), Err(true));
    }

    #[test]
    fn a_negative_rate_divides_unsigned_and_leaves_no_window() {
        let mut app = demo_app();
        assert_eq!(app.apply_file_rate(0, "-5"), Ok(()));
        let d = &app.docs[0];
        assert_eq!(d.osc.rate_r, -5.0);
        assert_eq!(fvb_core::signal::samples_per_period(-5.0, 50.0), Some(85_899_345));
        assert_eq!(d.osc.samples_per_cycle(), d.osc.sample_count() + 1);
        assert!(d.analysis.phasors.iter().all(|p| p.iter().all(|z| z.norm() == 0.0)));
        assert!(d.set.modified);
    }

    #[test]
    fn rate_lists_by_line_frequency() {
        assert_eq!(rate_list(50.0).len(), 27);
        assert_eq!(rate_list(50.0)[0], 160000);
        assert_eq!(rate_list(60.0)[14], 3340);
        assert!(rate_list(55.0).is_empty());
    }

    #[test]
    fn display_encoding_codes_have_no_auto() {
        assert_eq!(encoding_code(Encoding::Utf8), 0);
        assert_eq!(encoding_of(1), Encoding::Cp1251);
        assert_eq!(encoding_of(3), Encoding::Koi8r);
        assert!(ENCODINGS.iter().all(|(e, _)| *e != Encoding::Auto));
        // The program default is Windows-1251, UTC on, the bar shown.
        let app = App::new();
        assert_eq!(app.encoding, Encoding::Cp1251);
        assert!(app.dlg.utc_2013 && app.dlg.param_bar);
    }

    #[test]
    fn param_bar_toggles() {
        let mut app = demo_app();
        app.dispatch(Command::ToggleParamBar);
        assert!(!app.dlg.param_bar);
        app.dispatch(Command::ToggleParamBar);
        assert!(app.dlg.param_bar);
    }

    #[test]
    fn primary_secondary_is_a_per_channel_radio() {
        let mut app = demo_app();
        assert!(!app.active_doc().unwrap().primary_values());
        app.dispatch(Command::SetPrimaryValues(true));
        let d = app.active_doc().unwrap();
        assert!(d.chans.iter().all(|c| c.primary));
        assert!(d.primary_values());
        app.dispatch(Command::SetPrimaryValues(false));
        assert!(!app.active_doc().unwrap().primary_values());
    }

    #[test]
    fn recompute_reevaluates_formula_channels_and_keeps_roles() {
        let mut app = demo_app();
        app.append_computed(&[("X".into(), "Ua*2".into())]);
        let before = app.active_doc().unwrap().computed[0].samples[10];
        let d = app.active_doc_mut().unwrap();
        d.analysis.u_abc = [Some(2), Some(1), Some(0)];
        d.osc.analog[0].primary = 100.0;
        app.dispatch(Command::SetPrimaryValues(true));
        let d = app.active_doc().unwrap();
        assert_eq!(d.analysis.u_abc, [Some(2), Some(1), Some(0)]);
        // Primary values scale Ua by primary/secondary = 100.
        assert!((d.computed[0].samples[10] - before * 100.0).abs() < 1e-9 * before.abs().max(1.0));
    }

    #[test]
    fn codepage_switch_redecodes_names() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        let raw = comtrade::encode_text("Ток", Encoding::Cp866);
        d.osc.analog[0].id = comtrade::decode_bytes(&raw, Encoding::Cp1251);
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.osc.analog[0].id, "Ток");
    }

    #[test]
    fn a_1251_name_read_as_866_takes_the_best_fit_and_comes_back() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[0].id = "IA ДОМНА 6".into();
        d.osc.analog[1].id = "UA I CН".into();
        d.init_set_keys();
        d.set_codepage(Encoding::Cp866);
        assert_eq!((d.osc.analog[0].id.as_str(), d.osc.analog[1].id.as_str()), ("IA -+¦=L 6", "UA I C="));
        assert_eq!(d.original_name(crate::app::ChanKey::Analog(0)), None, "still the file's name");
        d.set_codepage(Encoding::Cp1251);
        assert_eq!((d.osc.analog[0].id.as_str(), d.osc.analog[1].id.as_str()), ("IA ДОМНА 6", "UA I CН"));
        // A name renamed meanwhile is its alias, kept as typed (`0x425960`;
        // VM: the renamed text stays, its file name re-decoded beside it).
        d.set_codepage(Encoding::Cp866);
        d.osc.analog[0].id = "Ток".into();
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.osc.analog[0].id, "Ток");
        assert_eq!(d.original_name(crate::app::ChanKey::Analog(0)).as_deref(), Some("IA ДОМНА 6"));
        d.set_codepage(Encoding::Cp866);
        assert_eq!((d.osc.analog[0].id.as_str(), d.osc.analog[1].id.as_str()), ("Ток", "UA I C="));
        assert_eq!(d.original_name(crate::app::ChanKey::Analog(0)).as_deref(), Some("IA -+¦=L 6"));
    }

    /// The load's order: the guess switches the names before the keys are
    /// taken (lossy for 866's pseudo-graphics); the user's 1251 still gives
    /// the names back, and a rename made then is kept.
    #[test]
    fn a_record_read_as_866_at_load_comes_back_in_1251() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[0].id = "IA ДОМНА 6".into();
        d.osc.analog[1].id = "UA I CН".into();
        d.set_codepage(Encoding::Cp866);
        d.init_set_keys();
        assert_eq!(d.osc.analog[0].id, "IA -+¦=L 6");
        d.osc.analog[1].id = "Ток".into();
        // An empty rename shows the file's name as read now, not the lossy
        // key's («¦» kept), and stays the file's name.
        assert_eq!(crate::chandlg::rename(d, crate::app::ChanKey::Analog(0), ""), Ok(()));
        assert_eq!(d.osc.analog[0].id, "IA -+¦=L 6");
        assert_eq!(d.original_name(crate::app::ChanKey::Analog(0)), None);
        d.set_codepage(Encoding::Cp1251);
        assert_eq!((d.osc.analog[0].id.as_str(), d.osc.analog[1].id.as_str()), ("IA ДОМНА 6", "Ток"));
    }

    /// A channel merged in before the record's own moves them: each still
    /// takes its bytes back by the text last decoded from them.
    #[test]
    fn a_switch_after_a_merge_before_the_channels_gives_their_names_back() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[0].id = "IA ДОМНА 6".into();
        d.osc.analog[1].id = "IB ДОМНА 6".into();
        d.set_codepage(Encoding::Cp866);
        d.init_set_keys();
        d.selection = [ChanKey::Analog(2)].into();
        app.copy_channels_for_merge();
        app.active_doc_mut().unwrap().selection = [ChanKey::Analog(0)].into();
        app.paste_channels_for_merge();
        app.merge_records();
        let d = app.active_doc_mut().unwrap();
        assert_eq!(d.osc.analog[1].id, "IA -+¦=L 6", "moved by the merge");
        d.set_codepage(Encoding::Cp1251);
        assert_eq!((d.osc.analog[1].id.as_str(), d.osc.analog[2].id.as_str()), ("IA ДОМНА 6", "IB ДОМНА 6"));
    }

    /// Out of step (no bytes kept: an undo past a merge), a renamed
    /// channel of a UTF-8 record takes its file name's bytes from its key,
    /// exactly — not from the best-fitted name shown under 1251.
    #[test]
    fn an_out_of_step_alias_keeps_its_file_name_exact() {
        use crate::app::ChanKey;
        let mut osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        osc.text_enc = Some(Encoding::Utf8);
        osc.analog[0].id = "U√3 ─".into();
        let mut d = Doc::new(osc, "x".into());
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.osc.analog[0].id, "Uv3 -");
        assert_eq!(crate::chandlg::rename(&mut d, ChanKey::Analog(0), "Своє"), Ok(()));
        d.name_raw.clear();
        d.set_codepage(Encoding::Utf8);
        assert_eq!(d.osc.analog[0].id, "Своє", "the alias stays");
        assert_eq!(d.original_name(ChanKey::Analog(0)).as_deref(), Some("U√3 ─"), "its file name exact");
    }

    /// On a UTF-8 / KOI8-R record a pasted lane's name follows «Кодировка»
    /// as the record's own names do: 1251 its ANSI, 866 its OEM, and the
    /// record's own encoding the name as pasted.
    #[test]
    fn a_lane_pasted_into_a_utf8_record_keeps_its_name() {
        use crate::app::ChanKey;
        for read in [Encoding::Utf8, Encoding::Koi8r] {
            let mut app = demo_app();
            let d = app.active_doc_mut().unwrap();
            d.osc.text_enc = Some(read);
            d.osc.analog[3].id = "Ток фази В".into();
            d.init_record_text(Encoding::Cp1251);
            d.init_set_keys();
            d.selection = [ChanKey::Analog(3)].into();
            app.copy_channels_for_merge();
            app.paste_channels_for_merge();
            let d = app.active_doc_mut().unwrap();
            let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
            let pasted = d.computed[j].name.clone();
            assert!(pasted.starts_with("Ток фази В"), "{pasted}");
            for cp in [Encoding::Cp1251, Encoding::Cp866, read] {
                d.set_codepage(cp);
                let want = decode_name(&comtrade::encode_text(&pasted, read), read, cp);
                assert_eq!(d.computed[j].name, want, "{read:?} → {cp:?}");
                assert!(d.computed[j].name.starts_with(&d.osc.analog[3].id), "{read:?} → {cp:?}: as the record's own channel");
            }
            assert_eq!(d.computed[j].name, pasted, "{read:?}: back as pasted");
        }
    }

    /// #81: a name pasted into a KOI8-R record (no і ї є ґ) is kept as an
    /// alias when its text does not round-trip through the encoding, so a
    /// switch away and back gives it back; a name that does round-trip stays
    /// a plain stored name (re-decoded with the code page like the record's
    /// own).
    #[test]
    fn a_pasted_name_the_encoding_cannot_hold_survives_switches() {
        use crate::app::ChanKey;
        for (src, lossy) in [("Струм фази ї", true), ("Струм фази", false)] {
            let mut app = demo_app();
            let d = app.active_doc_mut().unwrap();
            d.codepage = Encoding::Cp1251;
            d.osc.analog[3].id = src.into();
            d.init_set_keys();
            d.selection = [ChanKey::Analog(3)].into();
            app.copy_channels_for_merge();
            let d = app.active_doc_mut().unwrap();
            // The record now reads in KOI8-R (the paste target).
            d.osc.analog[3].id = "Ток".into();
            d.osc.text_enc = Some(Encoding::Koi8r);
            d.name_raw.clear();
            d.init_record_text(Encoding::Cp1251);
            d.init_set_keys();
            d.selection.clear();
            app.paste_channels_for_merge();
            let d = app.active_doc_mut().unwrap();
            let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
            let pasted = d.computed[j].name.clone();
            assert!(pasted.starts_with(src), "{pasted}");
            assert_eq!(d.computed[j].name_raw.as_ref().unwrap().alias, lossy, "{src}: kept as an alias only when KOI8-R cannot hold it");
            for cp in [Encoding::Cp1251, Encoding::Cp866, Encoding::Koi8r, Encoding::Utf8, Encoding::Koi8r] {
                d.set_codepage(cp);
                if lossy {
                    assert_eq!(d.computed[j].name, pasted, "{src} → {cp:?}");
                }
            }
            assert_eq!(d.computed[j].name, pasted, "{src}: back as pasted");
        }
    }

    /// #81 on a record read in 1251 shown in 866 (what the improvement does
    /// and the original does there): a pasted name 866 cannot hold (і) is
    /// kept as typed; a sibling it can hold reads back in 1251 as its 866
    /// bytes, garbled, as the original's.
    #[test]
    fn on_a_1251_record_shown_in_866_only_the_name_866_cannot_hold_is_kept() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[3].id = "Струм фази і".into();
        d.osc.analog[4].id = "Струм фази".into();
        d.init_set_keys();
        d.selection = [ChanKey::Analog(3), ChanKey::Analog(4)].into();
        app.copy_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        d.set_codepage(Encoding::Cp866);
        d.selection.clear();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let lanes: Vec<usize> = (0..d.computed.len()).filter(|&j| d.computed[j].formula == crate::merge_ui::PENDING).collect();
        assert_eq!(lanes.len(), 2);
        let typed: Vec<String> = lanes.iter().map(|&j| d.computed[j].name.clone()).collect();
        let (with_i, plain) = (lanes.iter().position(|&j| d.computed[j].name.contains('і')).unwrap(), lanes.iter().position(|&j| !d.computed[j].name.contains('і')).unwrap());
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.computed[lanes[with_i]].name, typed[with_i], "kept as typed");
        let garbled: String = comtrade::decode_bytes(&comtrade::encode_shown(&typed[plain], Encoding::Cp866), Encoding::Cp1251).chars().map(comtrade::best_fit_1251).collect();
        assert_ne!(garbled, typed[plain]);
        assert_eq!(d.computed[lanes[plain]].name, garbled, "as the original's 866 bytes read in 1251");
    }

    /// #81 on a UTF-8 record: the bytes are lossless, so a name with letters
    /// 1251 lacks («√», «ä») is not an alias.
    #[test]
    fn a_name_a_utf8_record_holds_is_not_an_alias() {
        // Shown in 1251 / UTF-8 the name is the bytes as read (shown in 866 it is the OEM view, the next test).
        for shown in [Encoding::Utf8, Encoding::Cp1251] {
            let name = "Струм √ ä";
            let bytes = name_bytes_of(name, Encoding::Utf8, shown);
            assert!(!added_name(name, bytes, Encoding::Utf8, shown).alias, "{shown:?}");
        }
    }

    /// #81 on a UTF-8 / KOI8-R record while 866 is shown: a name 866 cannot
    /// hold (і) is kept as typed; one it holds stays plain on a UTF-8 record
    /// (it follows «Кодировка» as the record's own names do) and, KOI8-R
    /// lacking the OEM view's letters, is kept as typed on a KOI8-R record.
    #[test]
    fn a_name_pasted_under_866_into_a_utf8_or_koi8r_record() {
        for (read, plain_alias) in [(Encoding::Utf8, false), (Encoding::Koi8r, true)] {
            let with_i = "Струм і (вст.)";
            let bytes = name_bytes_of(with_i, read, Encoding::Cp866);
            assert!(added_name(with_i, bytes, read, Encoding::Cp866).alias, "{read:?}: і kept as typed");
            let plain = "Струм (вст.)";
            let bytes = name_bytes_of(plain, read, Encoding::Cp866);
            assert_eq!(added_name(plain, bytes, read, Encoding::Cp866).alias, plain_alias, "{read:?}");
        }
    }

    /// The out-of-step fallback of a switch (also the station and the
    /// recorder, which have no file name): a name KOI8-R cannot hold is kept
    /// as typed (#81).
    #[test]
    fn an_out_of_step_name_the_encoding_cannot_hold_survives_switches() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.osc.text_enc = Some(Encoding::Koi8r);
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        d.osc.station = "Ї станція".into();
        d.name_raw.clear();
        for cp in [Encoding::Cp1251, Encoding::Cp866, Encoding::Koi8r] {
            d.set_codepage(cp);
        }
        assert_eq!(d.osc.station, "Ї станція");
    }

    /// The same fallback for a name shown in 866 holding a best-fitted
    /// pseudo-graphic: it is re-decoded from the 866 byte it was read as,
    /// not kept as typed (final review, Minor 3).
    #[test]
    fn an_out_of_step_name_with_a_pseudo_graphic_is_decoded_from_its_866_byte() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        d.set_codepage(Encoding::Cp866);
        d.osc.station = "A¦".into();
        d.name_raw.clear();
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.osc.station, "Aі", "the byte 0xB3 read in 1251");
    }

    /// A channel appended past the kept names (`move_appended_keys`, merge)
    /// whose name KOI8-R cannot hold is an alias (#81).
    #[test]
    fn an_appended_channel_the_encoding_cannot_hold_is_an_alias() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.osc.text_enc = Some(Encoding::Koi8r);
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        let (nb, nd) = (d.osc.analog.len(), d.osc.digital.len());
        let mut c = d.osc.analog[0].clone();
        c.id = "Струм ї".into();
        d.osc.analog.push(c);
        let mut plain = d.osc.analog[0].clone();
        plain.id = "Струм".into();
        d.osc.analog.push(plain);
        d.move_appended_keys(nb, nb, nd, nd);
        assert!(d.name_raw[2 + nb].alias);
        assert!(!d.name_raw[2 + nb + 1].alias);
    }

    /// A pasted lane with no kept bytes whose name KOI8-R cannot hold keeps
    /// it across a switch (#81).
    #[test]
    fn a_pasted_lane_without_kept_bytes_keeps_a_letter_the_encoding_lacks() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.init_set_keys();
        d.selection = [ChanKey::Analog(3)].into();
        app.copy_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        d.osc.text_enc = Some(Encoding::Koi8r);
        d.name_raw.clear();
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        d.selection.clear();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        d.computed[j].name = "Ї лінія".into();
        d.computed[j].name_raw = None;
        for cp in [Encoding::Cp1251, Encoding::Cp866, Encoding::Koi8r] {
            d.set_codepage(cp);
        }
        assert_eq!(d.computed[j].name, "Ї лінія");
    }

    /// A pasted lane's name is re-decoded too (VM: a pasted lane's
    /// name read as 866 lists its best-fit form), and comes back.
    #[test]
    fn a_pasted_lane_is_re_decoded() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[3].id = "IA ДОМНА 6".into();
        d.init_set_keys();
        d.selection = [ChanKey::Analog(3)].into();
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        let pasted = d.computed[j].name.clone();
        assert!(pasted.starts_with("IA ДОМНА 6"), "{pasted}");
        d.set_codepage(Encoding::Cp866);
        let want: String = comtrade::decode_bytes(&comtrade::encode_text(&pasted, Encoding::Cp1251), Encoding::Cp866).chars().map(comtrade::best_fit_1251).collect();
        assert_eq!(d.computed[j].name, want);
        assert!(d.pending_of(j).is_some(), "still linked");
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.computed[j].name, pasted);
        // Renamed, it keeps its name.
        assert_eq!(crate::chandlg::rename(d, ChanKey::Computed(j), "Своё"), Ok(()));
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.computed[j].name, "Своё");
    }

    /// Pasted while the record shows 866, a lane's raw name is its name in
    /// 866 bytes (VM: a best-fit name pasted under 866 reads back in
    /// 1251 with «¦» as 0xB3, and itself again in 866).
    #[test]
    fn a_lane_pasted_while_866_is_shown_reads_its_866_bytes() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[3].id = "IA ДОМНА 6".into();
        d.init_set_keys();
        d.set_codepage(Encoding::Cp866);
        d.selection = [ChanKey::Analog(3)].into();
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        assert_eq!(d.computed[j].name, "IA -+¦=L 6 (вст.)");
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.computed[j].name, "IA -+і=L 6 (ўбв.)");
        assert_eq!(d.osc.analog[3].id, "IA ДОМНА 6");
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.computed[j].name, "IA -+¦=L 6 (вст.)");
    }

    /// A lane pasted while 866 is shown into a UTF-8 record stores the name
    /// as read (UTF-8), not 866 bytes: the next switch would show U+FFFD.
    #[test]
    fn a_lane_pasted_while_866_is_shown_into_a_utf8_record_keeps_its_name() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.osc.text_enc = Some(Encoding::Utf8);
        d.osc.analog[3].id = "Ток фази В".into();
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        d.set_codepage(Encoding::Cp866);
        d.selection = [ChanKey::Analog(3)].into();
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        let under_866 = d.computed[j].name.clone();
        d.set_codepage(Encoding::Cp1251);
        let under_1251 = d.computed[j].name.clone();
        assert!(!under_1251.contains('\u{FFFD}'), "{under_1251}");
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.computed[j].name, under_866, "866 → 1251 → 866");
        d.set_codepage(Encoding::Utf8);
        assert_eq!(d.computed[j].name, under_1251, "UTF-8 reads it as 1251 does");
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.computed[j].name, under_866, "and 866 again");
    }

    /// The record comment saved in a `.cff` is the text without a `.hdr`'s
    /// BOM: not raw BOM bytes in a UTF-8 file, not a «?» in a 1251 one.
    #[test]
    fn a_boms_hdr_is_saved_in_a_cff_without_its_bom() {
        for (utf8, enc) in [(true, Encoding::Utf8), (false, Encoding::Cp1251)] {
            let mut osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
            osc.rev_year = 2013;
            if utf8 {
                osc.text_enc = Some(Encoding::Utf8);
                osc.station = "Підстанція".into();
            }
            let mut d = Doc::new(osc, "x".into());
            d.codepage = enc;
            d.hdr = Some("\u{FEFF}Коментар".into());
            let bytes = comtrade::write_cff(&d.osc, &d.cff_sections(), d.codepage).unwrap();
            let mark = b"--- file type: HDR ---\r\n";
            let at = bytes.windows(mark.len()).position(|w| w == mark).expect("an HDR section") + mark.len();
            let body = &bytes[at..at + 16];
            let want = if utf8 { "Коментар".as_bytes().to_vec() } else { comtrade::encode_text("Коментар", Encoding::Cp1251) };
            assert!(body.starts_with(&want), "{enc:?}: {body:02x?}");
        }
    }

    /// A pasted lane renamed before any «Кодировка» keeps its name (its raw
    /// name is set at the paste); emptied, it shows its name as pasted.
    #[test]
    fn a_pasted_lane_renamed_first_keeps_its_name() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[3].id = "IA ДОМНА 6".into();
        d.init_set_keys();
        d.selection = [ChanKey::Analog(3)].into();
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        let pasted = d.computed[j].name.clone();
        assert_eq!(crate::chandlg::rename(d, ChanKey::Computed(j), "Своё"), Ok(()));
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.computed[j].name, "Своё");
        assert_eq!(crate::chandlg::rename(d, ChanKey::Computed(j), ""), Ok(()));
        let want: String = comtrade::decode_bytes(&comtrade::encode_text(&pasted, Encoding::Cp1251), Encoding::Cp866).chars().map(comtrade::best_fit_1251).collect();
        assert_eq!(d.computed[j].name, want, "its raw name, as read now");
        assert!(d.pending_of(j).is_some());
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.computed[j].name, pasted);
    }

    /// A merged channel keeps its lane's name as read: the bytes as pasted
    /// (866 → merge → 1251 gives the pasted name back), the alias of a
    /// rename (kept across a switch).
    #[test]
    fn a_merged_lane_keeps_its_name_as_read() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[3].id = "IA ДОМНА 6".into();
        d.osc.analog[4].id = "IB ДОМНА 6".into();
        d.init_set_keys();
        d.selection = [ChanKey::Analog(3), ChanKey::Analog(4)].into();
        app.copy_channels_for_merge();
        app.active_doc_mut().unwrap().selection.clear();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let lanes: Vec<usize> = (0..d.computed.len()).filter(|&j| d.computed[j].formula == crate::merge_ui::PENDING).collect();
        let pasted = d.computed[lanes[0]].name.clone();
        assert_eq!(crate::chandlg::rename(d, ChanKey::Computed(lanes[1]), "Своё"), Ok(()));
        d.set_codepage(Encoding::Cp866);
        let na = d.osc.analog.len();
        app.merge_records();
        let d = app.active_doc_mut().unwrap();
        d.set_codepage(Encoding::Cp1251);
        assert_eq!((d.osc.analog[na].id.as_str(), d.osc.analog[na + 1].id.as_str()), (pasted.as_str(), "Своё"));
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.osc.analog[na + 1].id, "Своё");
    }

    /// The names as read, taken from the names shown now (`ensure_name_raw`)
    /// or for channels a merge appends (`move_appended_keys`), keep a
    /// character 866 lacks but its reading shows as the 866 byte read so
    /// ([`comtrade::encode_shown`]), as a pasted lane's name does (#83).
    #[test]
    fn names_taken_from_the_shown_ones_use_the_shown_encoding() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.read_enc = Encoding::Cp1251;
        d.codepage = Encoding::Cp866;
        d.osc.analog[0].id = "A¦".into();
        d.name_raw.clear();
        d.ensure_name_raw();
        assert_eq!(d.name_raw[2].bytes, [b'A', 0xB3], "«¦» is the 866 byte it reads back from");
        // A channel appended by a merge, its name shown the same way.
        d.init_set_keys();
        let (na, nd) = (d.osc.analog.len(), d.osc.digital.len());
        let mut added = d.osc.analog[0].clone();
        added.id = "B¦".into();
        d.osc.analog.push(added);
        d.move_appended_keys(na, na, nd, nd);
        assert_eq!(d.name_raw[2 + na].bytes, [b'B', 0xB3]);
        assert_eq!(d.set.keys[na], "B\u{456}", "its file-name key: those bytes as 1251 text");
    }

    /// A rename on a document whose names as read were never taken (the
    /// demo) takes them first: an empty rename brings the file's name back.
    #[test]
    fn an_empty_rename_brings_the_name_back_without_kept_names() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        let own = d.osc.analog[0].id.clone();
        assert!(d.name_raw.is_empty());
        assert_eq!(crate::chandlg::rename(d, ChanKey::Analog(0), "X"), Ok(()));
        assert_eq!(crate::chandlg::rename(d, ChanKey::Analog(0), ""), Ok(()));
        assert_eq!(d.osc.analog[0].id, own);
    }

    /// An alias typed equal to the file's name as shown is still an alias
    /// (`+0x6c` non-empty): kept as typed across «Кодировка».
    #[test]
    fn an_alias_equal_to_the_file_name_is_kept() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[0].id = "Ток".into();
        d.osc.analog[1].id = "Ток 2".into();
        d.init_set_keys();
        // «Каналы» renames it away and back to the same text.
        assert_eq!(crate::chandlg::rename(d, ChanKey::Analog(0), "X"), Ok(()));
        assert_eq!(crate::chandlg::rename(d, ChanKey::Analog(0), "Ток"), Ok(()));
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.osc.analog[0].id, "Ток", "the alias as typed");
        assert_ne!(d.osc.analog[1].id, "Ток 2", "the file's name re-decoded");
    }

    /// Two pasted lanes whose names decode alike stay linked to their own
    /// pasted channels (by the lane, not the name) and merge their own
    /// values.
    #[test]
    fn pasted_lanes_named_alike_keep_their_own_channels() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[3].id = "Ток линии В".into();
        d.osc.analog[4].id = "Ток линии С".into();
        d.init_set_keys();
        let (b, c) = (d.osc.analog[3].samples.clone(), d.osc.analog[4].samples.clone());
        d.selection = [ChanKey::Analog(3), ChanKey::Analog(4)].into();
        app.copy_channels_for_merge();
        app.active_doc_mut().unwrap().selection.clear();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let lanes: Vec<usize> = (0..d.computed.len()).filter(|&j| d.computed[j].formula == crate::merge_ui::PENDING).collect();
        assert_eq!(lanes.len(), 2);
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.computed[lanes[0]].name, d.computed[lanes[1]].name, "alike in 866");
        let own = |d: &Doc, j: usize| match &d.pending_of(j).unwrap().pasted.channel {
            Ok(a) => a.samples.clone(),
            Err(_) => Vec::new(),
        };
        assert_eq!((own(d, lanes[0]), own(d, lanes[1])), (b.clone(), c.clone()));
        // An offset change redraws each lane from its own channel.
        d.set_merge_offset(0.0);
        let m = b.len() / 2;
        assert!((d.computed[lanes[0]].samples[m] - b[m]).abs() < 1e-9 && (d.computed[lanes[1]].samples[m] - c[m]).abs() < 1e-9);
        let na = d.osc.analog.len();
        app.merge_records();
        let d = app.active_doc().unwrap();
        let k = d.osc.analog.len() - na;
        assert_eq!(k, 2);
        let mid = b.len() / 2;
        let (mb, mc) = (&d.osc.analog[na].samples, &d.osc.analog[na + 1].samples);
        assert!((mb[mid] - b[mid]).abs() < 1e-9 && (mc[mid] - c[mid]).abs() < 1e-9, "{} {} / {} {}", mb[mid], b[mid], mc[mid], c[mid]);
    }

    /// An undo past a merge takes the names' bytes back with the channels:
    /// a merge elsewhere after it leaves each name its own.
    #[test]
    fn an_undo_past_a_merge_takes_the_bytes_back() {
        use crate::app::ChanKey;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[0].id = "IA ДОМНА 6".into();
        d.set_codepage(Encoding::Cp866);
        d.init_set_keys();
        d.selection = [ChanKey::Analog(2)].into();
        app.copy_channels_for_merge();
        app.active_doc_mut().unwrap().selection = [ChanKey::Analog(0)].into();
        app.paste_channels_for_merge();
        app.merge_records();
        while !app.active_doc().unwrap().undo.is_empty() {
            app.dispatch(Command::Undo);
        }
        let n = app.active_doc().unwrap().osc.analog.len();
        app.active_doc_mut().unwrap().selection.clear();
        app.paste_channels_for_merge();
        app.merge_records();
        let d = app.active_doc_mut().unwrap();
        assert_eq!(d.osc.analog.len(), n + 1, "merged at the end");
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.osc.analog[0].id, "IA ДОМНА 6");
    }

    /// «В» and «С» read as 866 both show «T»: two names alike each take
    /// their own bytes back, also after a channel merged in before them.
    #[test]
    fn names_that_decode_alike_keep_their_own_bytes() {
        use crate::app::ChanKey;
        for merge in [false, true] {
            let mut app = demo_app();
            let d = app.active_doc_mut().unwrap();
            d.codepage = Encoding::Cp1251;
            d.osc.analog[0].id = "Ток линии В".into();
            d.osc.analog[1].id = "Ток линии С".into();
            d.set_codepage(Encoding::Cp866);
            d.init_set_keys();
            assert_eq!(d.osc.analog[0].id, d.osc.analog[1].id);
            if merge {
                d.selection = [ChanKey::Analog(2)].into();
                app.copy_channels_for_merge();
                app.active_doc_mut().unwrap().selection = [ChanKey::Analog(0)].into();
                app.paste_channels_for_merge();
                app.merge_records();
            }
            let d = app.active_doc_mut().unwrap();
            let k = usize::from(merge);
            d.set_codepage(Encoding::Cp1251);
            assert_eq!((d.osc.analog[k].id.as_str(), d.osc.analog[k + 1].id.as_str()), ("Ток линии В", "Ток линии С"), "merge: {merge}");
            d.set_codepage(Encoding::Cp866);
            // В renamed meanwhile: С still takes its own bytes.
            d.osc.analog[k].id = "IB".into();
            d.set_codepage(Encoding::Cp1251);
            assert_eq!((d.osc.analog[k].id.as_str(), d.osc.analog[k + 1].id.as_str()), ("IB", "Ток линии С"), "merge: {merge}");
        }
    }

    /// Ours alone (#7): what named a channel before «Кодировка» names it
    /// after — a quick-analysis channel's formula keeps computing, a level
    /// stays on its channel, a page's pick and a pair follow (the original
    /// keeps the old names: the next «Рассчитать» fails, the level no longer
    /// draws). A renamed channel's level simply stays.
    #[test]
    fn references_follow_their_channel_across_a_code_page_switch() {
        use crate::app::{AnalysisTab, ChanKey};
        use crate::comments::{Comment, CommentKind};
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        for (i, n) in [(3, "IA ДОМНА 6"), (4, "IB ДОМНА 6"), (5, "IC ДОМНА 6")] {
            d.osc.analog[i].id = n.into();
        }
        d.osc.analog[1].id = "Свой".into();
        d.osc.digital[0].id = "Пуск ДОМНА".into();
        d.init_set_keys();
        d.osc.analog[1].id = "Другой".into();
        d.omp.params.stations[0].ia = "IA ДОМНА 6".into();
        for i in [3, 1] {
            d.comments.push(Comment { kind: CommentKind::Level { chan: ChanKey::Analog(i), value: 1.0, from: 10, to: 20 }, text: String::new(), locked: false, hidden: false });
        }
        d.dif_pairs.push(crate::diagram::ChanPair::new("IA ДОМНА 6", "IB ДОМНА 6"));
        app.analysis_tab = AnalysisTab::Zero;
        let z = crate::quick_ui::page_index(AnalysisTab::Zero).unwrap();
        let q = &mut app.quick[z];
        (q.ia, q.ib, q.ic) = ("IA ДОМНА 6".into(), "IB ДОМНА 6".into(), "IC ДОМНА 6".into());
        q.zero_u_off = true;
        let g = q.generate(AnalysisTab::Zero, &[]);
        app.apply_quick(&g);
        crate::quick_ui::save_page(z, &app.quick[z].clone(), app.active_doc_mut().unwrap().set.tree.section_mut(crate::quick_ui::PAGE_KEYS[z]));
        assert!(app.append_computed(&[("D".into(), "\"Пуск ДОМНА\"*2".into())]).is_some(), "{:?}", app.dlg.alert());
        let d = app.active_doc_mut().unwrap();
        // A differential arm's pick follows; a text that only equals a name
        // (an edit's) does not.
        let dif = d.set.tree.section_mut(crate::quick_ui::PAGE_KEYS[4]);
        dif.child_mut("Leg_0").set_str("IA", "IA ДОМНА 6");
        dif.set_str("brakeCoeff", "IA ДОМНА 6");
        let before = d.computed.iter().find(|c| c.name == "3I0").unwrap().samples.clone();
        d.set_codepage(Encoding::Cp866);
        let dif = d.set.tree.section(crate::quick_ui::PAGE_KEYS[4]).unwrap();
        assert_eq!(dif.children["Leg_0"].attrs["IA"], d.osc.analog[3].id);
        assert_eq!(dif.attrs["brakeCoeff"], "IA ДОМНА 6", "not a channel pick");
        assert_eq!(d.omp.params.stations[0].ia, d.osc.analog[3].id, "ОМП's channel too");
        let dig = d.osc.digital[0].id.clone();
        assert!(d.computed.iter().find(|c| c.name == "D").unwrap().formula.contains(&format!("\"{dig}\"")), "a digital channel's reference too");
        let ia = d.osc.analog[3].id.clone();
        assert_eq!(ia, "IA -+¦=L 6");
        let i0 = d.computed.iter().find(|c| c.name == "3I0").unwrap();
        assert!(i0.formula.contains(&format!("\"{ia}\"")), "{}", i0.formula);
        assert!(matches!(d.comments[0].kind, CommentKind::Level { chan: ChanKey::Analog(3), .. }), "the level stays on IA");
        assert!(matches!(d.comments[1].kind, CommentKind::Level { chan: ChanKey::Analog(1), .. }), "the alias's too");
        assert_eq!(d.dif_pairs[0].a, ia);
        let pick = d.set.tree.section(crate::quick_ui::PAGE_KEYS[z]).and_then(|s| s.children.get("IA")).and_then(|c| c.attrs.get("channelName")).cloned();
        assert_eq!(pick.as_deref(), Some(ia.as_str()));
        // A new calculation re-evaluates the old one: it still computes.
        assert!(app.append_computed(&[("X".into(), "Ua*2".into())]).is_some(), "{:?}", app.dlg.alert());
        let d = app.active_doc_mut().unwrap();
        assert_eq!(d.computed.iter().find(|c| c.name == "3I0").unwrap().samples, before);
        d.set_codepage(Encoding::Cp1251);
        assert!(d.computed.iter().find(|c| c.name == "3I0").unwrap().formula.contains("\"IA ДОМНА 6\""));
    }

    /// Names that decode alike are not followed: a formula on one channel
    /// is not rewritten to the text it shares with another channel's name in 866
    /// (it would read В) — it fails there as in the original, and works
    /// again back in 1251; a level on С stays on С.
    #[test]
    fn names_that_decode_alike_are_not_followed() {
        use crate::app::ChanKey;
        use crate::comments::{Comment, CommentKind};
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[3].id = "Ток линии В".into();
        d.osc.analog[4].id = "Ток линии С".into();
        d.init_set_keys();
        d.comments.push(Comment { kind: CommentKind::Level { chan: ChanKey::Analog(4), value: 1.0, from: 10, to: 20 }, text: String::new(), locked: false, hidden: false });
        assert!(app.append_computed(&[("X".into(), "\"Ток линии С\"*2".into())]).is_some());
        let d = app.active_doc_mut().unwrap();
        let before = d.computed[0].samples.clone();
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.osc.analog[3].id, d.osc.analog[4].id);
        assert_eq!(d.computed[0].formula, "\"Ток линии С\"*2", "not rewritten");
        assert!(matches!(d.comments[0].kind, CommentKind::Level { chan: ChanKey::Analog(4), .. }));
        d.set_codepage(Encoding::Cp1251);
        assert!(app.append_computed(&[("Y".into(), "1".into())]).is_some(), "{:?}", app.dlg.alert());
        assert_eq!(app.active_doc().unwrap().computed[0].samples, before);
    }

    /// The formula editor resolves a renamed channel by its new name.
    #[test]
    fn the_formula_editor_takes_a_new_name() {
        let mut app = demo_app();
        assert_eq!(crate::chandlg::rename(app.active_doc_mut().unwrap(), crate::app::ChanKey::Analog(0), "Новое"), Ok(()));
        app.dispatch(Command::QuickAnalysis);
        app.formula_page.rows.push(("X".into(), "Новое*2".into()));
        app.formula_page.dirty = true;
        app.quick_calc();
        assert_eq!(app.dlg.alert(), None);
        let d = app.active_doc().unwrap();
        let x = d.computed.iter().find(|c| c.name == "X").expect("X computed");
        assert_eq!(x.samples[10], d.flagged.analog[0].samples[10] * 2.0);
    }

    /// Issue #7: after «Кодировка» «Быстрый анализ» works on the names as
    /// now shown — the original's formulas look the channels up by their
    /// shown name (`+0x68`, `0x42edf0`), which `0x4543e0` → `0x425960`
    /// re-decodes in place. A formula on the old names no longer resolves.
    #[test]
    fn quick_analysis_takes_the_names_of_a_new_code_page() {
        use crate::app::AnalysisTab;
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        // A 1251 record whose names were read as 866 (the guess, or the
        // user's pick), and computed on.
        d.codepage = Encoding::Cp1251;
        for (i, n) in [(3, "IA ДОМНА 6"), (4, "IB ДОМНА 6"), (5, "IC ДОМНА 6")] {
            d.osc.analog[i].id = n.into();
        }
        d.init_set_keys();
        d.set_codepage(Encoding::Cp866);
        d.recompute(false);
        let old = d.osc.analog[3].id.clone();
        assert_eq!(old, "IA -+¦=L 6");
        d.set_codepage(Encoding::Cp1251);
        app.analysis_tab = AnalysisTab::Zero;
        let q = &mut app.quick[crate::quick_ui::page_index(AnalysisTab::Zero).unwrap()];
        (q.ia, q.ib, q.ic) = ("IA ДОМНА 6".into(), "IB ДОМНА 6".into(), "IC ДОМНА 6".into());
        (q.ua, q.ub, q.uc) = ("Ua".into(), "Ub".into(), "Uc".into());
        let g = q.generate(AnalysisTab::Zero, &[]);
        app.apply_quick(&g);
        assert_eq!(app.dlg.alert(), None);
        let d = app.active_doc().unwrap();
        let i0 = d.computed.iter().find(|c| c.name == "3I0").expect("3I0 computed");
        assert_eq!(i0.formula, "3I0: (\"IA ДОМНА 6\", \"IB ДОМНА 6\", \"IC ДОМНА 6\")");
        // The formula editor alike; the old name is the record's no more.
        assert!(app.append_computed(&[("X".into(), "\"IB ДОМНА 6\"*2".into())]).is_some());
        assert!(app.append_computed(&[("Y".into(), format!("\"{old}\"*2"))]).is_none());
        assert_eq!(app.dlg.alert(), Some("Имя канала задано неправильно."), "the bare 0x162 (VM)");
    }

    #[test]
    fn init_guesses_the_code_page_from_the_names() {
        let osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        let mut d = Doc::new(osc, "x".into());
        // Names as an 866 file read with the 1251 default.
        let as_1251 = |s: &str| comtrade::decode_bytes(&comtrade::encode_text(s, Encoding::Cp866), Encoding::Cp1251);
        let mut d866 = Doc::new(d.osc.clone(), "y".into());
        d866.osc.station = as_1251("Подстанция");
        d866.osc.analog[0].id = as_1251("Напряжение");
        d866.init_record_text(Encoding::Cp1251);
        assert_eq!(d866.codepage, Encoding::Cp866);
        assert_eq!((d866.osc.station.as_str(), d866.osc.analog[0].id.as_str()), ("Подстанция", "Напряжение"));
        // The demo: a 1251 station and Latin channel names stay 1251.
        d.init_record_text(Encoding::Cp1251);
        assert_eq!(d.codepage, Encoding::Cp1251);
        assert!(d.osc.station.starts_with("НТЦ"));
    }

    /// A record whose `.cfg` fixed UTF-8 (issue #8): that is its code page
    /// under any program encoding — no guess re-decodes its names, a U+FFFD
    /// kept —, «Кодировка» lists it as a third entry, current, and a switch
    /// to 866 or 1251 (its bytes read so) comes back to the same names; its
    /// UTF-8 `.hdr` is read as UTF-8 and written back byte for byte, while
    /// `Comments/Codepage` keeps the guess under the program encoding.
    #[test]
    fn a_utf8_record_keeps_utf8_names_and_hdr() {
        let dir = std::env::temp_dir().join(format!("fv_utf8_hdr_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let hdr = "Коментар: синтетичний запис\r\nПримітка: тест\r\n";
        std::fs::write(dir.join("rec.hdr"), hdr).unwrap();
        let mut osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        osc.text_enc = Some(Encoding::Utf8);
        osc.station = "Підстанція".into();
        osc.analog[0].id = "Струм фази А".into();
        osc.analog[1].id = "Шина 2 напр\u{FFFD}".into();
        let mut d = Doc::new(osc, "x".into());
        d.path = Some(dir.join("rec.cfg"));
        d.init_record_text(Encoding::Cp1251);
        let _ = std::fs::remove_dir_all(&dir);
        let names = |d: &Doc| [d.osc.station.clone(), d.osc.analog[0].id.clone(), d.osc.analog[1].id.clone()];
        let read = ["Підстанція", "Струм фази А", "Шина 2 напр\u{FFFD}"].map(String::from);
        assert_eq!((d.codepage, d.read_enc, names(&d)), (Encoding::Utf8, Encoding::Utf8, read.clone()));
        assert_eq!((d.hdr.as_deref(), d.hdr_cp), (Some(hdr), Encoding::Utf8));
        assert_eq!(d.hdr_file_bytes().unwrap(), hdr.as_bytes());
        // A BOM is saved back but kept out of the text others take.
        d.hdr = Some(format!("\u{FEFF}{hdr}"));
        assert_eq!(d.hdr_text(), hdr);
        assert!(d.hdr_file_bytes().unwrap().starts_with(b"\xEF\xBB\xBF"));
        let guess = crate::setfile::cp_code(comtrade::detect_codepage(hdr.as_bytes()));
        assert_eq!(d.set.tree.section("Comments").unwrap().get_int("Codepage", -1), guess);
        // «Кодировка»: the original's two, then UTF-8 — current.
        let items = codepage_items(d.read_enc);
        assert_eq!(items, [(Encoding::Cp866, "866 MS-DOS"), (Encoding::Cp1251, "Windows-1251"), (Encoding::Utf8, "UTF-8")]);
        assert_eq!(items.iter().position(|(e, _)| *e == d.codepage), Some(2));
        assert_eq!(codepage_items(Encoding::Cp1251).len(), 2, "a 1251 / 866 record: the original's two");
        // 1251 is the original's ANSI — the names narrowed with Windows' best
        // fit, «?» where there is none (VM: «√ ─ ä ∆» show «v - a ?») —, 866
        // its OEM over those bytes; UTF-8 gives the names back as read, «√»
        // and box-drawing included; none counts as renamed.
        d.osc.analog[2].id = "U√3 ─ Iä ∆".into();
        d.name_raw.clear();
        d.init_set_keys();
        let read = [read[0].clone(), read[1].clone(), read[2].clone(), "U√3 ─ Iä ∆".into()];
        let names = |d: &Doc| [d.osc.station.clone(), d.osc.analog[0].id.clone(), d.osc.analog[1].id.clone(), d.osc.analog[2].id.clone()];
        let ansi = ["Підстанція", "Струм фази А", "Шина 2 напр?", "Uv3 - Ia ?"].map(String::from);
        let oem = ansi.clone().map(|s| comtrade::recode(&s, Encoding::Cp1251, Encoding::Cp866).chars().map(comtrade::best_fit_1251).collect::<String>());
        for (cp, shown) in [(Encoding::Cp1251, &ansi), (Encoding::Cp866, &oem), (Encoding::Cp1251, &ansi)] {
            d.set_codepage(cp);
            assert_eq!((d.codepage, &names(&d)), (cp, shown), "{cp:?}");
            assert_eq!(d.original_name(crate::app::ChanKey::Analog(2)), None, "{cp:?}: the file's name");
            d.set_codepage(Encoding::Utf8);
            assert_eq!(names(&d), read, "{cp:?} and back");
        }
        // 866 straight from 1251 and back, as the original.
        d.set_codepage(Encoding::Cp1251);
        d.set_codepage(Encoding::Cp866);
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(names(&d), ansi);
        d.set_codepage(Encoding::Utf8);
        assert_eq!(names(&d), read);
        // Read under the program's UTF-8 or KOI8-R: that encoding, listed.
        for program in [Encoding::Utf8, Encoding::Koi8r] {
            let mut d = Doc::new(comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap(), "y".into());
            d.init_record_text(program);
            assert_eq!((d.codepage, d.read_enc), (program, program));
            assert_eq!(codepage_items(d.read_enc).last().map(|e| e.0), Some(program));
        }
    }

    /// On a UTF-8 record the names «Кодировка» shows carry what named the
    /// channels along (#7): a formula on «√» follows it to 866's name and
    /// back, a level stays on its channel.
    #[test]
    fn a_utf8_record_s_references_follow_the_code_page() {
        use crate::app::ChanKey;
        use crate::comments::{Comment, CommentKind};
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.osc.text_enc = Some(Encoding::Utf8);
        d.osc.analog[0].id = "Uл√3".into();
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        d.comments.push(Comment { kind: CommentKind::Level { chan: ChanKey::Analog(0), value: 1.0, from: 10, to: 20 }, text: String::new(), locked: false, hidden: false });
        assert!(app.append_computed(&[("X".into(), "\"Uл√3\"*2".into())]).is_some());
        let formula = |app: &App| app.active_doc().unwrap().computed.iter().find(|c| c.name == "X").unwrap().formula.clone();
        let d = app.active_doc_mut().unwrap();
        d.set_codepage(Encoding::Cp866);
        let oem = decode_name("Uл√3".as_bytes(), Encoding::Utf8, Encoding::Cp866);
        assert_eq!(d.osc.analog[0].id, oem);
        assert_eq!(formula(&app), format!("\"{oem}\"*2"));
        let d = app.active_doc_mut().unwrap();
        d.set_codepage(Encoding::Utf8);
        assert_eq!(d.osc.analog[0].id, "Uл√3");
        assert!(matches!(d.comments[0].kind, CommentKind::Level { chan: ChanKey::Analog(0), .. }));
        assert_eq!(formula(&app), "\"Uл√3\"*2");
    }

    /// A channel renamed on a UTF-8 / KOI8-R record before any «Кодировка»
    /// switch keeps its name through 866 and back (as on a 1251 record).
    #[test]
    fn a_utf8_record_s_alias_survives_the_first_switch() {
        for read in [Encoding::Utf8, Encoding::Koi8r] {
            let mut osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
            osc.text_enc = Some(read);
            osc.analog[0].id = "Струм".into();
            let mut d = Doc::new(osc, "x".into());
            d.init_record_text(Encoding::Cp1251);
            d.init_set_keys();
            crate::chandlg::rename(&mut d, crate::app::ChanKey::Analog(0), "Ток").unwrap();
            d.set_codepage(Encoding::Cp866);
            assert_eq!(d.osc.analog[0].id, "Ток", "{read:?}: 866");
            d.set_codepage(read);
            assert_eq!(d.osc.analog[0].id, "Ток", "{read:?}: back");
            assert_eq!(d.original_name(crate::app::ChanKey::Analog(0)).as_deref(), Some("Струм"), "{read:?}");
        }
    }

    /// A channel merged into a UTF-8 record while «866» shows keeps its
    /// bytes as that record reads them: back in UTF-8 it shows what a 1251
    /// record would, 866 shows it as merged again, and it is no rename.
    #[test]
    fn a_channel_merged_under_866_into_a_utf8_record_round_trips() {
        use crate::app::ChanKey;
        let mut osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        osc.text_enc = Some(Encoding::Utf8);
        let mut d = Doc::new(osc, "x".into());
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        d.set_codepage(Encoding::Cp866);
        let (na, nd) = (d.osc.analog.len(), d.osc.digital.len());
        let samples = vec![false; d.osc.sample_count()];
        d.osc.digital.push(fvb_core::DigitalChannel { index: nd as u32 + 1, id: "Ток".into(), phase: String::new(), normal: 0, samples, own_rate: 0.0 });
        d.digital_visible.resize(nd + 1, true);
        d.move_appended(na, na, nd, nd);
        assert_eq!(d.original_name(ChanKey::Digital(nd)), None, "the file's name");
        d.set_codepage(Encoding::Utf8);
        let as_1251 = comtrade::recode("Ток", Encoding::Cp866, Encoding::Cp1251);
        assert_eq!(d.osc.digital[nd].id, as_1251, "what a 1251 record shows, no U+FFFD");
        assert_eq!(d.original_name(ChanKey::Digital(nd)), None);
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.osc.digital[nd].id, "Ток");
    }

    /// A stored `Oscillogram/Codepage` applies to a UTF-8 record too
    /// (`0x44f808`): 0 = OEM (its bytes read as 866), 1 = ANSI, the names
    /// as read — UTF-8.
    #[test]
    fn a_stored_code_page_applies_to_a_utf8_record() {
        let mut app = demo_app();
        let ai = app.active.unwrap();
        let d = &mut app.docs[ai];
        d.osc.text_enc = Some(Encoding::Utf8);
        d.osc.analog[0].id = "Струм".into();
        d.init_record_text(Encoding::Cp1251);
        d.init_set_keys();
        d.set.tree.section_mut("Oscillogram").set_int("Codepage", 0);
        app.apply_record_settings(ai, false);
        let d = &mut app.docs[ai];
        let oem = decode_name("Струм".as_bytes(), Encoding::Utf8, Encoding::Cp866);
        assert_eq!((d.codepage, d.osc.analog[0].id.as_str()), (Encoding::Cp866, oem.as_str()));
        assert_eq!(crate::setfile::cp_code(d.codepage), 0, "saved back as 0");
        d.set.tree.section_mut("Oscillogram").set_int("Codepage", 1);
        app.apply_record_settings(ai, false);
        let d = &mut app.docs[ai];
        assert_eq!((d.codepage, d.osc.analog[0].id.as_str()), (Encoding::Utf8, "Струм"));
        assert_eq!(crate::setfile::cp_code(d.codepage), 1, "saved back as 1");
        // Any other non-zero value is its own encoding too (no KOI8-R the
        // combo would not list).
        d.set_codepage(Encoding::Cp866);
        d.set.tree.section_mut("Oscillogram").set_int("Codepage", 20866);
        app.apply_record_settings(ai, false);
        assert_eq!(app.docs[ai].codepage, Encoding::Utf8);
        // The combo's third entry is current for it, so the stored value is
        // not kept as a raw one (ours, #82): 1 is written back.
        let d = &app.docs[ai];
        assert_eq!(d.cp_raw, None);
        let mut t = d.set.tree.clone();
        d.store_settings(&mut t, false);
        assert_eq!(t.section("Oscillogram").unwrap().get_int("Codepage", -1), 1);
    }

    /// #82: a stored `Oscillogram/Codepage` other than 0 / 1 (65001, 20866,
    /// 866 …) is ANSI for the original — `0x4543e0` only tests for 0 — so a
    /// record read in 1251 / 866 shows its names as read, the combo shows
    /// no entry (`SetCurSel` out of range), and the value is written back
    /// as it is until «Кодировка» is used.
    #[test]
    fn a_stored_65001_or_20866_is_ansi_and_written_back() {
        let saved = |d: &Doc| {
            let mut t = d.set.tree.clone();
            d.store_settings(&mut t, false);
            t.section("Oscillogram").unwrap().get_int("Codepage", -1)
        };
        for stored in [65001, 20866, 866, 2] {
            let mut app = demo_app();
            let ai = app.active.unwrap();
            let d = &mut app.docs[ai];
            d.osc.analog[0].id = "Струм".into();
            d.init_record_text(Encoding::Cp1251);
            d.init_set_keys();
            d.set_codepage(Encoding::Cp866);
            d.set.tree.section_mut("Oscillogram").set_int("Codepage", stored);
            app.apply_record_settings(ai, false);
            let d = &mut app.docs[ai];
            assert_eq!(d.codepage, Encoding::Cp1251, "{stored}: ANSI");
            assert_eq!(d.osc.analog[0].id, "Струм", "{stored}: the names as read");
            assert_eq!(codepage_items(d.read_enc).len(), 2);
            assert_eq!(d.cp_raw, Some(stored));
            assert_eq!(saved(d), stored, "written back as stored");
            // Using «Кодировка» replaces it by the combo's index.
            d.cp_raw = None;
            assert_eq!(saved(d), 1);
            d.set_codepage(Encoding::Cp866);
            assert_eq!(saved(d), 0);
        }
        // 0 and 1 are the combo's own entries.
        let mut app = demo_app();
        let ai = app.active.unwrap();
        app.docs[ai].set.tree.section_mut("Oscillogram").set_int("Codepage", 1);
        app.apply_record_settings(ai, false);
        assert_eq!(app.docs[ai].cp_raw, None);
    }

    #[test]
    fn hdr_comment_only_for_cfg_and_cff() {
        let mut app = demo_app();
        assert!(!app.hdr_comment_enabled());
        app.active_doc_mut().unwrap().path = Some("/x/rec.osc".into());
        app.dispatch(Command::HdrComment);
        assert!(app.dlg.hdr_edit.is_none());
        app.active_doc_mut().unwrap().path = Some("/x/rec.CFG".into());
        app.dispatch(Command::HdrComment);
        assert!(app.dlg.hdr_edit.is_some());
        let d = app.active_doc_mut().unwrap();
        // The dialog edits `Comments/UserData`, never the `.hdr` text.
        d.hdr = Some("hdr".into());
        app.dlg.hdr_edit = None;
        app.dispatch(Command::HdrComment);
        assert_eq!(app.dlg.hdr_edit.as_ref().unwrap().text, "");
        app.dlg.hdr_edit.as_mut().unwrap().text = "a\r\nb".into();
        let d = app.active_doc_mut().unwrap();
        d.set.modified = false;
        let st = app.dlg.hdr_edit.take().unwrap();
        let d = app.active_doc_mut().unwrap();
        d.set.tree.section_mut("Comments").set_str("UserData", &st.text.replace('\r', ""));
        assert_eq!(d.set.tree.section("Comments").unwrap().get_str("UserData", ""), "a\nb");
        assert_eq!(d.hdr.as_deref(), Some("hdr"));
        let d = app.active_doc_mut().unwrap();
        d.hdr = Some("строка 1\nстрока 2".into());
        d.hdr_cp = Encoding::Cp866;
        let b = d.hdr_file_bytes().unwrap();
        assert_eq!(comtrade::decode_bytes(&b, Encoding::Cp866), "строка 1\r\nстрока 2");
    }

    #[test]
    fn duration_is_percent_g_seconds() {
        let osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        let t = duration_text(&osc);
        assert!(t.ends_with(" с."), "{t}");
        assert_eq!(t, format!("{} с.", fvb_core::views::fmt_g(osc.duration())));
    }

    /// The bar renders headless with and without a document.
    #[test]
    fn param_bar_renders() {
        for with_doc in [false, true] {
            let mut app = if with_doc { demo_app() } else { App::new() };
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1320.0, 860.0))), ..Default::default() }, |ctx| {
                egui::TopBottomPanel::top("parambar").exact_height(75.0).show(ctx, |ui| app.param_bar_ui(ui));
            });
        }
    }
}

//! The record settings file (`<base>.set`) and the document-modified flag
//! (track S; `fvb_core::recset` holds the file format).
//!
//! Read (`0x44f5b0`, from the `.cfg` / `.osc` loaders): the tree replaces
//! the document's, then comments (unless loading settings by 32860),
//! `Oscillogram` (rate, code page — a stored one wins over the guess), every
//! channel's section, the formula channels, ОМП, the diagram pairs; the
//! diagram windows and the view read their own sections (`Circle Chart`,
//! `Diff Chart`, `Vector Chart`, `Frequency Chart`, `View`) when they attach,
//! and their toggles write straight into the tree.
//!
//! Written only by `OnSaveDocument` (`0x44c940` → `0x44cbc0`): Save onto the
//! document's own path rewrites only the `.set`, Save As writes the record
//! and its `.set`; success clears the modified flag. Nothing is written on a
//! change or on close: closing a modified document asks MFC's «save changes»
//! question (`SaveModified`, not overridden except for `.sto`), and so does
//! leaving the program, once per modified document.

use crate::app::{App, ChanKey, Command, Doc, TimeUnit, ValueMode};
use crate::comments::{Comment, CommentKind};
use eframe::egui;
use fvb_core::comtrade::{self, Encoding};
use fvb_core::recset::{self, flag, ChannelRec, CommentKindRec, CommentRec, PairRec, SetNode, SetTree};

/// Per document: the settings tree (`doc+0x300`), the modified flag
/// (`doc+0x48`) and our change detector.
#[derive(Debug, Default, Clone)]
pub struct DocSet {
    pub tree: SetTree,
    pub modified: bool,
    /// The settings as last saved / loaded / seen (see `App::track_settings`).
    pub print: String,
    /// Section keys of the file channels (analog, then digital): the file's
    /// name bytes read in the ANSI code page (`CStringW(ch+0x64)`).
    pub keys: Vec<String>,
}

/// The answer pending in the «save changes» question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavePrompt {
    pub doc: usize,
    /// Asked while leaving the program.
    pub exit: bool,
}

/// Frame-level state of this module.
#[derive(Debug, Default)]
pub struct SetUi {
    pub prompt: Option<SavePrompt>,
    /// Documents still to ask about while leaving the program.
    pub exit_queue: Option<Vec<usize>>,
    /// Every question answered: let the window close.
    pub exit_ok: bool,
    /// The document whose diagram / view sections the app fields hold, and
    /// those values as last synchronised.
    pub sync: Option<(usize, ViewSnap)>,
}

/// The settings the diagram windows and the view keep in the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewSnap {
    pub circle: recset::PlaneChart,
    pub diff: recset::PlaneChart,
    pub vector: recset::VectorChart,
    pub freq: recset::FrequencyChart,
    pub y_analog: i32,
    pub ruler: i32,
    pub grid: bool,
}

/// `Codepage`: the loader stores the guess `0x459e60` (`0x450d56`) — 1
/// when the text has more Cyrillic letters as it stands than converted
/// from OEM, i.e. 1 = the ANSI code page (1251), 0 = the OEM one (866), a
/// tie 0 (seen on the VM: a CP1251 record's new `.set` says 1); other values
/// are code page numbers (`0x4543e0`).
/// ANSI case folding for name lookups ignoring case (`0x42f020`: mfc #2955,
/// `_mbsicmp` under the Russian_Russia.1251 locale): ASCII and every
/// Windows-1251 letter pair — Russian, and the Ukrainian, Belarusian and
/// Serbian ones (INFERRED: the CRT's 1251 case tables).
pub fn ansi_fold(b: &[u8]) -> Vec<u8> {
    b.iter()
        .map(|&c| match c {
            b'A'..=b'Z' => c + 32,
            0xC0..=0xDF => c + 32,
            0x80 => 0x90,
            0x81 => 0x83,
            0x8A | 0x8C..=0x8F => c + 0x10,
            0xA1 => 0xA2,
            0xA3 => 0xBC,
            0xA5 => 0xB4,
            0xA8 | 0xAA | 0xAF => c + 0x10,
            0xB2 => 0xB3,
            0xBD => 0xBE,
            _ => c,
        })
        .collect()
}

pub fn cp_code(e: Encoding) -> i32 {
    i32::from(e != Encoding::Cp866)
}

/// The record code page a stored `Oscillogram/Codepage` selects on a record
/// read in 1251 / 866: 0 is OEM, any other value ANSI (`0x4543e0` tests
/// the value against 0 only, so 65001 / 20866 are ANSI too, #82).
pub fn cp_of(code: i32) -> Encoding {
    match code {
        0 => Encoding::Cp866,
        _ => Encoding::Cp1251,
    }
}

/// COLORREF ↔ colour.
fn colorref(c: egui::Color32) -> u32 {
    u32::from(c.r()) | u32::from(c.g()) << 8 | u32::from(c.b()) << 16
}

fn from_colorref(v: u32) -> egui::Color32 {
    egui::Color32::from_rgb(v as u8, (v >> 8) as u8, (v >> 16) as u8)
}

/// Formula channels (the calculation manager `doc+0x190` holds only these;
/// merged, pending and ОМП channels carry no formula).
pub(crate) fn is_formula(c: &crate::app::Computed) -> bool {
    c.formula != "ОМП" && c.formula != crate::merge_ui::PENDING
}

/// The commands whose handlers call `SetModifiedFlag` even when the state
/// ends up as it was (toggles; spec §4.6, §4.10, zoom `0x45c4f0`, 32860).
/// Everything else that changes saved state is caught by the change
/// detector.
pub fn marks_modified(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::ToggleFlag(_)
            | Command::SetValueMode(_)
            | Command::StatsRow(_)
            | Command::ZoomX(_)
            | Command::ZoomY(_)
            | Command::SetPrimaryValues(_)
    )
}

/// `SaveModified` (`0x454fa0`): a `.sto` record closes without a question;
/// otherwise MFC asks when the document is modified.
pub fn needs_save_prompt(d: &Doc) -> bool {
    let sto = d.path.as_ref().is_some_and(|p| p.to_string_lossy().to_lowercase().ends_with(".sto"));
    d.set.modified && !sto
}

/// The format `OnSaveDocument` dispatches on (last four characters
/// lowercased); anything else is refused.
fn save_kind(p: &std::path::Path) -> Option<&'static str> {
    let s = p.to_string_lossy().to_lowercase();
    [".cfg", ".osc", ".cff"].into_iter().find(|e| s.ends_with(e))
}

/// Save As's filter (`CDocManagerEx::DoPromptFileName` `0x447e30`, by the
/// record path's last four characters): string 394 for an `.osc` record,
/// 569 for the rest (`.cfg`, `.cff`, `.sto`; 399 for a `.zip`, which
/// cannot be opened).
pub fn save_filters(record: Option<&std::path::Path>) -> [(&'static str, &'static str); 2] {
    let osc = record.is_some_and(|p| p.to_string_lossy().to_lowercase().ends_with(".osc"));
    if osc {
        [("Осциллограммы COMTRADE (*.cfg)", "cfg"), ("Осциллограммы OSC2 (*.osc)", "osc")]
    } else {
        [("Осциллограммы COMTRADE (*.cfg)", "cfg"), ("Осциллограммы COMTRADE'13 (*.cff)", "cff")]
    }
}

/// The name after OK (`0x448208`–`0x448368`): lowered, tested against
/// every extension of the filter list; one found that differs from the
/// chosen filter's is cut (four characters) and the chosen one appended,
/// none found the chosen one is appended ("x.txt" → "x.txt.cfg"), the
/// chosen one is kept. `chosen` None (a dialog that does not report it):
/// any listed extension kept, else the first filter's.
pub fn save_target(p: std::path::PathBuf, exts: &[&str], chosen: Option<usize>) -> std::path::PathBuf {
    let s = p.to_string_lossy().to_lowercase();
    let found = exts.iter().find(|e| s.ends_with(&format!(".{e}")));
    let want = chosen.and_then(|i| exts.get(i)).copied();
    match (found, want) {
        (Some(_), None) => p,
        (Some(f), Some(w)) if *f == w => p,
        (Some(_), Some(w)) => {
            let t = p.to_string_lossy();
            let cut: String = t.chars().take(t.chars().count().saturating_sub(4)).collect();
            std::path::PathBuf::from(format!("{cut}.{w}"))
        }
        (None, w) => {
            let mut o = p.into_os_string();
            o.push(format!(".{}", w.unwrap_or(exts[0])));
            o.into()
        }
    }
}

impl Doc {
    /// The section key of file channel `i` (analog, then digital).
    pub(crate) fn file_key(&self, i: usize) -> String {
        if let Some(k) = self.set.keys.get(i) {
            return k.clone();
        }
        let id = if i < self.osc.analog.len() { &self.osc.analog[i].id } else { &self.osc.digital[i - self.osc.analog.len()].id };
        // Its bytes as read (`recdlg::name_bytes_of`), as 1251 text.
        comtrade::decode_bytes(&crate::recdlg::name_bytes_of(id, self.read_enc, self.codepage), Encoding::Cp1251)
    }

    /// A group member's name as the member set keeps it (`+0x64`, ANSI
    /// bytes, `0x42e270`): a file channel's file name (a rename leaves it),
    /// a computed one's name.
    pub fn member_name(&self, k: ChanKey) -> Vec<u8> {
        let text = match k {
            ChanKey::Analog(i) => self.file_key(i),
            ChanKey::Computed(j) => self.computed[j].name.clone(),
            ChanKey::Digital(d) => self.file_key(self.osc.analog.len() + d),
        };
        comtrade::encode_text(&text, Encoding::Cp1251)
    }

    /// The file-name keys after channels were appended past `nb` analog /
    /// `nd` digital ones and are about to move to `at_a` / `at_d` (merge,
    /// the ОМП's flags): each key follows its channel, a new one keyed by
    /// its own name. Called before the channels move.
    pub fn move_appended_keys(&mut self, at_a: usize, nb: usize, at_d: usize, nd: usize) {
        // The names' bytes kept by «Кодировка» move alike (a new name's are
        // its text shown in the record's code page taken back to the bytes
        // as read, `recdlg::shown_bytes_of`).
        if self.name_raw.len() == 2 + nb + nd {
            let mut old = std::mem::take(&mut self.name_raw);
            let mut d = old.split_off(2 + nb);
            let mut a = old.split_off(2);
            let (read, cp) = (self.read_enc, self.codepage);
            // The bytes are the shown ones (`shown_bytes_of`: a best-fitted
            // character keeps the byte it reads back from); whether the name
            // is an alias is `added_name`'s (a name the code page can't hold).
            let own = |id: &String| crate::recdlg::added_name(id, crate::recdlg::shown_bytes_of(id, read, cp), read, cp);
            a.extend(self.osc.analog[nb..].iter().map(|c| own(&c.id)));
            d.extend(self.osc.digital[nd..].iter().map(|c| own(&c.id)));
            a[at_a..].rotate_right(self.osc.analog.len() - nb);
            d[at_d..].rotate_right(self.osc.digital.len() - nd);
            old.extend(a);
            old.extend(d);
            self.name_raw = old;
        }
        if self.set.keys.is_empty() {
            return;
        }
        let old = std::mem::take(&mut self.set.keys);
        let own = |id: &str| comtrade::decode_bytes(&crate::recdlg::shown_bytes_of(id, self.read_enc, self.codepage), Encoding::Cp1251);
        let mut a: Vec<String> = self.osc.analog.iter().enumerate().map(|(i, c)| old.get(i).filter(|_| i < nb).cloned().unwrap_or_else(|| own(&c.id))).collect();
        // The ОМП's flags are named from the program's own strings, 1251
        // text in any record (a UTF-8 record's `.set` still has «КЗ фазы»,
        // not the UTF-8 bytes read as 1251; a clone run, #28).
        let omp = |k: usize| self.dig_flags.get(k).is_some_and(|f| f.omp);
        let mut d: Vec<String> = self.osc.digital.iter().enumerate().map(|(k, c)| old.get(nb + k).filter(|_| k < nd).cloned().unwrap_or_else(|| if omp(k) { c.id.clone() } else { own(&c.id) })).collect();
        let (ka, kd) = (a.len() - nb, d.len() - nd);
        a[at_a..].rotate_right(ka);
        d[at_d..].rotate_right(kd);
        a.extend(d);
        self.set.keys = a;
    }

    /// Drop the keys of the digital channels not kept (the ОМП's clear).
    pub fn retain_digital_keys(&mut self, keep: &[bool]) {
        let na = self.osc.analog.len();
        if self.name_raw.len() == 2 + na + keep.len() {
            let mut it = keep.iter();
            let digital: Vec<_> = self.name_raw.split_off(2 + na).into_iter().filter(|_| *it.next().unwrap_or(&true)).collect();
            self.name_raw.extend(digital);
        }
        if self.set.keys.len() <= na {
            return;
        }
        let mut it = keep.iter();
        let digital = self.set.keys.split_off(na).into_iter().filter(|_| *it.next().unwrap_or(&true));
        self.set.keys.extend(digital);
    }

    /// Remember the channels' file names (after the names were decoded).
    pub fn init_set_keys(&mut self) {
        self.set.keys.clear();
        let keys: Vec<String> = (0..self.osc.analog.len() + self.osc.digital.len()).map(|i| self.file_key(i)).collect();
        self.set.keys = keys;
        self.ensure_name_raw();
    }

    /// The names as read, taken from the names shown now when none were
    /// kept yet (no «Кодировка» so far: they are the file's, in the
    /// record's code page).
    pub fn ensure_name_raw(&mut self) {
        if !self.name_raw.is_empty() {
            return;
        }
        let (read, cp) = (self.read_enc, self.codepage);
        let own = |s: &String| crate::app::RawName { bytes: crate::recdlg::shown_bytes_of(s, read, cp), seen: s.clone(), alias: false };
        let v: Vec<_> = [&self.osc.station, &self.osc.device].into_iter().chain(self.osc.analog.iter().map(|c| &c.id)).chain(self.osc.digital.iter().map(|c| &c.id)).map(own).collect();
        self.name_raw = v;
    }

    /// Channel `k` now shows an alias (a rename, `AliasName`) or, `on`
    /// false, its file's name again.
    pub fn mark_alias(&mut self, k: ChanKey, on: bool) {
        let read = self.read_enc;
        let p = match k {
            ChanKey::Analog(i) => 2 + i,
            ChanKey::Digital(d) => 2 + self.osc.analog.len() + d,
            ChanKey::Computed(j) => {
                let cp = self.codepage;
                if let Some(c) = self.computed.get_mut(j).filter(|c| c.formula == crate::merge_ui::PENDING) {
                    c.name_raw.get_or_insert_with(|| crate::recdlg::added_name(&c.name, crate::recdlg::shown_bytes_of(&c.name, read, cp), read, cp)).alias = on;
                }
                return;
            }
        };
        self.ensure_name_raw();
        if self.name_raw.len() == 2 + self.osc.analog.len() + self.osc.digital.len() {
            self.name_raw[p].alias = on;
        }
    }

    /// File channel `i`'s name as read, in the record's code page (as
    /// `Doc::set_codepage` decodes the names).
    fn file_name(&self, i: usize) -> String {
        // The keys are the names' bytes as read, as 1251 text (taken at the
        // open, `init_set_keys`, while the names showed as read).
        let bytes = comtrade::encode_text(&self.file_key(i), Encoding::Cp1251);
        crate::recdlg::decode_name(&bytes, self.read_enc, self.codepage)
    }

    /// A renamed file channel's original name (`+0x64`, decoded,
    /// `0x4292e0`) — `None` while the name is the file's.
    pub fn original_name(&self, k: ChanKey) -> Option<String> {
        let (i, id) = match k {
            ChanKey::Analog(i) => (i, &self.osc.analog.get(i)?.id),
            ChanKey::Digital(d) => (self.osc.analog.len() + d, &self.osc.digital.get(d)?.id),
            ChanKey::Computed(_) => return None,
        };
        // After a switch the name's bytes decoded now (the keys are lossy
        // once an 866 reading best-fitted them: «¦» comes back as «?»).
        let kept = (self.name_raw.len() == 2 + self.osc.analog.len() + self.osc.digital.len()).then(|| self.name_raw.get(2 + i)).flatten();
        let file = kept.map_or_else(|| self.file_name(i), |r| r.seen.clone());
        (file != *id).then_some(file)
    }

    /// The displayed name of a channel for the comment records.
    fn key_name(&self, k: ChanKey) -> String {
        crate::chandlg::row_name(self, k)
    }

    fn key_by_name(&self, n: &str) -> Option<ChanKey> {
        crate::chandlg::rows(self).into_iter().find(|&k| self.key_name(k) == n)
    }

    /// Delete channels (`remove`), as the remove-by-name routine `0x42ecf0`
    /// does: when anything went, the review list is swept (`0x440800`) —
    /// a level or an area stays while its channel(s) still resolve by name
    /// (`0x42edf0`), re-bound to their new places; a moment always stays;
    /// an element already naming a missing channel goes; the hot element
    /// is cleared. Returns whether any channel went.
    pub fn removing_channels(&mut self, remove: impl FnOnce(&mut Doc)) -> bool {
        self.removing_channels_inner(remove, false)
    }

    /// The sweep alone (`0x440800`), as the rollback of a failed quick analysis runs it after deleting the channels
    /// it had added (`0x416f1b`, `0x42ecf0` per name; #31): the batch's channels are never added in ours, so only
    /// the sweep is left.
    pub fn sweep_review_list(&mut self) {
        self.removing_channels_inner(|_| {}, true);
    }

    fn removing_channels_inner(&mut self, remove: impl FnOnce(&mut Doc), force: bool) -> bool {
        let before = (self.computed.len(), self.osc.digital.len());
        let names: Vec<Vec<String>> = self
            .comments
            .iter()
            .map(|c| match c.kind {
                CommentKind::Level { chan, .. } => vec![self.key_name(chan)],
                CommentKind::Area { chan_from, chan_to, .. } => vec![self.key_name(chan_from), self.key_name(chan_to)],
                _ => Vec::new(),
            })
            .collect();
        remove(self);
        if !force && (self.computed.len(), self.osc.digital.len()) == before {
            return false;
        }
        // The places behind the removed ones moved (`0x42b5f0`).
        self.recolor();
        let comments = std::mem::take(&mut self.comments);
        self.comments = comments
            .into_iter()
            .zip(names)
            .filter_map(|(mut c, n)| {
                match &mut c.kind {
                    CommentKind::Moment { .. } => {}
                    CommentKind::Level { chan, .. } => *chan = self.key_by_name(&n[0])?,
                    CommentKind::Area { chan_from, chan_to, .. } => {
                        *chan_from = self.key_by_name(&n[0])?;
                        *chan_to = self.key_by_name(&n[1])?;
                    }
                    CommentKind::Kept(_) => return None,
                }
                Some(c)
            })
            .collect();
        self.comment_hot = None;
        true
    }

    /// Digital channel `k` shows an alias now (`+0x6c` non-empty): kept as
    /// typed across «Кодировка», or a name that differs from the file's.
    pub(crate) fn digital_shows_alias(&self, k: usize) -> bool {
        let p = 2 + self.osc.analog.len() + k;
        match self.name_raw.get(p).filter(|_| self.name_raw.len() == 2 + self.osc.analog.len() + self.osc.digital.len()) {
            Some(r) => r.alias,
            None => !self.alias(self.osc.analog.len() + k, &self.osc.digital[k].id).is_empty(),
        }
    }

    /// `AliasName`: the displayed name when it differs from the file's.
    fn alias(&self, i: usize, id: &str) -> String {
        let file = self.file_name(i);
        if id == file {
            String::new()
        } else {
            id.to_string()
        }
    }

    fn analog_flags(&self, i: usize) -> u32 {
        let c = &self.chans[i];
        let bit = |on: bool, b: u32| if on { b } else { 0 };
        bit(c.visible, flag::VISIBLE)
            | bit(c.on_freq, flag::FREQUENCY)
            | bit(c.on_vector, flag::VECTOR)
            | bit(c.on_circle, flag::CIRCLE)
            | bit(c.stats.minmax, flag::MINMAX)
            | bit(c.stats.a1, flag::A1)
            | bit(c.stats.rms, flag::RMS_ROW)
            | mode_bits(c.mode)
            | bit(c.remove_dc, flag::REMOVE_DC)
            | bit(c.invert, flag::INVERT)
            | bit(c.adc_counts, flag::ADC)
    }

    fn set_analog_flags(&mut self, i: usize, f: u32) {
        let c = &mut self.chans[i];
        c.visible = f & flag::VISIBLE != 0;
        c.on_freq = f & flag::FREQUENCY != 0;
        c.on_vector = f & flag::VECTOR != 0;
        c.on_circle = f & flag::CIRCLE != 0;
        c.stats.minmax = f & flag::MINMAX != 0;
        c.stats.a1 = f & flag::A1 != 0;
        c.stats.rms = f & flag::RMS_ROW != 0;
        c.mode = mode_of(f);
        c.remove_dc = f & flag::REMOVE_DC != 0;
        c.invert = f & flag::INVERT != 0;
        c.adc_counts = f & flag::ADC != 0;
    }

    fn computed_flags(&self, j: usize) -> u32 {
        let c = &self.computed[j];
        let bit = |on: bool, b: u32| if on { b } else { 0 };
        bit(c.visible, flag::VISIBLE)
            | bit(c.on_freq, flag::FREQUENCY)
            | bit(c.on_vector, flag::VECTOR)
            | bit(c.on_circle, flag::CIRCLE)
            | bit(c.stats.minmax, flag::MINMAX)
            | bit(c.stats.a1, flag::A1)
            | bit(c.stats.rms, flag::RMS_ROW)
            | mode_bits(c.mode)
            | bit(c.remove_dc, flag::REMOVE_DC)
            | bit(c.invert, flag::INVERT)
            | flag::COMPUTED
    }

    /// A digital channel's ShowFlags (the whole ch+0x60: visibility, its
    /// mode bits — 0x80 by default —, DC, invert).
    fn digital_flags(&self, k: usize) -> u32 {
        let f = self.dig(k);
        let bit = |on: bool, b: u32| if on { b } else { 0 };
        bit(self.digital_visible.get(k).copied().unwrap_or(true), flag::VISIBLE) | mode_bits(f.mode) | bit(f.remove_dc, flag::REMOVE_DC) | bit(f.invert, flag::INVERT) | bit(self.dig_computed(k), flag::COMPUTED)
    }

    /// The record as the `.cfg` / `.cff` writers see it: the ОМП's flag
    /// channels (bit 13) left out (VM: `7,6A,1D` after a run), the others
    /// numbered by their place among all digital channels (VM: «4,
    /// <name> …» after the three flags); likewise the analog channels by
    /// their place among all analog-kind records, a pasted lane before them
    /// counting (VM, #28).
    fn file_record(&self) -> (std::borrow::Cow<'_, fvb_core::Oscillogram>, comtrade::Numbers) {
        let pending_before = |analog: bool| -> Vec<usize> {
            (0..self.computed.len())
                .filter(|&j| self.computed[j].formula == crate::merge_ui::PENDING && self.pending_digital(j) != analog)
                .filter_map(|j| match (self.computed[j].before, analog) {
                    (Some(ChanKey::Analog(a)), true) => Some(a),
                    (Some(ChanKey::Digital(k)), false) => Some(k),
                    _ => None,
                })
                .collect()
        };
        let (pa, pd) = (pending_before(true), pending_before(false));
        let analog: Vec<usize> = (0..self.osc.analog.len()).map(|i| i + 1 + pa.iter().filter(|&&a| a <= i).count()).collect();
        let place = |k: usize| k + 1 + pd.iter().filter(|&&d| d <= k).count();
        if !(0..self.osc.digital.len()).any(|k| self.dig_computed(k)) {
            let digital = if pd.is_empty() { Vec::new() } else { (0..self.osc.digital.len()).map(place).collect() };
            return (std::borrow::Cow::Borrowed(&self.osc), comtrade::Numbers { analog: if pa.is_empty() { Vec::new() } else { analog }, digital });
        }
        let keep: Vec<usize> = (0..self.osc.digital.len()).filter(|&k| !self.dig_computed(k)).collect();
        let mut osc = self.osc.clone();
        osc.digital = keep.iter().map(|&k| self.osc.digital[k].clone()).collect();
        (std::borrow::Cow::Owned(osc), comtrade::Numbers { analog: if pa.is_empty() { Vec::new() } else { analog }, digital: keep.iter().map(|&k| place(k)).collect() })
    }

    /// The modelled bits replace their stored values; the others (bits we
    /// do not model, a stored `NetFreq`, …) are kept.
    fn merged_flags(t: &SetTree, key: &str, ours: u32, known: u32) -> u32 {
        // Without a stored word the constructor's 0xE1 (`0x422536`): its
        // СКЗ / A1 bits stay on a discrete channel, which nothing reads.
        let old = t.section(key).and_then(|s| s.attrs.get("ShowFlags")).map_or(0xE1, |v| recset::wtoi(v) as u32);
        ours | (old & !known)
    }

    /// `0x44cbc0` minus the serialisation: update the tree in place from the
    /// document. `print` = the change detector's variant (visibility, which
    /// 32845–32848 change without marking the document, and ОМП / pending
    /// channels left out).
    pub fn store_settings(&self, t: &mut SetTree, print: bool) {
        // Sections and subtrees the original's reads get or create
        // (`0x48dc80`; the block load `0x409030`) are saved even empty.
        for k in ["Frequency Chart", "Vector Chart", "View"] {
            t.section_mut(k);
        }
        for k in ["Circle Chart", "Diff Chart"] {
            t.section_mut(k).child_mut("Selected Block");
        }
        // The calculation manager's list (kill-active's rows and their order included, `0x4153a0`).
        let calc = self.definition_pairs();
        recset::write_calculated(t.section_mut("CalculatedChannels"), &calc);
        recset::write_omp(t.section_mut("WrongPlaceFinding"), &self.omp.params);
        let mask = if print { !flag::VISIBLE } else { u32::MAX };
        let known_a = (1 << 13) - 1;
        for i in 0..self.osc.analog.len() {
            let key = self.file_key(i);
            let a = &self.osc.analog[i];
            let rec = ChannelRec {
                show_flags: Self::merged_flags(t, &key, self.analog_flags(i), known_a) & mask,
                primary_mul: a.primary,
                secondary_mul: a.secondary,
                alias: self.alias(i, &a.id),
                custom_color: self.chans[i].custom_color.then(|| colorref(self.chans[i].color)),
            };
            recset::write_channel(t.section_mut(&key), &rec);
            recset::write_net_freq(t.section_mut(&key), &a.own_freq);
        }
        let na = self.osc.analog.len();
        for k in 0..self.osc.digital.len() {
            // The ОМП run's phase lanes are not a change (it never marks
            // the document, `0x4a00b0`).
            if print && self.dig(k).omp {
                continue;
            }
            let key = self.file_key(na + k);
            let known_d = flag::VISIBLE | flag::MODE_INSTANT | flag::MODE_RMS | flag::MODE_HARMONIC | flag::REMOVE_DC | flag::INVERT | flag::COMPUTED;
            let rec = ChannelRec {
                show_flags: Self::merged_flags(t, &key, self.digital_flags(k), known_d) & mask,
                primary_mul: 1.0,
                secondary_mul: 1.0,
                // The ОМП's flags show their stored names: no alias, in any
                // code page (#66).
                // A flag the run made has none (#66); one that took over a
                // channel showing a user alias keeps it (`0x425f90`, #114).
                alias: match self.dig(k) {
                    f if f.omp && f.omp_alias => self.osc.digital[k].id.clone(),
                    f if f.omp => String::new(),
                    _ => self.alias(na + k, &self.osc.digital[k].id),
                },
                custom_color: self.dig(k).color.map(colorref),
            };
            recset::write_channel(t.section_mut(&key), &rec);
        }
        for (j, c) in self.computed.iter().enumerate() {
            if print && !is_formula(c) {
                continue;
            }
            let known_c = flag::VISIBLE | flag::VECTOR | flag::CIRCLE | flag::MINMAX | flag::A1 | flag::RMS_ROW | flag::MODE_INSTANT | flag::MODE_RMS | flag::MODE_HARMONIC | flag::REMOVE_DC | flag::INVERT | flag::COMPUTED;
            // A pasted lane is bit 14, not bit 13, and writes its own frequency
            // vector as `NetFreq` (a clone run, #28).
            let pending = c.formula == crate::merge_ui::PENDING;
            // A pasted lane's section is named by its raw name's bytes read as
            // 1251, as a file channel's (a clone run: «(вст.)» pasted in a record
            // shown in 866 is saved as «(ўбв.)»).
            let key = match (&c.name_raw, pending) {
                (Some(r), true) => comtrade::decode_bytes(&r.bytes, Encoding::Cp1251),
                _ => c.name.clone(),
            };
            let (flags, known) = if pending {
                ((self.computed_flags(j) & !flag::COMPUTED) | flag::PENDING | if c.adc_counts { flag::ADC } else { 0 }, (known_c & !flag::COMPUTED) | flag::PENDING | flag::ADC)
            } else {
                (self.computed_flags(j), known_c)
            };
            // A pasted analog lane is a copy of the channel's record: its
            // ratios are the source's (a clone run).
            let (primary_mul, secondary_mul) = match pending.then(|| self.pending_of(j).map(|p| &p.pasted.channel)).flatten() {
                Some(Ok(a)) => (a.primary, a.secondary),
                _ => (1.0, 1.0),
            };
            let rec = ChannelRec {
                show_flags: Self::merged_flags(t, &key, flags, known) & mask,
                primary_mul,
                secondary_mul,
                alias: String::new(),
                // `ch+0x140` as it is (a VM run, L9.213): a place colour
                // writes CustomColorSet 0, colorCustom 0.
                custom_color: c.custom_color.then(|| colorref(c.color)),
            };
            recset::write_channel(t.section_mut(&key), &rec);
            if let Some(Ok(a)) = pending.then(|| self.pending_of(j).map(|p| &p.pasted.channel)).flatten() {
                recset::write_net_freq(t.section_mut(&key), &a.own_freq);
            }
        }
        let o = t.section_mut("Oscillogram");
        // The stored value as loaded while «Кодировка» was not used since
        // (`+0x6e8` is written back as it is, `0x44ccf2`).
        o.set_int("Codepage", self.cp_raw.unwrap_or_else(|| cp_code(self.codepage)));
        // rec+0x228, `%i` (`0x44cbc0`).
        o.set_int("DiscretisationFrequensy", self.osc.rate_r as i32);
        let pairs = |v: &[crate::diagram::ChanPair]| -> Vec<PairRec> { v.iter().map(|p| PairRec { a: p.a.clone(), b: p.b.clone(), visible: p.on }).collect() };
        // `doc+0x878` holds (Iторм `BrakeCurr`, Iдиф `DiffCurr`); our
        // differential pairs are (Iдиф, Iторм) (L9.81).
        let swapped = |v: &[crate::diagram::ChanPair]| -> Vec<PairRec> { v.iter().map(|p| PairRec { a: p.b.clone(), b: p.a.clone(), visible: p.on }).collect() };
        recset::write_pairs(t.section_mut("Diff Chart"), recset::DIFF_KEYS, &swapped(&self.dif_pairs));
        recset::write_pairs(t.section_mut("Circle Chart"), recset::CIRCLE_KEYS, &pairs(&self.rx_pairs));
        let items: Vec<CommentRec> = self.comments.iter().map(|c| self.comment_rec(c)).collect();
        recset::write_comments(t.section_mut("Comments"), &items);
        // The summary comment a paste makes is a `.hdr` text with its code
        // page (`Comments/Codepage`, 1), set at every paste, whatever the reader
        // guessed before (a clone run, #28).
        if !print && self.hdr.is_some() && self.hdr_pasted {
            t.section_mut("Comments").set_int("Codepage", 1);
        }
    }

    pub(crate) fn comment_rec(&self, c: &Comment) -> CommentRec {
        let kind = match &c.kind {
            CommentKind::Kept(r) => return (**r).clone(),
            CommentKind::Moment { pos } => CommentKindRec::Moment { position: *pos as i32 },
            CommentKind::Level { chan, value, from, to } => CommentKindRec::Level {
                channel: self.key_name(*chan),
                y: *value,
                from: *from as i32,
                to: *to as i32,
            },
            CommentKind::Area { chan_from, chan_to, from, to } => CommentKindRec::Area {
                channel_from: self.key_name(*chan_from),
                channel_to: self.key_name(*chan_to),
                from: *from as i32,
                to: *to as i32,
            },
        };
        CommentRec { kind, text: c.text.clone(), locked: c.locked, visible: !c.hidden }
    }

    /// An element read back; one naming a channel we do not have stays in
    /// its place as [`CommentKind::Kept`] (`0x43fcf0` appends every item).
    pub(crate) fn comment_of(&self, r: &CommentRec) -> Comment {
        let s = |v: i32| v.max(0) as usize;
        let kind = match &r.kind {
            CommentKindRec::Moment { position } => Some(CommentKind::Moment { pos: s(*position) }),
            CommentKindRec::Level { channel, y, from, to } => self.key_by_name(channel).map(|chan| CommentKind::Level { chan, value: *y, from: s(*from), to: s(*to) }),
            CommentKindRec::Area { channel_from, channel_to, from, to } => self.key_by_name(channel_from).zip(self.key_by_name(channel_to)).map(|(chan_from, chan_to)| {
                CommentKind::Area { chan_from, chan_to, from: s(*from), to: s(*to) }
            }),
        };
        let kind = kind.unwrap_or_else(|| CommentKind::Kept(Box::new(r.clone())));
        Comment { kind, text: r.text.clone(), locked: r.locked, hidden: !r.visible }
    }

    /// The change detector's view of the saved state.
    pub fn settings_print(&self) -> String {
        let mut t = self.set.tree.clone();
        self.store_settings(&mut t, true);
        // The quick-analysis pages save into the tree with no modified mark
        // (L9.55; VM: the sheet opened and cancelled, no question at the
        // close).
        for k in crate::quick_ui::PAGE_KEYS {
            t.sections.remove(k);
        }
        t.to_xml()
    }
}

/// The display-mode bits 7–9.
fn mode_bits(m: ValueMode) -> u32 {
    match m {
        ValueMode::Instant => flag::MODE_INSTANT,
        ValueMode::Rms => flag::MODE_RMS,
        ValueMode::FirstHarmonic => flag::MODE_HARMONIC,
    }
}

fn mode_of(f: u32) -> ValueMode {
    if f & flag::MODE_RMS != 0 {
        ValueMode::Rms
    } else if f & flag::MODE_HARMONIC != 0 {
        ValueMode::FirstHarmonic
    } else {
        ValueMode::Instant
    }
}

/// A file analog channel's inputs to a recompute besides its samples:
/// a, b, the unit multiplier, primary, secondary, P / S, its own
/// frequencies, the sample count, the unit.
type ChanInputs = (f64, f64, f64, f64, f64, bool, Vec<f64>, usize, String);

/// What a full recompute (`Doc::recompute`) reads besides the samples —
/// the window, rate, nominal frequency, correction choice and candidates,
/// the fundamental's gain and each channel's coefficients — when its
/// values would be the file's as read, as `Doc::new` computes them: no
/// channel or discrete flag that changes a value, no computed channel, no
/// group. `None` otherwise.
#[allow(clippy::type_complexity)]
fn recompute_inputs(d: &Doc) -> Option<(usize, f64, f64, i32, Vec<fvb_core::comtrade::ExtraFreq>, f64, Vec<ChanInputs>)> {
    let flagged = d.chans.iter().any(|c| c.adc_counts || c.remove_dc || c.invert) || d.dig_flags.iter().any(|f| f.invert);
    if flagged || !d.computed.is_empty() || !d.group_names.is_empty() {
        return None;
    }
    let o = &d.osc;
    let chans = o.analog.iter().map(|c| (c.a, c.b, c.unit_mult, c.primary, c.secondary, c.is_primary, c.own_freq.clone(), c.samples.len(), c.unit.clone())).collect();
    Some((o.samples_per_cycle(), o.rate_r, o.line_freq, o.freq_sel, o.extra_freq.clone(), o.harmonic1_gain, chans))
}

impl App {
    /// Read the settings for document `di` (`0x44f5b0`); `from_command` =
    /// 32860 (comments are not re-read).
    pub fn apply_record_settings(&mut self, di: usize, from_command: bool) {
        self.apply_settings(di, from_command, false);
    }

    /// [`App::apply_record_settings`]; `fresh`: the document holds exactly
    /// what `Doc::new` computed from the record as read (an open, on the
    /// loader's thread), so the recompute may be spared.
    fn apply_settings(&mut self, di: usize, from_command: bool, fresh: bool) {
        let Some(d) = self.docs.get_mut(di) else { return };
        let as_read = if fresh { recompute_inputs(d) } else { None };
        // The review elements name their channels; the painters resolve
        // those names among the shown ones (aliases included) — bound here
        // once every channel is in place (VM, L9.227).
        let mut comment_recs = None;
        if !from_command {
            if let Some(c) = d.set.tree.section("Comments") {
                comment_recs = Some(recset::read_comments(c));
                // The record comment edited by 32805.
                if let Some(u) = c.attrs.get("UserData") {
                    d.hdr = Some(u.clone());
                }
            }
        }
        // Oscillogram: the rate, then the formula channels are dropped, then
        // the stored code page wins over the guess.
        let osc = d.set.tree.section("Oscillogram").cloned().unwrap_or_default();
        // Into rec+0x228 only (`0x44f72c`): nothing retimed, the
        // frequency track not rebuilt (L9.142).
        d.osc.rate_r = f64::from(osc.get_int("DiscretisationFrequensy", d.osc.rate_r as i32));
        // `0x4156e0` deletes them one by one through `0x42ecf0`, which
        // sweeps the review list (32860; at an open there are none yet).
        if d.computed.iter().any(is_formula) {
            d.removing_channels(|d| d.computed.retain(|c| !is_formula(c)));
            d.selection.retain(|k| !matches!(k, ChanKey::Computed(_)));
            d.channels_changed();
        }
        // The manager's lists are emptied with them (`0x4156e0`): kept rows and their order go too.
        d.loose_defs.clear();
        d.def_order.clear();
        if let Some(cp_raw) = osc.attrs.get("Codepage").map(|v| recset::wtoi(v)) {
            // 0 is OEM; anything else for a record read in UTF-8 or KOI8-R
            // (ours keeps it, `Doc::init_record_text`) its own encoding —
            // 1, ANSI, the names as read.
            let cp = match d.read_enc {
                Encoding::Cp1251 | Encoding::Cp866 => cp_of(cp_raw),
                _ if cp_raw == 0 => Encoding::Cp866,
                read => read,
            };
            d.set_codepage(cp);
            // The original keeps the value itself (`+0x6e8`, `0x44f808`) and
            // writes it back; only 0 and 1 are its combo's entries.
            // Only on a record read in 1251 / 866: for one read in UTF-8 /
            // KOI8-R the combo has a third entry of ours, the current one.
            d.cp_raw = (!matches!(cp_raw, 0 | 1) && matches!(d.read_enc, Encoding::Cp1251 | Encoding::Cp866)).then_some(cp_raw);
        }
        // File channels.
        let na = d.osc.analog.len();
        for i in 0..na + d.osc.digital.len() {
            let key = d.file_key(i);
            let Some(sec) = d.set.tree.section(&key).cloned() else { continue };
            if i < na {
                let a = &d.osc.analog[i];
                let cur = ChannelRec {
                    show_flags: d.analog_flags(i),
                    primary_mul: a.primary,
                    secondary_mul: a.secondary,
                    alias: String::new(),
                    custom_color: None,
                };
                let r = recset::read_channel(&sec, &cur);
                d.set_analog_flags(i, r.show_flags);
                let a = &mut d.osc.analog[i];
                a.own_freq = recset::read_net_freq(&sec, a.samples.len(), &a.own_freq);
                a.primary = r.primary_mul;
                a.secondary = r.secondary_mul;
                if !r.alias.is_empty() {
                    a.id = r.alias;
                    d.mark_alias(ChanKey::Analog(i), true);
                }
                if let Some(c) = r.custom_color {
                    (d.chans[i].color, d.chans[i].custom_color) = (from_colorref(c), true);
                }
            } else {
                let k = i - na;
                let cur = ChannelRec {
                    show_flags: d.digital_flags(k),
                    primary_mul: 1.0,
                    secondary_mul: 1.0,
                    alias: String::new(),
                    custom_color: None,
                };
                let r = recset::read_channel(&sec, &cur);
                d.digital_visible[k] = r.show_flags & flag::VISIBLE != 0;
                *d.dig_mut(k) = crate::app::DigFlags { invert: r.show_flags & flag::INVERT != 0, remove_dc: r.show_flags & flag::REMOVE_DC != 0, mode: mode_of(r.show_flags), color: r.custom_color.map(from_colorref), omp: d.dig(k).omp, omp_alias: d.dig(k).omp_alias };
                if !r.alias.is_empty() {
                    d.osc.digital[k].id = r.alias;
                    d.mark_alias(ChanKey::Digital(k), true);
                }
            }
        }
        // The formula channels, ОМП, then the new channels' own sections.
        let calc = d.set.tree.section("CalculatedChannels").map(recset::read_calculated).unwrap_or_default();
        if let Some(p) = d.set.tree.section("WrongPlaceFinding").and_then(recset::read_omp) {
            d.omp.params = p;
        }
        let pairs = |c: Option<&SetNode>, keys| -> Vec<crate::diagram::ChanPair> {
            c.map(|c| recset::read_pairs(c, keys)).unwrap_or_default().into_iter().map(|p| crate::diagram::ChanPair { a: p.a, b: p.b, on: p.visible }).collect()
        };
        // (Iторм, Iдиф) in the file → our (Iдиф, Iторм) (L9.81).
        d.dif_pairs = pairs(d.set.tree.section("Diff Chart"), recset::DIFF_KEYS).into_iter().map(|p| crate::diagram::ChanPair { a: p.b, b: p.a, on: p.on }).collect();
        d.rx_pairs = pairs(d.set.tree.section("Circle Chart"), recset::CIRCLE_KEYS);
        if let Some(x) = recset::read_view(d.set.tree.section("View")).xscale.filter(|f| *f > 0.0 && *f <= 1.0) {
            d.xscale = Some(x);
        }
        // The original recomputes here (`0x44f5b0`, per channel); when the
        // settings changed nothing a recompute reads, it would give back
        // what the open computed: ours keeps that (2 × the time on the UI
        // thread for a big record otherwise) and only takes the flagged
        // copy anew, for the names.
        let d = &mut self.docs[di];
        if as_read.is_some() && as_read == recompute_inputs(d) && !d.omp.shown {
            d.flagged = d.osc.clone();
            d.series_gen += 1;
        } else {
            self.recompute_doc(di);
        }
        if !calc.is_empty() {
            let (active, status) = (self.active, self.status.clone());
            self.active = Some(di);
            let batch: Vec<(String, String)> = calc;
            self.append_computed(&batch);
            self.active = active;
            self.status = status;
        }
        let d = &mut self.docs[di];
        for j in 0..d.computed.len() {
            let Some(sec) = d.set.tree.section(&d.computed[j].name).cloned() else { continue };
            let cur = ChannelRec { show_flags: d.computed_flags(j), primary_mul: 1.0, secondary_mul: 1.0, alias: String::new(), custom_color: None };
            let r = recset::read_channel(&sec, &cur);
            let c = &mut d.computed[j];
            c.visible = r.show_flags & flag::VISIBLE != 0;
            c.on_freq = r.show_flags & flag::FREQUENCY != 0;
            c.on_vector = r.show_flags & flag::VECTOR != 0;
            c.on_circle = r.show_flags & flag::CIRCLE != 0;
            c.stats.minmax = r.show_flags & flag::MINMAX != 0;
            c.stats.a1 = r.show_flags & flag::A1 != 0;
            c.stats.rms = r.show_flags & flag::RMS_ROW != 0;
            // Restored after every formula was evaluated, the cache rebuilt
            // without re-evaluating (`0x44f5b0` loop 2, `0x428490`).
            c.invert = r.show_flags & flag::INVERT != 0;
            c.remove_dc = r.show_flags & flag::REMOVE_DC != 0;
            c.mode = mode_of(r.show_flags);
            if let Some(v) = r.custom_color {
                (c.color, c.custom_color) = (from_colorref(v), true);
            }
            d.reflag_computed(j);
        }
        if let Some(recs) = comment_recs {
            let d = &mut self.docs[di];
            d.comments = recs.iter().map(|r| d.comment_of(r)).collect();
        }
        // The diagram windows and the view re-read their sections.
        self.setfile.sync = None;
        let d = &mut self.docs[di];
        d.set.print = d.settings_print();
    }

    /// After a record was opened: read `<base>.set` (or the legacy `ini`).
    pub fn load_record_settings(&mut self, di: usize) {
        let Some(d) = self.docs.get_mut(di) else { return };
        d.init_set_keys();
        if let Some(p) = d.path.clone() {
            d.set.tree.load_path(&recset::set_path(&p));
        }
        self.apply_settings(di, false, true);
        self.docs[di].set.modified = false;
    }

    /// 32860 «Загрузка настроек...» (`0x44fd10`): pick a `.set`, read it
    /// without the comments, mark modified, refresh.
    pub fn load_settings_command(&mut self) {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else { return };
        let serial = self.docs[ai].serial;
        let dlg = crate::dialogs::file_dialog().add_filter("Настройки осциллограммы FastView (*.set)", &["set"]).add_filter("Все файлы (*.*)", &["*"]);
        self.pick(crate::sysdlg::Job::Open(dlg), move |app, _, picked| {
            // The document the command was given for, if it is still open.
            if let (Some((path, _)), Some(ai)) = (picked, app.doc_by_serial(serial)) {
                app.load_settings_from(ai, &path);
            }
        });
    }

    pub fn load_settings_from(&mut self, di: usize, path: &std::path::Path) {
        self.docs[di].set.tree.load_path(path);
        self.apply_record_settings(di, true);
        self.docs[di].set.modified = true;
        // Then the diagrams' refresh: the block re-picked by name (the
        // values stay — the same document, `this+0x98`).
        if self.active == Some(di) {
            self.refresh_blocks();
        }
    }

    /// `OnSaveDocument` (`0x44c940`): write the record when the target is not
    /// the document's own file, then its `.set`; clear the modified flag.
    pub fn save_doc_to(&mut self, di: usize, target: &std::path::Path) -> Result<(), String> {
        let d = self.docs.get(di).ok_or("Нет осциллограммы для сохранения.")?;
        let kind = save_kind(target).ok_or_else(|| format!("{} - неизвестный формат", target.display()))?;
        // `0x44d772`: the record path (never updated, case-sensitive
        // `wcscmp`) — the same file names the record's files are not
        // rewritten; the `.hdr` is written either way.
        let same = d.rec_path.as_ref().is_some_and(|p| p.to_string_lossy() == target.to_string_lossy());
        if !same {
            match kind {
                ".cff" => {
                    let (rec, no) = d.file_record();
                    let bytes = comtrade::write_cff_numbered(&rec, &d.cff_sections(), d.codepage, &no).map_err(|e| e.to_string())?;
                    std::fs::write(target, bytes).map_err(|e| e.to_string())?;
                }
                ".osc" => {
                    // `0x44ca80` writes the record's own bytes to the new name.
                    let src = d.rec_path.clone().filter(|p| save_kind(p) == Some(".osc")).ok_or("Нет исходного файла .osc")?;
                    std::fs::copy(src, target).map_err(|e| e.to_string())?;
                }
                _ => {
                    let (rec, no) = d.file_record();
                    let (cfg, dat) = comtrade::write_numbered(&rec, d.codepage, &no);
                    std::fs::write(target, cfg).map_err(|e| e.to_string())?;
                    std::fs::write(target.with_extension("dat"), dat).map_err(|e| e.to_string())?;
                }
            }
        }
        if kind == ".cfg" {
            if let Some(b) = d.hdr_file_bytes() {
                std::fs::write(target.with_extension("hdr"), b).map_err(|e| e.to_string())?;
            }
        }
        let d = &mut self.docs[di];
        // The `.cfg` / `.cff` writers set the trigger sample to 0 whether
        // or not they wrote the record (`0x44d6c5`, `0x44daaa`).
        if kind != ".osc" {
            d.osc.trigger_sample = 0;
        }
        let mut t = std::mem::take(&mut d.set.tree);
        d.store_settings(&mut t, false);
        d.set.tree = t;
        // `_wfopen_s` failing is silent in the original.
        let _ = std::fs::write(recset::set_path(target), d.set.tree.to_file_bytes());
        if !same {
            d.path = Some(target.to_path_buf());
            d.name = target.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
        d.set.modified = false;
        d.set.print = d.settings_print();
        Ok(())
    }

    /// The file Сохранить writes onto: the document's own when it has one the
    /// save dispatcher knows.
    pub fn save_path_of(&self, ai: usize) -> Option<std::path::PathBuf> {
        self.docs.get(ai)?.path.clone().filter(|p| save_kind(p).is_some())
    }

    /// Сохранить (57603, `DoFileSave`) onto the document's own file; false
    /// when it has none (that asks for one: [`App::save_doc_then`]).
    pub fn save_active(&mut self) -> bool {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else { return false };
        match self.save_path_of(ai) {
            Some(p) => self.report_save(ai, &p),
            None => {
                self.ask_save_as(ai, |_, _, _| {});
                false
            }
        }
    }

    /// Сохранить for document `ai` onto its own file (it has one: [`App::save_path_of`]).
    pub(crate) fn save_active_at(&mut self, ai: usize) -> bool {
        match self.save_path_of(ai) {
            Some(p) => self.report_save(ai, &p),
            None => false,
        }
    }

    /// Сохранить for document `ai`: onto its own file, else Save As; `then`
    /// runs with whether it was saved — at once, or once the file dialog
    /// (outside the frame, [`crate::sysdlg`]) has answered.
    pub fn save_doc_then(&mut self, ctx: &egui::Context, ai: usize, then: impl FnOnce(&mut App, &egui::Context, bool) + 'static) {
        match self.save_path_of(ai) {
            Some(p) => {
                let ok = self.report_save(ai, &p);
                then(self, ctx, ok);
            }
            None => self.ask_save_as(ai, then),
        }
    }

    /// Сохранить как (57604): the file dialog, then `OnSaveDocument`.
    pub fn save_active_as(&mut self) {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else {
            self.status = "Нет осциллограммы для сохранения.".into();
            return;
        };
        self.ask_save_as(ai, |_, _, _| {});
    }

    /// The Save As dialog of document `ai`; `then` gets whether the record was saved.
    pub fn ask_save_as(&mut self, ai: usize, then: impl FnOnce(&mut App, &egui::Context, bool) + 'static) {
        let Some(d) = self.docs.get(ai) else { return };
        let serial = d.serial;
        let filters = save_filters(d.path.as_deref());
        // Proposed: the record's folder and its title without extension.
        // `0x447e30` lowers the name in place (three MakeLower calls) before
        // splitting it: folder and title come out lowercase — harmless on
        // Windows' case-blind names, so only there (elsewhere the lowered
        // folder may not exist).
        let lower = |s: String| if cfg!(windows) { s.to_lowercase() } else { s };
        let stem = lower(d.path.as_ref().and_then(|p| p.file_stem()).map_or_else(|| d.name.clone(), |s| s.to_string_lossy().into_owned()));
        let dir = d.path.as_ref().and_then(|p| p.parent()).filter(|p| !p.as_os_str().is_empty()).map(|p| lower(p.to_string_lossy().into_owned()));
        let job = crate::sysdlg::Job::SaveTyped { filters: filters.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), dir, name: stem.clone(), owner: crate::sysdlg::owner() };
        self.pick(job, move |app, ctx, picked| {
            let Some((path, chosen)) = picked else { return then(app, ctx, false) };
            let Some(ai) = app.doc_by_serial(serial) else { return then(app, ctx, false) };
            let path = save_target(path, &filters.map(|f| f.1), chosen);
            // The proposed name taken as it is: the original keeps the text it
            // proposed (lowercase), where a shell dialog may hand back an
            // existing file's own case — the files written anew (the `.set`)
            // come out lowercase (VM: «win_1999.set» beside «WIN_1999.cfg»).
            let path = match path.file_stem().map(|s| s.to_string_lossy().into_owned()) {
                Some(s) if cfg!(windows) && s != stem && s.to_lowercase() == stem => {
                    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                    path.with_file_name(format!("{stem}.{ext}"))
                }
                _ => path,
            };
            let ok = app.report_save(ai, &path);
            then(app, ctx, ok);
        });
    }

    fn report_save(&mut self, di: usize, path: &std::path::Path) -> bool {
        match self.save_doc_to(di, path) {
            Ok(()) => true,
            // The writer's exception is reported in an MB_OK box (`0x475680`
            // → `0x4756e0`): "<message> - <file>" (`0x42dfe0`).
            Err(e) => {
                self.dlg.push_alert(format!("{e} - {}", path.display()));
                false
            }
        }
    }

    fn view_snap(&self) -> ViewSnap {
        let circle_sel = self.setfile.sync.as_ref().map(|s| s.1.circle.selected_block.clone()).unwrap_or_default();
        let diff_sel = self.setfile.sync.as_ref().map(|s| s.1.diff.selected_block.clone()).unwrap_or_default();
        ViewSnap {
            circle: recset::PlaneChart {
                sights_only: self.circle_between_cursors,
                show_off: self.show_reset,
                view_type: i32::from(self.circle_readout),
                selected_block: circle_sel,
            },
            diff: recset::PlaneChart {
                sights_only: self.diag.diff_between,
                show_off: self.diag.diff_show_reset,
                view_type: i32::from(self.diag.diff_values),
                selected_block: diff_sel,
            },
            vector: recset::VectorChart {
                angle_in_radians: self.vec_radians,
                only_direction: self.vec_direction_only,
                show_angles_value: self.vec_show_values,
                angle_channel: self.vec_angle_ref.clone().unwrap_or_default(),
            },
            freq: recset::FrequencyChart {
                num_periods: self.freq_periods as i32,
                num_harmonics: self.freq_harmonics as i32,
                show_persents: i32::from(!self.freq_percent),
                use_rms: self.freq_use_rms,
            },
            y_analog: self.y_analog.round() as i32,
            ruler: i32::from(crate::persist::ruler_state(self.time_unit)),
            grid: self.show_grid,
        }
    }

    fn apply_view_snap(&mut self, s: &ViewSnap) {
        self.circle_between_cursors = s.circle.sights_only;
        self.show_reset = s.circle.show_off;
        self.circle_readout = s.circle.view_type.clamp(0, 2) as u8;
        self.diag.diff_between = s.diff.sights_only;
        self.diag.diff_show_reset = s.diff.show_off;
        self.diag.diff_values = s.diff.view_type != 0;
        self.vec_radians = s.vector.angle_in_radians;
        self.vec_direction_only = s.vector.only_direction;
        self.vec_show_values = s.vector.show_angles_value;
        self.vec_angle_ref = Some(s.vector.angle_channel.clone()).filter(|n| !n.is_empty());
        self.freq_periods = s.freq.num_periods.max(1) as usize;
        self.freq_harmonics = s.freq.num_harmonics.max(1) as usize;
        self.freq_percent = s.freq.show_persents == 0;
        self.freq_use_rms = s.freq.use_rms;
        self.y_analog = s.y_analog as f32;
        self.time_unit = crate::persist::time_unit(s.ruler.clamp(0, 255) as u8).unwrap_or(TimeUnit::Milli);
        self.show_grid = s.grid;
    }

    fn read_view_snap(d: &Doc) -> ViewSnap {
        let t = &d.set.tree;
        let v = recset::read_view(t.section("View"));
        ViewSnap {
            circle: recset::read_plane_chart(t.section("Circle Chart"), false),
            diff: recset::read_plane_chart(t.section("Diff Chart"), true),
            vector: recset::read_vector_chart(t.section("Vector Chart")),
            freq: recset::read_frequency_chart(t.section("Frequency Chart"), d.osc.sample_rate, d.osc.line_freq),
            y_analog: v.y_analog,
            ruler: v.ruler_state,
            grid: v.sells,
        }
    }

    /// Every frame: the diagram / view settings follow the active document
    /// (read from its tree when it becomes active, written into it — and the
    /// document marked modified — when a toggle changes them); any other
    /// change of the saved state marks its document modified.
    pub fn track_settings(&mut self) {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else {
            self.setfile.sync = None;
            return;
        };
        match self.setfile.sync.clone() {
            Some((di, old)) if di == ai => {
                let now = self.view_snap();
                if now != old {
                    let t = &mut self.docs[ai].set.tree;
                    if now.circle != old.circle {
                        recset::write_plane_chart(t.section_mut("Circle Chart"), &old.circle, &now.circle);
                    }
                    if now.diff != old.diff {
                        recset::write_plane_chart(t.section_mut("Diff Chart"), &old.diff, &now.diff);
                    }
                    if now.vector != old.vector {
                        recset::write_vector_chart(t.section_mut("Vector Chart"), &old.vector, &now.vector);
                    }
                    if now.freq != old.freq {
                        recset::write_frequency_chart(t.section_mut("Frequency Chart"), &now.freq);
                    }
                    let v = t.section_mut("View");
                    if now.y_analog != old.y_analog {
                        v.set_int("YAnalogSize", now.y_analog);
                    }
                    if now.ruler != old.ruler {
                        v.set_int("RulerState", now.ruler);
                    }
                    if now.grid != old.grid {
                        v.set_bool("Sells", now.grid);
                    }
                    self.docs[ai].set.modified = true;
                    self.setfile.sync = Some((ai, now));
                }
            }
            _ => {
                let s = Self::read_view_snap(&self.docs[ai]);
                self.apply_view_snap(&s);
                self.setfile.sync = Some((ai, s));
                // The view's activation refreshes the diagrams (`0x45f7d0` →
                // `0x475420`): the document's block.
                self.refresh_blocks();
            }
        }
        let d = &mut self.docs[ai];
        let print = d.settings_print();
        if print != d.set.print {
            // A document never read from a file (demo, tests) starts clean.
            d.set.modified |= !d.set.print.is_empty();
            d.set.print = print;
        }
    }

    /// Закрыть / the tab's ✕ (`OnCloseDocument` after `SaveModified`).
    pub fn request_close(&mut self, i: usize) {
        if self.docs.get(i).is_some_and(needs_save_prompt) {
            self.setfile.prompt = Some(SavePrompt { doc: i, exit: false });
        } else {
            self.close_doc_now(i);
        }
    }

    /// The window's close button: one question per modified document
    /// (`SaveAllModified`); «Отмена» keeps the program open.
    pub fn handle_exit_request(&mut self, ctx: &egui::Context) {
        // The restored rect, kept while the frame is maximised.
        if let Some(r) = ctx.input(|i| {
            let v = i.viewport();
            let restored = !v.maximized.unwrap_or(false) && !v.minimized.unwrap_or(false);
            let inner = v.inner_rect.filter(|_| restored)?;
            Some(egui::Rect::from_min_size(v.outer_rect.map_or(inner.min, |o| o.min), inner.size()))
        }) {
            self.frame_normal = Some(r);
        }
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        let queue: Vec<usize> = if self.setfile.exit_ok { Vec::new() } else { (0..self.docs.len()).filter(|&i| needs_save_prompt(&self.docs[i])).collect() };
        if queue.is_empty() {
            // The frame closes: the workspace is saved (`0x100a079f`).
            // The inner size at the outer position (egui's `with_position`).
            let placement = ctx.input(|i| {
                let v = i.viewport();
                let inner = v.inner_rect?;
                let at = v.outer_rect.map_or(inner.min, |o| o.min);
                Some((egui::Rect::from_min_size(at, inner.size()), v.maximized.unwrap_or(false)))
            });
            self.save_workspace(placement);
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        if self.setfile.prompt.is_none() {
            self.setfile.exit_queue = Some(queue);
            self.next_exit_prompt(ctx);
        }
    }

    fn next_exit_prompt(&mut self, ctx: &egui::Context) {
        let Some(q) = self.setfile.exit_queue.as_mut() else { return };
        if q.is_empty() {
            self.setfile.exit_queue = None;
            self.setfile.exit_ok = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let doc = q.remove(0);
        self.setfile.prompt = Some(SavePrompt { doc, exit: true });
    }

    /// The answer to the question: Some(true) Да, Some(false) Нет, None
    /// Отмена.
    pub fn answer_save_prompt(&mut self, ctx: &egui::Context, answer: Option<bool>) {
        let Some(p) = self.setfile.prompt.take() else { return };
        // The question's document is saved first (a Save As waits for its
        // dialog, outside the frame); the close or the exit goes on after.
        let go_on = move |app: &mut App, ctx: &egui::Context, go_on: bool| {
            if !go_on {
                app.setfile.exit_queue = None;
            } else if p.exit {
                app.next_exit_prompt(ctx);
            } else {
                app.close_doc_now(p.doc);
            }
        };
        match answer {
            None => go_on(self, ctx, false),
            Some(false) => go_on(self, ctx, true),
            Some(true) => self.save_doc_then(ctx, p.doc, go_on),
        }
    }

    /// MFC's question (`AFX_IDP_ASK_TO_SAVE`, Да / Нет / Отмена).
    pub fn save_prompt_window(&mut self, ctx: &egui::Context) {
        let Some(p) = self.setfile.prompt else { return };
        let Some(d) = self.docs.get(p.doc) else {
            self.setfile.prompt = None;
            return;
        };
        let title = d.path.as_ref().and_then(|p| p.file_name()).map_or_else(|| d.name.clone(), |s| s.to_string_lossy().into_owned());
        let mut answer: Option<Option<bool>> = None;
        crate::dialogs::modal_backdrop(ctx, "fv_save_prompt_block", egui::Order::Middle);
        crate::dialogs::dlg(ctx, crate::app::APP_TITLE)
            .id(egui::Id::new("fv_save_prompt"))
            .margin(0.0)
                .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                match crate::dialogs::message_box(ui, crate::dialogs::MsgIcon::Question, &format!("Сохранить изменения в {title}?"), &["Да", "Нет", "Отмена"]) {
                    Some(0) => answer = Some(Some(true)),
                    Some(1) => answer = Some(Some(false)),
                    Some(_) => answer = Some(None),
                    None => {}
                }
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            answer = Some(None);
        }
        if let Some(a) = answer {
            self.answer_save_prompt(ctx, a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{DEMO_CFG, DEMO_DAT};
    use std::path::{Path, PathBuf};

    /// `FV_SAVE_AS=<record>,<target> cargo test save_as_dump`: open a record
    /// and save it to `target` as Save As does (for comparing the writers
    /// with the original's output, COMPLETION_PLAN W4.2).
    #[test]
    fn save_as_dump() {
        let Ok(v) = std::env::var("FV_SAVE_AS") else { return };
        let (rec, target) = v.split_once(',').expect("FV_SAVE_AS=<record>,<target>");
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, Path::new(rec));
        // `FV_SAVE_EDIT=invert,rms,dc`: the first analog channel's menu
        // commands applied in that order, as a VM run did before saving.
        for e in std::env::var("FV_SAVE_EDIT").unwrap_or_default().split(',') {
            app.docs[di].selection = [ChanKey::Analog(0)].into();
            match e {
                "invert" => app.dispatch(crate::app::Command::ToggleFlag(crate::chansel::ChanFlag::Invert)),
                "dc" => app.dispatch(crate::app::Command::ToggleFlag(crate::chansel::ChanFlag::RemoveDc)),
                "rms" => app.dispatch(crate::app::Command::SetValueMode(crate::app::ValueMode::Rms)),
                // A formula channel X = Ua shown on the frequency diagram.
                "calc" => {
                    assert!(app.append_computed(&[("X".into(), "Ua".into())]).is_some());
                    app.docs[di].computed[0].on_freq = true;
                }
                // Copy the digital channel and paste it in front of itself.
                "pastedig" => {
                    app.docs[di].selection = [ChanKey::Digital(0)].into();
                    app.copy_channels_for_merge();
                    app.docs[di].selection = [ChanKey::Digital(0)].into();
                    app.paste_channels_for_merge();
                }
                // ADC on a pasted lane: copy the second channel, paste it in front of the first, ADC.
                "pasteadc" => {
                    app.docs[di].selection = [ChanKey::Analog(1)].into();
                    app.copy_channels_for_merge();
                    app.docs[di].selection = [ChanKey::Analog(0)].into();
                    app.paste_channels_for_merge();
                    app.dispatch(crate::app::Command::ToggleFlag(crate::chansel::ChanFlag::Adc));
                }
                // Copy the second channel and paste it in front of the first
                // (a pending lane among the file channels), then save.
                "paste" => {
                    app.docs[di].selection = [ChanKey::Analog(1)].into();
                    app.copy_channels_for_merge();
                    app.docs[di].selection = [ChanKey::Analog(0)].into();
                    app.paste_channels_for_merge();
                }
                // Quick analysis, «Симметричные составляющие»: Ia / Ib / Ic =
                // the first three analog channels, voltages «Отключить».
                "symm" => {
                    app.dispatch(crate::app::Command::QuickAnalysis);
                    app.analysis_tab = crate::app::AnalysisTab::Symmetric;
                    app.quick_show_page();
                    let names: Vec<String> = app.docs[di].osc.analog.iter().take(3).map(|c| c.id.clone()).collect();
                    let q = &mut app.quick[crate::quick_ui::page_index(crate::app::AnalysisTab::Symmetric).unwrap()];
                    (q.ia, q.ib, q.ic) = (names[0].clone(), names[1].clone(), names[2].clone());
                    q.voltages_off = true;
                    app.quick_calc();
                }
                _ => {}
            }
        }
        app.save_doc_to(di, Path::new(target)).expect("saved");
    }

    /// `FV_OMP_DUMP=<record>,<file.omp>,<target> cargo test omp_dump`: an
    /// isolated-neutral description (channels 0–2 the currents, 3–5 the line
    /// voltages, Inom 1000, one 10 km section R 0.1 / X 0.4) written as an
    /// `.omp`, run on the record, the record saved as `target` — for a
    /// comparison with the original loading the same file (W4.2).
    #[test]
    fn omp_dump() {
        let Ok(v) = std::env::var("FV_OMP_DUMP") else { return };
        let parts: Vec<&str> = v.split(',').collect();
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, Path::new(parts[0]));
        let ids: Vec<String> = app.docs[di].osc.analog.iter().map(|c| c.id.clone()).collect();
        let mut p = fvb_core::omp_params::OmpParams::default();
        let st = &mut p.stations[0];
        st.inom = std::env::var("FV_OMP_INOM").unwrap_or_else(|_| "1000".into());
        (st.ia, st.ib, st.ic) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
        (st.uab, st.ubc, st.uca) = (ids[3].clone(), ids[4].clone(), ids[5].clone());
        st.sections.push(fvb_core::omp_params::SectionParams { length: "10".into(), supports: "30".into(), r: "0.1".into(), x: "0.4".into(), ..Default::default() });
        std::fs::write(parts[1], fvb_core::recset::omp_file_bytes(&p)).expect("omp written");
        app.docs[di].omp.params = p;
        app.dispatch(crate::app::Command::OmpAnalysis);
        eprintln!("ОМП: {:?}", app.omp_ui.bar);
        // `FV_OMP_REPORT=<file.txt>`: the bar's «Сохранить...» bytes.
        if let (Ok(out), Some(text)) = (std::env::var("FV_OMP_REPORT"), app.omp_report()) {
            std::fs::write(out, fvb_core::omp_params::utf16le_crlf(&text)).expect("report written");
        }
        app.save_doc_to(di, Path::new(parts[2])).expect("saved");
    }

    /// `FV_OMP_FILE=<file.omp> [FV_OMP_INOM=<text>] [FV_OMP_SECTION=<length>] cargo test omp_file_dump`:
    /// the `.omp` of «Сохранить» for the default parameters, with the
    /// first station's rated current and one section's length typed in — for
    /// a byte compare with the original's after the same entries by hand
    /// (W4.2, #28).
    #[test]
    fn omp_file_dump() {
        let Ok(out) = std::env::var("FV_OMP_FILE") else { return };
        let mut p = fvb_core::omp_params::OmpParams::default();
        if let Ok(v) = std::env::var("FV_OMP_INOM") {
            p.stations[0].inom = v;
        }
        if let Ok(v) = std::env::var("FV_OMP_SECTION") {
            p.stations[0].sections.push(fvb_core::omp_params::SectionParams { length: v, ..Default::default() });
        }
        std::fs::write(out, fvb_core::recset::omp_file_bytes(&p)).expect("omp written");
    }

    /// `FV_CLIP_DUMP=<record>,<file> cargo test clip_dump`: open a record,
    /// select its first analog channel and write the clipboard document as
    /// Copy puts it up (UTF-16LE, a NUL at the end) — for a byte compare
    /// with the original's (W5.2).
    #[test]
    fn clip_dump() {
        let Ok(v) = std::env::var("FV_CLIP_DUMP") else { return };
        let (rec, target) = v.split_once(',').expect("FV_CLIP_DUMP=<record>,<file>");
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, Path::new(rec));
        app.docs[di].selection = [ChanKey::Analog(0)].into();
        let text = crate::merge_ui::clip_record(&app.docs[di]).to_text();
        let bytes: Vec<u8> = text.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes).collect();
        std::fs::write(target, bytes).expect("written");
    }

    /// MFC's OpenDocumentFile: a file already open is brought forward, not
    /// loaded again.
    #[test]
    fn opening_an_open_file_activates_it() {
        let dir = temp_dir("reopen");
        let cfg = write_record(&dir);
        let other = dir.join("other.cfg");
        std::fs::write(&other, DEMO_CFG).unwrap();
        std::fs::write(dir.join("other.dat"), DEMO_DAT).unwrap();
        let mut app = App::new();
        app.persist = false;
        let first = open(&mut app, &cfg);
        let second = open(&mut app, &other);
        assert_ne!(first, second);
        app.load_comtrade_path(&cfg);
        assert!(app.loading.is_empty(), "nothing loads");
        assert_eq!((app.docs.len(), app.active), (2, Some(first)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `0x44d772` / `0x44da96`: the same-file test uses the path the record
    /// was opened from, which a Save As never updates — saving onto the
    /// opened file writes only its `.set`, but after a Save As every save
    /// to the new name writes the record again.
    #[test]
    fn the_record_path_is_never_updated() {
        let dir = temp_dir("recpath");
        let cfg = write_record(&dir);
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &cfg);
        std::fs::remove_file(dir.join("rec.dat")).unwrap();
        app.save_doc_to(di, &cfg).unwrap();
        assert!(!dir.join("rec.dat").exists(), "the opened file: only the .set");
        let other = dir.join("other.cfg");
        app.save_doc_to(di, &other).unwrap();
        std::fs::remove_file(dir.join("other.dat")).unwrap();
        app.save_doc_to(di, &other).unwrap();
        assert!(dir.join("other.dat").exists(), "after a Save As the record is written again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_as_filters_and_extension() {
        let f = save_filters(Some(Path::new("/r/a.OSC")));
        assert_eq!(f.map(|x| x.1), ["cfg", "osc"]);
        assert_eq!(save_filters(Some(Path::new("/r/a.sto"))).map(|x| x.1), ["cfg", "cff"]);
        assert_eq!(save_filters(None).map(|x| x.1), ["cfg", "cff"]);
        let exts = ["cfg", "cff"];
        assert_eq!(save_target(PathBuf::from("/r/x.CFF"), &exts, None), PathBuf::from("/r/x.CFF"));
        assert_eq!(save_target(PathBuf::from("/r/x.txt"), &exts, None), PathBuf::from("/r/x.txt.cfg"));
        assert_eq!(save_target(PathBuf::from("/r/x.osc"), &exts, None), PathBuf::from("/r/x.osc.cfg"));
        // The chosen type (`0x448208`): a different listed extension is
        // replaced (VM: «aout.cff» under «*.cfg» → aout.cfg), the chosen
        // one kept, an unlisted one gets the chosen appended.
        assert_eq!(save_target(PathBuf::from("/r/aout.cff"), &exts, Some(0)), PathBuf::from("/r/aout.cfg"));
        assert_eq!(save_target(PathBuf::from("/r/x.CFG"), &exts, Some(1)), PathBuf::from("/r/x.cff"));
        assert_eq!(save_target(PathBuf::from("/r/x.cff"), &exts, Some(1)), PathBuf::from("/r/x.cff"));
        assert_eq!(save_target(PathBuf::from("/r/x"), &exts, Some(1)), PathBuf::from("/r/x.cff"));
    }

    /// `0x42ecf0` → `0x440800`: deleting a channel drops the levels / areas
    /// on it and the kept ones, re-binds the rest by name, keeps moments.
    #[test]
    fn removing_a_channel_sweeps_the_review_list() {
        use crate::app::Computed;
        let mut app = App::new();
        app.dispatch(Command::Demo);
        let d = &mut app.docs[0];
        let n = d.osc.sample_count();
        for name in ["X", "Y"] {
            d.computed.push(Computed::new(name.into(), "Ua".into(), vec![0.0; n], egui::Color32::RED));
        }
        d.channels_changed();
        let c = |kind| Comment { kind, text: String::new(), locked: false, hidden: false };
        let kept = CommentRec {
            kind: CommentKindRec::Level { channel: "Нет".into(), y: 0.0, from: 0, to: 1 },
            text: String::new(),
            locked: false,
            visible: true,
        };
        d.comments = vec![
            c(CommentKind::Level { chan: ChanKey::Computed(1), value: 1.0, from: 0, to: 5 }),
            c(CommentKind::Level { chan: ChanKey::Computed(0), value: 2.0, from: 0, to: 5 }),
            c(CommentKind::Area { chan_from: ChanKey::Analog(0), chan_to: ChanKey::Analog(1), from: 0, to: 5 }),
            c(CommentKind::Moment { pos: 3 }),
            c(CommentKind::Kept(Box::new(kept))),
        ];
        d.comment_hot = Some(1);
        assert!(!d.removing_channels(|_| {}), "nothing removed: no sweep");
        assert_eq!(d.comments.len(), 5);
        assert!(d.removing_channels(|d| d.computed.retain(|c| c.name != "X")));
        let kinds: Vec<_> = d.comments.iter().map(|c| c.kind.clone()).collect();
        assert_eq!(
            kinds,
            [
                CommentKind::Level { chan: ChanKey::Computed(0), value: 1.0, from: 0, to: 5 },
                CommentKind::Area { chan_from: ChanKey::Analog(0), chan_to: ChanKey::Analog(1), from: 0, to: 5 },
                CommentKind::Moment { pos: 3 },
            ]
        );
        assert_eq!(d.comment_hot, None);
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fv_setfile_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A channel inserted in front of the record's digital ones (the ОМП's
    /// flags) moves the file-name keys with it, is left out of the record
    /// files and still counts in their numbering (VM, WIN_1999).
    #[test]
    fn inserted_flags_keep_the_keys_and_the_numbering() {
        let dir = temp_dir("flags");
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &write_record(&dir));
        let d = &mut app.docs[di];
        let (na, nd) = (d.osc.analog.len(), d.osc.digital.len());
        assert!(nd > 0, "the demo record has digital channels");
        let first = d.file_key(na);
        let samples = vec![false; d.osc.sample_count()];
        d.osc.digital.push(fvb_core::DigitalChannel { index: nd as u32 + 1, id: crate::omp_ui::KZ_PHASES[0].into(), phase: String::new(), normal: 0, samples, own_rate: 0.0 });
        d.digital_visible.resize(nd + 1, true);
        d.dig_mut(nd).omp = true;
        d.move_appended(na, na, 0, nd);
        assert_eq!(d.file_key(na), crate::omp_ui::KZ_PHASES[0]);
        assert_eq!(d.file_key(na + 1), first);
        let (rec, no) = d.file_record();
        assert_eq!(rec.digital.len(), nd);
        assert_eq!(no.digital, (2..=nd + 1).collect::<Vec<_>>());
        let keep: Vec<bool> = (0..=nd).map(|k| k > 0).collect();
        d.retain_digital_keys(&keep);
        d.osc.digital.remove(0);
        assert_eq!(d.file_key(na), first);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pasted lane in front of the file channels is left out of the record
    /// files and still counts in the analog numbering (a clone run: the first
    /// file channel is «2»; #28, L9.217's residual).
    #[test]
    fn a_pending_lane_counts_in_the_analog_numbering() {
        let dir = temp_dir("pendnum");
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &write_record(&dir));
        let n = app.docs[di].osc.analog.len();
        app.docs[di].selection = [ChanKey::Analog(1)].into();
        app.copy_channels_for_merge();
        app.docs[di].selection = [ChanKey::Analog(0)].into();
        app.paste_channels_for_merge();
        let (rec, no) = app.docs[di].file_record();
        assert_eq!(rec.analog.len(), n, "the lane is not a record of the file");
        assert_eq!(no.analog, (2..=n + 1).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A formula channel cannot be moved in front of the file channels (#192):
    /// the original adds it at the end of the analog block (`0x42e810`,
    /// `0x42e940`), its copy leaves formula channels out, and only a pasted
    /// lane lands among the file channels. With a formula channel present a
    /// lane pasted before the third file channel numbers them 1, 2, 4, …, as
    /// the original's `.cfg` does (a clone run, a synthetic record: the same
    /// bytes, and the same in front of the first).
    #[test]
    fn a_formula_channel_and_a_lane_among_the_file_channels_number_by_place() {
        let dir = temp_dir("formulanum");
        let cfg = write_record(&dir);
        let ids: Vec<String> = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap().analog.iter().map(|a| a.id.clone()).collect();
        let xml = format!("<settings><category name=\"CalculatedChannels\" Channel0=\"Xs,{}+{}\"/></settings>", ids[0], ids[1]);
        std::fs::write(dir.join("rec.set"), fvb_core::settree::utf16le_crlf(&xml)).unwrap();
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &cfg);
        let n = app.docs[di].osc.analog.len();
        assert!(n >= 3 && app.docs[di].computed.len() == 1, "{n} channels and a formula channel");
        // Its copy takes nothing: the formula channel is not a lane to move.
        app.docs[di].selection = [ChanKey::Computed(0)].into();
        assert!(!app.active_doc().unwrap().can_copy(), "the original's «Копировать» is disabled for it");
        app.copy_channels_for_merge();
        assert_eq!(app.status, "Не выбраны каналы для копирования.");
        assert_eq!(app.docs[di].file_record().1.analog, Vec::<usize>::new(), "no lane, the plain numbering");
        app.docs[di].selection = [ChanKey::Analog(1)].into();
        app.copy_channels_for_merge();
        app.docs[di].selection = [ChanKey::Analog(2)].into();
        app.paste_channels_for_merge();
        let (rec, no) = app.docs[di].file_record();
        assert_eq!(rec.analog.len(), n, "neither the lane nor the formula channel is a record of the file");
        let want: Vec<usize> = (0..n).map(|i| if i < 2 { i + 1 } else { i + 2 }).collect();
        assert_eq!(no.analog, want);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pasted lane's `.set` section (a clone run): ShowFlags with bit 14 (not
    /// bit 13), its own frequency vector as `NetFreq`; the paste's summary
    /// comment stores `Comments/Codepage` 1 (#28).
    #[test]
    fn a_pending_lane_is_saved_as_bit_14_with_its_net_freq() {
        let dir = temp_dir("pendset");
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &write_record(&dir));
        // A reader's OEM guess stored before is overwritten by the paste.
        app.docs[di].set.tree.section_mut("Comments").set_int("Codepage", 0);
        app.docs[di].selection = [ChanKey::Analog(1)].into();
        app.copy_channels_for_merge();
        app.docs[di].selection = [ChanKey::Analog(0)].into();
        app.paste_channels_for_merge();
        let d = &app.docs[di];
        let mut t = d.set.tree.clone();
        d.store_settings(&mut t, false);
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        let sec = t.section(&d.computed[j].name).expect("the lane's section");
        let flags: u32 = sec.get_str("ShowFlags", "0").parse().unwrap();
        assert_eq!((flags >> 13) & 3, 2, "bit 14 only: {flags}");
        assert_eq!(sec.get_str("NetFreq", "").split(',').count(), d.osc.sample_count());
        assert_eq!(t.section("Comments").unwrap().get_str("Codepage", ""), "1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pasted lane's `.set` section keeps the source channel's ratios and the
    /// summary comment names both records by their file names (a clone run, a
    /// synthetic 1234:5 channel; #192's comparison).
    #[test]
    fn a_pending_lane_keeps_the_ratios_and_the_summary_names_the_file() {
        let dir = temp_dir("pendmul");
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &write_record(&dir));
        app.docs[di].osc.analog[1].primary = 1234.0;
        app.docs[di].osc.analog[1].secondary = 5.0;
        app.docs[di].selection = [ChanKey::Analog(1)].into();
        app.copy_channels_for_merge();
        app.docs[di].selection = [ChanKey::Analog(0)].into();
        app.paste_channels_for_merge();
        let d = &app.docs[di];
        let mut t = d.set.tree.clone();
        d.store_settings(&mut t, false);
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        let sec = t.section(&d.computed[j].name).expect("the lane's section");
        assert_eq!((sec.get_str("PrimaryMul", ""), sec.get_str("SecondaryMul", "")), ("1234.000000".to_string(), "5.000000".to_string()));
        let hdr = d.hdr.clone().unwrap();
        assert_eq!(hdr.matches("Имя осциллограммы: rec.cfg\r\n").count(), 2, "{hdr}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pasted lane's `.set` section is named by its raw name's bytes read as
    /// 1251, as a file channel's is: in a record shown in 866 «(вст.)» is its
    /// 866 bytes, so the section reads «(ўбв.)» (a clone run, a synthetic
    /// record without Cyrillic letters, whose reader's guess is OEM).
    #[test]
    fn a_pending_lane_section_is_named_by_its_raw_bytes() {
        let dir = temp_dir("pendname");
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &write_record(&dir));
        app.docs[di].codepage = Encoding::Cp866;
        app.docs[di].selection = [ChanKey::Analog(1)].into();
        app.copy_channels_for_merge();
        app.docs[di].selection = [ChanKey::Analog(0)].into();
        app.paste_channels_for_merge();
        let d = &app.docs[di];
        let mut t = d.set.tree.clone();
        d.store_settings(&mut t, false);
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        let shown = &d.computed[j].name;
        assert!(shown.ends_with("(вст.)"), "{shown}");
        assert!(t.section(shown).is_none(), "not under the shown name");
        let raw = comtrade::decode_bytes(&d.computed[j].name_raw.as_ref().unwrap().bytes, Encoding::Cp1251);
        assert!(raw.ends_with("(ўбв.)"), "{raw}");
        assert!(t.section(&raw).is_some(), "under the bytes read as 1251");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pasted DIGITAL lane in front of a digital channel (a clone run, byte for
    /// byte): the channel after it is numbered 2, the lane's `.set` section has
    /// bit 14 and no `NetFreq` (#28).
    #[test]
    fn a_pending_digital_lane_counts_in_the_digital_numbering_and_has_no_net_freq() {
        let dir = temp_dir("penddig");
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &write_record(&dir));
        let nd = app.docs[di].osc.digital.len();
        app.docs[di].selection = [ChanKey::Digital(0)].into();
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = &app.docs[di];
        let (rec, no) = d.file_record();
        assert_eq!(rec.digital.len(), nd);
        assert_eq!(no.digital, (2..=nd + 1).collect::<Vec<_>>());
        let mut t = d.set.tree.clone();
        d.store_settings(&mut t, false);
        let j = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).unwrap();
        let sec = t.section(&d.computed[j].name).expect("the lane's section");
        assert_eq!(sec.get_str("ShowFlags", "0"), "16609");
        assert!(!sec.attrs.contains_key("NetFreq"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A UTF-8 record keeps the ОМП flags' section names as the program's own
    /// text in the `.set` (the original's, in a clone run); the UTF-8 bytes
    /// of a name read as 1251 are only the file's own channels' keys (#28).
    #[test]
    fn omp_flags_in_a_utf8_record_keep_their_own_names_as_keys() {
        let dir = temp_dir("flags8");
        let cfg = dir.join("rec.cfg");
        let text = DEMO_CFG.replacen(",1999", ",2013", 1);
        std::fs::write(&cfg, text).unwrap();
        std::fs::write(dir.join("rec.dat"), DEMO_DAT).unwrap();
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &cfg);
        let d = &mut app.docs[di];
        assert_eq!(d.read_enc, Encoding::Utf8, "the demo is UTF-8");
        let (na, nd) = (d.osc.analog.len(), d.osc.digital.len());
        let samples = vec![false; d.osc.sample_count()];
        d.osc.digital.push(fvb_core::DigitalChannel { index: nd as u32 + 1, id: crate::omp_ui::KZ_PHASES[0].into(), phase: String::new(), normal: 0, samples, own_rate: 0.0 });
        d.digital_visible.resize(nd + 1, true);
        d.dig_flags.resize(nd + 1, Default::default());
        d.dig_mut(nd).omp = true;
        d.move_appended(na, na, 0, nd);
        assert_eq!(d.file_key(na), crate::omp_ui::KZ_PHASES[0]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn write_record(dir: &Path) -> PathBuf {
        let cfg = dir.join("rec.cfg");
        std::fs::write(&cfg, DEMO_CFG).unwrap();
        std::fs::write(dir.join("rec.dat"), DEMO_DAT).unwrap();
        cfg
    }

    fn open(app: &mut App, path: &Path) -> usize {
        app.load_comtrade_path(path);
        for _ in 0..1000 {
            app.poll_loading();
            if app.loading.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let ai = app.active.expect("document opened");
        // First frame: the view / diagram sections are read.
        app.track_settings();
        ai
    }

    /// What a recompute leaves in a document: the phasors and roles, the
    /// RMS and DC, the flagged copy's values and names.
    fn computed_state(d: &Doc) -> String {
        let a = &d.analysis;
        let analog: Vec<_> = d.flagged.analog.iter().map(|c| (&c.id, &c.samples)).collect();
        let digital: Vec<_> = d.flagged.digital.iter().map(|c| (&c.id, &c.samples)).collect();
        format!("{:?}", (&a.phasors, a.u_abc, a.i_abc, a.calc_ib, a.n, &d.rms, &d.dc_mean, analog, digital, &d.flagged.station))
    }

    /// Opened, then recomputed in full once more: the same state; and how
    /// many full recomputes the open ran.
    fn open_and_recompute(cfg: &Path) -> (usize, String, String) {
        let mut app = App::new();
        app.persist = false;
        crate::recdlg::RECOMPUTES.with(|c| c.set(0));
        let di = open(&mut app, cfg);
        let runs = crate::recdlg::RECOMPUTES.with(|c| c.get());
        let opened = computed_state(&app.docs[di]);
        app.recompute_doc(di);
        (runs, opened, computed_state(&app.docs[di]))
    }

    /// An open computes the record once (`Doc::new`, on the loader's
    /// thread): the settings apply recomputes only when they changed what
    /// a recompute reads — the result the same as a full recompute either
    /// way, names from the `.set` (an alias) included.
    #[test]
    fn an_open_recomputes_only_when_the_settings_change_the_values() {
        let dir = temp_dir("once");
        let cfg = write_record(&dir);
        let (runs, opened, again) = open_and_recompute(&cfg);
        assert_eq!(runs, 0, "no .set: nothing to recompute");
        assert_eq!(opened, again);
        // A rename in the `.set`: the names change, not the values.
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &cfg);
        crate::chandlg::rename(&mut app.docs[di], ChanKey::Analog(0), "Ua'").unwrap();
        app.track_settings();
        assert!(app.save_active());
        drop(app);
        let (runs, opened, again) = open_and_recompute(&cfg);
        assert_eq!(runs, 0, "an alias: no recompute");
        assert_eq!(opened, again);
        assert!(opened.contains("\"Ua'\""), "the flagged copy has the alias");
        // An inverted channel: recomputed, once.
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &cfg);
        app.docs[di].chans[1].invert = true;
        app.recompute_doc(di);
        app.track_settings();
        assert!(app.save_active());
        drop(app);
        let (runs, opened, again) = open_and_recompute(&cfg);
        assert_eq!(runs, 1);
        assert_eq!(opened, again);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_survive_save_and_reopen() {
        let dir = temp_dir("roundtrip");
        let cfg = write_record(&dir);
        let mut app = App::new();
        let di = open(&mut app, &cfg);
        assert!(!app.docs[di].set.modified);
        {
            let d = &mut app.docs[di];
            d.chans[0].invert = true;
            (d.chans[1].color, d.chans[1].custom_color) = (egui::Color32::from_rgb(1, 2, 3), true);
            d.chans[4].stats.minmax = true;
            d.comments.push(Comment::new(CommentKind::Moment { pos: 100 }));
            let mut lvl = Comment::new(CommentKind::Level { chan: ChanKey::Analog(3), value: 2.5, from: 10, to: 20 });
            lvl.text = "уровень\r\nвторая".into();
            lvl.locked = true;
            d.comments.push(lvl);
            crate::chandlg::rename(d, ChanKey::Analog(2), "Uc'").unwrap();
            d.dif_pairs.push(crate::diagram::ChanPair::new("Ia", "Ib"));
            d.omp.params.method = fvb_core::omp_params::GROUNDED;
            d.omp.params.stations[0].ia = "Ia".into();
        }
        app.recompute_doc(di);
        app.append_computed(&[("X".into(), "Ua+Ub".into())]).unwrap();
        {
            // A digital channel's invert, a computed one's invert and mode,
            // an analog one's own frequency (`ShowFlags`, `NetFreq`).
            let d = &mut app.docs[di];
            d.dig_mut(0).invert = true;
            let n = d.osc.analog[0].samples.len();
            d.osc.analog[0].own_freq = vec![50.5; n];
            d.computed[0].invert = true;
            d.computed[0].mode = crate::app::ValueMode::Rms;
            d.reflag_computed(0);
        }
        app.vec_show_values = !app.vec_show_values;
        app.track_settings();
        assert!(app.docs[di].set.modified);
        let cfg_before = std::fs::read(&cfg).unwrap();
        assert!(app.save_active());
        // Save onto the document's own file rewrites only the .set.
        assert_eq!(std::fs::read(&cfg).unwrap(), cfg_before);
        assert!(!app.docs[di].set.modified);
        let set = std::fs::read(dir.join("rec.set")).unwrap();
        assert_eq!(&set[..2], &[0xFF, 0xFE]);
        let text = fvb_core::decode_fvb(&set);
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"utf-16\"?>\r\n<settings>\r\n"), "{text}");
        assert!(text.contains("<category Channel0=\"X,Ua+Ub\" name=\"CalculatedChannels\"/>"), "{text}");
        assert!(text.contains("name=\"Uc\"") && text.contains("AliasName=\"Uc'\""), "{text}");
        // A differential pair (Iдиф Ia, Iторм Ib) is stored as `doc+0x878`
        // holds it: BrakeCurr = Iторм, DiffCurr = Iдиф (L9.81).
        assert!(text.contains("BrakeCurr=\"Ib\" DiffCurr=\"Ia\""), "{text}");
        let vec_shown = app.vec_show_values;

        let mut back = App::new();
        let bi = open(&mut back, &cfg);
        let d = &back.docs[bi];
        assert!(d.chans[0].invert && !d.chans[1].invert);
        assert_eq!(d.chans[1].color, egui::Color32::from_rgb(1, 2, 3));
        assert!(d.chans[4].stats.minmax);
        assert_eq!(d.osc.analog[2].id, "Uc'");
        assert_eq!(d.dif_pairs, vec![crate::diagram::ChanPair::new("Ia", "Ib")]);
        assert_eq!(d.omp.params.method, fvb_core::omp_params::GROUNDED);
        assert_eq!(d.omp.params.stations[0].ia, "Ia");
        assert_eq!(d.computed.len(), 1);
        assert_eq!((d.computed[0].name.as_str(), d.computed[0].formula.as_str()), ("X", "Ua+Ub"));
        assert!(d.dig(0).invert, "a digital channel's ShowFlags carry its invert");
        assert_eq!((d.osc.analog[0].own_freq.len(), d.osc.analog[0].own_freq.first().copied()), (d.osc.analog[0].samples.len(), Some(50.5)), "NetFreq");
        let x = &d.computed[0];
        assert!(x.invert && x.mode == crate::app::ValueMode::Rms, "restored after the evaluation");
        assert!(x.samples.iter().zip(&x.raw).all(|(s, r)| (s + r).abs() < 1e-9), "and applied");
        // Levels are read before moments; the text comes back with CR LF.
        assert_eq!(d.comments.len(), 2);
        assert!(matches!(d.comments[0].kind, CommentKind::Level { chan: ChanKey::Analog(3), value, from: 10, to: 20 } if value == 2.5));
        assert_eq!(d.comments[0].text, "уровень\r\nвторая");
        assert!(d.comments[0].locked);
        assert_eq!(d.comments[1].kind, CommentKind::Moment { pos: 100 });
        assert_eq!(back.vec_show_values, vec_shown);
        assert!(!d.set.modified);
        let _ = std::fs::remove_dir_all(&dir);
        // An element on a channel the record lacks stays in its place in the
        // list and is saved back (a level and an area alike).
        let rec = |kind| fvb_core::recset::CommentRec { kind, text: String::new(), locked: false, visible: true };
        let lost = rec(fvb_core::recset::CommentKindRec::Level { channel: "нет такого".into(), y: 1.0, from: 1, to: 2 });
        let lost_area = rec(fvb_core::recset::CommentKindRec::Area { channel_from: "Ia".into(), channel_to: "нет".into(), from: 1, to: 2 });
        let d = &mut back.docs[bi];
        let (a, b) = (d.comment_of(&lost), d.comment_of(&lost_area));
        assert!(matches!(a.kind, CommentKind::Kept(_)) && matches!(b.kind, CommentKind::Kept(_)));
        d.comments.insert(0, a);
        d.comments.push(b);
        let mut t = d.set.tree.clone();
        d.store_settings(&mut t, true);
        let saved = fvb_core::recset::read_comments(t.section("Comments").unwrap());
        assert!(saved.contains(&lost) && saved.contains(&lost_area), "{saved:?}");
    }

    #[test]
    fn modified_flag_rules() {
        let dir = temp_dir("modified");
        let cfg = write_record(&dir);
        let mut app = App::new();
        let di = open(&mut app, &cfg);
        // Discrete visibility (32845–32848) does not mark the document.
        app.dispatch(Command::Discrete(1));
        app.track_settings();
        assert!(!app.docs[di].set.modified);
        // A toggle marks it even when toggled back.
        app.dispatch(Command::StatsRow(0));
        app.dispatch(Command::StatsRow(0));
        app.track_settings();
        assert!(app.docs[di].set.modified);
        assert!(app.save_active());
        assert!(!app.docs[di].set.modified && dir.join("rec.set").exists());
        // Any saved state changed through the UI marks it.
        app.docs[di].comments.push(Comment::new(CommentKind::Moment { pos: 5 }));
        app.track_settings();
        assert!(app.docs[di].set.modified);
        // Closing a modified document asks; «Нет» closes without writing.
        let ctx = egui::Context::default();
        app.dispatch(Command::CloseDoc(di));
        assert_eq!(app.setfile.prompt, Some(SavePrompt { doc: di, exit: false }));
        assert_eq!(app.docs.len(), 1);
        app.answer_save_prompt(&ctx, None);
        assert_eq!(app.docs.len(), 1, "Отмена keeps it open");
        app.dispatch(Command::CloseDoc(di));
        app.answer_save_prompt(&ctx, Some(false));
        assert!(app.docs.is_empty());
        let saved = fvb_core::decode_fvb(&std::fs::read(dir.join("rec.set")).unwrap());
        assert!(!saved.contains("Moment1"), "Нет writes nothing");
        // A document not read from a file starts clean and closes at once.
        let osc = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        app.add_doc(Doc::new(osc, "demo".into()));
        app.track_settings();
        assert!(!app.docs[0].set.modified);
        app.dispatch(Command::CloseDoc(0));
        assert!(app.docs.is_empty() && app.setfile.prompt.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_a_file_in_the_original_layout_and_the_legacy_ini() {
        let dir = temp_dir("original");
        let cfg = write_record(&dir);
        let xml = "<?xml version=\"1.0\" encoding=\"utf-16\"?>\n<settings>\n\
            \t<category name=\"Oscillogram\" DiscretisationFrequensy=\"1000\" Codepage=\"0\"/>\n\
            \t<category name=\"Ua\" ShowFlags=\"2181\" AliasName=\"Фаза A\" CustomColorSet=\"1\" colorCustom=\"16711680\"/>\n\
            \t<category name=\"CalculatedChannels\" Channel0=\"U0,Ub+Uc\"/>\n\
            \t<category name=\"View\" XScale=\"0.125000\" YAnalogSize=\"121\" RulerState=\"4\" Sells=\"0\"/>\n\
            \t<category name=\"Comments\">\n\t\t<category name=\"Moments\">\n\
            \t\t\t<category name=\"Moment1\" position=\"42\" text=\"пуск\" visible=\"0\"/>\n\
            \t\t</category>\n\t</category>\n</settings>\n";
        std::fs::write(dir.join("rec.set"), fvb_core::settree::utf16le_crlf(xml)).unwrap();
        let mut app = App::new();
        let di = open(&mut app, &cfg);
        let d = &app.docs[di];
        // Into rec+0x228 only: the time axis stays (`0x44f72c`, L9.142).
        assert_eq!(d.osc.rate_r, 1000.0);
        assert!(d.osc.time.len() < 2 || d.osc.time[1] - d.osc.time[0] > 0.0);
        // 2181 = visible | vector | instantaneous | invert.
        assert!(d.chans[0].invert && d.chans[0].visible && d.chans[0].on_vector && !d.chans[0].on_freq);
        assert_eq!(d.osc.analog[0].id, "Фаза A");
        assert_eq!(d.chans[0].color, egui::Color32::from_rgb(0, 0, 255));
        assert_eq!(d.computed.len(), 1);
        assert_eq!(d.computed[0].name, "U0");
        assert_eq!(d.xscale, Some(0.125));
        assert_eq!((app.y_analog, app.time_unit, app.show_grid), (121.0, TimeUnit::Samples, false));
        assert_eq!(d.comments, vec![Comment { kind: CommentKind::Moment { pos: 42 }, text: "пуск".into(), locked: false, hidden: true }]);
        assert!(!d.set.modified);

        // 32860 reads another file without its comments and marks the
        // document; the formula channels are replaced.
        let other = dir.join("other.set");
        let xml2 = "<settings><category name=\"CalculatedChannels\" Channel0=\"I0,Ia+Ib+Ic\"/>\
            <category name=\"Comments\"><category name=\"Moments\"><category name=\"Moment1\" position=\"1\"/></category></category></settings>";
        std::fs::write(&other, fvb_core::settree::utf16le_crlf(xml2)).unwrap();
        app.load_settings_from(di, &other);
        let d = &app.docs[di];
        assert_eq!(d.computed.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["I0"]);
        assert_eq!(d.comments.len(), 1);
        assert_eq!(d.comments[0].kind, CommentKind::Moment { pos: 42 });
        assert!(d.set.modified);

        // No .set: the legacy `<base>.ini` is read instead.
        let dir2 = temp_dir("ini");
        let cfg2 = write_record(&dir2);
        std::fs::write(dir2.join("rec.ini"), b"[Ub]\r\nAliasName=Phase B\r\n").unwrap();
        let mut app2 = App::new();
        let d2 = open(&mut app2, &cfg2);
        assert_eq!(app2.docs[d2].osc.analog[1].id, "Phase B");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[test]
    fn save_prompt_needed_only_when_modified_and_not_sto() {
        let osc = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        let mut d = Doc::new(osc, "x".into());
        assert!(!needs_save_prompt(&d));
        d.set.modified = true;
        assert!(needs_save_prompt(&d));
        d.path = Some("/a/b.STO".into());
        assert!(!needs_save_prompt(&d));
        assert_eq!(save_kind(Path::new("/a/b.CFG")), Some(".cfg"));
        assert_eq!(save_kind(Path::new("/a/b.dat")), None);
        assert_eq!((cp_code(Encoding::Cp866), cp_code(Encoding::Cp1251)), (0, 1));
        assert_eq!((cp_of(0), cp_of(1)), (Encoding::Cp866, Encoding::Cp1251));
    }

    /// #31: the definition list kill-active writes — rows without a channel and the order ↑ / ↓ gave — is what the
    /// `.set` saves as `CalculatedChannels` (`0x4153a0`): a reopened record has them as channels, in that order.
    #[test]
    fn the_definition_list_is_saved_in_its_order() {
        let dir = temp_dir("deflist");
        let cfg = write_record(&dir);
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &cfg);
        app.active = Some(di);
        assert!(app.append_computed(&[("X".into(), "Ua*2".into()), ("Y".into(), "Ub*2".into())]).is_some());
        app.dispatch(Command::QuickAnalysis);
        app.formula_page.selected = Some(1);
        app.fp_move(true);
        app.fp_add_row("Z1".into(), "Uc*2".into());
        app.formula_page.selected = Some(2);
        app.fp_move(true);
        app.fp_kill_active();
        app.quick_open = false;
        app.docs[di].set.modified = true;
        assert_eq!(app.docs[di].definition_pairs().iter().map(|p| p.0.as_str()).collect::<Vec<_>>(), ["Y", "Z1", "X"]);
        assert!(app.save_active());
        drop(app);
        let mut back = App::new();
        back.persist = false;
        let bi = open(&mut back, &cfg);
        let names: Vec<_> = back.docs[bi].computed.iter().map(|c| (c.name.as_str(), c.formula.as_str())).collect();
        assert_eq!(names, [("Y", "Ub*2"), ("Z1", "Uc*2"), ("X", "Ua*2")]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// #31: «Загрузка настроек» (32860, `0x4156e0`) empties the calculation manager's lists with the channels: a
    /// row kept by kill-active and the list's order do not survive into the loaded file's channels.
    #[test]
    fn loading_settings_clears_the_kept_rows() {
        let dir = temp_dir("loadclear");
        let cfg = write_record(&dir);
        let mut app = App::new();
        app.persist = false;
        let di = open(&mut app, &cfg);
        app.active = Some(di);
        assert!(app.append_computed(&[("X".into(), "Ua*2".into())]).is_some());
        app.docs[di].set.modified = true;
        assert!(app.save_active());
        app.dispatch(Command::QuickAnalysis);
        app.fp_add_row("Z1".into(), "Uc*2".into());
        app.fp_kill_active();
        app.quick_open = false;
        assert_eq!(app.docs[di].loose_defs.len(), 1);
        app.load_settings_from(di, &dir.join("rec.set"));
        let d = &app.docs[di];
        assert_eq!(d.computed.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["X"]);
        assert!(d.loose_defs.is_empty() && d.def_order.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

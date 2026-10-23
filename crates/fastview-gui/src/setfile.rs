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

/// `Codepage`: 0 = the ANSI code page (1251 here), 1 = the OEM one (866);
/// other values are code page numbers (`0x4543e0`).
pub fn cp_code(e: Encoding) -> i32 {
    i32::from(e == Encoding::Cp866)
}

pub fn cp_of(code: i32) -> Encoding {
    match code {
        1 | 866 => Encoding::Cp866,
        20866 => Encoding::Koi8r,
        65001 => Encoding::Utf8,
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

/// The name after OK: an extension of the filter list is kept, any other
/// name gets one appended ("x.txt" → "x.txt.cfg"). The original appends
/// the chosen filter's; the file dialog here does not report the choice,
/// so the first filter's is used.
pub fn save_target(p: std::path::PathBuf, exts: &[&str]) -> std::path::PathBuf {
    let s = p.to_string_lossy().to_lowercase();
    if exts.iter().any(|e| s.ends_with(&format!(".{e}"))) {
        return p;
    }
    let mut o = p.into_os_string();
    o.push(format!(".{}", exts[0]));
    o.into()
}

impl Doc {
    /// The section key of file channel `i` (analog, then digital).
    fn file_key(&self, i: usize) -> String {
        if let Some(k) = self.set.keys.get(i) {
            return k.clone();
        }
        let id = if i < self.osc.analog.len() { &self.osc.analog[i].id } else { &self.osc.digital[i - self.osc.analog.len()].id };
        comtrade::recode(id, self.codepage, Encoding::Cp1251)
    }

    /// Remember the channels' file names (after the names were decoded).
    pub fn init_set_keys(&mut self) {
        self.set.keys.clear();
        let keys: Vec<String> = (0..self.osc.analog.len() + self.osc.digital.len()).map(|i| self.file_key(i)).collect();
        self.set.keys = keys;
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
        if (self.computed.len(), self.osc.digital.len()) == before {
            return false;
        }
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

    /// `AliasName`: the displayed name when it differs from the file's.
    fn alias(&self, i: usize, id: &str) -> String {
        let file = comtrade::recode(&self.file_key(i), Encoding::Cp1251, self.codepage);
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
        bit(self.digital_visible.get(k).copied().unwrap_or(true), flag::VISIBLE) | mode_bits(f.mode) | bit(f.remove_dc, flag::REMOVE_DC) | bit(f.invert, flag::INVERT)
    }

    /// The modelled bits replace their stored values; the others (bits we
    /// do not model, a stored `NetFreq`, …) are kept.
    fn merged_flags(t: &SetTree, key: &str, ours: u32, known: u32) -> u32 {
        let old = t.section(key).map_or(0, |s| s.get_int("ShowFlags", 0) as u32);
        ours | (old & !known)
    }

    /// `0x44cbc0` minus the serialisation: update the tree in place from the
    /// document. `print` = the change detector's variant (visibility, which
    /// 32845–32848 change without marking the document, and ОМП / pending
    /// channels left out).
    pub fn store_settings(&self, t: &mut SetTree, print: bool) {
        let calc: Vec<(String, String)> = self.computed.iter().filter(|c| is_formula(c)).map(|c| (c.name.clone(), c.formula.clone())).collect();
        recset::write_calculated(t.section_mut("CalculatedChannels"), &calc);
        recset::write_omp(t.section_mut("WrongPlaceFinding"), &self.omp.params);
        let mask = if print { !flag::VISIBLE } else { u32::MAX };
        let known_a = (1 << 13) - 1;
        for i in 0..self.osc.analog.len() {
            let key = self.file_key(i);
            let a = &self.osc.analog[i];
            let def = crate::app::default_channel_color(a, i);
            let rec = ChannelRec {
                show_flags: Self::merged_flags(t, &key, self.analog_flags(i), known_a) & mask,
                primary_mul: a.primary,
                secondary_mul: a.secondary,
                alias: self.alias(i, &a.id),
                custom_color: (self.chans[i].color != def).then(|| colorref(self.chans[i].color)),
            };
            recset::write_channel(t.section_mut(&key), &rec);
            recset::write_net_freq(t.section_mut(&key), &a.own_freq);
        }
        let na = self.osc.analog.len();
        for k in 0..self.osc.digital.len() {
            // The ОМП run's phase lanes are not a change (it never marks
            // the document, `0x4a00b0`).
            if print && crate::omp_ui::KZ_PHASES.contains(&self.osc.digital[k].id.as_str()) {
                continue;
            }
            let key = self.file_key(na + k);
            let known_d = flag::VISIBLE | flag::MODE_INSTANT | flag::MODE_RMS | flag::MODE_HARMONIC | flag::REMOVE_DC | flag::INVERT;
            let rec = ChannelRec {
                show_flags: Self::merged_flags(t, &key, self.digital_flags(k), known_d) & mask,
                primary_mul: 1.0,
                secondary_mul: 1.0,
                alias: self.alias(na + k, &self.osc.digital[k].id),
                custom_color: None,
            };
            recset::write_channel(t.section_mut(&key), &rec);
        }
        for (j, c) in self.computed.iter().enumerate() {
            if print && !is_formula(c) {
                continue;
            }
            let known_c = flag::VISIBLE | flag::VECTOR | flag::CIRCLE | flag::MINMAX | flag::A1 | flag::RMS_ROW | flag::MODE_INSTANT | flag::MODE_RMS | flag::MODE_HARMONIC | flag::REMOVE_DC | flag::INVERT | flag::COMPUTED;
            let rec = ChannelRec {
                show_flags: Self::merged_flags(t, &c.name, self.computed_flags(j), known_c) & mask,
                primary_mul: 1.0,
                secondary_mul: 1.0,
                alias: String::new(),
                custom_color: Some(colorref(c.color)),
            };
            recset::write_channel(t.section_mut(&c.name), &rec);
        }
        let o = t.section_mut("Oscillogram");
        o.set_int("Codepage", cp_code(self.codepage));
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
    }

    fn comment_rec(&self, c: &Comment) -> CommentRec {
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
    fn comment_of(&self, r: &CommentRec) -> Comment {
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
        t.to_xml()
    }

    /// 32805 OK (`0x44c2e0`): the text without CR into `Comments/UserData`.
    pub fn store_user_data(&mut self) {
        let text = self.hdr.clone().unwrap_or_default().replace('\r', "");
        let c = self.set.tree.section_mut("Comments");
        c.set_str("UserData", &text);
        c.set_int("Codepage", cp_code(self.hdr_cp));
        self.set.modified = true;
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

impl App {
    /// Read the settings for document `di` (`0x44f5b0`); `from_command` =
    /// 32860 (comments are not re-read).
    pub fn apply_record_settings(&mut self, di: usize, from_command: bool) {
        let Some(d) = self.docs.get_mut(di) else { return };
        if !from_command {
            if let Some(c) = d.set.tree.section("Comments") {
                d.comments = recset::read_comments(c).iter().map(|r| d.comment_of(r)).collect();
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
        if matches!(d.codepage, Encoding::Cp1251 | Encoding::Cp866) {
            if let Some(cp) = osc.attrs.get("Codepage").map(|v| recset::wtoi(v)) {
                d.set_codepage(cp_of(cp));
            }
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
                }
                if let Some(c) = r.custom_color {
                    d.chans[i].color = from_colorref(c);
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
                *d.dig_mut(k) = crate::app::DigFlags { invert: r.show_flags & flag::INVERT != 0, remove_dc: r.show_flags & flag::REMOVE_DC != 0, mode: mode_of(r.show_flags) };
                if !r.alias.is_empty() {
                    d.osc.digital[k].id = r.alias;
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
        self.recompute_doc(di);
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
                c.color = from_colorref(v);
            }
            d.reflag_computed(j);
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
        self.apply_record_settings(di, false);
        self.docs[di].set.modified = false;
    }

    /// 32860 «Загрузка настроек...» (`0x44fd10`): pick a `.set`, read it
    /// without the comments, mark modified, refresh.
    pub fn load_settings_command(&mut self) {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else { return };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Настройки осциллограммы FastView (*.set)", &["set"])
            .add_filter("Все файлы (*.*)", &["*"])
            .pick_file()
        else {
            return;
        };
        self.load_settings_from(ai, &path);
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
        let same = d.path.as_deref() == Some(target);
        if !same {
            match kind {
                ".cff" => {
                    let bytes = comtrade::write_cff(&d.osc, &d.cff_sections()).map_err(|e| e.to_string())?;
                    std::fs::write(target, bytes).map_err(|e| e.to_string())?;
                }
                ".osc" => {
                    // `0x44ca80` writes the record's own bytes to the new name.
                    let src = d.path.clone().filter(|p| save_kind(p) == Some(".osc")).ok_or("Нет исходного файла .osc")?;
                    std::fs::copy(src, target).map_err(|e| e.to_string())?;
                }
                _ => {
                    let (cfg, dat) = comtrade::write(&d.osc);
                    std::fs::write(target, cfg).map_err(|e| e.to_string())?;
                    std::fs::write(target.with_extension("dat"), dat).map_err(|e| e.to_string())?;
                    if let Some(b) = d.hdr_file_bytes() {
                        std::fs::write(target.with_extension("hdr"), b).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        let d = &mut self.docs[di];
        // The `.cfg` / `.cff` writers set the trigger sample to 0
        // (`0x44d6c5`, `0x44daaa`): a record saved so loses its trigger.
        if !same && kind != ".osc" {
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

    /// Сохранить (57603, `DoFileSave`): onto the document's own file when it
    /// has one the save dispatcher knows, else Save As.
    pub fn save_active(&mut self) -> bool {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else { return false };
        match self.docs[ai].path.clone().filter(|p| save_kind(p).is_some()) {
            Some(p) => self.report_save(ai, &p),
            None => self.save_active_as(),
        }
    }

    /// Сохранить как (57604): the file dialog, then `OnSaveDocument`.
    pub fn save_active_as(&mut self) -> bool {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else {
            self.status = "Нет осциллограммы для сохранения.".into();
            return false;
        };
        let d = &self.docs[ai];
        let filters = save_filters(d.path.as_deref());
        let mut dlg = rfd::FileDialog::new();
        for (name, ext) in filters {
            dlg = dlg.add_filter(name, &[ext]);
        }
        // Proposed: the record's folder and its title without extension.
        let stem = d.path.as_ref().and_then(|p| p.file_stem()).map_or_else(|| d.name.clone(), |s| s.to_string_lossy().into_owned());
        if let Some(dir) = d.path.as_ref().and_then(|p| p.parent()).filter(|p| !p.as_os_str().is_empty()) {
            dlg = dlg.set_directory(dir);
        }
        let Some(path) = dlg.set_file_name(stem).save_file() else { return false };
        let path = save_target(path, &filters.map(|f| f.1));
        self.report_save(ai, &path)
    }

    fn report_save(&mut self, di: usize, path: &std::path::Path) -> bool {
        match self.save_doc_to(di, path) {
            Ok(()) => {
                self.status = format!("Сохранено: {}", path.display());
                true
            }
            // The writer's exception is reported in an MB_OK box (`0x475680`
            // → `0x4756e0`).
            Err(e) => {
                self.dlg.push_alert(format!("{} - {e}", path.display()));
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
                        recset::write_plane_chart(t.section_mut("Circle Chart"), &now.circle);
                    }
                    if now.diff != old.diff {
                        recset::write_plane_chart(t.section_mut("Diff Chart"), &now.diff);
                    }
                    if now.vector != old.vector {
                        recset::write_vector_chart(t.section_mut("Vector Chart"), &now.vector);
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
        let go_on = match answer {
            None => false,
            Some(false) => true,
            Some(true) => {
                let prev = self.active;
                self.active = Some(p.doc);
                let ok = self.save_active();
                self.active = prev;
                ok
            }
        };
        if !go_on {
            self.setfile.exit_queue = None;
            return;
        }
        if p.exit {
            self.next_exit_prompt(ctx);
        } else {
            self.close_doc_now(p.doc);
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
        egui::Window::new(crate::app::APP_TITLE)
            .id(egui::Id::new("fv_save_prompt"))
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                crate::dialogs::message_body(ui, crate::dialogs::MsgIcon::Question, &format!("Сохранить изменения в {title}?"));
                ui.horizontal(|ui| {
                    if ui.button("   Да   ").clicked() {
                        answer = Some(Some(true));
                    }
                    if ui.button("   Нет   ").clicked() {
                        answer = Some(Some(false));
                    }
                    if ui.button(" Отмена ").clicked() {
                        answer = Some(None);
                    }
                });
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

    #[test]
    fn save_as_filters_and_extension() {
        let f = save_filters(Some(Path::new("/r/a.OSC")));
        assert_eq!(f.map(|x| x.1), ["cfg", "osc"]);
        assert_eq!(save_filters(Some(Path::new("/r/a.sto"))).map(|x| x.1), ["cfg", "cff"]);
        assert_eq!(save_filters(None).map(|x| x.1), ["cfg", "cff"]);
        let exts = ["cfg", "cff"];
        assert_eq!(save_target(PathBuf::from("/r/x.CFF"), &exts), PathBuf::from("/r/x.CFF"));
        assert_eq!(save_target(PathBuf::from("/r/x.txt"), &exts), PathBuf::from("/r/x.txt.cfg"));
        assert_eq!(save_target(PathBuf::from("/r/x.osc"), &exts), PathBuf::from("/r/x.osc.cfg"));
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
            d.chans[1].color = egui::Color32::from_rgb(1, 2, 3);
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
        assert_eq!((cp_code(Encoding::Cp866), cp_code(Encoding::Cp1251)), (1, 0));
        assert_eq!((cp_of(1), cp_of(0)), (Encoding::Cp866, Encoding::Cp1251));
    }
}

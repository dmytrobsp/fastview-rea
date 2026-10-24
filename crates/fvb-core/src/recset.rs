//! The per-record settings file (`<base>.set`, legacy `<base>.ini`): the
//! document's settings tree (`doc+0x300`) — a map of named sections, each a
//! node with attributes and named children, both kept ordered by name
//! (`CString::Compare`). Loader `0x48e470`, writer `0x48e1c0`; the record
//! settings are read by `0x44f5b0` and written by `0x44cbc0`. See
//! `notes/UI_BEHAVIOUR_SPEC.md` §5.6 / §6.7 and the ledger (track S).
//!
//! On disk the tree is the in-house XML ([`crate::settree`]): a root
//! `<settings>` whose every section and child is a `<category name="KEY" …>`
//! carrying the node's attributes (`name` sorted in among them). Reading
//! ignores the tag names and keys every element by its `name` attribute,
//! which stays among the node's attributes.

use crate::settree::{self, Node};
use std::collections::BTreeMap;

/// One node of the tree.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SetNode {
    pub attrs: BTreeMap<String, String>,
    pub children: BTreeMap<String, SetNode>,
}

/// `_wtoi`: leading white space, a sign, decimal digits; 0 without digits.
pub fn wtoi(s: &str) -> i32 {
    let t = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let (neg, digits) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let mut v: i64 = 0;
    for b in digits.bytes().take_while(u8::is_ascii_digit) {
        v = (v * 10 + i64::from(b - b'0')).min(i64::from(u32::MAX) + 1);
    }
    let v = if neg { -v } else { v };
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl SetNode {
    /// `0x48da40`: the value, or `default` when the key is absent.
    pub fn get_str(&self, k: &str, default: &str) -> String {
        self.attrs.get(k).cloned().unwrap_or_else(|| default.into())
    }

    /// `0x48db10`: `_wtoi` of the value, `default` only when absent.
    pub fn get_int(&self, k: &str, default: i32) -> i32 {
        self.attrs.get(k).map_or(default, |v| wtoi(v))
    }

    /// `0x48dbb0`: `_wtof` of the value, `default` only when absent.
    pub fn get_dbl(&self, k: &str, default: f64) -> f64 {
        self.attrs.get(k).map_or(default, |v| crate::omp_params::wtof(v))
    }

    /// A boolean stored as an int (non-zero = set).
    pub fn get_bool(&self, k: &str, default: bool) -> bool {
        self.get_int(k, i32::from(default)) != 0
    }

    /// `0x48d970`.
    pub fn set_str(&mut self, k: &str, v: &str) {
        self.attrs.insert(k.into(), v.into());
    }

    /// `0x48d780`: `"%i"`.
    pub fn set_int(&mut self, k: &str, v: i32) {
        self.attrs.insert(k.into(), v.to_string());
    }

    pub fn set_bool(&mut self, k: &str, v: bool) {
        self.set_int(k, i32::from(v));
    }

    /// `0x48d870`: `"%f"` (six decimals).
    pub fn set_dbl(&mut self, k: &str, v: f64) {
        self.attrs.insert(k.into(), format!("{v:.6}"));
    }

    pub fn child(&self, k: &str) -> Option<&SetNode> {
        self.children.get(k)
    }

    /// `0x48dea0`: the child, created empty when missing.
    pub fn child_mut(&mut self, k: &str) -> &mut SetNode {
        self.children.entry(k.into()).or_default()
    }

    fn to_xml(&self, key: &str) -> Node {
        let mut n = Node::new("category");
        n.set("name", key);
        for (k, v) in &self.attrs {
            n.set(k, v);
        }
        n.children = self.children.iter().map(|(k, c)| c.to_xml(k)).collect();
        n
    }

    /// `0x48e370`: every attribute (`name` included) set into the node, each
    /// child element into the child its `name` selects — got or created, so
    /// elements sharing a name merge (later attributes win).
    fn merge_xml(&mut self, n: &Node) {
        for (k, v) in &n.attrs {
            self.attrs.insert(k.clone(), v.clone());
        }
        for c in &n.children {
            self.children.entry(c.get("name", "")).or_default().merge_xml(c);
        }
    }
}

/// The whole tree.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SetTree {
    pub sections: BTreeMap<String, SetNode>,
}

impl SetTree {
    pub fn section(&self, k: &str) -> Option<&SetNode> {
        self.sections.get(k)
    }

    /// `0x48dc80`: the section, created empty when missing.
    pub fn section_mut(&mut self, k: &str) -> &mut SetNode {
        self.sections.entry(k.into()).or_default()
    }

    /// The document text `0x48e1c0` writes.
    pub fn to_xml(&self) -> String {
        let mut root = Node::new("settings");
        root.children = self.sections.iter().map(|(k, s)| s.to_xml(k)).collect();
        root.to_document()
    }

    /// The file bytes: BOM + UTF-16LE, CR LF line ends (`"w, ccs=UTF-16LE"`).
    pub fn to_file_bytes(&self) -> Vec<u8> {
        settree::utf16le_crlf(&self.to_xml())
    }

    /// Load a settings file's bytes into the tree (`0x48e470`). The XML
    /// reading skips the first two bytes (the BOM) and reads UTF-16LE; any
    /// error is swallowed and loads nothing. A non-empty root replaces the
    /// whole tree; an empty one falls back to the INI reading
    /// (`[section]` lines and `key=value` lines), which merges into the
    /// tree as it is.
    // `chunks_exact(2)` is portable; the suggested `as_chunks` would raise the MSRV.
    #[allow(clippy::chunks_exact_to_as_chunks)]
    pub fn load_bytes(&mut self, bytes: &[u8]) {
        let wide = bytes.get(2..).unwrap_or_default();
        let units: Vec<u16> = wide.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let text = String::from_utf16_lossy(&units);
        if let Ok(root) = Node::parse(&text) {
            if !root.children.is_empty() || !root.attrs.is_empty() {
                self.sections.clear();
                for c in &root.children {
                    self.sections.entry(c.get("name", "")).or_default().merge_xml(c);
                }
                return;
            }
        }
        self.load_ini(&crate::comtrade::decode_bytes(bytes, crate::comtrade::Encoding::Cp1251));
    }

    /// The INI fallback of `0x48e470` (`fgetws` lines; `"[%[^]]"` selects or
    /// creates a section, `"%[^=\n\r]=%[^\n\r"` with a non-empty key sets a
    /// value; lines before any section are dropped).
    fn load_ini(&mut self, text: &str) {
        let mut cur: Option<String> = None;
        for line in text.split('\n') {
            let line = line.trim_end_matches('\r');
            if let Some(rest) = line.strip_prefix('[') {
                let name: String = rest.chars().take_while(|&c| c != ']').collect();
                if !name.is_empty() {
                    self.section_mut(&name);
                    cur = Some(name);
                }
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            if k.is_empty() || v.is_empty() {
                continue;
            }
            if let Some(s) = &cur {
                self.section_mut(s).set_str(k, v);
            }
        }
    }

    /// Read a file as the record loader does: `path` when it exists, else
    /// the path minus its last three characters + `ini` (`0x44f5b0`). A
    /// missing file loads nothing (the parser swallows the open error).
    pub fn load_path(&mut self, path: &std::path::Path) {
        let p = if path.exists() {
            path.to_path_buf()
        } else {
            let s = path.to_string_lossy();
            let n = s.chars().count();
            if n <= 3 {
                return;
            }
            std::path::PathBuf::from(format!("{}ini", s.chars().take(n - 3).collect::<String>()))
        };
        if let Ok(b) = std::fs::read(p) {
            self.load_bytes(&b);
        }
    }
}

/// The settings file of a record (`0x44c710`): when the last four
/// characters, lowercased, are `.cfg`, `.osc`, `.sto`, `.zip` or `.cff` they
/// are dropped, otherwise the whole path is kept; then `.set` (`0x4c85d0`)
/// is appended (`x.OSC` → `x.set`, `x.dat` → `x.dat.set`).
pub fn set_path(record: &std::path::Path) -> std::path::PathBuf {
    let s = record.to_string_lossy();
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let tail: String = chars[n.saturating_sub(4)..].iter().collect::<String>().to_lowercase();
    let base: String = if [".cfg", ".osc", ".sto", ".zip", ".cff"].contains(&tail.as_str()) {
        chars[..n - 4].iter().collect()
    } else {
        s.into_owned()
    };
    std::path::PathBuf::from(format!("{base}.set"))
}

// ---------------------------------------------------------------- Comments

/// The kind fields of a review element.
#[derive(Debug, Clone, PartialEq)]
pub enum CommentKindRec {
    /// `MomentItem` (`0x47a2f0`): `position` (sample).
    Moment { position: i32 },
    /// `LevelItem` (`0x470b20` / `0x470c10`): `channel` (name), `positionY`
    /// (value, `%f`), `positionFrom` / `positionTo` (samples).
    Level { channel: String, y: f64, from: i32, to: i32 },
    /// `AreaItem` (`0x403820` / `0x403930`): `channelFrom` / `channelTo`
    /// (names), `positionFrom` / `positionTo`.
    Area { channel_from: String, channel_to: String, from: i32, to: i32 },
}

/// One review element as stored (`0x43ddd0` / `0x43deb0`: `text`, `locked`
/// default 0, `visible` default 1).
#[derive(Debug, Clone, PartialEq)]
pub struct CommentRec {
    pub kind: CommentKindRec,
    pub text: String,
    pub locked: bool,
    pub visible: bool,
}

/// `0x43fcf0`: children `Areas`, `Levels`, `Moments` in that order; in each
/// the items `Area<n>` / `Level<n>` / `Moment<n>` from n = child count down
/// to 1, appended in that (descending) order. The text is taken as stored.
pub fn read_comments(sec: &SetNode) -> Vec<CommentRec> {
    let mut out = Vec::new();
    for (list, item) in [("Areas", "Area"), ("Levels", "Level"), ("Moments", "Moment")] {
        let Some(l) = sec.child(list) else { continue };
        for n in (1..=l.children.len()).rev() {
            let Some(e) = l.child(&format!("{item}{n}")) else { continue };
            let kind = match item {
                "Area" => CommentKindRec::Area {
                    channel_from: e.get_str("channelFrom", ""),
                    channel_to: e.get_str("channelTo", ""),
                    from: e.get_int("positionFrom", 0),
                    to: e.get_int("positionTo", 0),
                },
                "Level" => CommentKindRec::Level {
                    channel: e.get_str("channel", ""),
                    y: e.get_dbl("positionY", 0.0),
                    from: e.get_int("positionFrom", 0),
                    to: e.get_int("positionTo", 0),
                },
                _ => CommentKindRec::Moment { position: e.get_int("position", 0) },
            };
            out.push(CommentRec {
                kind,
                text: e.get_str("text", ""),
                locked: e.get_bool("locked", false),
                visible: e.get_bool("visible", true),
            });
        }
    }
    out
}

/// `0x4402d0`: `Moments`, `Areas` and `Levels` are cleared, then every item
/// in document order becomes `Moment<n>` / `Level<n>` / `Area<n>` with a
/// counter per kind from 1; the text with `\r\n` replaced by `\n`.
pub fn write_comments(sec: &mut SetNode, items: &[CommentRec]) {
    for l in ["Moments", "Areas", "Levels"] {
        *sec.child_mut(l) = SetNode::default();
    }
    let mut count = [0usize; 3];
    for c in items {
        let (list, item, k) = match c.kind {
            CommentKindRec::Moment { .. } => ("Moments", "Moment", 0),
            CommentKindRec::Level { .. } => ("Levels", "Level", 1),
            CommentKindRec::Area { .. } => ("Areas", "Area", 2),
        };
        count[k] += 1;
        let e = sec.child_mut(list).child_mut(&format!("{item}{}", count[k]));
        e.set_str("text", &c.text.replace("\r\n", "\n"));
        e.set_bool("locked", c.locked);
        e.set_bool("visible", c.visible);
        match &c.kind {
            CommentKindRec::Moment { position } => e.set_int("position", *position),
            CommentKindRec::Level { channel, y, from, to } => {
                e.set_str("channel", channel);
                e.set_dbl("positionY", *y);
                e.set_int("positionFrom", *from);
                e.set_int("positionTo", *to);
            }
            CommentKindRec::Area { channel_from, channel_to, from, to } => {
                e.set_int("positionFrom", *from);
                e.set_int("positionTo", *to);
                e.set_str("channelFrom", channel_from);
                e.set_str("channelTo", channel_to);
            }
        }
    }
}

// ---------------------------------------------------------------- channels

/// Channel flag bits (`ch+0x60`, `ShowFlags`).
pub mod flag {
    pub const VISIBLE: u32 = 1 << 0;
    pub const FREQUENCY: u32 = 1 << 1;
    pub const VECTOR: u32 = 1 << 2;
    pub const CIRCLE: u32 = 1 << 3;
    pub const MINMAX: u32 = 1 << 4;
    pub const A1: u32 = 1 << 5;
    pub const RMS_ROW: u32 = 1 << 6;
    pub const MODE_INSTANT: u32 = 1 << 7;
    pub const MODE_RMS: u32 = 1 << 8;
    pub const MODE_HARMONIC: u32 = 1 << 9;
    pub const REMOVE_DC: u32 = 1 << 10;
    pub const INVERT: u32 = 1 << 11;
    pub const ADC: u32 = 1 << 12;
    pub const COMPUTED: u32 = 1 << 13;
    pub const PENDING: u32 = 1 << 14;
}

/// A channel's section (`0x427bc0` writer / `0x427d20` reader), keyed by the
/// channel's file name; never cleared, so keys it does not write (a loaded
/// `NetFreq`) survive.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelRec {
    pub show_flags: u32,
    pub primary_mul: f64,
    pub secondary_mul: f64,
    /// `AliasName`: a non-empty alias becomes the displayed name.
    pub alias: String,
    /// `CustomColorSet` / `colorCustom` (a COLORREF 0x00BBGGRR, signed `%i`).
    pub custom_color: Option<u32>,
}

/// `0x427d20`: every attribute defaults to the channel's current value
/// (`cur`); `CustomColorSet` defaults to 0.
pub fn read_channel(sec: &SetNode, cur: &ChannelRec) -> ChannelRec {
    let custom = sec.get_bool("CustomColorSet", false);
    ChannelRec {
        show_flags: sec.get_int("ShowFlags", cur.show_flags as i32) as u32,
        primary_mul: sec.get_dbl("PrimaryMul", cur.primary_mul),
        secondary_mul: sec.get_dbl("SecondaryMul", cur.secondary_mul),
        alias: sec.get_str("AliasName", &cur.alias),
        custom_color: custom.then(|| sec.get_int("colorCustom", 0) as u32),
    }
}

/// `0x427bc0`.
pub fn write_channel(sec: &mut SetNode, c: &ChannelRec) {
    sec.set_int("ShowFlags", c.show_flags as i32);
    sec.set_dbl("PrimaryMul", c.primary_mul);
    sec.set_dbl("SecondaryMul", c.secondary_mul);
    sec.set_str("AliasName", &c.alias);
    sec.set_bool("CustomColorSet", c.custom_color.is_some());
    sec.set_int("colorCustom", c.custom_color.unwrap_or(0) as i32);
}

/// `NetFreq` (`0x427bc0` via `0x427a00`): an analog channel's own
/// frequency vector, every element `%g`, joined by "," — written only when
/// it is not empty.
pub fn write_net_freq(sec: &mut SetNode, v: &[f64]) {
    if !v.is_empty() {
        let s: Vec<String> = v.iter().map(|&x| crate::views::fmt_g(x)).collect();
        sec.set_str("NetFreq", &s.join(","));
    }
}

/// `0x427af0`: a non-empty `NetFreq` resizes the vector to the channel's
/// `count` values (old ones kept, new ones 0) and fills it from the
/// comma-separated `_wtof` pieces, stopping at `count` or the string's end;
/// none or an empty one leaves `cur`.
pub fn read_net_freq(sec: &SetNode, count: usize, cur: &[f64]) -> Vec<f64> {
    let s = sec.get_str("NetFreq", "");
    if s.is_empty() {
        return cur.to_vec();
    }
    let mut v = cur.to_vec();
    v.resize(count, 0.0);
    for (slot, piece) in v.iter_mut().zip(s.split(',')) {
        *slot = crate::clipdata::wtof(piece);
    }
    v
}

// ---------------------------------------------------------------- formulas

/// `0x415500` over `CalculatedChannels`: `Channel<i>` for i below the
/// attribute map's size (a `name` attribute counts too; its extra index
/// finds no key), each `name,formula` (`"%[^,\n\r],%[^\r\n"`: both parts
/// non-empty, the formula is the rest of the line).
pub fn read_calculated(sec: &SetNode) -> Vec<(String, String)> {
    let n = sec.attrs.len();
    let mut out = Vec::new();
    for i in 0..n {
        let v = sec.get_str(&format!("Channel{i}"), "");
        let name: String = v.chars().take_while(|&c| !matches!(c, ',' | '\n' | '\r')).collect();
        let Some(rest) = v[name.len()..].strip_prefix(',') else { continue };
        let formula: String = rest.chars().take_while(|&c| !matches!(c, '\r' | '\n')).collect();
        if !name.is_empty() && !formula.is_empty() {
            out.push((name, formula));
        }
    }
    out
}

/// `0x4153a0`: the section cleared, then `Channel<i>` = `name,formula`.
pub fn write_calculated(sec: &mut SetNode, chans: &[(String, String)]) {
    *sec = SetNode::default();
    for (i, (n, f)) in chans.iter().enumerate() {
        sec.set_str(&format!("Channel{i}"), &format!("{n},{f}"));
    }
}

// ---------------------------------------------------------------- ОМП

/// `WrongPlaceFinding`: the `.omp` element (`0x4a2620` writer, `0x4a3050`
/// reader) stored as tree nodes, the element tags becoming the keys.
pub fn write_omp(sec: &mut SetNode, p: &crate::omp_params::OmpParams) {
    fn conv(n: &Node) -> SetNode {
        SetNode { attrs: n.attrs.clone(), children: n.children.iter().map(|c| (c.tag.clone(), conv(c))).collect() }
    }
    *sec = conv(&p.to_node("WrongPlaceFinding"));
}

/// The `.omp` file of «Сохранить» (32877, `0x4a91c0`): a fresh settings
/// document whose one category `omp` holds what `0x4a2620` writes (as
/// `WrongPlaceFinding`), through the settings writer `0x48e1c0`.
pub fn omp_file_bytes(p: &crate::omp_params::OmpParams) -> Vec<u8> {
    let mut t = SetTree::default();
    write_omp(t.section_mut("omp"), p);
    t.to_file_bytes()
}

/// «Загрузить» (32878, `0x4a9010`): the file read as a settings document
/// (`0x48e470`); with a category, `omp` (got or created) through the
/// version-2 reader `0x4a3050`; with none, the legacy reader `0x4a45f0`
/// (not reproduced: `None`). Nothing is ever reported.
pub fn omp_from_file_bytes(bytes: &[u8]) -> Option<crate::omp_params::OmpParams> {
    let mut t = SetTree::default();
    t.load_bytes(bytes);
    if t.sections.is_empty() {
        return None;
    }
    read_omp(t.section_mut("omp"))
}

/// The ОМП settings of a `WrongPlaceFinding` section (None when it holds no
/// version-2 settings).
pub fn read_omp(sec: &SetNode) -> Option<crate::omp_params::OmpParams> {
    fn conv(tag: &str, s: &SetNode) -> Node {
        Node { tag: tag.into(), attrs: s.attrs.clone(), children: s.children.iter().map(|(k, c)| conv(k, c)).collect() }
    }
    crate::omp_params::OmpParams::from_node(&conv("WrongPlaceFinding", sec)).ok()
}

// ---------------------------------------------------------------- pairs

/// A channel pair of the differential (`BrakeCurr`, `DiffCurr`) or circle
/// (`ActiveResist`, `ReactiveResist`) diagram, by channel names.
#[derive(Debug, Clone, PartialEq)]
pub struct PairRec {
    pub a: String,
    pub b: String,
    pub visible: bool,
}

pub const DIFF_KEYS: (&str, &str) = ("BrakeCurr", "DiffCurr");
pub const CIRCLE_KEYS: (&str, &str) = ("ActiveResist", "ReactiveResist");

/// `0x421150` / `0x420f80`: child `Channels`, items `"%u"` from 0 to the
/// child count − 1 (names default `?`, `visible` 1).
pub fn read_pairs(chart: &SetNode, (ka, kb): (&str, &str)) -> Vec<PairRec> {
    let Some(c) = chart.child("Channels") else { return Vec::new() };
    let empty = SetNode::default();
    (0..c.children.len())
        .map(|u| {
            let e = c.child(&u.to_string()).unwrap_or(&empty);
            PairRec { a: e.get_str(ka, "?"), b: e.get_str(kb, "?"), visible: e.get_bool("visible", true) }
        })
        .collect()
}

/// `0x421090` / `0x420ea0`: only the child `Channels` is cleared and
/// rebuilt.
pub fn write_pairs(chart: &mut SetNode, (ka, kb): (&str, &str), pairs: &[PairRec]) {
    let c = chart.child_mut("Channels");
    *c = SetNode::default();
    for (u, p) in pairs.iter().enumerate() {
        let e = c.child_mut(&u.to_string());
        e.set_bool("visible", p.visible);
        e.set_str(ka, &p.a);
        e.set_str(kb, &p.b);
    }
}

// ---------------------------------------------------------------- views

/// `Circle Chart` (`0x436b30`) / `Diff Chart` (`0x442a10`): SightsOnly
/// (default 1), ShowOffCharacteristics (1), ViewChannelType (circle 0 = none
/// / 1 Re+jIm / 2 |·|∠; differential a boolean, default 1), SelectedBlock.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaneChart {
    pub sights_only: bool,
    pub show_off: bool,
    pub view_type: i32,
    pub selected_block: String,
}

pub fn read_plane_chart(sec: Option<&SetNode>, diff: bool) -> PlaneChart {
    let empty = SetNode::default();
    let s = sec.unwrap_or(&empty);
    let vt = s.get_int("ViewChannelType", i32::from(diff));
    PlaneChart {
        sights_only: s.get_bool("SightsOnly", true),
        show_off: s.get_bool("ShowOffCharacteristics", true),
        view_type: if diff { i32::from(vt != 0) } else { vt },
        selected_block: s.get_str("SelectedBlock", ""),
    }
}

/// The toggles' setters (`0x436f10` / `0x437390` / `0x43b930` and the
/// differential's `0x442cc0` / `0x442ee0` / `0x4461f0`): each writes only
/// its own key, so after a block save has emptied the section (L9.162) the
/// untouched ones stay absent.
pub fn write_plane_chart(sec: &mut SetNode, old: &PlaneChart, c: &PlaneChart) {
    if c.sights_only != old.sights_only {
        sec.set_bool("SightsOnly", c.sights_only);
    }
    if c.show_off != old.show_off {
        sec.set_bool("ShowOffCharacteristics", c.show_off);
    }
    if c.view_type != old.view_type {
        sec.set_int("ViewChannelType", c.view_type);
    }
}

/// One characteristic's saved state: name, visible, (parameter, value text).
pub type SavedChar = (String, bool, Vec<(String, String)>);

/// The block state of a plane, as `0x408cf0` saves it into `Circle Chart` /
/// `Diff Chart`: the section emptied first (its keys and subtrees — so
/// SightsOnly, ShowOffCharacteristics and ViewChannelType go too until
/// their toggles write them again), then `SelectedBlock` = the block's
/// name and, under `Selected Block`, one child per characteristic with
/// `visible` (`%i`) and one child per parameter with `value` (the grid's
/// text). Only the current block is written.
pub fn write_block(sec: &mut SetNode, name: &str, chars: &[SavedChar]) {
    sec.attrs.clear();
    sec.children.clear();
    sec.set_str("SelectedBlock", name);
    let sel = sec.child_mut("Selected Block");
    for (ch, visible, params) in chars {
        let c = sel.child_mut(ch);
        c.set_int("visible", i32::from(*visible));
        for (p, v) in params {
            c.child_mut(p).set_str("value", v);
        }
    }
}

/// `0x409030`: a characteristic's saved state under `Selected Block` —
/// `visible` (default 1) and a parameter's `value` (default: the
/// library's text).
pub fn saved_char<'a>(sec: Option<&'a SetNode>, ch: &str) -> Option<&'a SetNode> {
    sec?.child("Selected Block")?.child(ch)
}

/// `Vector Chart` (`0x48fc80`): flags true only when stored as exactly 1.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VectorChart {
    pub angle_in_radians: bool,
    pub only_direction: bool,
    pub show_angles_value: bool,
    pub angle_channel: String,
}

pub fn read_vector_chart(sec: Option<&SetNode>) -> VectorChart {
    let empty = SetNode::default();
    let s = sec.unwrap_or(&empty);
    VectorChart {
        angle_in_radians: s.get_int("AngleInRadians", 0) == 1,
        only_direction: s.get_int("OnlyDirection", 0) == 1,
        show_angles_value: s.get_int("ShowAnglesValue", 0) == 1,
        angle_channel: s.get_str("AngleChannelName", ""),
    }
}

/// `0x48faa0` / `0x48fb40` / `0x48fbe0` / `0x492940`: each its own key.
pub fn write_vector_chart(sec: &mut SetNode, old: &VectorChart, c: &VectorChart) {
    if c.angle_in_radians != old.angle_in_radians {
        sec.set_bool("AngleInRadians", c.angle_in_radians);
    }
    if c.only_direction != old.only_direction {
        sec.set_bool("OnlyDirection", c.only_direction);
    }
    if c.show_angles_value != old.show_angles_value {
        sec.set_bool("ShowAnglesValue", c.show_angles_value);
    }
    if c.angle_channel != old.angle_channel {
        sec.set_str("AngleChannelName", &c.angle_channel);
    }
}

/// `Frequency Chart` (`0x46f4a0` reader, `0x46ee70` writer).
#[derive(Debug, Clone, PartialEq)]
pub struct FrequencyChart {
    pub num_periods: i32,
    pub num_harmonics: i32,
    /// `ShowPersents`: 0 = values in percent (the default).
    pub show_persents: i32,
    pub use_rms: bool,
}

/// `NumHarmonics` is clamped to ⌊rate / f · ½ · NumPeriods⌋.
pub fn read_frequency_chart(sec: Option<&SetNode>, rate: f64, f_net: f64) -> FrequencyChart {
    let empty = SetNode::default();
    let s = sec.unwrap_or(&empty);
    let num_periods = s.get_int("NumPeriods", 1);
    let cap = if f_net > 0.0 { (rate / f_net * 0.5 * f64::from(num_periods)) as i32 } else { i32::MAX };
    FrequencyChart {
        num_periods,
        num_harmonics: s.get_int("NumHarmonics", 10).min(cap),
        show_persents: s.get_int("ShowPersents", 0),
        use_rms: s.get_int("UseRMS", 0) != 0,
    }
}

pub fn write_frequency_chart(sec: &mut SetNode, c: &FrequencyChart) {
    sec.set_int("NumPeriods", c.num_periods);
    sec.set_int("NumHarmonics", c.num_harmonics);
    sec.set_int("ShowPersents", c.show_persents);
    sec.set_bool("UseRMS", c.use_rms);
}

/// `View` (read at the view's initial update `0x45ab70`; written by the zoom
/// 32800–32803 `0x45c4f0`, ruler / grid `0x45caa0` and 33007 `0x45d290` handlers):
/// XScale (default 2 × fit), RulerState (1 = ms), YAnalogSize (93), Sells
/// (grid, 1).
#[derive(Debug, Clone, PartialEq)]
pub struct ViewRec {
    pub xscale: Option<f64>,
    pub ruler_state: i32,
    pub y_analog: i32,
    pub sells: bool,
}

pub fn read_view(sec: Option<&SetNode>) -> ViewRec {
    let empty = SetNode::default();
    let s = sec.unwrap_or(&empty);
    ViewRec {
        xscale: s.attrs.get("XScale").map(|v| crate::omp_params::wtof(v)),
        ruler_state: s.get_int("RulerState", 1),
        y_analog: s.get_int("YAnalogSize", 93),
        sells: s.get_int("Sells", 1) != 0,
    }
}

pub fn write_view(sec: &mut SetNode, v: &ViewRec) {
    if let Some(x) = v.xscale {
        sec.set_dbl("XScale", x);
    }
    sec.set_int("RulerState", v.ruler_state);
    sec.set_int("YAnalogSize", v.y_analog);
    sec.set_bool("Sells", v.sells);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> SetTree {
        let mut t = SetTree::default();
        write_comments(
            t.section_mut("Comments"),
            &[
                CommentRec { kind: CommentKindRec::Moment { position: 12 }, text: "a\r\nb".into(), locked: true, visible: true },
                CommentRec {
                    kind: CommentKindRec::Level { channel: "Ua".into(), y: 1.5, from: 3, to: 9 },
                    text: String::new(),
                    locked: false,
                    visible: false,
                },
                CommentRec { kind: CommentKindRec::Moment { position: 40 }, text: "m2".into(), locked: false, visible: true },
            ],
        );
        let o = t.section_mut("Oscillogram");
        o.set_int("Codepage", 0);
        o.set_int("DiscretisationFrequensy", 2400);
        write_calculated(t.section_mut("CalculatedChannels"), &[("X".into(), "Ua+Ub".into()), ("Y".into(), "fourie(Ia, 1)".into())]);
        write_channel(
            t.section_mut("Ua"),
            &ChannelRec { show_flags: 0x45, primary_mul: 1100.0, secondary_mul: 1.0, alias: String::new(), custom_color: Some(0x0000_00FF) },
        );
        write_pairs(t.section_mut("Diff Chart"), DIFF_KEYS, &[PairRec { a: "It".into(), b: "Id".into(), visible: true }]);
        t
    }

    #[test]
    fn written_layout_matches_the_original() {
        let x = sample_tree().to_xml();
        let lines: Vec<&str> = x.lines().collect();
        assert_eq!(lines[0], "<?xml version=\"1.0\" encoding=\"utf-16\"?>");
        assert_eq!(lines[1], "<settings>");
        // Sections in name order (ordinal: upper case first).
        assert_eq!(lines[2], "\t<category Channel0=\"X,Ua+Ub\" Channel1=\"Y,fourie(Ia, 1)\" name=\"CalculatedChannels\"/>");
        assert_eq!(lines[3], "\t<category name=\"Comments\">");
        assert_eq!(lines[4], "\t\t<category name=\"Areas\"/>");
        assert_eq!(lines[5], "\t\t<category name=\"Levels\">");
        assert_eq!(
            lines[6],
            "\t\t\t<category channel=\"Ua\" locked=\"0\" name=\"Level1\" positionFrom=\"3\" positionTo=\"9\" positionY=\"1.500000\" text=\"\" visible=\"0\"/>"
        );
        assert_eq!(lines[7], "\t\t</category>");
        assert_eq!(lines[8], "\t\t<category name=\"Moments\">");
        assert_eq!(lines[9], "\t\t\t<category locked=\"1\" name=\"Moment1\" position=\"12\" text=\"a");
        assert_eq!(lines[10], "b\" visible=\"1\"/>");
        assert_eq!(lines[11], "\t\t\t<category locked=\"0\" name=\"Moment2\" position=\"40\" text=\"m2\" visible=\"1\"/>");
        assert_eq!(lines[14], "\t<category name=\"Diff Chart\">");
        assert_eq!(lines[15], "\t\t<category name=\"Channels\">");
        assert_eq!(lines[16], "\t\t\t<category BrakeCurr=\"It\" DiffCurr=\"Id\" name=\"0\" visible=\"1\"/>");
        assert_eq!(lines[19], "\t<category Codepage=\"0\" DiscretisationFrequensy=\"2400\" name=\"Oscillogram\"/>");
        assert_eq!(
            lines[20],
            "\t<category AliasName=\"\" CustomColorSet=\"1\" PrimaryMul=\"1100.000000\" SecondaryMul=\"1.000000\" ShowFlags=\"69\" colorCustom=\"255\" name=\"Ua\"/>"
        );
        assert_eq!(lines[21], "</settings>");
        // On disk: BOM, UTF-16LE, CR LF (also inside the multi-line text).
        let b = sample_tree().to_file_bytes();
        assert_eq!(&b[..4], &[0xFF, 0xFE, b'<', 0]);
        let back = crate::decode_fvb(&b);
        assert!(back.contains("text=\"a\r\nb\""));
    }

    /// `NetFreq` (`0x427bc0` / `0x427af0`): `%g` joined by ","; read back
    /// into the channel's count, old values past the string kept.
    #[test]
    fn net_freq_round_trip() {
        let mut sec = SetNode::default();
        write_net_freq(&mut sec, &[]);
        assert!(!sec.attrs.contains_key("NetFreq"), "nothing for an empty vector");
        write_net_freq(&mut sec, &[50.0, 49.5]);
        assert_eq!(sec.get_str("NetFreq", ""), "50,49.5");
        assert_eq!(read_net_freq(&sec, 3, &[1.0, 2.0, 3.0]), vec![50.0, 49.5, 3.0]);
        assert_eq!(read_net_freq(&sec, 1, &[]), vec![50.0]);
        assert_eq!(read_net_freq(&SetNode::default(), 3, &[7.0]), vec![7.0], "none: unchanged");
    }

    #[test]
    fn round_trip_through_the_file() {
        let t = sample_tree();
        let mut back = SetTree::default();
        back.load_bytes(&t.to_file_bytes());
        // Loaded nodes carry their `name` attribute.
        assert_eq!(back.section("Oscillogram").unwrap().get_str("name", ""), "Oscillogram");
        assert_eq!(back.section("Oscillogram").unwrap().get_int("DiscretisationFrequensy", 0), 2400);
        let cur = ChannelRec { show_flags: 1, primary_mul: 1.0, secondary_mul: 1.0, alias: String::new(), custom_color: None };
        let ch = read_channel(back.section("Ua").unwrap(), &cur);
        assert_eq!((ch.show_flags, ch.primary_mul, ch.custom_color), (0x45, 1100.0, Some(255)));
        assert_eq!(
            read_calculated(back.section("CalculatedChannels").unwrap()),
            vec![("X".to_string(), "Ua+Ub".to_string()), ("Y".to_string(), "fourie(Ia, 1)".to_string())]
        );
        assert_eq!(read_pairs(back.section("Diff Chart").unwrap(), DIFF_KEYS), vec![PairRec { a: "It".into(), b: "Id".into(), visible: true }]);
        // Items come back per kind in descending order, the CR LF the text
        // was written with kept.
        let c = read_comments(back.section("Comments").unwrap());
        assert_eq!(c.len(), 3);
        assert!(matches!(c[0].kind, CommentKindRec::Level { ref channel, y, from: 3, to: 9 } if channel == "Ua" && y == 1.5));
        assert_eq!(c[1].kind, CommentKindRec::Moment { position: 40 });
        assert_eq!(c[2].kind, CommentKindRec::Moment { position: 12 });
        assert_eq!(c[2].text, "a\r\nb");
        assert!(c[2].locked && c[2].visible && !c[0].visible);
        // The comment writer normalises the CR LF again; the loaded `name`
        // attributes coincide with the keys, so the document is unchanged
        // (the items of each kind in reverse order).
        let mut again = c.clone();
        again.reverse();
        write_comments(back.section_mut("Comments"), &again);
        assert_eq!(back.to_xml(), t.to_xml());
    }

    #[test]
    fn reads_a_hand_made_file_in_the_original_layout() {
        let xml = "<?xml version=\"1.0\" encoding=\"utf-16\"?>\r\n<settings>\r\n\
            \t<category name=\"Comments\">\r\n\
            \t\t<category name=\"Areas\">\r\n\
            \t\t\t<category name=\"Area1\" channelFrom=\"Ia\" channelTo=\"Ic\" positionFrom=\"5\" positionTo=\"50\"/>\r\n\
            \t\t</category>\r\n\
            \t\t<category name=\"Moments\">\r\n\
            \t\t\t<category name=\"Moment1\" position=\"7\" text=\"x &amp;lt; y\"/>\r\n\
            \t\t</category>\r\n\
            \t</category>\r\n\
            \t<category name=\"Vector Chart\" AngleInRadians=\"2\" OnlyDirection=\"1\" AngleChannelName=\"Ua\"/>\r\n\
            \t<category name=\"Frequency Chart\" NumPeriods=\"2\" NumHarmonics=\"40\"/>\r\n\
            \t<category name=\"Circle Chart\"/>\r\n\
            \t<category name=\"View\" XScale=\"0.250000\"/>\r\n\
            \t<category name=\"CalculatedChannels\" Channel0=\"A,Ua\" Channel1=\"bad\" Channel2=\"C,Ub\"/>\r\n\
            </settings>\r\n";
        let mut t = SetTree::default();
        t.load_bytes(&settree::utf16le_crlf(xml));
        let c = read_comments(t.section("Comments").unwrap());
        assert_eq!(
            c[0],
            CommentRec {
                kind: CommentKindRec::Area { channel_from: "Ia".into(), channel_to: "Ic".into(), from: 5, to: 50 },
                text: String::new(),
                locked: false,
                visible: true
            }
        );
        // Unescaped `&amp;` first, then `&lt;`.
        assert_eq!(c[1].text, "x < y");
        let v = read_vector_chart(t.section("Vector Chart"));
        assert_eq!(v, VectorChart { angle_in_radians: false, only_direction: true, show_angles_value: false, angle_channel: "Ua".into() });
        // 1200 Hz / 50 Hz × ½ × 2 periods = 24 harmonics at most.
        let f = read_frequency_chart(t.section("Frequency Chart"), 1200.0, 50.0);
        assert_eq!((f.num_periods, f.num_harmonics, f.show_persents, f.use_rms), (2, 24, 0, false));
        let p = read_plane_chart(t.section("Circle Chart"), false);
        assert_eq!(p, PlaneChart { sights_only: true, show_off: true, view_type: 0, selected_block: String::new() });
        assert!(read_plane_chart(None, true).view_type == 1);
        assert_eq!(read_view(t.section("View")), ViewRec { xscale: Some(0.25), ruler_state: 1, y_analog: 93, sells: true });
        // Three Channel attributes + name: i = 0, 1, 2; "bad" has no comma.
        assert_eq!(read_calculated(t.section("CalculatedChannels").unwrap()), vec![("A".into(), "Ua".into()), ("C".into(), "Ub".into())]);
    }

    #[test]
    fn broken_or_missing_files_load_nothing_and_ini_falls_back() {
        let mut t = SetTree::default();
        t.section_mut("Keep").set_int("a", 1);
        // A syntax error is swallowed: nothing loaded, the tree as it was.
        t.load_bytes(&settree::utf16le_crlf("<settings><category name=\"X\" a=1/></settings>"));
        assert!(t.section("Keep").is_some() && t.section("X").is_none());
        // A legacy ini (single-byte) fails the XML reading and is merged.
        t.load_bytes(b"[Oscillogram]\r\nDiscretisationFrequensy=1000\r\nnokey\r\n=v\r\n");
        assert_eq!(t.section("Oscillogram").unwrap().get_int("DiscretisationFrequensy", 0), 1000);
        assert!(t.section("Keep").is_some());
        // A non-empty XML root replaces the whole tree.
        t.load_bytes(&settree::utf16le_crlf("<settings><category name=\"Y\"/></settings>"));
        assert!(t.section("Keep").is_none() && t.section("Y").is_some());
    }

    #[test]
    fn set_path_follows_0x44c710() {
        use std::path::Path;
        assert_eq!(set_path(Path::new("/d/rec.cfg")), Path::new("/d/rec.set"));
        assert_eq!(set_path(Path::new("/d/rec.OSC")), Path::new("/d/rec.set"));
        assert_eq!(set_path(Path::new("/d/rec.cff")), Path::new("/d/rec.set"));
        assert_eq!(set_path(Path::new("/d/rec.dat")), Path::new("/d/rec.dat.set"));
    }

    #[test]
    fn omp_settings_embed_as_wrong_place_finding() {
        let mut p = crate::omp_params::OmpParams { method: crate::omp_params::GROUNDED, ..Default::default() };
        p.stations[0].sections.push(crate::omp_params::SectionParams { r1: "0.1".into(), ..Default::default() });
        let mut t = SetTree::default();
        write_omp(t.section_mut("WrongPlaceFinding"), &p);
        let x = t.to_xml();
        assert!(x.contains("\t<category name=\"WrongPlaceFinding\" type=\"2\" ver=\"2\">"), "{x}");
        assert!(x.contains("\t\t\t<category comment=\"\" lenght=\"1\" name=\"Section1\""), "{x}");
        let mut back = SetTree::default();
        back.load_bytes(&t.to_file_bytes());
        assert_eq!(read_omp(back.section("WrongPlaceFinding").unwrap()), Some(p));
        assert_eq!(read_omp(&SetNode::default()), None);
    }

    #[test]
    fn wtoi_is_lenient() {
        assert_eq!(wtoi(" 42x"), 42);
        assert_eq!(wtoi("-7"), -7);
        assert_eq!(wtoi("x"), 0);
        assert_eq!(wtoi("99999999999"), i32::MAX);
    }

    /// `0x48e370`: categories sharing a name merge (later attributes win,
    /// the rest and the children kept) — at the top level and nested.
    #[test]
    fn same_named_categories_merge() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n<settings>\r\n\
            \t<category name=\"View\" A=\"1\" B=\"2\"><category name=\"K\" x=\"1\"/></category>\r\n\
            \t<category name=\"View\" A=\"3\"><category name=\"K\" y=\"2\"/></category>\r\n\
            </settings>\r\n";
        let mut t = SetTree::default();
        t.load_bytes(&settree::utf16le_crlf(xml));
        let v = t.section("View").unwrap();
        assert_eq!((v.get_str("A", ""), v.get_str("B", "")), ("3".to_string(), "2".to_string()));
        let k = v.child("K").unwrap();
        assert_eq!((k.get_str("x", ""), k.get_str("y", "")), ("1".to_string(), "2".to_string()));
    }
}

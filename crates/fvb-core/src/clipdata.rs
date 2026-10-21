//! «Mechanotronica FastView Clipboard Data» — the system-clipboard document
//! of 57634 Копировать / 57637 Вставить (`0x430af0` + `0x427f20` write,
//! `0x45ad02` + `0x424cb0` read; ledger L9.20).
//!
//! The text is the settings-tree XML ([`crate::settree::Node`]) without a
//! declaration or BOM, CR LF line ends and one TAB per level: a `channels`
//! root with the record's time base and header, one `channel` child per
//! copied channel. Numbers keep the original's printf formats, quirks
//! included: `num_values` is `%g` (10⁶ samples print `1e+06`, read back by
//! `_wtoi` as 1), `times` are µs printed `%i` (only the low 32 bits), the
//! samples `%f,` with a trailing comma.

use crate::settree::Node;
use crate::views::fmt_g;

/// The record part of the clipboard document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipRecord {
    /// `discr_frequency` (`doc+0x228`, written `%i`, read `_wtof`).
    pub rate: f64,
    /// `time_multiplier` (`doc+0x230`).
    pub time_multiplier: f64,
    /// `times`: the record's sample times in µs.
    pub times_us: Vec<i64>,
    /// `net_frequency`: the nominal frequency per sample (`doc+0x1fc`).
    pub net_frequency: Vec<f64>,
    /// `num_values` as written; [`ClipRecord::parse`] reads it with `_wtoi`.
    pub num_values: String,
    pub station_id: String,
    pub block_id: String,
    pub title: String,
    pub hdr_file: String,
    /// `startTime` = start date + ", " + start time.
    pub start_time: String,
    pub channels: Vec<ClipChannel>,
    /// The root tag was `channels` (otherwise the paste adds no channels
    /// but still rewrites the header and shows the bar).
    pub valid: bool,
}

/// One copied channel (`0x427f20`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipChannel {
    pub name: String,
    pub dimension: String,
    pub discrete: bool,
    pub desc2: String,
    pub desc3: String,
    /// `values_type` = `primary` (else `secondary`).
    pub primary_values: bool,
    /// `primary_mode` (`ch+0x89`).
    pub primary_mode: bool,
    pub mul: f64,
    pub offset: f64,
    pub time_shift: f64,
    pub primary_value: f64,
    pub secondary_value: f64,
    pub file_primary_value: f64,
    pub file_secondary_value: f64,
    pub normal_state: i32,
    /// `color_custom` when `custom_color_set` is true (a COLORREF, 0x00BBGGRR).
    pub color: Option<u32>,
    pub record_start_date: String,
    pub record_start_time: String,
    pub duration_ms: String,
    pub data: Vec<f64>,
}

fn join<T>(v: &[T], f: impl Fn(&T) -> String) -> String {
    v.iter().map(f).collect::<Vec<_>>().join(",")
}

fn flag(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

/// `_wtoi` / `_wtoi64`: the leading signed integer, 0 if none.
fn wtoi(s: &str) -> i64 {
    let s = s.trim_start();
    let (sign, rest) = match s.as_bytes().first() {
        Some(b'-') => (-1, &s[1..]),
        Some(b'+') => (1, &s[1..]),
        _ => (1, s),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    sign * digits.parse::<i64>().unwrap_or(0)
}

/// `_wtof`: the longest leading decimal number (with an exponent), 0 if none.
fn wtof(s: &str) -> f64 {
    let s = s.trim_start();
    (1..=s.len()).rev().filter(|&e| s.is_char_boundary(e)).find_map(|e| s[..e].parse::<f64>().ok().filter(|v| v.is_finite())).unwrap_or(0.0)
}

impl ClipRecord {
    /// The clipboard text (`0x4aabc0`): no declaration, CR LF, TAB indent,
    /// attributes in name order.
    pub fn to_text(&self) -> String {
        let mut root = Node::new("channels");
        root.set("num_values", &self.num_values);
        root.set("time_multiplier", &fmt_g(self.time_multiplier));
        root.set("discr_frequency", &format!("{}", self.rate as i32));
        root.set("times", &join(&self.times_us, |&t| format!("{}", t as i32)));
        root.set("net_frequency", &join(&self.net_frequency, |&f| fmt_g(f)));
        root.set("stationID", &self.station_id);
        root.set("blockID", &self.block_id);
        root.set("title", &self.title);
        root.set("hdrFile", &self.hdr_file);
        root.set("startTime", &self.start_time);
        for c in &self.channels {
            let mut n = Node::new("channel");
            n.set("name", &c.name);
            n.set("dimension", &c.dimension);
            n.set("type", if c.discrete { "discret" } else { "analog" });
            n.set("desc2", &c.desc2);
            n.set("desc3", &c.desc3);
            n.set("values_type", if c.primary_values { "primary" } else { "secondary" });
            n.set("primary_mode", flag(c.primary_mode));
            for (k, v) in [
                ("mul", c.mul),
                ("offset", c.offset),
                ("timeShift", c.time_shift),
                ("primary_value", c.primary_value),
                ("secondary_value", c.secondary_value),
                ("file_primary_value", c.file_primary_value),
                ("file_secondary_value", c.file_secondary_value),
            ] {
                n.set(k, &fmt_g(v));
            }
            n.set("normal_state", &c.normal_state.to_string());
            n.set("values_count", &c.data.len().to_string());
            n.set("custom_color_set", flag(c.color.is_some()));
            n.set("color_custom", &c.color.unwrap_or(0).to_string());
            n.set("record_start_date", &c.record_start_date);
            n.set("record_start_time", &c.record_start_time);
            n.set("duration_milli_sec", &c.duration_ms);
            n.set("data", &c.data.iter().map(|v| format!("{v:.6},")).collect::<String>());
            root.children.push(n);
        }
        let mut out = String::new();
        root.write("", &mut out);
        out.replace('\n', "\r\n")
    }

    /// Read a clipboard document as the paste does: `num_values` by `_wtoi`
    /// bounds the `times` (`_wtoi64`) and `net_frequency` (`_wtof`) arrays;
    /// channels only under a `channels` root.
    pub fn parse(text: &str) -> Option<ClipRecord> {
        let root = Node::parse(text).ok()?;
        let n = wtoi(&root.get("num_values", "")).max(0) as usize;
        let times_attr = root.get("times", "");
        let freq_attr = root.get("net_frequency", "");
        let mut rec = ClipRecord {
            rate: wtof(&root.get("discr_frequency", "")),
            time_multiplier: wtof(&root.get("time_multiplier", "")),
            times_us: times_attr.split(',').take(n).map(wtoi).collect(),
            net_frequency: freq_attr.split(',').take(n).map(wtof).collect(),
            num_values: root.get("num_values", ""),
            station_id: root.get("stationID", ""),
            block_id: root.get("blockID", ""),
            title: root.get("title", ""),
            hdr_file: root.get("hdrFile", ""),
            start_time: root.get("startTime", ""),
            channels: Vec::new(),
            valid: root.tag == "channels",
        };
        if !rec.valid {
            return Some(rec);
        }
        for c in root.children.iter().filter(|c| c.tag == "channel") {
            let g = |k: &str| c.get(k, "");
            let count = wtoi(&g("values_count")).max(0) as usize;
            let data_attr = g("data");
            let mut data: Vec<f64> = data_attr.split(',').map(wtof).collect();
            if data_attr.ends_with(',') {
                data.pop();
            }
            if c.attrs.contains_key("values_count") {
                data.truncate(count);
            }
            rec.channels.push(ClipChannel {
                name: g("name"),
                dimension: g("dimension"),
                discrete: g("type") == "discret",
                desc2: g("desc2"),
                desc3: g("desc3"),
                primary_values: g("values_type") == "primary",
                primary_mode: g("primary_mode") == "true",
                mul: wtof(&g("mul")),
                offset: wtof(&g("offset")),
                time_shift: wtof(&g("timeShift")),
                primary_value: wtof(&g("primary_value")),
                secondary_value: wtof(&g("secondary_value")),
                file_primary_value: wtof(&g("file_primary_value")),
                file_secondary_value: wtof(&g("file_secondary_value")),
                normal_state: wtoi(&g("normal_state")) as i32,
                color: (g("custom_color_set") == "true").then(|| wtoi(&g("color_custom")) as u32),
                record_start_date: g("record_start_date"),
                record_start_time: g("record_start_time"),
                duration_ms: g("duration_milli_sec"),
                data,
            });
        }
        Some(rec)
    }
}

/// The record comment a paste writes (`0x45ad02`, strings 3272, 3221, 3048,
/// 3218, 3479, 3308, 3297, 3249, 3050): the document's own record as
/// «Осциллограмма 1», the clipboard's as «Осциллограмма 2». The CR LF
/// placement after each value is inferred from the concatenation count.
pub struct HeaderSide<'a> {
    pub title: &'a str,
    pub station: &'a str,
    pub block: &'a str,
    pub start: &'a str,
    pub comment: &'a str,
}

pub fn merged_header(own: &HeaderSide, clip: &HeaderSide) -> String {
    let side = |s: &HeaderSide, head: &str| {
        format!(
            "{head}Имя осциллограммы: {}\r\nСтанция:{}\r\nНомер регистратора: {}\r\nНачало записи: {}\r\nКомментарий: \r\n{}",
            s.title, s.station, s.block, s.start, s.comment
        )
    };
    format!(
        "Механотроника. FastView, Объединенная осциллограмма\r\nИсходные осциллограммы: \r\n{}\r\n\r\n{}",
        side(own, "1) Осциллограмма 1\r\n"),
        side(clip, "2) Осциллограмма 2\r\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ClipRecord {
        ClipRecord {
            rate: 1000.0,
            time_multiplier: 1.0,
            times_us: vec![0, 1000, 2000],
            net_frequency: vec![50.0; 3],
            num_values: "3".into(),
            station_id: "ПС <1>".into(),
            block_id: "7".into(),
            title: "rec".into(),
            hdr_file: String::new(),
            start_time: "01/02/2020, 10:00:00.000000".into(),
            channels: vec![ClipChannel { name: "Ia".into(), dimension: "A".into(), mul: 0.5, data: vec![1.0, -2.25, 1e-7], ..Default::default() }],
            valid: true,
        }
    }

    #[test]
    fn layout_and_formats() {
        let t = sample().to_text();
        assert!(t.starts_with("<channels blockID=\"7\" discr_frequency=\"1000\" hdrFile=\"\" net_frequency=\"50,50,50\" num_values=\"3\""));
        assert!(t.contains("stationID=\"ПС &lt;1&gt;\""));
        assert!(t.contains("\r\n\t<channel color_custom=\"0\" custom_color_set=\"false\" data=\"1.000000,-2.250000,0.000000,\""));
        assert!(t.contains(" mul=\"0.5\" "));
        assert!(t.ends_with("</channels>\r\n"));
        assert!(!t.contains("<?xml"));
    }

    #[test]
    fn round_trip_with_the_original_quirks() {
        let back = ClipRecord::parse(&sample().to_text()).unwrap();
        assert_eq!(back.channels[0].data, [1.0, -2.25, 0.0], "%f drops values below 5e-7");
        assert_eq!(back.times_us, [0, 1000, 2000]);
        assert_eq!(back.station_id, "ПС <1>");
        // 10⁶ samples: num_values "1e+06" bounds the arrays to one entry.
        let mut big = sample();
        big.num_values = fmt_g(1e6);
        assert_eq!(big.num_values, "1e+06");
        assert_eq!(ClipRecord::parse(&big.to_text()).unwrap().times_us, [0]);
        // Times above 2³¹ µs keep only the low 32 bits.
        big.times_us = vec![(1i64 << 31) + 5];
        assert!(big.to_text().contains("times=\"-2147483643\""));
        // Another root tag: no channels.
        let other = ClipRecord::parse("<other num_values=\"1\"><channel name=\"x\"/></other>").unwrap();
        assert!(!other.valid && other.channels.is_empty());
    }

    #[test]
    fn merged_header_text() {
        let a = HeaderSide { title: "A", station: "S1", block: "1", start: "d, t", comment: "c1" };
        let b = HeaderSide { title: "B", station: "S2", block: "2", start: "d2, t2", comment: "" };
        let h = merged_header(&a, &b);
        assert!(h.starts_with("Механотроника. FastView, Объединенная осциллограмма\r\nИсходные осциллограммы: \r\n1) Осциллограмма 1\r\nИмя осциллограммы: A\r\nСтанция:S1\r\n"));
        assert!(h.contains("Комментарий: \r\nc1\r\n\r\n2) Осциллограмма 2\r\n"));
        assert!(h.ends_with("Комментарий: \r\n"));
    }
}

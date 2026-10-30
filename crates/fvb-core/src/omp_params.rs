//! ОМП parameters as the original keeps them (`WrongPlaceFinding`, DIALOG 336
//! «Параметры ОМП»): every value is the raw string typed into the property
//! grid and is read with `_wtof` only when the analysis runs. Also the `.omp`
//! settings file (`0x4a2620` / `0x4a2740` writer, `0x4a3050` / `0x4a38c0`
//! reader, serializer `0x48e1c0` / `0x4aa750`), the OK validation of the
//! dialog and the `.txt` report (`0x49dbf0`). See `docs/original/specs/ui-behaviour.md`
//! §8.

use crate::omp::{OmpResult, ParallelLine, Section, Tap};
use num_complex::Complex64;
use crate::settree::Node;

/// Method code `obj+0x64`: 1 isolated neutral, 2 grounded neutral, 4
/// double-ended. A new document starts with 1.
pub const ISOLATED: u32 = 1;
pub const GROUNDED: u32 = 2;
pub const DOUBLE_ENDED: u32 = 4;

/// A parallel line under a section (methods 2/4).
#[derive(Debug, Clone, PartialEq)]
pub struct ParLineParams {
    pub rm: String,
    pub xm: String,
    /// Name of the channel whose phasor is the line's current.
    pub i0: String,
    pub comment: String,
}

impl Default for ParLineParams {
    fn default() -> Self {
        ParLineParams { rm: "0".into(), xm: "0".into(), i0: String::new(), comment: String::new() }
    }
}

/// A tap (отпайка) under a section (methods 2/4); R0, X0 in ohms.
#[derive(Debug, Clone, PartialEq)]
pub struct TapParams {
    pub r0: String,
    pub x0: String,
    pub comment: String,
}

impl Default for TapParams {
    fn default() -> Self {
        TapParams { r0: "0".into(), x0: "0".into(), comment: String::new() }
    }
}

/// A line section (участок).
#[derive(Debug, Clone, PartialEq)]
pub struct SectionParams {
    pub length: String,
    pub supports: String,
    pub r: String,
    pub x: String,
    pub r1: String,
    pub x1: String,
    pub r0: String,
    pub x0: String,
    pub comment: String,
    pub lines: Vec<ParLineParams>,
    pub taps: Vec<TapParams>,
}

impl Default for SectionParams {
    /// A new section is 1 km with 1 support, impedances "0".
    fn default() -> Self {
        let z = || "0".to_string();
        SectionParams {
            length: "1".into(),
            supports: "1".into(),
            r: z(),
            x: z(),
            r1: z(),
            x1: z(),
            r0: z(),
            x0: z(),
            comment: String::new(),
            lines: Vec::new(),
            taps: Vec::new(),
        }
    }
}

/// A substation: name, nominal positive-sequence current and the channel
/// names; station 1 also owns the sections.
#[derive(Debug, Clone, PartialEq)]
pub struct StationParams {
    pub name: String,
    pub inom: String,
    pub ia: String,
    pub ib: String,
    pub ic: String,
    pub uab: String,
    pub ubc: String,
    pub uca: String,
    pub ua: String,
    pub ub: String,
    pub uc: String,
    pub sections: Vec<SectionParams>,
}

impl Default for StationParams {
    fn default() -> Self {
        StationParams {
            name: "Подстанция".into(),
            inom: "0".into(),
            ia: String::new(),
            ib: String::new(),
            ic: String::new(),
            uab: String::new(),
            ubc: String::new(),
            uca: String::new(),
            ua: String::new(),
            ub: String::new(),
            uc: String::new(),
            sections: Vec::new(),
        }
    }
}

/// The ОМП settings of one document.
#[derive(Debug, Clone, PartialEq)]
pub struct OmpParams {
    pub method: u32,
    pub stations: [StationParams; 2],
}

impl Default for OmpParams {
    fn default() -> Self {
        OmpParams { method: ISOLATED, stations: Default::default() }
    }
}

/// `_wtof`: leading white space, an optional sign, digits with at most one
/// `.`, an optional exponent; the longest such prefix, else 0.
pub fn wtof(s: &str) -> f64 {
    let b = s.trim_start().as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits = |i: &mut usize| {
        let s = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - s
    };
    let mut n = digits(&mut i);
    if i < b.len() && b[i] == b'.' {
        i += 1;
        n += digits(&mut i);
    }
    if n == 0 {
        return 0.0;
    }
    let mantissa_end = i;
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if digits(&mut j) > 0 {
            i = j;
        }
    }
    let text = std::str::from_utf8(&b[..i]).unwrap_or("0");
    text.parse::<f64>()
        .or_else(|_| std::str::from_utf8(&b[..mantissa_end]).unwrap_or("0").trim_end_matches('.').parse())
        .unwrap_or(0.0)
}

/// The validation messages of the dialog's OK (string table ids).
pub fn message(id: u32) -> &'static str {
    match id {
        349 => "Данные для определения места повреждения не введены.",
        354 => "Имя канала задано неправильно.",
        3068 => "R и X удельные не могут одновременно быть равны нулю",
        3071 => "R0 и X0 не могут одновременно быть равны нулю",
        3074 => "R1 и X1 не могут одновременно быть равны нулю",
        3076 => "Rm и Xm не могут одновременно быть равны нулю",
        3187 => "Длина участка не может быть равна нулю",
        3535 => "Число опор не может быть равна нулю",
        _ => "",
    }
}

impl OmpParams {
    /// The dialog's OK check over station 1's sections in order; the first
    /// failure's message id. Inom and the channel names are not checked here.
    pub fn validate(&self) -> Result<(), u32> {
        let zero = |a: &str, b: &str| wtof(a) == 0.0 && wtof(b) == 0.0;
        for s in &self.stations[0].sections {
            if wtof(&s.length) == 0.0 {
                return Err(3187);
            }
            if wtof(&s.supports) == 0.0 {
                return Err(3535);
            }
            if self.method == ISOLATED {
                if zero(&s.r, &s.x) {
                    return Err(3068);
                }
                continue;
            }
            if zero(&s.r1, &s.x1) {
                return Err(3074);
            }
            if zero(&s.r0, &s.x0) {
                return Err(3071);
            }
            if s.lines.iter().any(|l| zero(&l.rm, &l.xm)) {
                return Err(3076);
            }
            if s.taps.iter().any(|t| zero(&t.r0, &t.x0)) {
                return Err(3071);
            }
        }
        Ok(())
    }

    /// Engine sections from station 1 (`_wtof` of every value). A parallel
    /// line's current is the phasor series of the channel its «I0» names,
    /// supplied by `current` (None → the run fails with message 354); lines
    /// are only read by the methods that show them (2, 4).
    pub fn sections(&self, mut current: impl FnMut(&str) -> Option<Vec<Complex64>>) -> Result<Vec<Section>, u32> {
        let mut out = Vec::new();
        for s in &self.stations[0].sections {
            let mut parallel = Vec::new();
            if self.method != ISOLATED {
                for l in &s.lines {
                    let cur = current(&l.i0).ok_or(354u32)?;
                    parallel.push(ParallelLine { rm_per_km: wtof(&l.rm), xm_per_km: wtof(&l.xm), current: cur });
                }
            }
            out.push(Section {
                r_per_km: wtof(&s.r),
                x_per_km: wtof(&s.x),
                length_km: wtof(&s.length),
                r0_per_km: wtof(&s.r0),
                x0_per_km: wtof(&s.x0),
                r1_per_km: wtof(&s.r1),
                x1_per_km: wtof(&s.x1),
                supports: wtof(&s.supports).max(0.0) as u32,
                parallel,
                taps: s.taps.iter().map(|t| Tap { r0: wtof(&t.r0), x0: wtof(&t.x0) }).collect(),
            });
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------- .omp file

fn station_node(tag: &str, st: &StationParams) -> Node {
    let mut n = Node::new(tag);
    n.set("obj_name", &st.name);
    n.set("In", &st.inom);
    for (k, v) in [
        ("Ia", &st.ia),
        ("Ib", &st.ib),
        ("Ic", &st.ic),
        ("Uab", &st.uab),
        ("Ubc", &st.ubc),
        ("Uca", &st.uca),
        ("Ua", &st.ua),
        ("Ub", &st.ub),
        ("Uc", &st.uc),
    ] {
        n.set(k, v);
    }
    for (k, s) in st.sections.iter().enumerate() {
        let mut e = Node::new(&format!("Section{}", k + 1));
        for (a, v) in [
            ("lenght", &s.length),
            ("sup_number", &s.supports),
            ("r", &s.r),
            ("x", &s.x),
            ("r1", &s.r1),
            ("x1", &s.x1),
            ("r0", &s.r0),
            ("x0", &s.x0),
            ("comment", &s.comment),
        ] {
            e.set(a, v);
        }
        // Children numbered from 1 within the section, lines first.
        let mut n_child = 0;
        for l in &s.lines {
            n_child += 1;
            let mut c = Node::new(&format!("child{n_child}"));
            c.set("type", "parLine");
            c.set("rm", &l.rm);
            c.set("xm", &l.xm);
            c.set("I0", &l.i0);
            c.set("comment", &l.comment);
            e.children.push(c);
        }
        for t in &s.taps {
            n_child += 1;
            let mut c = Node::new(&format!("child{n_child}"));
            c.set("type", "connection");
            c.set("r0", &t.r0);
            c.set("x0", &t.x0);
            c.set("comment", &t.comment);
            e.children.push(c);
        }
        n.children.push(e);
    }
    n
}

/// The legacy, non-XML `.omp` (`0x4a45f0`, the dialog's fallback when the
/// XML parse fails, `0x4a8f10`): the tokenizer's fields (ending at tab,
/// LF, CR or comma, leading blanks skipped, `0x477050`) — an integer (not
/// used), station 1's Ia, Ib, Ic, Uab, Ubc, Uca and its nominal current
/// (string 3309), a section count N, then N sections of length, R, X per
/// km, supports and comment. The method becomes 1, station 1's sections
/// are replaced, station 2 is left as it was. A field past the end reads
/// empty (the scan finds nothing; an integer 0), and an empty field's
/// leading-blank skip runs on into the next line, as the tokenizer does.
/// None never comes back today (the reader's CException path, `0x4a4c9c`,
/// is the text conversion's).
pub fn apply_legacy(text: &str, cur: &OmpParams) -> Option<OmpParams> {
    let mut rest = text;
    let mut field = || -> Option<String> {
        let t = rest.trim_start_matches(['\t', '\n', '\r', ' ']);
        let end = t.find(['\t', '\n', '\r', ',']).unwrap_or(t.len());
        let v = t[..end].to_string();
        rest = t.get(end + 1..).unwrap_or("");
        Some(v)
    };
    field()?;
    let mut p = cur.clone();
    p.method = ISOLATED;
    let st = &mut p.stations[0];
    for slot in [&mut st.ia, &mut st.ib, &mut st.ic, &mut st.uab, &mut st.ubc, &mut st.uca, &mut st.inom] {
        *slot = field()?;
    }
    let n = crate::comtrade::read_int(&field()?).max(0);
    let mut sections = Vec::new();
    for _ in 0..n {
        let (length, r, x, supports, comment) = (field()?, field()?, field()?, field()?, field()?);
        sections.push(SectionParams { length, r, x, supports, comment, ..Default::default() });
    }
    st.sections = sections;
    Some(p)
}

fn read_station(n: &Node) -> StationParams {
    let g = |k: &str| n.get(k, "");
    let mut st = StationParams {
        name: g("obj_name"),
        inom: n.get("In", "0"),
        ia: g("Ia"),
        ib: g("Ib"),
        ic: g("Ic"),
        uab: g("Uab"),
        ubc: g("Ubc"),
        uca: g("Uca"),
        ua: g("Ua"),
        ub: g("Ub"),
        uc: g("Uc"),
        sections: Vec::new(),
    };
    // Section{k} for k = 1 ..= the number of children (`0x4a3d0f`); a
    // missing one reads as an empty element — a section of "" values, and
    // a missing child{j} a tap of "0" / "0" / "" (`0x4a38c0`).
    let empty = Node::new("");
    for k in 1..=n.children.len() {
        let e = n.child(&format!("Section{k}")).unwrap_or(&empty);
        let g = |a: &str| e.get(a, "");
        let mut s = SectionParams {
            length: g("lenght"),
            supports: g("sup_number"),
            r: g("r"),
            x: g("x"),
            r1: g("r1"),
            x1: g("x1"),
            r0: g("r0"),
            x0: g("x0"),
            comment: g("comment"),
            lines: Vec::new(),
            taps: Vec::new(),
        };
        for j in 1..=e.children.len() {
            let c = e.child(&format!("child{j}")).unwrap_or(&empty);
            if c.get("type", "") == "parLine" {
                s.lines.push(ParLineParams {
                    rm: c.get("rm", "0"),
                    xm: c.get("xm", "0"),
                    i0: c.get("I0", "0"),
                    comment: c.get("comment", ""),
                });
            } else {
                s.taps.push(TapParams { r0: c.get("r0", "0"), x0: c.get("x0", "0"), comment: c.get("comment", "") });
            }
        }
        st.sections.push(s);
    }
    st
}

impl OmpParams {
    /// The settings as one tree element `<tag type ver="2">` with both
    /// stations (`0x4a2620`): the `omp` category of an `.omp` file
    /// (`recset::omp_file_bytes`) or `WrongPlaceFinding` in the record's
    /// settings file — the same writer serves both.
    pub fn to_node(&self, tag: &str) -> Node {
        let mut root = Node::new(tag);
        root.set("ver", "2");
        root.set("type", &self.method.to_string());
        root.children.push(station_node("Station1", &self.stations[0]));
        root.children.push(station_node("Station2", &self.stations[1]));
        root
    }

    /// Read an element written by [`OmpParams::to_node`] (version 2 only;
    /// the legacy layouts are not reproduced).
    pub fn from_node(root: &Node) -> Result<OmpParams, String> {
        if root.get("ver", "") != "2" {
            return Err("unsupported .omp version".into());
        }
        let mut p = OmpParams { method: wtof(&root.get("type", "1")) as u32, ..Default::default() };
        if let Some(s) = root.child("Station1") {
            p.stations[0] = read_station(s);
        }
        if let Some(s) = root.child("Station2") {
            p.stations[1] = read_station(s);
        }
        Ok(p)
    }
}

/// A wide text file as the original writes it (see [`crate::settree::utf16le_crlf`]).
pub use crate::settree::utf16le_crlf;

// ---------------------------------------------------------------- report

/// Record facts the report header names.
#[derive(Debug, Clone, Default)]
pub struct ReportInfo {
    /// Document title (`doc+0x24`).
    pub title: String,
    pub station: String,
    pub recorder: String,
    pub start: String,
    pub trigger: String,
}

/// The `.txt` report (`0x49dbf0`), `\n` line ends. `ktr` gives the found
/// channel's own name and primary / secondary coefficients when the
/// document has a channel of that name; station 1's channel lines then read `%-10s Ктр = %g/%g`
/// (`0x49db00`), station 2's are the bare names.
pub fn report(p: &OmpParams, r: &OmpResult, info: &ReportInfo, ktr: impl Fn(&str) -> Option<(String, f64, f64)>) -> String {
    let mut s = format!("Механотроника FastView\n\nОсциллограмма: {}\n\n", info.title);
    s.push_str(match p.method {
        ISOLATED => "Алгоритм: ОМП для сетей с изолированной нейтралью\n\n",
        GROUNDED => "Алгоритм: ОМП для сетей с заземленной нейтралью\n\n",
        DOUBLE_ENDED => "Алгоритм: ОМП двухстороннего замера\n\n",
        _ => "",
    });
    s.push_str(&format!("Станция: {}\nРегистратор: {}\n\n", info.station, info.recorder));
    s.push_str(&format!("Начало записи:  {}\n", info.start));
    s.push_str(&format!("Момент запуска: {}\n\n", info.trigger));
    s.push_str(&result_line(p, r));
    let st1 = &p.stations[0];
    let chan = |name: &str| match ktr(name) {
        Some((found, a, b)) => format!("{found:<10} Ктр = {}/{}", msvc_g(a), msvc_g(b)),
        None => name.to_string(),
    };
    let station_head = |s: &mut String, title: &str, st: &StationParams| {
        s.push_str(&format!("{title}{}\n", st.name));
        s.push_str(&format!("Номинальный ток прямой последовательности, A: {}\n\n", st.inom));
    };
    let phase_lines = |s: &mut String, st: &StationParams, f: &dyn Fn(&str) -> String| {
        s.push_str(&format!("Канал Ia:  {}\n", f(&st.ia)));
        s.push_str(&format!("Канал Ib:  {}\n", f(&st.ib)));
        s.push_str(&format!("Канал Ic:  {}\n", f(&st.ic)));
        s.push_str(&format!("Канал Ua:  {}\n", f(&st.ua)));
        s.push_str(&format!("Канал Ub:  {}\n", f(&st.ub)));
        s.push_str(&format!("Канал Uc:  {}\n\n", f(&st.uc)));
    };
    match p.method {
        ISOLATED => {
            station_head(&mut s, "Подстанция: ", st1);
            s.push_str(&format!("Канал Ia:  {}\n", chan(&st1.ia)));
            s.push_str(&format!("Канал Ib:  {}\n", chan(&st1.ib)));
            s.push_str(&format!("Канал Ic:  {}\n", chan(&st1.ic)));
            s.push_str(&format!("Канал Uab: {}\n", chan(&st1.uab)));
            s.push_str(&format!("Канал Ubc: {}\n", chan(&st1.ubc)));
            s.push_str(&format!("Канал Uca: {}\n\n", chan(&st1.uca)));
        }
        GROUNDED => {
            station_head(&mut s, "Подстанция: ", st1);
            phase_lines(&mut s, st1, &chan);
        }
        DOUBLE_ENDED => {
            station_head(&mut s, "Подстанция №1: ", st1);
            phase_lines(&mut s, st1, &chan);
        }
        _ => {}
    }
    let lines_shown = p.method == GROUNDED || p.method == DOUBLE_ENDED;
    for (k, sec) in st1.sections.iter().enumerate() {
        s.push_str(&format!("\nУчасток № {}: \n", k + 1));
        s.push_str(&format!("    Длина:               {}\n", sec.length));
        s.push_str(&format!("    Число опор:          {}\n", sec.supports));
        if p.method == ISOLATED {
            s.push_str(&format!("    R удельное (Ом/км): {}\n", sec.r));
            s.push_str(&format!("    X удельное (Ом/км): {}\n", sec.x));
        } else if lines_shown {
            s.push_str(&format!("    R1 удельное (Ом/км): {}\n", sec.r1));
            s.push_str(&format!("    X1 удельное (Ом/км): {}\n", sec.x1));
            s.push_str(&format!("    R0 удельное (Ом/км): {}\n", sec.r0));
            s.push_str(&format!("    X0 удельное (Ом/км): {}\n", sec.x0));
        }
        s.push_str(&format!("    Комментарий:   {}\n\n", sec.comment));
        if !lines_shown {
            continue;
        }
        for (n, l) in sec.lines.iter().enumerate() {
            s.push_str(&format!("    Параллельная линия № {}: \n", n + 1));
            s.push_str(&format!("        Rm удельное(Ом/км): {}\n", l.rm));
            s.push_str(&format!("        Xm удельное(Ом/км): {}\n", l.xm));
            s.push_str(&format!("        I0:      {}\n", l.i0));
            s.push_str(&format!("        Комментарий: {}\n\n", l.comment));
        }
        for (n, t) in sec.taps.iter().enumerate() {
            s.push_str(&format!("    Отпайка № {}: \n", n + 1));
            s.push_str(&format!("        R0 (Ом):     {}\n", t.r0));
            s.push_str(&format!("        X0 (Ом):     {}\n", t.x0));
            s.push_str(&format!("        Комментарий: {}\n\n", t.comment));
        }
    }
    if p.method == DOUBLE_ENDED {
        let st2 = &p.stations[1];
        station_head(&mut s, "Подстанция №2: ", st2);
        phase_lines(&mut s, st2, &|n: &str| n.to_string());
    }
    s
}

/// `%g` as the MSVC 2013 CRT prints it: a three-digit exponent (`1e+006`).
fn msvc_g(v: f64) -> String {
    let g = crate::views::fmt_g(v);
    match g.find(['e']) {
        Some(i) if g.len() - i == 4 => format!("{}{}0{}", &g[..i + 1], &g[i + 1..i + 2], &g[i + 2..]),
        _ => g,
    }
}

/// The report's result paragraph: the banner text without its leading
/// indent, the distance on the same line and a blank line after — except the
/// adjacent-line case, which ends without a line break (as the original).
fn result_line(p: &OmpParams, r: &OmpResult) -> String {
    if !(r.e || r.f2 || r.f3) {
        return "КЗ отсутствует.\n\n".into();
    }
    let mut s = if r.f3 {
        "3-х фазное КЗ: фазы "
    } else if r.f2 {
        "2-х фазное КЗ: фазы "
    } else {
        "1-х фазное КЗ: фазы "
    }
    .to_string();
    for (on, name) in r.phases.iter().zip(["A", "B", "C"]) {
        if *on {
            s.push_str(name);
        }
    }
    let total: f64 = p.stations[0].sections.iter().map(|x| wtof(&x.length)).sum();
    if r.distance > total || r.distance <= 0.0 {
        s.push_str(" на смежной линии.");
        return s;
    }
    let (v1, p1) = crate::omp::si(r.distance * 1000.0);
    let (v2, p2) = crate::omp::si(r.in_section_km * 1000.0);
    s.push_str(&format!(
        " на участке № {}, опоры {} - {}.    Расстояние = {v1:.2} {p1}m.     Расстояние от нач. участка = {v2:.2} {p2}m.    Погрешность расчета = {:.2} %.\n\n",
        r.section + 1,
        r.support as i64 - 1,
        r.support,
        r.error
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> OmpParams {
        let mut p = OmpParams { method: GROUNDED, ..Default::default() };
        p.stations[0].inom = "100".into();
        p.stations[0].ia = "Ia".into();
        let mut s = SectionParams { r1: "0.1".into(), x1: "0.4".into(), r0: "0.3".into(), x0: "1.2".into(), ..Default::default() };
        s.lines.push(ParLineParams { rm: "0.1".into(), xm: "0.2".into(), i0: "3I0 \"пар\"".into(), comment: "a<b".into() });
        s.taps.push(TapParams { r0: "5".into(), x0: "7".into(), comment: String::new() });
        p.stations[0].sections.push(s);
        p
    }

    #[test]
    fn wtof_takes_the_longest_number_prefix() {
        assert_eq!(wtof("  12.5abc"), 12.5);
        assert_eq!(wtof("-3"), -3.0);
        assert_eq!(wtof("1e3"), 1000.0);
        assert_eq!(wtof("1e"), 1.0);
        assert_eq!(wtof("1."), 1.0);
        assert_eq!(wtof(".5"), 0.5);
        assert_eq!(wtof("abc"), 0.0);
        assert_eq!(wtof(""), 0.0);
        assert_eq!(wtof("0,5"), 0.0);
    }

    /// «Сохранить» `0x4a91c0`: a settings document, the category `omp`
    /// (type, ver) holding Station1 / Station2 → Section{k} → child{n}, every
    /// node a `category` with its key as `name`, children sorted by key.
    #[test]
    fn omp_file_layout_matches_the_original_writer() {
        let x = crate::decode_fvb(&crate::recset::omp_file_bytes(&sample()));
        let lines: Vec<&str> = x.lines().map(|l| l.trim_end_matches('\r')).collect();
        assert_eq!(lines[0], "<?xml version=\"1.0\" encoding=\"utf-16\"?>");
        assert_eq!(lines[1], "<settings>");
        assert!(lines[2].starts_with("\t<category ") && lines[2].contains(" name=\"omp\"") && lines[2].contains("ver=\"2\""), "{}", lines[2]);
        assert!(lines[3].starts_with("\t\t<category ") && lines[3].contains("name=\"Station1\"") && lines[3].contains("obj_name=\"Подстанция\""), "{}", lines[3]);
        assert!(lines[4].contains("name=\"Section1\"") && lines[4].contains("lenght=\"1\""), "{}", lines[4]);
        assert!(lines[5].contains("name=\"child1\"") && lines[5].contains("type=\"parLine\""), "{}", lines[5]);
        assert!(lines[6].contains("name=\"child2\"") && lines[6].contains("type=\"connection\""), "{}", lines[6]);
        assert_eq!(*lines.last().unwrap(), "</settings>");
    }

    /// «Загрузить» `0x4a9010`: the file back; one without any category
    /// (the legacy layouts) or of another version gives nothing, silently.
    #[test]
    fn omp_file_round_trips() {
        let p = sample();
        let bytes = crate::recset::omp_file_bytes(&p);
        assert_eq!(&bytes[..4], &[0xFF, 0xFE, b'<', 0]);
        assert_eq!(crate::recset::omp_from_file_bytes(&bytes), Some(p));
        assert_eq!(crate::recset::omp_from_file_bytes(b"\xff\xfe"), None);
        let old = utf16le_crlf("<settings><category name=\"omp\" ver=\"1\"/></settings>");
        assert_eq!(crate::recset::omp_from_file_bytes(&old), None);
    }

    #[test]
    fn validation_reports_the_first_failure() {
        let mut p = sample();
        assert_eq!(p.validate(), Ok(()));
        p.stations[0].sections[0].taps[0].r0 = "0".into();
        p.stations[0].sections[0].taps[0].x0 = "x".into();
        assert_eq!(p.validate(), Err(3071));
        p.stations[0].sections[0].lines[0].xm = "0".into();
        p.stations[0].sections[0].lines[0].rm = "".into();
        assert_eq!(p.validate(), Err(3076));
        p.stations[0].sections[0].supports = "0".into();
        assert_eq!(p.validate(), Err(3535));
        p.stations[0].sections[0].length = "".into();
        assert_eq!(p.validate(), Err(3187));
        let mut q = OmpParams::default();
        q.stations[0].sections.push(SectionParams::default());
        assert_eq!(q.validate(), Err(3068), "method 1 checks R/X");
    }

    #[test]
    fn g_format_has_three_exponent_digits() {
        assert_eq!(msvc_g(1e6), "1e+006");
        assert_eq!(msvc_g(2.5e-7), "2.5e-007");
        assert_eq!(msvc_g(600.0), "600");
    }

    #[test]
    fn report_layout() {
        let p = sample();
        let r = OmpResult {
            window: (0, 9),
            distance: 0.5,
            error: 1.5,
            e: true,
            f2: false,
            f3: false,
            phases: [true, false, false],
            section: 0,
            in_section_km: 0.5,
            support: 1,
            text: String::new(),
        };
        let info = ReportInfo { title: "rec.cfg".into(), station: "ПС".into(), recorder: "Рег".into(), ..Default::default() };
        let t = report(&p, &r, &info, |n| (n == "Ia").then(|| ("Ia".to_string(), 600.0, 5.0)));
        let expect_head = "Механотроника FastView\n\nОсциллограмма: rec.cfg\n\nАлгоритм: ОМП для сетей с заземленной нейтралью\n\n\
            Станция: ПС\nРегистратор: Рег\n\nНачало записи:  \nМомент запуска: \n\n\
            1-х фазное КЗ: фазы A на участке № 1, опоры 0 - 1.    Расстояние = 500.00 m.     Расстояние от нач. участка = 500.00 m.    Погрешность расчета = 1.50 %.\n\n\
            Подстанция: Подстанция\nНоминальный ток прямой последовательности, A: 100\n\n\
            Канал Ia:  Ia         Ктр = 600/5\nКанал Ib:  \n";
        assert!(t.starts_with(expect_head), "{t}");
        assert!(t.contains("Канал Uc:  \n\n\nУчасток № 1: \n    Длина:               1\n"));
        assert!(t.contains("    X0 удельное (Ом/км): 1.2\n    Комментарий:   \n\n    Параллельная линия № 1: \n"));
        assert!(t.ends_with("    Отпайка № 1: \n        R0 (Ом):     5\n        X0 (Ом):     7\n        Комментарий: \n\n"));
        let none = OmpResult { e: false, ..r.clone() };
        assert!(report(&p, &none, &info, |_| None).contains("Момент запуска: \n\nКЗ отсутствует.\n\nПодстанция: "));
        let far = OmpResult { distance: 5.0, ..r };
        assert!(report(&p, &far, &info, |_| None).contains("фазы A на смежной линии.Подстанция: "));
    }

    /// `0x4a38c0`: Section{k} up to the child count, a missing one read as
    /// a section of "" values; a missing child{j} as a tap of "0" / "0".
    #[test]
    fn gaps_read_as_defaulted_elements() {
        let xml = r#"<omp ver="2" type="1"><Station1 In="5"><Section2 r1="1"><child2 type="parLine" rm="2"/><x/></Section2><y/></Station1></omp>"#;
        let p = OmpParams::from_node(&Node::parse(xml).unwrap()).unwrap();
        let st = &p.stations[0];
        assert_eq!(st.sections.len(), 2);
        assert_eq!((st.sections[0].r1.as_str(), st.sections[0].length.as_str()), ("", ""));
        let s2 = &st.sections[1];
        assert_eq!(s2.r1, "1");
        assert_eq!(s2.taps.len(), 1, "child1 missing: a tap");
        assert_eq!((s2.taps[0].r0.as_str(), s2.taps[0].x0.as_str()), ("0", "0"));
        assert_eq!(s2.lines[0].rm, "2");
    }

    /// `0x4a45f0`: the legacy text — a discarded integer, Ia … Uca, the
    /// nominal current, N, N sections of five fields; method 1; station 2
    /// untouched; a short file loads nothing.
    #[test]
    fn legacy_omp_text() {
        let mut cur = sample();
        cur.stations[1].name = "B".into();
        let p = apply_legacy("7\nIa,Ib,Ic\tUab,Ubc,Uca\n100\n2\n1.5,0.1,0.4,12,first\n2,0.2,0.5,8,\n", &cur).unwrap();
        assert_eq!(p.method, ISOLATED);
        let st = &p.stations[0];
        assert_eq!((st.ia.as_str(), st.uca.as_str(), st.inom.as_str()), ("Ia", "Uca", "100"));
        assert_eq!(st.sections.len(), 2);
        assert_eq!((st.sections[0].length.as_str(), st.sections[0].r.as_str(), st.sections[0].supports.as_str(), st.sections[0].comment.as_str()), ("1.5", "0.1", "12", "first"));
        assert_eq!(p.stations[1].name, "B", "station 2 untouched");
        let short = apply_legacy("7\nIa,Ib\n", &cur).unwrap();
        assert_eq!((short.stations[0].ib.as_str(), short.stations[0].ic.as_str(), short.stations[0].sections.len()), ("Ib", "", 0), "past the end: empty, N = 0");
    }
}

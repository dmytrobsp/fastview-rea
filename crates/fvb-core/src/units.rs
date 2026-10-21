//! Channel unit normalisation (`0x422b10`, ledger L9.34): the COMTRADE unit
//! as written in the file (`ch+0x78`) becomes a canonical Latin base unit
//! (`ch+0x7c`) and a multiplier (`ch+0x98`) folded into the sample values,
//! so «кА» is "A" ×1000. Every eligibility test (vector / circle diagrams,
//! the «Каналы» dialog) and the SI-prefixed readouts work on the canonical
//! unit.
//!
//! The original compares CP1251 literals case-sensitively against the unit
//! and an upper-cased copy (`MakeUpper`, taken here as ASCII-only — Cyrillic
//! case variants are listed by hand in the code); nothing is trimmed.

/// A normalised unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// The canonical unit (`ch+0x7c`), or the raw unit when unrecognised.
    pub unit: String,
    /// The SI-prefix multiplier applied to the sample values.
    pub mult: f64,
    /// A trailing «/с» or "/s" was removed (`ch+0x154`).
    pub per_second: bool,
    /// No mapping matched (`ch+0x155`).
    pub unknown: bool,
}

fn ends_per_second(s: &str) -> bool {
    s.ends_with("/с") || s.ends_with("/s")
}

fn is_nom(s: &str) -> bool {
    matches!(s, "ном" | "Ном" | "НОМ") || s.eq_ignore_ascii_case("NOM")
}

/// The SI prefix of `raw` (in the original's test order): its length in
/// characters and its multiplier.
fn prefix(raw: &str) -> Option<(usize, f64)> {
    let chars: Vec<char> = raw.chars().collect();
    let len = chars.len();
    let first = chars.first().copied()?;
    // 3 chars ending in /с or /s: a prefix letter that is the whole unit.
    let short_rate = |n: usize| len == n && ends_per_second(raw);
    if raw.starts_with("мк") || raw.starts_with("mk") {
        return Some((2, 1e-6));
    }
    if matches!(first, 'u' | 'µ') {
        return Some((1, 1e-6));
    }
    if matches!(first, 'м' | 'm') {
        return (!(len == 1 || short_rate(3))).then_some((1, 1e-3));
    }
    if matches!(first, 'н' | 'Н' | 'n' | 'N') {
        let nom_rate = len == 5 && ends_per_second(raw) && is_nom(&chars[..3].iter().collect::<String>());
        return (!(len == 1 || short_rate(3) || is_nom(raw) || nom_rate)).then_some((1, 1e-9));
    }
    if matches!(first, 'к' | 'К' | 'k' | 'K') {
        return Some((1, 1e3));
    }
    if matches!(first, 'М' | 'M') {
        return (!(len == 1 || short_rate(3))).then_some((1, 1e6));
    }
    if matches!(first, 'Г' | 'G') {
        let two: String = chars.iter().take(2).collect::<String>().to_uppercase();
        let base = len == 2 && matches!(two.as_str(), "ГН" | "ГЦ" | "ГР");
        return (!(base || short_rate(4))).then_some((1, 1e9));
    }
    None
}

/// Normalise a COMTRADE unit as the channel constructor does (`0x422b10`).
pub fn normalize(raw: &str) -> Unit {
    let plain = |unit: &str, unknown: bool| Unit { unit: unit.to_string(), mult: 1.0, per_second: false, unknown };
    if raw.is_empty() || raw.eq_ignore_ascii_case("NONE") {
        return plain("", false);
    }
    let (mut s, mult) = match prefix(raw) {
        Some((n, m)) => (raw.chars().skip(n).collect::<String>(), m),
        None => (raw.to_string(), 1.0),
    };
    let mut per_second = false;
    for tail in ["/с", "/s"] {
        if let Some(head) = s.strip_suffix(tail) {
            s = head.to_string();
            per_second = true;
            break;
        }
    }
    let u = s.to_ascii_uppercase();
    let canon = |c: &str| Unit { unit: c.to_string(), mult, per_second, unknown: false };
    match () {
        _ if matches!(u.as_str(), "А" | "A") => canon("A"),
        _ if matches!(u.as_str(), "В" | "V") => canon("V"),
        _ if matches!(u.as_str(), "ВА" | "В·А" | "V·A" | "VA") => canon("VA"),
        _ if matches!(u.as_str(), "ВТ" | "W") => canon("W"),
        _ if matches!(u.as_str(), "ВАР" | "VAR") => canon("var"),
        _ if s == "Ом" || s == "Ω" || u == "OHM" => canon("Ohm"),
        _ if matches!(u.as_str(), "Ф" | "F") => canon("F"),
        _ if matches!(u.as_str(), "ГН" | "H") => canon("H"),
        // Degrees, nominal and per-unit drop the prefix again.
        // (String 3105, a further spelling, is not decoded.)
        _ if matches!(s.as_str(), "гр" | "ГР" | "Гр" | "гР") || u == "DEG" => plain("DEG", false),
        _ if matches!(s.as_str(), "н" | "Н" | "ном" | "Ном" | "НОМ") || u == "NOM" => plain("nom", false),
        _ if matches!(u.as_str(), "О. Е." | "О.Е." | "ОЕ" | "О Е" | "P. U." | "P.U." | "PU" | "P U") => plain("p. u.", false),
        _ if matches!(u.as_str(), "ГЦ" | "HZ") => canon("Hz"),
        _ if matches!(s.as_str(), "м" | "М" | "m" | "M") => canon("m"),
        _ if matches!(s.as_str(), "с" | "s") => canon("s"),
        _ if matches!(s.as_str(), "См" | "S") => canon("S"),
        _ => plain(raw, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_and_cyrillic_map_to_latin_base_units() {
        let n = |r: &str| {
            let u = normalize(r);
            (u.unit, u.mult)
        };
        assert_eq!(n("кА"), ("A".into(), 1e3));
        assert_eq!(n("кВ"), ("V".into(), 1e3));
        assert_eq!(n("А"), ("A".into(), 1.0));
        assert_eq!(n("В"), ("V".into(), 1.0));
        assert_eq!(n("a"), ("A".into(), 1.0), "Latin case folds");
        assert_eq!(n("мА"), ("A".into(), 1e-3));
        assert_eq!(n("мкА"), ("A".into(), 1e-6));
        assert_eq!(n("МВТ"), ("W".into(), 1e6));
        // Upper-casing taken as ASCII-only (Cyrillic folding NOT DETERMINED):
        // a lower-case «Вт» stays unrecognised.
        assert_eq!(n("МВт"), ("МВт".into(), 1.0));
        assert_eq!(n("Ом"), ("Ohm".into(), 1.0));
        assert_eq!(n("кОм"), ("Ohm".into(), 1e3));
        assert_eq!(n("m"), ("m".into(), 1.0), "a lone prefix letter is the unit");
        assert_eq!(n("ном"), ("nom".into(), 1.0));
        assert_eq!(n("B"), ("B".into(), 1.0), "a Latin B is not a volt");
        assert!(normalize("B").unknown);
        assert_eq!(n(""), ("".into(), 1.0));
        assert_eq!(n("none"), ("".into(), 1.0));
        let r = normalize("м/с");
        assert_eq!((r.unit.as_str(), r.mult, r.per_second), ("m", 1.0, true), "a lone м before /с is the metre");
    }
}

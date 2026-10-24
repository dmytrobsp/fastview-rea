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
        // Degrees, nominal and per-unit drop the prefix again; string 3105
        // «°» is the fifth degree spelling (`0x42314c`).
        _ if matches!(s.as_str(), "гр" | "ГР" | "Гр" | "гР" | "°") || u == "DEG" => plain("DEG", false),
        _ if matches!(s.as_str(), "н" | "Н" | "ном" | "Ном" | "НОМ") || u == "NOM" => plain("nom", false),
        _ if matches!(u.as_str(), "О. Е." | "О.Е." | "ОЕ" | "О Е" | "P. U." | "P.U." | "PU" | "P U") => plain("p. u.", false),
        _ if matches!(u.as_str(), "ГЦ" | "HZ") => canon("Hz"),
        _ if matches!(s.as_str(), "м" | "М" | "m" | "M") => canon("m"),
        _ if matches!(s.as_str(), "с" | "s") => canon("s"),
        _ if matches!(s.as_str(), "См" | "S") => canon("S"),
        _ => plain(raw, true),
    }
}

/// The unit as the Russian UI shows it (`0x42b740`; the language global
/// `0x4ed17c` is never written, so always Russian): "DEG" → «°» (string
/// 3105), then each Latin spelling replaced in this order wherever it
/// occurs (`CString::Replace`), then "Ohm" → «Ω» (string 3546, a no-op by
/// then). Applied to recognised units only (`0x42bc50` skips `ch+0x155`).
pub fn ru(unit: &str) -> String {
    const PAIRS: [(&str, &str); 23] = [
        ("OHM", "Ом"),
        ("var", "вар"),
        ("VAr", "ВАр"),
        ("Ohm", "Ом"),
        ("nom", "ном"),
        ("VA", "ВА"),
        ("Hz", "Гц"),
        ("mk", "мк"),
        ("A", "А"),
        ("V", "В"),
        ("W", "Вт"),
        ("F", "Ф"),
        ("H", "Гн"),
        ("m", "м"),
        ("n", "н"),
        ("K", "К"),
        ("k", "к"),
        ("M", "М"),
        ("G", "Г"),
        ("s", "с"),
        ("S", "См"),
        ("p. u.", "o. e."),
        ("µ", "мк"),
    ];
    let mut u = unit.replace("DEG", "°");
    for (from, to) in PAIRS {
        u = u.replace(from, to);
    }
    u.replace("Ohm", "Ω")
}

/// Is `unit` one of the normaliser's canonical outputs (a recognised unit,
/// `ch+0x155` clear)?
pub fn is_canonical(unit: &str) -> bool {
    matches!(unit, "A" | "V" | "VA" | "W" | "var" | "Ohm" | "F" | "H" | "DEG" | "nom" | "p. u." | "Hz" | "m" | "s" | "S")
}

/// A channel's unit as the readouts show it (`0x42bc50`): the canonical
/// unit (`0x414c30`), which picks the SI prefix (`0x43d960`), and the text
/// printed after the prefix.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shown {
    pub canon: String,
    pub text: String,
}

impl Shown {
    /// A file channel's: an unrecognised unit (`ch+0x155`) as is, else the
    /// unit with «/с» (string `0x4c5eac`) when per-second (`ch+0x154`),
    /// through [`ru`].
    pub fn file(unit: &str, per_second: bool) -> Shown {
        let text = if is_canonical(unit) || unit.is_empty() {
            ru(&format!("{unit}{}", if per_second { "/с" } else { "" }))
        } else {
            unit.to_string()
        };
        Shown { canon: unit.to_string(), text }
    }

    /// A computed channel's: its unit is stored as the formula gave it with
    /// both flags clear (`0x424504`, `0x425db0`), so any text is translated
    /// (`[kV]` → «кВ»).
    pub fn computed(unit: &str) -> Shown {
        Shown { canon: unit.to_string(), text: ru(unit) }
    }
}

impl From<&str> for Shown {
    fn from(unit: &str) -> Shown {
        Shown::file(unit, false)
    }
}

impl From<&String> for Shown {
    fn from(unit: &String) -> Shown {
        Shown::file(unit, false)
    }
}

impl From<&Shown> for Shown {
    fn from(s: &Shown) -> Shown {
        s.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_display_units() {
        assert_eq!(ru("V"), "В");
        assert_eq!(ru("A"), "А");
        assert_eq!(ru("VA"), "ВА");
        assert_eq!(ru("var"), "вар");
        assert_eq!(ru("Ohm"), "Ом");
        assert_eq!(ru("Hz"), "Гц");
        assert_eq!(ru("W"), "Вт");
        assert_eq!(ru("nom"), "ном");
        assert_eq!(ru("DEG"), "°");
        assert_eq!(normalize("°").unit, "DEG", "string 3105");
        assert_eq!(ru("S"), "См");
        // Plain substring replacement in table order.
        assert_eq!(ru("p. u."), "o. e.");
    }

    #[test]
    fn shown_unit_texts() {
        // A file channel: «кВ/с» is V per second, shown «В/с».
        let n = normalize("кВ/с");
        assert_eq!(Shown::file(&n.unit, n.per_second), Shown { canon: "V".into(), text: "В/с".into() });
        // An unrecognised file unit is shown as written.
        assert_eq!(Shown::file("kVs", false).text, "kVs");
        // A computed channel's unit is always translated, but only the
        // canonical one picks a prefix.
        assert_eq!(Shown::computed("kV"), Shown { canon: "kV".into(), text: "кВ".into() });
        assert_eq!(Shown::computed("В").text, "В");
    }

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

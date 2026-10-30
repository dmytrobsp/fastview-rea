//! Pure computations of the original's analysis views (ledger L8,
//! `docs/original/specs/views.md`), kept here so they can be tested without a GUI.

use num_complex::Complex64;
use std::f64::consts::PI;

/// Vector-diagram group scale (`0x4910c3`): `p = 10^(⌊log10 max⌋ − 1)`,
/// `scale = ⌈⌈max/p⌉/5⌉·5p`; a result that is not > 0 (an empty group's
/// 0 gives NaN) is 0 (`0x49123f`).
pub fn nice_scale(max: f64) -> f64 {
    let p = 10f64.powf(max.log10().floor() - 1.0);
    let v = ((max / p).ceil() / 5.0).ceil() * 5.0 * p;
    if v > 0.0 && v.is_finite() {
        v
    } else {
        0.0
    }
}

/// The header value (`0x491331`): `%.1f` above 1 or at 0, else
/// `%w.(w−2)f` with `w = |⌊log10 v⌋| + 3`. `0x43d960` is passed the unit
/// "U" / "I", which it never prefixes, so the value stays in V / A.
pub fn vector_header_value(v: f64) -> String {
    if v > 1.0 || v == 0.0 {
        format!("{v:.1}")
    } else {
        let w = (v.log10().floor().abs() + 3.0) as usize;
        format!("{v:w$.p$}", p = w - 2)
    }
}

/// The values table's magnitude (`0x4925c5`): `"%.2f %s%s"` above 1 or at
/// 0, else `"%w.(w−2)f"` with `w = |⌊log10 |z|⌋| + 3` (of the unscaled
/// value — INFERRED); value and prefix from `0x43d960` on the canonical unit.
pub fn vector_value_text(v: f64, unit: impl Into<crate::units::Shown>) -> String {
    let (scaled, u) = si_scaled(v, unit);
    if v > 1.0 || v == 0.0 {
        format!("{scaled:.2} {u}")
    } else {
        let w = (v.log10().floor().abs() + 3.0) as usize;
        format!("{scaled:w$.p$} {u}", p = w - 2)
    }
}

/// Angle of `z` relative to `reference`, wrapped to (−π, π]; absolute
/// (raw sine-referenced) when there is no reference.
pub fn vector_angle(z: Complex64, reference: Option<Complex64>) -> f64 {
    let mut d = z.arg() - reference.map_or(0.0, |r| r.arg());
    while d <= -PI {
        d += 2.0 * PI;
    }
    while d > PI {
        d -= 2.0 * PI;
    }
    d
}

/// The vector diagram's angle text: degrees `" %8.2fº"`, or in radians the
/// value divided by π as `" %8.2fπ"`.
pub fn angle_text(rad: f64, radians: bool) -> String {
    if radians {
        format!(" {:8.2}π", rad / PI)
    } else {
        format!(" {:8.2}º", rad.to_degrees())
    }
}

/// A NaN phasor is drawn as 0.
pub fn finite_or_zero(z: Complex64) -> Complex64 {
    if z.re.is_nan() || z.im.is_nan() {
        Complex64::default()
    } else {
        z
    }
}

/// Frequency-diagram bar label (`0x46f5a0`): below 0.01 the literal "0",
/// below 10 `"%1.1f"`, below 100 `"%2.1f"`, else `"%10.0f"`.
pub fn freq_label(v: f64) -> String {
    if v < 0.01 {
        "0".into()
    } else if v < 100.0 {
        format!("{v:.1}")
    } else {
        format!("{v:10.0}")
    }
}

/// C's `%g` (precision 6): exponent form when the decimal exponent is below
/// −4 or at least 6, trailing zeros removed — with MSVC 12's three exponent
/// digits (the VS2013 CRT; seen on the VM: «-3.79074e-005»).
pub fn fmt_g(v: f64) -> String {
    fmt_gp(v, 6)
}

/// C's `%.*g` with precision `p` (≥ 1).
pub fn fmt_gp(v: f64, p: usize) -> String {
    let p = p.max(1);
    if v == 0.0 {
        return "0".into();
    }
    if !v.is_finite() {
        return if v.is_nan() { "nan".into() } else if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let e = format!("{v:.*e}", p - 1);
    let (mant, exp) = e.split_once('e').expect("exponent");
    let x: i32 = exp.parse().expect("exponent digits");
    let trim = |t: &str| -> String {
        if t.contains('.') {
            t.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            t.to_string()
        }
    };
    if (-4..p as i32).contains(&x) {
        trim(&format!("{v:.*}", (p as i32 - 1 - x) as usize))
    } else {
        format!("{}e{}{:03}", trim(mant), if x < 0 { '-' } else { '+' }, x.abs())
    }
}

/// The per-channel readout text (`0x461440`): `" %-8g %s"` with an SI
/// prefix on the unit.
pub fn readout_text(v: f64, unit: impl Into<crate::units::Shown>) -> String {
    let (scaled, unit) = si_scaled(v, unit);
    if v < 0.0 {
        format!("{:<8} {unit}", fmt_g(scaled))
    } else {
        format!(" {:<8} {unit}", fmt_g(scaled))
    }
}

/// Units that get an SI prefix (`0x43d960`, exact case-sensitive compares):
/// V, A, VA, W, ОM (Cyrillic О), Ohm, OHM, var, F, H, GN, Hz, m. Any other
/// unit prints the raw value.
pub fn si_unit(unit: &str) -> bool {
    matches!(unit, "V" | "A" | "VA" | "W" | "ОM" | "Ohm" | "OHM" | "var" | "F" | "H" | "GN" | "Hz" | "m")
}

/// `v` scaled for display and its unit with the SI prefix, when the
/// canonical unit takes one; the text is the unit as the Russian UI shows
/// it (`0x42bc50`: «В», «мА», «Ом», «В/с»).
pub fn si_scaled(v: f64, unit: impl Into<crate::units::Shown>) -> (f64, String) {
    let unit = unit.into();
    if si_unit(&unit.canon) {
        let (s, p) = crate::omp::si(v);
        (s, format!("{p}{}", unit.text))
    } else {
        (v, unit.text)
    }
}

/// The stats-panel value at В1 (`0x461440`): the sign, then `%-6g %s` of
/// |v|; the unit (prefixed) cut to 5 characters, "none" printed as nothing;
/// NaN is not drawn.
pub fn stats_value_text(v: f64, unit: impl Into<crate::units::Shown>) -> Option<String> {
    if v.is_nan() {
        return None;
    }
    let unit = unit.into();
    // The readout lower-cases the unit before the "none" test
    // (`_wcslwr_s`, `0x461c31`–`0x461c3e`): «None» / «NONE» blank too.
    let unit = if unit.canon.to_lowercase() == "none" { crate::units::Shown::default() } else { unit };
    let (s, u) = si_scaled(v.abs(), unit);
    let u: String = u.chars().take(5).collect();
    Some(format!("{}{:<6} {u}", if v < 0.0 { "-" } else { " " }, fmt_g(s)))
}

/// A grouped channel's one-line stats (`0x4683c0`): the enabled items
/// `Max:` / `Min:` (both with min/max), `СКЗ:` (string 3449), `A1:`, each
/// the SI-scaled value as `%-8g` trimmed — cut to two decimals and its
/// trailing zeros stripped when it has a `.` and no exponent (a bare `.` can
/// stay) — and the prefixed unit cut to 5 characters, joined by `|`.
pub fn group_stats_line(minmax: Option<(f64, f64)>, rms: Option<f64>, a1: Option<f64>, unit: impl Into<crate::units::Shown>) -> String {
    let unit = unit.into();
    let item = |label: &str, v: f64| {
        let (s, u) = si_scaled(v, &unit);
        let mut t = fmt_g(s);
        if let (Some(dot), false) = (t.find('.'), t.contains('e')) {
            t.truncate((dot + 3).min(t.len()));
            while t.len() > 1 && t.ends_with('0') {
                t.pop();
            }
        }
        format!("{label}{t}{}", u.chars().take(5).collect::<String>())
    };
    let mut items = Vec::new();
    if let Some((max, min)) = minmax {
        items.extend([item("Max:", max), item("Min:", min)]);
    }
    items.extend(rms.map(|v| item("СКЗ:", v)));
    items.extend(a1.map(|v| item("A1:", v)));
    items.join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A kV / kA channel's unit in every readout (#86): the value column,
    /// the single-lane rows and the group line all take the SI prefix from
    /// the magnitude (`0x43d960`) and the unit text through `0x42bc50`, so
    /// 354 V of a kV channel reads «В» and 70 kV «кВ» in all of them.
    #[test]
    fn kilovolt_and_kiloampere_units_in_every_row() {
        let kv = crate::units::normalize("kV");
        let ka = crate::units::normalize("кА");
        // The channel's values are in volts / amperes (the unit multiplier is applied).
        for (n, big, small, big_t, small_t) in [(&kv, 100_000.0, 353.5, "кВ", "В"), (&ka, 35_000.0, 0.25, "кА", "мА")] {
            let unit = crate::units::Shown::file(&n.unit, n.per_second);
            for (v, t) in [(big, big_t), (small, small_t)] {
                assert!(readout_text(v, &unit).ends_with(t), "row {v}");
                assert!(stats_value_text(v, &unit).unwrap().ends_with(t), "value column {v}");
                assert!(group_stats_line(None, Some(v), Some(v), &unit).split('|').all(|i| i.ends_with(t)), "group line {v}");
            }
        }
        assert_eq!(readout_text(70_710.2, crate::units::Shown::file("V", false)), " 70.7102  кВ");
    }

    #[test]
    fn group_line() {
        assert_eq!(group_stats_line(Some((1500.0, -0.125)), None, Some(10.004), "A"), "Max:1.5кА|Min:-125мА|A1:10.А", "a bare dot can stay");
        assert_eq!(group_stats_line(None, Some(0.10), None, "x"), "СКЗ:0.1x");
        assert_eq!(group_stats_line(None, None, None, "A"), "");
    }

    #[test]
    fn nice_scales() {
        assert_eq!(nice_scale(37.0), 40.0);
        assert_eq!(nice_scale(57_000.0), 60_000.0);
        assert_eq!(nice_scale(1234.0), 1500.0);
        assert_eq!(nice_scale(4.2), 4.5);
        assert!((nice_scale(0.37) - 0.4).abs() < 1e-12);
        assert_eq!(nice_scale(0.0), 0.0);
        assert_eq!(vector_header_value(1500.0), "1500.0");
        assert_eq!(vector_header_value(0.0), "0.0");
        assert_eq!(vector_header_value(0.4), "0.40");
        assert_eq!(vector_header_value(0.05), "0.050");
        assert_eq!(vector_header_value(1.0), "1.0", "w = 3: %3.1f");
        assert_eq!(vector_value_text(57.9232, "V"), "57.92 В");
        assert_eq!(vector_value_text(0.354, "A"), "354.00 мА");
        assert_eq!(vector_value_text(0.0, "A"), "0.00 А");
    }

    #[test]
    fn angles() {
        let r = Complex64::from_polar(1.0, 170f64.to_radians());
        let z = Complex64::from_polar(1.0, -170f64.to_radians());
        assert!((vector_angle(z, Some(r)).to_degrees() - 20.0).abs() < 1e-9);
        assert_eq!(vector_angle(r, Some(r)), 0.0);
        assert_eq!(angle_text(PI / 2.0, false), "    90.00º");
        assert_eq!(angle_text(PI / 2.0, true), "     0.50π");
    }

    #[test]
    fn printf_g() {
        assert_eq!(fmt_g(57.1234567), "57.1235");
        assert_eq!(fmt_g(0.0001), "0.0001");
        assert_eq!(fmt_g(0.00001234), "1.234e-005");
        assert_eq!(fmt_g(1234567.0), "1.23457e+006");
        assert_eq!(fmt_g(100.0), "100");
        assert_eq!(readout_text(57_100.0, "V"), " 57.1     кВ");
        assert_eq!(readout_text(-57_100.0, "V"), "-57.1    кВ");
        assert_eq!(readout_text(57_100.0, "kV"), " 57100    kV", "only listed units get a prefix");
        // A computed channel's `[kV]` is translated but takes no prefix; a
        // per-second file unit keeps its «/с» after the prefixed unit.
        assert_eq!(readout_text(57_100.0, crate::units::Shown::computed("kV")), " 57100    кВ");
        assert_eq!(readout_text(1500.0, crate::units::Shown::file("V", true)), " 1.5      кВ/с");
        assert_eq!(readout_text(0.012, "ОM"), " 12       мОM");
        assert_eq!(stats_value_text(-23.4375, "A").unwrap(), "-23.4375 А");
        assert_eq!(stats_value_text(1.5, "none").unwrap(), " 1.5    ");
        assert_eq!(stats_value_text(2.0e5, "VA").unwrap(), " 200    кВА");
        assert!(stats_value_text(f64::NAN, "A").is_none());
    }

    #[test]
    fn freq_labels() {
        assert_eq!(freq_label(0.005), "0");
        assert_eq!(freq_label(3.27), "3.3");
        assert_eq!(freq_label(42.25), "42.2");
        assert_eq!(freq_label(1234.6), "      1235");
    }
}

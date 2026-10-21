//! Pure computations of the original's analysis views (ledger L8,
//! `notes/VIEWS_SPEC.md`), kept here so they can be tested without a GUI.

use num_complex::Complex64;
use std::f64::consts::PI;

/// Vector-diagram group scale (`0x490360`): `p = 10^⌊log10 max⌋`,
/// `scale = ⌈⌈max/p⌉/5⌉·5p`.
pub fn nice_scale(max: f64) -> f64 {
    if max.is_nan() || max <= 0.0 || !max.is_finite() {
        return 1.0;
    }
    let p = 10f64.powf(max.log10().floor());
    ((max / p).ceil() / 5.0).ceil() * 5.0 * p
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
/// −4 or at least 6, trailing zeros removed.
pub fn fmt_g(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if !v.is_finite() {
        return if v.is_nan() { "nan".into() } else if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let e = format!("{v:.5e}");
    let (mant, exp) = e.split_once('e').expect("exponent");
    let x: i32 = exp.parse().expect("exponent digits");
    let trim = |t: &str| -> String {
        if t.contains('.') {
            t.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            t.to_string()
        }
    };
    if (-4..6).contains(&x) {
        trim(&format!("{v:.*}", (5 - x) as usize))
    } else {
        format!("{}e{}{:02}", trim(mant), if x < 0 { '-' } else { '+' }, x.abs())
    }
}

/// The per-channel readout text (`0x461440`): `" %-8g %s"` with an SI
/// prefix on the unit.
pub fn readout_text(v: f64, unit: &str) -> String {
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

/// `v` scaled for display and its unit with the SI prefix, when the unit
/// takes one.
pub fn si_scaled(v: f64, unit: &str) -> (f64, String) {
    if si_unit(unit) {
        let (s, p) = crate::omp::si(v);
        (s, format!("{p}{unit}"))
    } else {
        (v, unit.to_string())
    }
}

/// The stats-panel value at В1 (`0x461440`): the sign, then `%-6g %s` of
/// |v|; the unit (prefixed) cut to 5 characters, "none" printed as nothing;
/// NaN is not drawn.
pub fn stats_value_text(v: f64, unit: &str) -> Option<String> {
    if v.is_nan() {
        return None;
    }
    let unit = if unit == "none" { "" } else { unit };
    let (s, u) = si_scaled(v.abs(), unit);
    let u: String = u.chars().take(5).collect();
    Some(format!("{}{:<6} {u}", if v < 0.0 { "-" } else { " " }, fmt_g(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_scales() {
        assert_eq!(nice_scale(37.0), 50.0);
        assert_eq!(nice_scale(57_000.0), 100_000.0);
        assert_eq!(nice_scale(4.2), 5.0);
        assert_eq!(nice_scale(0.0), 1.0);
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
        assert_eq!(fmt_g(0.00001234), "1.234e-05");
        assert_eq!(fmt_g(1234567.0), "1.23457e+06");
        assert_eq!(fmt_g(100.0), "100");
        assert_eq!(readout_text(57_100.0, "V"), " 57.1     кV");
        assert_eq!(readout_text(-57_100.0, "V"), "-57.1    кV");
        assert_eq!(readout_text(57_100.0, "kV"), " 57100    kV", "only listed units get a prefix");
        assert_eq!(readout_text(0.012, "ОM"), " 12       мОM");
        assert_eq!(stats_value_text(-23.4375, "A").unwrap(), "-23.4375 A");
        assert_eq!(stats_value_text(1.5, "none").unwrap(), " 1.5    ");
        assert_eq!(stats_value_text(2.0e5, "VA").unwrap(), " 200    кVA");
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

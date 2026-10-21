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
}

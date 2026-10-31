//! The original's sin / cos, bit for bit (#72): `fvb_core::trig` against msvcr120's own outputs.

use fvb_core::trig::{cos, sin, tan};

/// msvcr120's outputs for synthetic angles (see the file's header): every rounding tie of the
/// table index in the DFT's twiddle grid, and a sample of the rest.
fn fixture() -> Vec<(f64, f64, f64)> {
    let h = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
    include_str!("data/trig_msvcr120.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let p: Vec<&str> = l.split(' ').collect();
            (h(p[0]), h(p[1]), h(p[2]))
        })
        .collect()
}

/// 32·x/π as the code computes it; a tie of the table index when exactly k + ½.
fn is_tie(x: f64) -> bool {
    let y = x * (32.0 / std::f64::consts::PI);
    y - y.floor() == 0.5
}

#[test]
fn matches_msvcr120_on_the_twiddle_grid() {
    let rows = fixture();
    assert!(rows.len() > 1000);
    let (mut ties, mut bad) = (0, Vec::new());
    for &(x, s, c) in &rows {
        ties += usize::from(is_tie(x));
        if sin(x).to_bits() != s.to_bits() || cos(x).to_bits() != c.to_bits() {
            bad.push(format!("{:016x}: sin {:016x} vs {:016x}, cos {:016x} vs {:016x}", x.to_bits(), sin(x).to_bits(), s.to_bits(), cos(x).to_bits(), c.to_bits()));
        }
    }
    assert!(ties > 500, "the fixture holds the rounding ties ({ties})");
    assert!(bad.is_empty(), "{} differ from msvcr120, first: {:?}", bad.len(), &bad[..bad.len().min(3)]);
}

/// Intel's own code rounds a tie of the table index away from zero; msvcr120 goes to even. Among the
/// twiddle angles that changes one sine, the angle below (both x86-64 and the 32-bit assembly agree
/// with msvcr120 once ties go to even): Intel's gives ...5c9f, msvcr120's ...5ca0.
#[test]
fn a_tie_goes_to_even_not_away() {
    let x = f64::from_bits(0x402a_042d_2a3a_92ad);
    assert!(is_tie(x));
    assert_eq!(sin(x).to_bits(), 0x3fdb_5d10_09e1_5ca0);
    assert_eq!(cos(x).to_bits(), 0x3fec_ed7a_f43c_c77b);
}

#[test]
fn quarter_periods_and_specials() {
    use std::f64::consts::{FRAC_PI_2, PI};
    assert_eq!(sin(FRAC_PI_2), 1.0);
    assert_eq!(cos(PI), -1.0);
    assert_eq!(cos(0.0), 1.0);
    assert_eq!(sin(PI).to_bits(), 1.2246467991473532e-16_f64.to_bits());
    assert_eq!(cos(FRAC_PI_2).to_bits(), 6.123233995736766e-17_f64.to_bits());
    assert_eq!(sin(2.0 * PI).to_bits(), (-2.4492935982947064e-16_f64).to_bits());
    assert_eq!(cos(2.0 * PI), 1.0);
    // Signed zeros, tiny and subnormal inputs: sin(x) = x, cos(x) = 1.
    assert_eq!(sin(0.0).to_bits(), 0.0f64.to_bits());
    assert_eq!(sin(-0.0).to_bits(), (-0.0f64).to_bits());
    assert_eq!(cos(-0.0), 1.0);
    for x in [1e-300, -1e-300, 5e-324, -5e-324, 1e-20, -1e-20] {
        assert_eq!(sin(x).to_bits(), x.to_bits(), "sin({x:e})");
        assert_eq!(cos(x), 1.0, "cos({x:e})");
    }
    // Non-finite: NaN, as the original's runtime.
    for x in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert!(sin(x).is_nan() && cos(x).is_nan(), "{x}");
    }
    // From 90 112 on, std's.
    for x in [90_112.0, -90_112.5, 1e6, -1e9, 1e300] {
        assert_eq!(sin(x).to_bits(), x.sin().to_bits());
        assert_eq!(cos(x).to_bits(), x.cos().to_bits());
    }
}

/// The port is a correct sine and cosine (within an ulp of std's) over the main range.
#[test]
fn agrees_with_std_within_an_ulp() {
    let ulp = |a: f64, b: f64| (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs();
    let mut x = -300.0;
    while x < 300.0 {
        assert!(ulp(sin(x), x.sin()) <= 1 || (sin(x) - x.sin()).abs() < 1e-15, "sin({x})");
        assert!(ulp(cos(x), x.cos()) <= 1 || (cos(x) - x.cos()).abs() < 1e-15, "cos({x})");
        x += 0.0137;
    }
}

/// #108: the crate contains Intel's BSD-3-Clause code (`fvb_core::trig`), so its licence field says so.
#[test]
fn the_licence_field_names_intels_bsd_code() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.lines().any(|l| l.trim() == r#"license = "(MIT OR Apache-2.0) AND BSD-3-Clause""#), "{manifest}");
}

// ---------------------------------------------------------------------------------------------
// tan (#107): the original's `tg:` / `ctg:` call msvcr120's `_libm_sse2_tan_precise` (0x4c1388).

fn tan_fixture() -> Vec<(f64, f64)> {
    let h = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
    include_str!("data/tan_msvcr120.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let p: Vec<&str> = l.split(' ').collect();
            (h(p[0]), h(p[1]))
        })
        .collect()
}

#[test]
fn tan_matches_msvcr120() {
    let rows = tan_fixture();
    assert!(rows.len() > 1000);
    let (mut bad, mut std_differs) = (Vec::new(), 0);
    for &(x, t) in &rows {
        assert!(x.abs() < 12_800.0);
        std_differs += usize::from(x.tan().to_bits() != t.to_bits());
        if tan(x).to_bits() != t.to_bits() {
            bad.push(format!("{:016x}: {:016x} vs {:016x}", x.to_bits(), tan(x).to_bits(), t.to_bits()));
        }
    }
    assert!(std_differs > 100, "the fixture tells the port from std ({std_differs})");
    assert!(bad.is_empty(), "{} differ from msvcr120, first: {:?}", bad.len(), &bad[..bad.len().min(3)]);
}

/// Intel's own code rounds a tie of the π/32 index (x·32/π exactly k + ½ in doubles) away from zero;
/// msvcr120 goes to even. Intel's gives ...8df6 here, msvcr120's ...8df5.
#[test]
fn tan_tie_goes_to_even_not_away() {
    let x = f64::from_bits(0x4099_31e2_d54f_803d);
    let y = x * (32.0 / std::f64::consts::PI);
    assert_eq!(y - y.floor(), 0.5);
    assert_eq!(tan(x).to_bits(), 0x3ff1_a73d_5527_8df5);
    let x = f64::from_bits(0xc09e_474a_7372_ea7c);
    assert_eq!(tan(x).to_bits(), 0x3fe3_2e18_8904_7eac);
}

#[test]
fn tan_specials_and_range() {
    use std::f64::consts::{FRAC_PI_4, PI};
    assert_eq!(tan(FRAC_PI_4).to_bits(), 0.9999999999999999_f64.to_bits());
    assert_eq!(tan(0.0).to_bits(), 0.0f64.to_bits());
    assert_eq!(tan(-0.0).to_bits(), (-0.0f64).to_bits());
    assert_eq!(tan(PI).to_bits(), (-1.2246467991473532e-16_f64).to_bits());
    // Tiny and subnormal: tan(x) = x.
    for x in [1e-300, -1e-300, 5e-324, -5e-324, 1e-30, -1e-30] {
        assert_eq!(tan(x).to_bits(), x.to_bits(), "tan({x:e})");
    }
    for x in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert!(tan(x).is_nan(), "{x}");
    }
    // From 12 800 on, std's (the original's own reduction differs from Intel's there).
    for x in [12_800.0, -12_800.5, 1e5, -1e9, 1e300] {
        assert_eq!(tan(x).to_bits(), x.tan().to_bits());
    }
}

/// The port is a correct tangent (within a few ulps of std's) over the main range.
#[test]
fn tan_is_accurate() {
    for i in -20_000..=20_000 {
        let x = f64::from(i) * 0.6137;
        let (a, b) = (tan(x), x.tan());
        assert!((a - b).abs() <= 4.0 * f64::EPSILON * b.abs().max(f64::MIN_POSITIVE), "tan({x}) = {a} vs {b}");
    }
}

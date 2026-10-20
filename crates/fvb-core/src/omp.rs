//! Fault location (ОМП — определение места повреждения).
//!
//! Single-ended (одностороннего замера) uses the reactance method: the fault
//! loop reactance seen from one terminal, divided by the line reactance per unit
//! length, gives the distance. Following the help's derivation, the arc (fault)
//! resistance is purely resistive and carried by a current co-directional with
//! the fault current, so taking the imaginary part of `U/I` cancels it.
//!
//! Double-ended (двустороннего замера) uses measurements from both terminals and
//! the total line impedance, removing the infeed/arc dependence entirely.

use num_complex::Complex64;

/// Per-unit-length positive-sequence line parameters (secondary ohms per km).
#[derive(Debug, Clone, Copy)]
pub struct LineParams {
    pub r1_per_km: f64,
    pub x1_per_km: f64,
    pub length_km: f64,
}

/// Single-ended reactance-method distance to fault, in km.
///
/// `u_loop`, `i_loop` are the faulted-loop phasors at the measuring terminal.
/// The reactance to the fault is `Im(U/I)`; distance = that / `x1_per_km`.
pub fn single_ended_distance(u_loop: Complex64, i_loop: Complex64, line: &LineParams) -> Option<f64> {
    if i_loop.norm() < 1e-9 || line.x1_per_km.abs() < 1e-12 {
        return None;
    }
    let z = u_loop / i_loop;
    let x_fault = z.im;
    Some(x_fault / line.x1_per_km)
}

/// Fraction (0..1) of the line length to the fault, single-ended.
pub fn single_ended_fraction(u_loop: Complex64, i_loop: Complex64, line: &LineParams) -> Option<f64> {
    single_ended_distance(u_loop, i_loop, line).map(|d| {
        if line.length_km > 0.0 {
            d / line.length_km
        } else {
            d
        }
    })
}

/// Double-ended distance from the local terminal, in km.
///
/// Given the faulted-loop voltage/current phasors at both line ends (`m` = local,
/// `n` = remote) and the total line impedance `z_line`, the per-unit distance `m`
/// from the local end solves the two-terminal voltage-balance relation. A common
/// synchronised-measurement estimator is:
///
/// ```text
///   m = Im( (U_m − U_n + Z_line·I_n) / (Z_line·(I_m + I_n)) )
/// ```
///
/// which is independent of fault resistance. Returns distance in km.
pub fn double_ended_distance(
    u_m: Complex64,
    i_m: Complex64,
    u_n: Complex64,
    i_n: Complex64,
    line: &LineParams,
) -> Option<f64> {
    let z_line = Complex64::new(
        line.r1_per_km * line.length_km,
        line.x1_per_km * line.length_km,
    );
    let denom = z_line * (i_m + i_n);
    if denom.norm() < 1e-12 {
        return None;
    }
    let m = ((u_m - u_n + z_line * i_n) / denom).re;
    Some(m.clamp(0.0, 1.0) * line.length_km)
}

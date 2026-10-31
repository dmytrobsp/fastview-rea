//! The sin / cos / tan of the signal maths and the formulas, bit for bit as the original's runtime computes them (#72, #107).
//!
//! The original's DFT, phasors, RMS and harmonics call the C runtime's `sin` / `cos` (msvcr120's
//! `_libm_sse2_sin_precise` / `_libm_sse2_cos_precise`). Those differ in the last bit from glibc's
//! and from mingw's on a few tenths of a percent of angles, which flips printed digits (a flat
//! channel's harmonic labels especially). This module computes the same doubles on every platform.
//!
//! It is a Rust transliteration of Intel's SSE2 `sin` / `cos` for x86 (bionic `libm/x86/s_sin.S`,
//! `s_cos.S`), main path only, with one change: Intel adds ±0.5 to 32·x/π and truncates (ties away
//! from zero), msvcr120 rounds to nearest with ties to even; the two differ only where 32·x/π is
//! exactly k + ½. With ties to even the results match msvcr120's on every angle measured below
//! 90 112 (about 896 000: the DFT's twiddle grid, random angles, constructed ties, ±0, tiny
//! values, ±inf, NaN). The tie rule was inferred from msvcr120's outputs only. From |x| ≥ 90 112
//! msvcr120's own reduction is less accurate and matches nothing else, so those arguments, and
//! non-finite ones, fall back to `std`. Plain Rust never contracts into FMA, so the result is the
//! same on every target.
//!
//! `tan` (the formulas' `tg:` / `ctg:`, msvcr120's `_libm_sse2_tan_precise`) is the same kind of port of
//! `s_tan.S`: its main path serves |x| < 12 800, where it matches msvcr120 on every angle measured
//! (about 3.4 million: degree grids, random angles, the poles' neighbourhoods, and 1.08 million
//! angles where either of its two table indices, x·2/π and x·32/π, is exactly k + ½ in doubles), once
//! both indices round to nearest, ties to even; Intel's ties away from zero differ at 610 of those.
//! From 12 800 msvcr120's reduction matches nothing else, so those arguments fall back to `std`.
//!
//! The tables and the sequence of operations are Intel's. Their notice, verbatim:
//!
//! ```text
//!     Copyright (c) 2014, Intel Corporation
//!     All rights reserved.
//!
//!     Redistribution and use in source and binary forms, with or without
//!     modification, are permitted provided that the following conditions are met:
//!
//!         * Redistributions of source code must retain the above copyright notice,
//!         * this list of conditions and the following disclaimer.
//!
//!         * Redistributions in binary form must reproduce the above copyright notice,
//!         * this list of conditions and the following disclaimer in the documentation
//!         * and/or other materials provided with the distribution.
//!
//!         * Neither the name of Intel Corporation nor the names of its contributors
//!         * may be used to endorse or promote products derived from this software
//!         * without specific prior written permission.
//!
//!     THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
//!     ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
//!     WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//!     DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR
//!     ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//!     (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
//!     LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON
//!     ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
//!     (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
//!     SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//! ```

mod tables;
use tables::{COS_TAB, SIN_TAB, TAN_TAB};

#[inline(always)]
fn t(tab: &[u64], off: usize) -> f64 {
    f64::from_bits(tab[off / 8])
}

/// Intel's main path (2⁻²⁵² ≤ |x| < 90 112) over the table `tab`; `quarter` is 0 for sin, 16 for
/// cos. The xmm registers of the assembly are kept as `[low, high]` pairs, so every operation
/// happens in the assembly's order.
#[inline(always)]
fn main_path(tab: &[u64], x: f64, quarter: i32) -> f64 {
    let mut x0 = [x, 0.0];
    let mut x1 = [t(tab, 2160) * x, 0.0];
    let mut x3 = [t(tab, 2128), 0.0];
    let mut x2 = [t(tab, 2112), t(tab, 2120)];
    // The table index: the nearest integer to 32·x/π, ties to even (the 1.5·2⁵² shifter; msvcr120).
    let sh = t(tab, 2176);
    let mut n = ((x1[0] + sh) - sh) as i32;
    x1[0] = n as f64;
    x3[0] *= x1[0];
    x1 = [x1[0], x1[0]];
    n = n.wrapping_add(1865216 + quarter) & 63;
    let e = (n as usize) * 32;
    let mut x4 = x0;
    let mut x5 = [t(tab, 2096), t(tab, 2104)];
    x2 = [x2[0] * x1[0], x2[1] * x1[1]];
    x0[0] -= x3[0];
    x1[0] *= t(tab, 2144);
    x4[0] -= x3[0];
    let mut x7 = [t(tab, e + 8), 0.0];
    x0 = [x0[0], x0[0]];
    x3 = x4;
    x4[0] -= x2[0];
    x5 = [x5[0] * x0[0], x5[1] * x0[1]];
    x0 = [x0[0] - x2[0], x0[1] - x2[1]];
    let mut x6 = [t(tab, 2064), t(tab, 2072)];
    x7[0] *= x4[0];
    x3[0] -= x4[0];
    x5 = [x5[0] * x0[0], x5[1] * x0[1]];
    x0 = [x0[0] * x0[0], x0[1] * x0[1]];
    x3[0] -= x2[0];
    x2 = [t(tab, e), t(tab, e + 8)];
    x1[0] -= x3[0];
    x3 = [t(tab, e + 24), 0.0];
    x2[0] += x3[0];
    x7[0] -= x2[0];
    x2[0] *= x4[0];
    x6 = [x6[0] * x0[0], x6[1] * x0[1]];
    x3[0] *= x4[0];
    x2 = [x2[0] * x0[0], x2[1] * x0[1]];
    x0 = [x0[0] * x0[0], x0[1] * x0[1]];
    x5 = [x5[0] + t(tab, 2080), x5[1] + t(tab, 2088)];
    x4[0] *= t(tab, e);
    x6 = [x6[0] + t(tab, 2048), x6[1] + t(tab, 2056)];
    x5 = [x5[0] * x0[0], x5[1] * x0[1]];
    x0 = x3;
    x3[0] += t(tab, e + 8);
    x1 = [x1[0] * x7[0], x1[1] * x7[1]];
    x7 = x4;
    x4[0] += x3[0];
    x6 = [x6[0] + x5[0], x6[1] + x5[1]];
    x5 = [t(tab, e + 8), 0.0];
    x5[0] -= x3[0];
    x3[0] -= x4[0];
    x1[0] += t(tab, e + 16);
    x6 = [x6[0] * x2[0], x6[1] * x2[1]];
    x5[0] += x0[0];
    x3[0] += x7[0];
    x1[0] += x5[0];
    x1[0] += x3[0];
    x1[0] += x6[0];
    x1[0] += x6[1];
    x4[0] += x1[0];
    x4[0]
}

#[inline(always)]
fn top(x: f64) -> u32 {
    ((x.to_bits() >> 48) as u32 & 0x7fff).wrapping_sub(12336)
}

/// `sin(x)` as msvcr120 computes it (std's beyond |x| ≥ 90 112 and for non-finite `x`).
pub fn sin(x: f64) -> f64 {
    let k = top(x);
    if k <= 4293 {
        return main_path(&SIN_TAB, x, 0);
    }
    if (k as i32) > 4293 {
        if x.is_finite() {
            return x.sin();
        }
        return x * t(&SIN_TAB, 2240);
    }
    if (k >> 4) == 268434685 {
        return x;
    }
    (x * t(&SIN_TAB, 2192) - x) * t(&SIN_TAB, 2208)
}

/// `cos(x)` as msvcr120 computes it (std's beyond |x| ≥ 90 112 and for non-finite `x`).
pub fn cos(x: f64) -> f64 {
    let k = top(x);
    if k <= 4293 {
        return main_path(&COS_TAB, x, 16);
    }
    if (k as i32) > 4293 {
        if x.is_finite() {
            return x.cos();
        }
        return x * t(&COS_TAB, 2208);
    }
    t(&COS_TAB, 2192) - f64::from_bits(x.to_bits() & 0x7fff_ffff_ffff_ffff)
}

/// Intel's main path of `tan` (2⁻¹²⁵ ≲ |x| < 12 800) over the table `tab`; the xmm registers of the
/// assembly are kept as `[low, high]` pairs, every operation in the assembly's order. Lane 0 of the
/// reduction works modulo π/2 (for the reciprocal), lane 1 modulo π/32 (for the polynomial).
#[inline(always)]
fn tan_main(tab: &[u64], x: f64) -> f64 {
    #[inline(always)]
    fn band(a: f64, b: f64) -> f64 {
        f64::from_bits(a.to_bits() & b.to_bits())
    }
    let (mut x1, mut x2, mut x3, mut x4, mut x5, mut x7);
    let mut x6 = [t(tab, 5856), t(tab, 5864)];
    let mut x0 = [x, x];
    // The two table indices: lane 0 counts quarter periods (x·2/π), lane 1 counts π/32 steps
    // (x·32/π). Intel adds ±0.5 and truncates (ties away from zero); msvcr120 rounds to nearest,
    // ties to even (the 1.5·2⁵² shifter, which sits in Intel's table at 5656).
    let sh = t(tab, 5656);
    let n0 = (((x * t(tab, 5632)) + sh) - sh) as i32;
    let n1 = (((x * t(tab, 5640)) + sh) - sh) as i32;
    x1 = [f64::from(n0), f64::from(n1)];
    let e = (n1.wrapping_add(469248) & 31) as usize * 176;
    x1 = [x1[0] * x6[0], x1[1] * x6[1]];
    x3 = [t(tab, 5664), t(tab, 5672)];
    x5 = [t(tab, 5728), 0.0];
    x4 = [t(tab, 5680), t(tab, 5688)];
    x3 = [x3[0] * x1[0], x3[1] * x1[1]];
    x5[0] *= x1[0];
    x4 = [x4[0] * x1[0], x4[1] * x1[1]];
    x0 = [x0[0] - x3[0], x0[1] - x3[1]];
    { let m = [t(tab, 5696), t(tab, 5704)]; x1 = [x1[0] * m[0], x1[1] * m[1]]; }
    x5[0] += x0[0];
    x2 = x0;
    x0 = [x0[0] - x4[0], x0[1] - x4[1]];
    x6 = [t(tab, 5744), 0.0];
    { let m = [t(tab, 5776), t(tab, 5784)]; x5 = [band(x5[0], m[0]), band(x5[1], m[1])]; }
    x3 = x0;
    x2 = [x2[0] - x0[0], x2[1] - x0[1]];
    x0 = [x0[1], x0[1]];
    x6[0] /= x5[0];
    x2 = [x2[0] - x4[0], x2[1] - x4[1]];
    x7 = [t(tab, e + 16), t(tab, e + 24)];
    x3[0] -= x5[0];
    x7 = [x7[0] * x0[0], x7[1] * x0[1]];
    x2 = [x2[0] - x1[0], x2[1] - x1[1]];
    x1 = [t(tab, e + 48), t(tab, e + 56)];
    x1 = [x1[0] * x0[0], x1[1] * x0[1]];
    x4 = [t(tab, e + 96), t(tab, e + 104)];
    x4 = [x4[0] * x0[0], x4[1] * x0[1]];
    x2[0] += x3[0];
    x3 = x0;
    x0 = [x0[0] * x0[0], x0[1] * x0[1]];
    { let m = [t(tab, e), t(tab, e + 8)]; x7 = [x7[0] + m[0], x7[1] + m[1]]; }
    { let m = [t(tab, e + 32), t(tab, e + 40)]; x1 = [x1[0] + m[0], x1[1] + m[1]]; }
    x1 = [x1[0] * x0[0], x1[1] * x0[1]];
    { let m = [t(tab, e + 80), t(tab, e + 88)]; x4 = [x4[0] + m[0], x4[1] + m[1]]; }
    x7 = [x7[0] + x1[0], x7[1] + x1[1]];
    x1 = [t(tab, e + 112), t(tab, e + 120)];
    x1 = [x1[0] * x0[0], x1[1] * x0[1]];
    x0 = [x0[0] * x0[0], x0[1] * x0[1]];
    x4 = [x4[0] + x1[0], x4[1] + x1[1]];
    x1 = [t(tab, e + 64), t(tab, e + 72)];
    x1 = [x1[0] * x0[0], x1[1] * x0[1]];
    x7 = [x7[0] + x1[0], x7[1] + x1[1]];
    x1 = x3;
    x3 = [x3[0] * x0[0], x3[1] * x0[1]];
    x0[0] *= x0[0];
    { let m = [t(tab, e + 144), t(tab, e + 152)]; x1 = [x1[0] * m[0], x1[1] * m[1]]; }
    x4 = [x4[0] * x3[0], x4[1] * x3[1]];
    x3 = x1;
    x7 = [x7[0] + x4[0], x7[1] + x4[1]];
    x4 = x1;
    x0[0] *= x7[0];
    x7 = [x7[1], x7[1]];
    x0[0] += x7[0];
    x1 = [x1[1], x1[1]];
    x3[0] += x1[0];
    x4[0] -= x3[0];
    x1[0] += x4[0];
    x4 = x2;
    x7 = [t(tab, e + 144), 0.0];
    x2 = [x2[1], x2[1]];
    x7[0] += t(tab, e + 152);
    x7[0] *= x2[0];
    x7[0] += t(tab, e + 136);
    x7[0] += x1[0];
    x0[0] += x7[0];
    x7 = [t(tab, 5744), 0.0];
    x4[0] *= x6[0];
    x2 = [t(tab, e + 168), 0.0];
    x2 = [band(x2[0], x6[0]), band(x2[1], x6[1])];
    x5[0] *= x2[0];
    x6[0] *= t(tab, e + 160);
    x7[0] -= x5[0];
    x2[0] -= t(tab, e + 128);
    x7[0] -= x4[0];
    x7[0] *= x6[0];
    x4 = x3;
    x3[0] -= x2[0];
    x2[0] += x3[0];
    x4[0] -= x2[0];
    x0[0] += x4[0];
    x0[0] -= x7[0];
    x0[0] += x3[0];
    x0[0]
}

/// `tan(x)` as msvcr120 computes it (std's from |x| ≥ 12 800 and for non-finite `x`).
pub fn tan(x: f64) -> f64 {
    let k = ((x.to_bits() >> 48) as u32 & 0x7fff).wrapping_sub(14368);
    if k <= 2216 {
        return tan_main(&TAN_TAB, x);
    }
    if (k as i32) > 2216 {
        if x.is_finite() {
            return x.tan();
        }
        // inf − inf: x86's default NaN (a NaN argument stays as it is).
        return if x.is_nan() { x } else { f64::from_bits(0xfff8_0000_0000_0000) };
    }
    ((t(&TAN_TAB, 5792) * x) + x) * t(&TAN_TAB, 5808)
}

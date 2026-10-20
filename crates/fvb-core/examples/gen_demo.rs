//! Generate a synthetic COMTRADE A-phase-to-ground fault record for demoing and
//! validating the oscillogram/analysis/overlay features.
//!
//! Usage: `cargo run -p fvb-core --example gen_demo -- <out_stem>`
//! writes `<out_stem>.cfg` and `<out_stem>.dat` (ASCII).

use std::f64::consts::PI;
use std::io::Write;

const FS: f64 = 2000.0;
const F0: f64 = 50.0;
const CYCLES: usize = 12;

fn main() {
    let stem = std::env::args().nth(1).unwrap_or_else(|| "demo_fault".into());
    let n = (FS / F0) as usize * CYCLES;
    let fault_on = (FS / F0) as usize * 3; // fault at cycle 3
    let fault_off = (FS / F0) as usize * 9; // clears at cycle 9

    // Secondary nominal: 57.74 V phase, load current 1 A at -20°.
    // During fault on phase A: Ua collapses toward the fault, Ia surges and its
    // angle rotates toward the line angle (~ -75°), so the A-phase impedance
    // locus drops into the first quadrant near the reach.
    let up = 57.74 * 2f64.sqrt();
    let load = 1.0 * 2f64.sqrt();
    let fault_i = 8.0 * 2f64.sqrt();

    let mut ua = Vec::new();
    let mut ub = Vec::new();
    let mut uc = Vec::new();
    let mut ia = Vec::new();
    let mut ib = Vec::new();
    let mut ic = Vec::new();
    let mut trip = Vec::new();

    for i in 0..n {
        let t = i as f64 / FS;
        let w = 2.0 * PI * F0 * t;
        let faulted = i >= fault_on && i < fault_off;
        // voltages
        let (uam, uai) = if faulted { (up * 0.35, 0.0) } else { (up, 0.0) };
        ua.push(uam * (w + uai).sin());
        ub.push(up * (w - 120f64.to_radians()).sin());
        uc.push(up * (w + 120f64.to_radians()).sin());
        // currents
        let (iam, iai) = if faulted {
            (fault_i, (-75f64).to_radians())
        } else {
            (load, (-20f64).to_radians())
        };
        ia.push(iam * (w + iai).sin());
        ib.push(load * (w - 120f64.to_radians() - 20f64.to_radians()).sin());
        ic.push(load * (w + 120f64.to_radians() - 20f64.to_radians()).sin());
        trip.push(if i >= fault_on + 10 && faulted { 1 } else { 0 });
    }

    // .cfg (1999, ASCII)
    let mut cfg = String::new();
    cfg.push_str("НТЦ Механотроника (demo),BMRZ-demo,1999\r\n");
    cfg.push_str("7,6A,1D\r\n");
    let analog = [
        ("Ua", "A", "V"),
        ("Ub", "B", "V"),
        ("Uc", "C", "V"),
        ("Ia", "A", "A"),
        ("Ib", "B", "A"),
        ("Ic", "C", "A"),
    ];
    // store as raw floats with a=1,b=0 (ASCII keeps full precision)
    for (k, (id, ph, uu)) in analog.iter().enumerate() {
        cfg.push_str(&format!(
            "{},{},{},,{},1.0,0.0,0.0,-1000000,1000000,1.0,1.0,S\r\n",
            k + 1,
            id,
            ph,
            uu
        ));
    }
    cfg.push_str("1,Trip,,,0\r\n");
    cfg.push_str("50\r\n1\r\n");
    cfg.push_str(&format!("{},{}\r\n", FS as u64, n));
    cfg.push_str("01/01/2024,00:00:00.000000\r\n");
    cfg.push_str("01/01/2024,00:00:00.030000\r\n");
    cfg.push_str("ASCII\r\n1\r\n");
    std::fs::write(format!("{stem}.cfg"), cfg).unwrap();

    // .dat
    let mut f = std::fs::File::create(format!("{stem}.dat")).unwrap();
    for i in 0..n {
        let ts = (i as f64 / FS * 1e6).round() as u64; // microseconds
        writeln!(
            f,
            "{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{}",
            i + 1,
            ts,
            ua[i],
            ub[i],
            uc[i],
            ia[i],
            ib[i],
            ic[i],
            trip[i]
        )
        .unwrap();
    }
    eprintln!("wrote {stem}.cfg and {stem}.dat ({n} samples)");
}

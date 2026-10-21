//! "Быстрый анализ" — the original's quick-analysis property sheet
//! (`CQAnPropSheet`, ledger L8, `notes/VIEWS_SPEC.md`) is a *channel
//! generator*: the active tab turns its controls into a list of computed
//! channels (name + formula in the formula language), plus which of them to
//! toggle on the vector / circle diagrams, a vector-diagram angle reference
//! and (Iдиф, Iторм) pairs for the differential diagram. The channels are then
//! appended and evaluated like any computed channel.
//!
//! Formulas reference channels by quoted name. Like the original, a formula
//! names the channels of the same batch by their plain names ("3I0"), even
//! when appending gives the new channel a "_2" suffix.

/// What one tab produces.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Generated {
    /// (name, formula, unit) in evaluation order.
    pub channels: Vec<(String, String, String)>,
    /// Names whose vector-diagram flag is toggled (XOR).
    pub vector: Vec<String>,
    /// Names whose circle-diagram flag is toggled (XOR).
    pub circle: Vec<String>,
    /// Vector-diagram angle reference channel.
    pub angle_ref: Option<String>,
    /// Vector-diagram "show values" switched on.
    pub show_values: bool,
    /// Circle readout: 1 = Re ± jIm, 2 = |·|∠.
    pub circle_readout: Option<u8>,
    /// (Iдиф, Iторм) channel pairs for the differential diagram.
    pub dif_pairs: Vec<(String, String)>,
}

impl Generated {
    fn add(&mut self, name: &str, formula: String) {
        self.channels.push((name.to_string(), formula, String::new()));
    }
}

fn q(name: &str) -> String {
    format!("\"{name}\"")
}

/// A numeric edit field as the original passes it: "," becomes ".".
pub fn num(field: &str) -> String {
    let t = field.trim().replace(',', ".");
    if t.is_empty() {
        "0".into()
    } else {
        t
    }
}

/// Voltage inputs: phase voltages or line voltages.
#[derive(Debug, Clone, PartialEq)]
pub enum Voltages {
    Phase { ua: String, ub: String, uc: String },
    Line { uab: String, ubc: String },
}

/// Three phase currents, optionally computing Ib = −(Ia + Ic).
#[derive(Debug, Clone, PartialEq)]
pub struct Currents {
    pub ia: String,
    pub ib: String,
    pub ic: String,
    pub calc_ib: bool,
}

impl Currents {
    /// Emit "Ib" if computed; return the name to use for phase B.
    fn ib(&self, g: &mut Generated) -> String {
        if self.calc_ib {
            g.add("Ib", format!("Ib:({},{},0)", q(&self.ia), q(&self.ic)));
            "Ib".into()
        } else {
            self.ib.clone()
        }
    }
}

/// Симметричные составляющие (`0x48c2b0`).
pub fn symmetric(i: Option<&Currents>, u: Option<&Voltages>) -> Generated {
    let mut g = Generated::default();
    if let Some(c) = i {
        let ib = c.ib(&mut g);
        let args = format!("{},{},{}", q(&c.ia), q(&ib), q(&c.ic));
        g.add("I1", format!("I1:({args})"));
        g.add("I2", format!("I2:({args})"));
        if !c.calc_ib {
            g.add("3I0", format!("3I0:({args})"));
        }
    }
    match u {
        Some(Voltages::Phase { ua, ub, uc }) => {
            let args = format!("{},{},{}", q(ua), q(ub), q(uc));
            g.add("U1", format!("U1f:({args})"));
            g.add("U2", format!("U2f:({args})"));
            g.add("3U0", format!("3U0:({args})"));
        }
        Some(Voltages::Line { uab, ubc }) => {
            let args = format!("{},{}", q(uab), q(ubc));
            g.add("U1", format!("U1l:({args})"));
            g.add("U2", format!("U2l:({args})"));
        }
        None => {}
    }
    g
}

/// Zero-sequence current source.
#[derive(Debug, Clone, PartialEq)]
pub enum ZeroCurrent {
    Calculated { ia: String, ib: String, ic: String },
    Measured(String),
}

/// Zero-sequence voltage source.
#[derive(Debug, Clone, PartialEq)]
pub enum ZeroVoltage {
    FromPhases { ua: String, ub: String, uc: String },
    FromUniUik { uni: String, uik: String },
    Measured(String),
}

/// Нулевая последовательность (`0x4887c0`).
pub fn zero(i: Option<&ZeroCurrent>, u: Option<&ZeroVoltage>) -> Generated {
    let mut g = Generated::default();
    match i {
        Some(ZeroCurrent::Calculated { ia, ib, ic }) => g.add("3I0", format!("3I0:({},{},{})", q(ia), q(ib), q(ic))),
        Some(ZeroCurrent::Measured(ch)) => g.add("3I0", format!("Chan:({})", q(ch))),
        None => {}
    }
    match u {
        Some(ZeroVoltage::FromPhases { ua, ub, uc }) => g.add("3U0", format!("3U0:({},{},{})", q(ua), q(ub), q(uc))),
        Some(ZeroVoltage::FromUniUik { uni, uik }) => g.add("3U0", format!("Uniik:({},{})", q(uni), q(uik))),
        Some(ZeroVoltage::Measured(ch)) => g.add("3U0", format!("Chan:({})", q(ch))),
        None => {}
    }
    if i.is_some() {
        g.vector.push("3I0".into());
    }
    if u.is_some() {
        g.vector.push("3U0".into());
        g.angle_ref = Some("3U0".into());
    }
    if i.is_some() && u.is_some() {
        g.channels.push(("S0".into(), format!("Mod:(Pf:({},{}))", q("3I0"), q("3U0")), "VA".into()));
        g.circle.push("S0".into());
        g.circle_readout = Some(1);
    }
    g.show_values = i.is_some() || u.is_some();
    g
}

/// Междуфазные сопротивления (`0x487640`).
pub fn phase_phase(i: &Currents, u: &Voltages, itr: &str) -> Generated {
    let mut g = Generated::default();
    let ib = i.ib(&mut g);
    let (uab, ubc, uca) = match u {
        Voltages::Phase { ua, ub, uc } => {
            g.add("Uab", format!("Ulf:({},{})", q(ua), q(ub)));
            g.add("Ubc", format!("Ulf:({},{})", q(ub), q(uc)));
            g.add("Uca", format!("Ulf:({},{})", q(uc), q(ua)));
            ("Uab".to_string(), "Ubc".to_string(), "Uca".to_string())
        }
        Voltages::Line { uab, ubc } => {
            g.add("Uca", format!("Ull:({},{})", q(uab), q(ubc)));
            (uab.clone(), ubc.clone(), "Uca".to_string())
        }
    };
    let itr = num(itr);
    for (name, uu, i1, i2) in [("Zab", &uab, &i.ia, &ib), ("Zbc", &ubc, &ib, &i.ic), ("Zca", &uca, &i.ic, &i.ia)] {
        g.add(name, format!("Mod:(Zl:({},{},{},{itr}))", q(uu), q(i1), q(i2)));
    }
    if matches!(u, Voltages::Phase { .. }) {
        g.circle = vec!["Zab".into(), "Zbc".into(), "Zca".into()];
    }
    g.circle_readout = Some(2);
    g
}

/// Compensation parameters of the phase-impedance tab.
#[derive(Debug, Clone, PartialEq)]
pub enum PhaseComp {
    /// По удельным параметрам: R0 R1 Rm X0 X1 Xm.
    PerUnit { r0: String, r1: String, rm: String, x0: String, x1: String, xm: String },
    /// По коэффициентам компенсации: K0 Km Ф0 Фm.
    Coefficients { k0: String, km: String, f0: String, fm: String },
}

/// Фазные сопротивления (`0x48a4c0`).
pub fn phase_z(i: &Currents, u: [&str; 3], i0_parallel: &str, comp: &PhaseComp, itr: &str) -> Generated {
    let mut g = Generated::default();
    let ib = i.ib(&mut g);
    g.add("3I0", format!("3I0:({},{},{})", q(&i.ia), q(&ib), q(&i.ic)));
    let i0n = if i0_parallel.trim().is_empty() { "0".to_string() } else { q(i0_parallel) };
    let itr = num(itr);
    for (name, uu, ii) in [("Za", u[0], &i.ia), ("Zb", u[1], &ib), ("Zc", u[2], &i.ic)] {
        let f = match comp {
            PhaseComp::PerUnit { r0, r1, rm, x0, x1, xm } => format!(
                "Mod:(Zf:({},{},{},{i0n},{},{},{},{},{},{},{itr}))",
                q(uu),
                q(ii),
                q("3I0"),
                num(r0),
                num(r1),
                num(rm),
                num(x0),
                num(x1),
                num(xm)
            ),
            PhaseComp::Coefficients { k0, km, f0, fm } => format!(
                "Mod:(Zk:({},{},{},{i0n},{},{},{},{},{itr}))",
                q(uu),
                q(ii),
                q("3I0"),
                num(k0),
                num(km),
                num(f0),
                num(fm)
            ),
        };
        g.add(name, f);
    }
    g.circle = vec!["Za".into(), "Zb".into(), "Zc".into()];
    g.circle_readout = Some(2);
    g
}

/// How an arm's currents are reduced to the base side.
#[derive(Debug, Clone, PartialEq)]
pub enum Reduction {
    Priv { k: String },
    Tt { ktt: String, ktt_base: String },
    PowerPriv { ktt: String, snom: String, unom: String },
    AmpPriv { ktt: String, inom: String },
}

/// Winding-group compensation (three-phase only).
#[derive(Debug, Clone, PartialEq)]
pub enum GroupComp {
    None,
    Posl { group: String, remove_i0: bool },
    Triangle { group: String },
}

/// Braking-current method.
#[derive(Debug, Clone, PartialEq)]
pub enum Braking {
    /// Полусумма модулей (default).
    Sum { k: String },
    /// Arms 1 and 2.
    Sub { k: String },
    Max,
}

/// One arm (плечо): its phase current channels and reduction.
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub currents: Vec<String>,
    pub reduction: Reduction,
    pub group: GroupComp,
}

/// Дифференциальные токи (`0x47ff60`): reduced arm currents, optional group
/// compensation, then per phase the differential (vector sum, 24 arm slots)
/// and braking current; pairs go to the differential diagram. The exact
/// channel naming and the order of `CoeffTT`'s arguments are not determined.
pub fn differential(phases: usize, arms: &[Arm], braking: &Braking) -> Generated {
    let mut g = Generated::default();
    let letters: Vec<&str> = if phases == 3 { vec!["a", "b", "c"] } else { vec![""] };
    // reduced[arm][phase] = channel name to use downstream.
    let mut reduced: Vec<Vec<String>> = Vec::new();
    for (k, arm) in arms.iter().enumerate() {
        let mut names = Vec::new();
        for (p, letter) in letters.iter().enumerate() {
            let src = arm.currents.get(p).cloned().unwrap_or_default();
            let name = format!("I{letter} плечо {} прив.", k + 1);
            let f = match &arm.reduction {
                Reduction::Priv { k } => format!("CoeffPriv:({},{})", q(&src), num(k)),
                Reduction::Tt { ktt, ktt_base } => format!("CoeffTT:({},{},{})", q(&src), num(ktt), num(ktt_base)),
                Reduction::PowerPriv { ktt, snom, unom } => {
                    format!("CoeffPowerPriv:({},{},{},{})", q(&src), num(ktt), num(snom), num(unom))
                }
                Reduction::AmpPriv { ktt, inom } => format!("CoeffAmpPriv:({},{},{})", q(&src), num(ktt), num(inom)),
            };
            g.add(&name, f);
            names.push(name);
        }
        if phases == 3 && arm.group != GroupComp::None {
            let args = format!("{},{},{}", q(&names[0]), q(&names[1]), q(&names[2]));
            let mut turned = Vec::new();
            for letter in &letters {
                let name = format!("I{letter} плечо {} прив. пов.", k + 1);
                let f = match &arm.group {
                    GroupComp::Posl { group, remove_i0 } => {
                        format!("GrsPosl{letter}:({args},{},{})", num(group), if *remove_i0 { 0 } else { 1 })
                    }
                    GroupComp::Triangle { group } => format!("GrsTriangle{letter}:({args},{})", num(group)),
                    GroupComp::None => unreachable!(),
                };
                g.add(&name, f);
                turned.push(name);
            }
            names = turned;
        }
        reduced.push(names);
    }
    for (p, letter) in letters.iter().enumerate() {
        let slots: Vec<String> = (0..24)
            .map(|k| reduced.get(k).map(|r| q(&r[p])).unwrap_or_else(|| "0".into()))
            .collect();
        let dif = format!("I{letter} диф.");
        let tor = format!("I{letter} торм.");
        g.add(&dif, format!("GrsSumCur:({})", slots.join(",")));
        let f = match braking {
            Braking::Sum { k } => format!("TormSumCur:({},{})", num(k), slots.join(",")),
            Braking::Sub { k } => format!("TormSubCur:({},{},{})", slots[0], slots[1], num(k)),
            Braking::Max => format!("TormMaxCur:({})", slots.join(",")),
        };
        g.add(&tor, f);
        g.dif_pairs.push((dif, tor));
    }
    g
}

/// The name an appended channel gets (`0x48bff0`): unchanged if free, else
/// the first free "name_2", "name_3", …
pub fn unique_name(name: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == name) {
        return name.to_string();
    }
    (2..).map(|i| format!("{name}_{i}")).find(|n| !taken.contains(n)).expect("free name")
}

/// A computed channel: its samples and, when the formula's complex value was
/// valid at every sample, that complex series (used as the channel's phasor).
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedChannel {
    pub name: String,
    pub unit: String,
    pub formula: String,
    pub samples: Vec<f64>,
    pub phasors: Option<Vec<num_complex::Complex64>>,
}

/// Evaluate `(name, formula, unit)` channels in order on `osc` plus the
/// already-existing computed channels (`0x415780`): each formula sees the
/// channels added before it; names are made unique ("_2", …); a channel keeps
/// its scalar if valid at every sample, else Re(complex) if that is, else
/// error 0x42e. Any failure discards the whole batch (the original removes
/// the additions) and reports `"<formula>: <message>"`.
pub fn evaluate_batch(
    osc: &crate::comtrade::Oscillogram,
    existing: &[ComputedChannel],
    batch: &[(String, String, String)],
    show_primary: bool,
) -> Result<Vec<ComputedChannel>, crate::expr::FormulaError> {
    use crate::expr::{Context, Expr, ExprError, FormulaError};
    let mut all: Vec<ComputedChannel> = existing.to_vec();
    let mut added = Vec::new();
    for (name, formula, unit) in batch {
        let fail = |error: ExprError| FormulaError { formula: formula.clone(), error };
        let chans = crate::channels::OscChannels::new(osc, &all, show_primary);
        let expr = Expr::parse_channels(formula, &chans).map_err(fail)?;
        let ctx = Context::new();
        let n = chans.len();
        let (mut scal, mut cplx) = (Vec::with_capacity(n), Vec::with_capacity(n));
        let (mut all_s, mut all_c) = (true, true);
        for i in 0..n {
            let (s, z) = expr.eval_sample(&ctx, &chans, i).map_err(fail)?;
            all_s &= s.is_some();
            all_c &= z.is_some();
            scal.push(s.unwrap_or(0.0));
            cplx.push(z.unwrap_or_default());
        }
        let samples = if all_s {
            scal
        } else if all_c {
            cplx.iter().map(|z| z.re).collect()
        } else {
            return Err(fail(ExprError::InvalidResult));
        };
        let taken: Vec<String> = osc.analog.iter().map(|c| c.id.clone()).chain(all.iter().map(|c| c.name.clone())).collect();
        let ch = ComputedChannel {
            name: unique_name(name, &taken),
            unit: unit.clone(),
            formula: formula.clone(),
            samples,
            phasors: all_c.then_some(cplx),
        };
        all.push(ch.clone());
        added.push(ch);
    }
    Ok(added)
}

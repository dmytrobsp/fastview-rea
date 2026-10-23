//! "Быстрый анализ" — the original's quick-analysis property sheet
//! (`CQAnPropSheet`, ledger L8, `notes/VIEWS_SPEC.md`) is a *channel
//! generator*: the active tab turns its controls into a list of computed
//! channels (name + formula in the formula language), plus which of them to
//! toggle on the vector / circle diagrams, a vector-diagram angle reference
//! and (Iдиф, Iторм) pairs for the differential diagram. The channels are then
//! appended and evaluated like any computed channel.
//!
//! Every formula comes from the «Добавить…» builder (`0x4129e0`) in its
//! quick mode (ledger L9.73): `Fn: (a, b)`, a channel name quoted unless it
//! is a letter and alphanumerics, a numeric field passed as typed ("," →
//! "."; empty stays empty), and a channel of the same batch named as it
//! will be appended (`uniq`, `0x48bff0`: "3I0", or "3I0_2" when the record
//! has one).

/// What one tab produces.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Generated {
    /// (name, formula) in evaluation order; units come from the formulas.
    pub channels: Vec<(String, String)>,
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
        self.channels.push((name.to_string(), formula));
    }
}

/// A channel field's text (`0x412700` quoting).
fn c(name: &str) -> String {
    crate::addformula::arg(crate::addformula::id::IA, name)
}

/// A numeric field's text: digits, spaces, "." and "," with "," as "."; any
/// other text as typed; an empty field stays empty.
pub fn num(field: &str) -> String {
    crate::addformula::arg(crate::addformula::id::R0, field)
}

/// `Fn: (a, b, …)`.
fn call(f: &str, args: &[String]) -> String {
    format!("{f}: ({})", args.join(", "))
}

/// The name a channel of this batch will be appended under, as a field.
fn uniq(name: &str, taken: &[String]) -> String {
    c(&unique_name(name, taken))
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

/// Симметричные составляющие (`0x48c2b0`): with «Рассчитать Ib» Ib =
/// form 401 (3Io absent → 0) and I1 / I2 = forms 800 / 801 naming it,
/// no 3I0; else I1, I2, 3I0 = 403, 404, 402. Phase voltages U1, U2, 3U0 =
/// 410, 411, 407; line voltages U1, U2 = 408, 409.
pub fn symmetric(i: Option<&Currents>, u: Option<&Voltages>, taken: &[String]) -> Generated {
    let mut g = Generated::default();
    if let Some(i) = i {
        let (ia, ic) = (c(&i.ia), c(&i.ic));
        if i.calc_ib {
            g.add("Ib", call("Ib", &[ia.clone(), ic.clone(), "0".into()]));
            let ib = uniq("Ib", taken);
            g.add("I1", call("I1", &[ia.clone(), ib.clone(), ic.clone()]));
            g.add("I2", call("I2", &[ia, ib, ic]));
        } else {
            let args = [ia, c(&i.ib), ic];
            g.add("I1", call("I1", &args));
            g.add("I2", call("I2", &args));
            g.add("3I0", call("3I0", &args));
        }
    }
    match u {
        Some(Voltages::Phase { ua, ub, uc }) => {
            let args = [c(ua), c(ub), c(uc)];
            g.add("U1", call("U1f", &args));
            g.add("U2", call("U2f", &args));
            g.add("3U0", call("3U0", &args));
        }
        Some(Voltages::Line { uab, ubc }) => {
            let args = [c(uab), c(ubc)];
            g.add("U1", call("U1l", &args));
            g.add("U2", call("U2l", &args));
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

/// Нулевая последовательность (`0x4887c0`): 3I0 = form 402 or 515
/// `Chan: (…)`; 3U0 = 407, 517 `Uniik: (…)` or 516 `Chan: (…)`; with both,
/// S0 = 518 `Mod:(Pf: (3I0, 3U0))[VA]` naming them as appended.
pub fn zero(i: Option<&ZeroCurrent>, u: Option<&ZeroVoltage>, taken: &[String]) -> Generated {
    let mut g = Generated::default();
    match i {
        Some(ZeroCurrent::Calculated { ia, ib, ic }) => g.add("3I0", call("3I0", &[c(ia), c(ib), c(ic)])),
        Some(ZeroCurrent::Measured(ch)) => g.add("3I0", call("Chan", &[c(ch)])),
        None => {}
    }
    match u {
        Some(ZeroVoltage::FromPhases { ua, ub, uc }) => g.add("3U0", call("3U0", &[c(ua), c(ub), c(uc)])),
        Some(ZeroVoltage::FromUniUik { uni, uik }) => g.add("3U0", call("Uniik", &[c(uni), c(uik)])),
        Some(ZeroVoltage::Measured(ch)) => g.add("3U0", call("Chan", &[c(ch)])),
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
        g.add("S0", format!("Mod:(Pf: ({}, {}))[VA]", uniq("3I0", taken), uniq("3U0", taken)));
        g.circle.push("S0".into());
        g.circle_readout = Some(1);
    }
    g.show_values = i.is_some() || u.is_some();
    g
}

/// Междуфазные сопротивления (`0x487640`): Ib = 401 when computed; phase
/// voltages: Uab, Ubc, Uca = 449, 511, 512 and Z = 802 / 804, 803 / 513,
/// 514 naming them as appended (never quoted: those fields are not channel
/// fields); line voltages: Uca = 450 and Z = 806 / 805, 808 / 807, 514.
pub fn phase_phase(i: &Currents, u: &Voltages, itr: &str, taken: &[String]) -> Generated {
    let mut g = Generated::default();
    let (ia, ic) = (c(&i.ia), c(&i.ic));
    let ib = if i.calc_ib {
        g.add("Ib", call("Ib", &[ia.clone(), ic.clone(), "0".into()]));
        uniq("Ib", taken)
    } else {
        c(&i.ib)
    };
    let raw = |n: &str| unique_name(n, taken);
    let (uab, ubc) = match u {
        Voltages::Phase { ua, ub, uc } => {
            g.add("Uab", call("Ulf", &[c(ua), c(ub)]));
            g.add("Ubc", call("Ulf", &[c(ub), c(uc)]));
            g.add("Uca", call("Ulf", &[c(uc), c(ua)]));
            (raw("Uab"), raw("Ubc"))
        }
        Voltages::Line { uab, ubc } => {
            g.add("Uca", call("Ull", &[c(uab), c(ubc)]));
            (c(uab), c(ubc))
        }
    };
    let itr = num(itr);
    let zl = |a: String, b: String, d: String| format!("Mod:(Zl: ({a}, {b}, {d}, {itr}))");
    g.add("Zab", zl(uab, ia.clone(), ib.clone()));
    g.add("Zbc", zl(ubc, ib, ic.clone()));
    g.add("Zca", zl(raw("Uca"), ic, ia));
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

/// Фазные сопротивления (`0x48a4c0`): 3I0 = 402, then Za, Zb, Zc = 519 /
/// 544 / 545 `Mod:(Zf: (U, I, 3I0, 3Ion, R0, R1, Rm, X0, X1, Xm, Iтр))` or
/// 546–548 `Mod:(Zk: (U, I, 3I0, 3Ion, K0, Km, Ф0, Фm, Iтр))`; 3Ion
/// «Не учитывать» (empty here) → 0. The page has no «Рассчитать Ib».
pub fn phase_z(i: &Currents, u: [&str; 3], i0_parallel: &str, comp: &PhaseComp, itr: &str, taken: &[String]) -> Generated {
    let mut g = Generated::default();
    let ii = [c(&i.ia), c(&i.ib), c(&i.ic)];
    g.add("3I0", call("3I0", &ii));
    let i0 = uniq("3I0", taken);
    let i0n = if i0_parallel.trim().is_empty() { "0".to_string() } else { c(i0_parallel) };
    let itr = num(itr);
    for (k, name) in ["Za", "Zb", "Zc"].into_iter().enumerate() {
        let mut args = vec![c(u[k]), ii[k].clone(), i0.clone(), i0n.clone()];
        let f = match comp {
            PhaseComp::PerUnit { r0, r1, rm, x0, x1, xm } => {
                args.extend([r0, r1, rm, x0, x1, xm].map(|v| num(v)));
                "Zf"
            }
            PhaseComp::Coefficients { k0, km, f0, fm } => {
                args.extend([k0, km, f0, fm].map(|v| num(v)));
                "Zk"
            }
        };
        args.push(itr.clone());
        g.add(name, format!("Mod:({})", call(f, &args)));
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

/// Winding-group compensation (three-phase only): the page's method; the
/// group numbers are per arm.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupComp {
    None,
    Posl,
    Triangle,
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

/// One arm (плечо): its phase current channels, the reduction
/// coefficients (the method is the page's), its group number and «Удалить
/// I0».
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub currents: Vec<String>,
    pub reduction: Reduction,
    pub group: String,
    pub remove_i0: bool,
}

/// Дифференциальные токи (`0x47ff60`), in the original's order: every arm's
/// reduced phase currents (forms 501–504, «Ia плечо k прив.»), then with
/// three phases and a group method every arm's turned currents (505–507
/// `Mod:(GrsPoslI?: …)` / 508, 520, 521 `Mod:(GrsTriangleI?: …)`,
/// «Ia плечо k прив. пов.»), then per phase «Ia диф.» (522
/// `Mod:(GrsSumCur: …)`), then per phase «Ia торм.» (523 / 524 / 525) over
/// the 24 arm slots, unused ones 0. One phase is named «Ia».
pub fn differential(phases: usize, arms: &[Arm], group: &GroupComp, braking: &Braking, taken: &[String]) -> Generated {
    let mut g = Generated::default();
    let letters: Vec<&str> = if phases == 3 { vec!["a", "b", "c"] } else { vec!["a"] };
    let reduced = |k: usize, l: &str| format!("I{l} плечо {} прив.", k + 1);
    let turned = |k: usize, l: &str| format!("I{l} плечо {} прив. пов.", k + 1);
    for (k, arm) in arms.iter().enumerate() {
        for (p, l) in letters.iter().enumerate() {
            let src = c(arm.currents.get(p).map_or("", String::as_str));
            let (f, args) = match &arm.reduction {
                Reduction::Priv { k } => ("CoeffPriv", vec![src, num(k)]),
                Reduction::Tt { ktt, ktt_base } => ("CoeffTT", vec![src, num(ktt), num(ktt_base)]),
                Reduction::PowerPriv { ktt, snom, unom } => ("CoeffPowerPriv", vec![src, num(ktt), num(snom), num(unom)]),
                Reduction::AmpPriv { ktt, inom } => ("CoeffAmpPriv", vec![src, num(ktt), num(inom)]),
            };
            g.add(&reduced(k, l), call(f, &args));
        }
    }
    let grouped = phases == 3 && *group != GroupComp::None;
    if grouped {
        for (k, arm) in arms.iter().enumerate() {
            let three = ["a", "b", "c"].map(|l| uniq(&reduced(k, l), taken));
            for l in ["a", "b", "c"] {
                let mut args = three.to_vec();
                args.push(num(&arm.group));
                let f = if *group == GroupComp::Posl {
                    args.push(if arm.remove_i0 { "0" } else { "1" }.into());
                    call(&format!("GrsPoslI{l}"), &args)
                } else {
                    call(&format!("GrsTriangleI{l}"), &args)
                };
                g.add(&turned(k, l), format!("Mod:({f})"));
            }
        }
    }
    let slots = |l: &str| -> Vec<String> {
        (0..24)
            .map(|k| if k < arms.len() { uniq(&if grouped { turned(k, l) } else { reduced(k, l) }, taken) } else { "0".into() })
            .collect()
    };
    for l in &letters {
        g.add(&format!("I{l} диф."), format!("Mod:({})", call("GrsSumCur", &slots(l))));
    }
    for l in &letters {
        let s = slots(l);
        let f = match braking {
            Braking::Sub { k } => call("TormSubCur", &[s[0].clone(), s[1].clone(), num(k)]),
            Braking::Max => call("TormMaxCur", &s),
            Braking::Sum { k } => {
                let mut args = vec![num(k)];
                args.extend(s);
                call("TormSumCur", &args)
            }
        };
        g.add(&format!("I{l} торм."), f);
        g.dif_pairs.push((format!("I{l} диф."), format!("I{l} торм.")));
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

/// Evaluate `(name, formula)` channels in order on `osc` plus the
/// already-existing computed channels (`0x415780`): each formula sees the
/// channels added before it; names are made unique ("_2", …); a channel keeps
/// its scalar if valid at every sample, else Re(complex) if that is, else
/// error 0x42e. Any failure discards the whole batch (the original removes
/// the additions) and reports `"<formula>: <message>"`.
pub fn evaluate_batch(
    osc: &crate::comtrade::Oscillogram,
    existing: &[ComputedChannel],
    batch: &[(String, String)],
    show_primary: bool,
) -> Result<Vec<ComputedChannel>, crate::expr::FormulaError> {
    evaluate_batch_with(osc, crate::freqcorr::of(osc), existing, batch, show_primary)
}

/// [`evaluate_batch`] with the frequency correction `corr` (taken from the
/// record as stored when `osc` is a flagged copy).
pub fn evaluate_batch_with(
    osc: &crate::comtrade::Oscillogram,
    corr: Option<crate::freqcorr::FreqCorr<'_>>,
    existing: &[ComputedChannel],
    batch: &[(String, String)],
    show_primary: bool,
) -> Result<Vec<ComputedChannel>, crate::expr::FormulaError> {
    use crate::expr::{Context, Expr, ExprError, FormulaError};
    let mut all: Vec<ComputedChannel> = existing.to_vec();
    let mut added = Vec::new();
    for (name, formula) in batch {
        let fail = |error: ExprError| FormulaError { formula: formula.clone(), error };
        let chans = crate::channels::OscChannels::new_with(osc, &all, show_primary, corr);
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
            unit: crate::expr::formula_unit(formula, &chans),
            formula: formula.clone(),
            samples,
            phasors: all_c.then_some(cplx),
        };
        all.push(ch.clone());
        added.push(ch);
    }
    Ok(added)
}

/// «Загрузить» of the formula editor (`0x414ee0`): the text (already
/// decoded with the program's code page) as `name,formula,` lines — tab,
/// space, CR and LF skipped before each line, a line ending at CR or LF,
/// the name up to the first comma, the formula the rest minus the line's
/// last character (the trailing comma «Сохранить» writes). No validation.
pub fn parse_frm(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    loop {
        rest = rest.trim_start_matches(['\t', ' ', '\r', '\n']);
        if rest.is_empty() {
            return out;
        }
        let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
        let line = &rest[..end];
        rest = &rest[end..];
        let (name, formula) = match line.split_once(',') {
            Some((n, f)) => {
                let mut f = f.to_string();
                f.pop();
                (n.to_string(), f)
            }
            None => (line.to_string(), String::new()),
        };
        out.push((name, formula));
    }
}

/// «Сохранить» (`0x415230`): one `name,formula,` line per entry, CR LF
/// (text mode; encoded CP1251 by the caller — the process locale).
pub fn write_frm(rows: &[(String, String)]) -> String {
    rows.iter().map(|(n, f)| format!("{n},{f},\r\n")).collect()
}

#[cfg(test)]
mod frm_tests {
    use super::*;

    #[test]
    fn frm_round_trip() {
        let rows = vec![("Uab".to_string(), "Ua-Ub".to_string()), ("P".to_string(), "Ua*Ia [W]".to_string())];
        let text = write_frm(&rows);
        assert_eq!(text, "Uab,Ua-Ub,\r\nP,Ua*Ia [W],\r\n");
        assert_eq!(parse_frm(&text), rows);
        // Leading blanks and empty lines vanish; the last character goes.
        assert_eq!(parse_frm("\r\n  X,a+b;\n\tY,c"), vec![("X".into(), "a+b".into()), ("Y".into(), "".into())]);
    }
}

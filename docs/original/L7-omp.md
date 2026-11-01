# L7 — ОМП (fault location)  ·  ✅ engine + GUI (parallel-line UI ✅ since L9.11/L9.12)

Evidence (subagent decompile; main `0x4a00b0`). Our `omp.rs` (single
cursor-instant phasor, plain reactance / textbook two-terminal formula) is
**structurally different** from the original:
- Inputs: tree editor (DIALOG 336 "Параметры ОМП"), values via `_wtof`.
  Method `obj+0x64`: 1 isolated, 2 grounded, 4 double-ended. Per substation
  Inom (positive-sequence nominal current). Line = list of **sections**
  (0x58-byte records): R,X per km (isolated), length km, R0,X0, R1,X1 per km,
  number of supports, parallel lines (Rm,Xm per km + current channel), taps
  (R0,X0 total Ω). Channels by name: isolated Uab/Ubc/Uca + Ia/Ib/Ic; grounded
  Ua/Ub/Uc + Ia/Ib/Ic. No sections or Inom = 0 → message 349. Validation
  0x4a5d70 (R0&X0 / R1&X1 not both 0, section length ≠ 0). Primary vs
  secondary — settled by L9.7 / L9.134 (ОМП copies `ch+0x20`, which
  `0x426020` fills with the ratio applied).
- Phasors (0x477280, ✔ read directly): fixed-rate path = Σ_{j=0}^{N−1}
  x[i−N+j]·(sin θj + j·cos θj), θj = 2πj/N, × 2/N (**peak**, window
  [i−N, i−1], excludes i) — i.e. the channel phasor (L6) at i−1 times √2;
  0 for i < N. A variable-rate path (param ≠ −1) integrates over one measured
  period using the record's per-sample rate (1e-6 time scaling). Symmetrical comps I1, I2, I0=(Ia+Ib+Ic)/3,
  U0 (0x497090).
- Per-sample fault-type detection (0x498580, hysteresis 0.9): 3ph if
  |I1|>4|I2| and |I1|>√2·Inom; asymmetric if 4|I2|>|I1|; no-earth if
  |I2|>6|I0|; 2ph/1ph need |I1|>0.1·√2·Inom. Debounce (0x499930): rises after
  N/4 samples, falls immediately. Phase selection 0x498c64.. by hysteretic
  current comparisons. Isolated variant 0x497f10.
- Loops: grounded (0x499e90) 1ph U_ph/I_ph ref I0; 2ph difference loop; 3ph
  AB. Isolated (0x499c80) Uab/(Ia−Ib)…, phase-earth not located.
- Grounded formula (0x49a670): K = Z0/Z1 − 1 on I0; Iloop = Iph + K·I0 +
  Σ(Zm/Z1)·I_par; d = Im(U/Iref′)/Im(Z1·Iloop/Iref′), Iref′ = Iref·e^{jθ}
  (Takagi-type; θ, `obj+0x380`, is zero before the first pass and written only by `0x49adc0`: arg(Mb − Mc·e^{jMa}) from the medians over the window — as ours' `refine_theta`; L9.194, spec omp.md §6); section walking with voltage drop
  and tap admittances; d<0 → 0. Isolated (0x49a430): Z′ = U/I − Z_acc; if
  arg(R+jX) − arg(Z′) > 0 → d = Im Z′/X else Re Z′/R.
- Double-ended (0x49cf00/0x49cd10): minimise | |U_f(end1,x)| − |U_f(end2,L−x)| |
  (magnitudes only, no angle alignment); 1ph via U0/I0/Z0 incl. mutual, 2/3ph
  via U2/I2/Z1; bisection-like search step L/2 … ≤ 0.01 km, x<0 → 0;
  direction check: median arg(Z1end)−arg(Z2end) within ±90° → adjacent line.
  Second end = second oscillogram; separate time-sync tool (0x47bb90: by
  record start / emergency component / pre-fault, messages 3259/3305/3456).
- Window (0x499b50, 0x477db0, 0x477a20, 0x477c40): longest run of one fault
  type, one-period window chosen by min σ of distance + min |mean| of loop
  current (≤1000 iterations), written to cursors 1/2; moving cursors
  recomputes. Result = **median** of per-sample distances; error = 2σ/mean·100
  (population σ); section/support location; texts 3020/3026/3006–3008/3025
  ("на смежной линии" if d > L or ≤ 0, no clamping); extra channels "Lкз" (m)
  and "КЗ фазы A/B/C"; save .txt / settings .omp.

**L7 detection spec (subagent, read in code; constants 4.0@4d16e0 6.0@4d16f0
3.0@4d16d0 0.9@4d1658 0.1@4d1630 1.15@4d1688; I1,I2,I0 already /3):**
- Hysteresis H(set, reset, prev): set → 1, then reset → 0 (reset wins), else
  hold prev; reset level = 0.9 × set threshold; strict `>`; arrays start 0.
- 0x498580 (grounded/double-ended), k = 1..N−1, outputs[0] = 0.
  Pass 1: s = H(|I1| > 4|I2|, |I1| < 3.6|I2|); h = H(|I1| > √2·In,
  |I1| < 0.9√2·In); F3 = s∧h. Pass 2: a = H(4|I2| > |I1|, 0.9|I1| > 4|I2|);
  n = H(|I2| > 6|I0|, 5.4|I0| > |I2|); G = |I1| > 0.1√2·In; F2 = a∧n∧G;
  E = a∧¬n∧G. Then debounce E, F2, F3 in place.
- 0x497f10 (isolated, no hysteresis/debounce), k = 0..N−1: if |I1| ≤ 0.1√2·In
  → F2=F3=0; else F2 = 3|I2| > |I1|, F3 = |I1| ≥ 3|I2| ∧ |I1| > 1.15√2·In.
  Phases: A = F3 ∨ (F2 ∧ (|Ia|>|Ib| ∨ |Ia|>|Ic|)), B, C likewise.
- 0x499930 debounce: T_rise = trunc(P/4); y[0]=0, c=0; for i≥1: y=y[i−1];
  c = (c+1)·[x[i] ≠ y[i−1]]; fall when y[i−1]=1, x=0, c≥0 → y=0, c=−1; rise
  when y[i−1]=0, x=1, c ≥ T_rise → y=1, c=−1; x[0] ignored.
- Grounded phase selection (498be3–498f6c): p_ab = H(|Ia|>|Ib|, 0.9|Ib|>|Ia|),
  p_ac, p_bc likewise; A = F3 ∨ F2∧(p_ab∨p_ac) ∨ E∧p_ab∧p_ac;
  B = F3 ∨ F2∧(¬p_ab∨p_bc) ∨ E∧¬p_ab∧p_bc; C = F3 ∨ F2∧(¬p_ac∨¬p_bc) ∨
  E∧¬p_ac∧¬p_bc.
- Loop choice grounded 0x499e90 (k = P..N−1, later writes override): F3 → AB
  (Ua−Ub, Ia−Ib); F2 → first of AB, BC, CA by phase pairs (else 1+0j); E →
  first of A, B, C (Uph, Iph; else 1+0j), inst = ia+ib+ic; Iref = Iloop if
  F2∨F3 else I0 (=(Ia+Ib+Ic)/3) if E; none → U=I=Iref=1+0j, inst 1.
  Isolated 0x499c80: defaults 1+0j; if F2∨F3: AB, BC, CA tested in turn, last
  match wins (3-ph → CA); line-voltage phasors at obj+0x1e8/1f4/200 (likely
  Uab/Ubc/Uca, NOT confirmed); Iref not written.
- Still NOT specified (agents stopped before reporting): section math
  (0x49a670/0x49a430/θ), window choice (0x499b50/0x477db0), median/σ/output
  text, double-ended (0x49cf00…), interval finders 0x499aa0/0x499b50,
  double-ended detection 0x499000. Calculator channel accessors (L6) also
  pending — their agent was stopped too.

Full behavioural spec (detection, debounce, phase selection, loop choice,
grounded/isolated section distance, θ, median) is kept in **`docs/original/specs/omp.md`**.

Implemented (`omp.rs`, new per-sample engine; the old single-instant
functions remain only until the GUI moves over): `omp_phasors`, components,
grounded detection with hysteresis + `debounce`, phase selection, loop choice
(grounded/isolated), grounded and isolated per-sample distance with section
walking, voltage drops, taps, parallel-line term, clamp and the over-count
quirk, `median`. θ: see the tap re-estimation below. Stage 2 (spec §8–§13): `fault_interval` (first qualifying run),
`choose_window` (alternating σ / |mean| searches with the later-window bias,
1000-iteration fallback), `result_for_window` (median, 2σ/mean·100 error,
upper-median flags, section/support, the original's text with SI prefixes,
strings 3006–3008/3016/3020/3025/3026 read from the PE), `locate`;
`double_ended` (two-end detection, loop current, U0/I0 or U2/I2 propagation
from both ends with taps and mutual coupling, |U| matching search) +
`locate_double` (direction check zeroes all distances). Pinned by
`tests/omp_engine.rs` (11). θ tap re-estimation implemented (`0x49adc0`: A_k zero-sequence carry, B_k
nodal ladder `0x49bd40` solved with the original's zero-pivot elimination
`0x477f70`, incl. the boundary bug using the whole remaining length; one
repeat pass when the last section has taps). Residual: SI prefixes assume the Russian UI (Cyrillic);
(which signal feeds a parallel line's current: the channel its «I0» field
names, message 354 when there is none — `omp_params::sections`, L9.12).
GUI (done): method 1/2/4, Inom per substation, editable sections with taps,
"Рассчитать" runs the engine on the role-mapped channels (isolated uses line
voltages derived as Ua−Ub etc. — the original reads Uab/Ubc/Uca channels by
name), writes the window to the cursors, adds "Lкз" (m) and "КЗ фазы A/B/C"
channels, shows the original's text; moving the cursors recomputes the result
(double-ended re-applies the direction check). The old single-instant API and
its 4 tests were removed. GUI tests: message 349, window/cursors/channels.
(Not done at the time, all done since in L9.11 / L9.12: the parallel-line
entry in the UI, the .omp settings file and .txt result save, the
tree-editor layout of DIALOG 336.)

Rust action: rewrite `omp.rs` as this engine (sections, per-sample distances,
detection, window, median/σ) with tests; GUI dialog as a section tree.

# L6 — Signal engine (channel methods)  ·  ✅ (freq-channel DFT ✅ L9.65; 0x46 retag ❌)

Evidence (subagent objdump/decompile; all double constants read from bytes).
OscillHelper.dll exports only `ComtradeConverter` — no signal code there.
- **N (samples/period)** (0x427062, 0x426320, 0x427320, 0x426d40, 0x4276f0):
  per-channel resampled rate `chan+0x108` ≠ 0 → `round(rate/50.0)` (0x4d1718);
  else `N = rate / trunc(f_nom)` (unsigned int division; `rate` = `rec+0x228`,
  `f_nom` = `rec+0x220` from the CFG). Never a measured frequency.
- **Phasor** `FUN_00428ac0` reads a cache filled by `FUN_00428490` via
  `FUN_00426fc0`: static `sinT[k]=sin(2πk/N)`, `cosT[k]=cos(2πk/N)`, k=1..N;
  window = the N samples ending at idx **inclusive** (s = idx−N+k);
  `Re = (√2/N)Σ sinT[k]·x[s]`, `Im = (√2/N)Σ cosT[k]·x[s]` (RMS-scaled,
  √2 = sqrt(2.0) at runtime); 0 while idx < N−1. ⇒ P_FV = j·e^{−j2π/N}·P_ours:
  **sine-referenced at the newest sample** (magnitudes & relative angles equal
  ours, absolute angles differ by 90° − 360°/N). No DC/mimic filtering.
  Flag 0x2000: cache copied from stored phasors × ±k. Frequency-channel
  variant `FUN_004267e0` (record with one rate segment + a selected "Гц"
  channel `rec+0x874`): trapezoidal integral over exactly one measured period
  with fractional end correction, × √2·f/P.
- **rms** `FUN_004272a0(idx, periods)`: true RMS over W = N·periods samples
  ending at idx inclusive; 0 while W−1 > idx. Calculator passes periods=1.
- **fourie:(ch,n)** `FUN_00426240`: RMS magnitude of harmonic n over the phasor
  window (`sqrt(C²+S²)·√2/N`); n==1 additionally × `rec+0x278` (1.0128 /
  1.00259 / 1.014065 by a non-COMTRADE header type; COMTRADE default NOT
  DETERMINED); n==0 → |a0|.
- **a0** `FUN_00426c90(idx, P)`: signed mean over W = N·P ending at idx.
- **fault** `FUN_00427670`: `d = x[s] − 2x[s−N] + x[s−2N]` (**one-period lag**,
  terms with s < 2N are 0), result `sqrt(Σd²)/sqrt(N)` over k=1..N.
  ← ours uses a lag-1 second difference / dt². WRONG.
- **Instantaneous** `FUN_00426020`: raw (COMTRADE a·x+b applied at load,
  0x451904) × k, k = Primary/Secondary ratio by flags `+0x88/+0x89`; flag word
  `+0x60`: 0x1000 ADC code `round((raw−b)/(a·m))`, 0x400 subtract
  k·mean-over-whole-periods, 0x800 invert. Digital: raw.
- **frequency:** = `rec+0x1fc[idx]` = nominal (per rate segment), NOT
  measured. `arg:` warm-up = N (Calculator `.fvb` context: frequency 0.0,
  warm-up 0 — vtable 0x4c3df4 slots 0x4084f0/0x404ba0; channels context
  `CalcChannels` vtable 0x4c65c4).
- Formula accessors: `3i0:`/`3u0:` = Ia+Ib+Ic (not /3), scalar = |·|; `i1:` =
  (Ia + a·Ib + a²·Ic)/3, `i2:` = (Ia + a²·Ib + a·Ic)/3 (√3 = sqrt(3.0), /6 const
  0x4d1a00); `zl:(U,I1,I2,Imin)` = U/(I1−I2), divides by 0 when
  Imin > |I1/k1 − I2/k2|; `zf:` (11 args) = U/(I + K0·a2 + Km·a3) with
  K0 = ((a4+ja7)/(a5+ja8) − 1)/3, Km = ((a6+ja9)/(a5+ja8))/3 (roles inferred);
  `zk:` NOT DETERMINED. The "Быстрый анализ" view code itself NOT located.

Reconciled (`signal.rs`): `samples_per_period` = `trunc(rate)/trunc(f)` integer
division (used by `Oscillogram::samples_per_cycle`); `sliding_phasors` = the
original sin/cos-table formula (sine reference at the newest sample; only
absolute angles change — impedances, symmetric components and relative angles
are unaffected); `harmonic_rms` = `fourie:` (also drives the frequency diagram);
`sliding_fault` replaces our lag-1 `sliding_emergency` (lag-N second
difference, s < 2N terms 0, /sqrt(N)). rms/a0 already matched. Pinned by
`tests/signal.rs` (4).

Residual: (frequency-channel variant `FUN_004267e0`: done in L9.65);
`rec+0x278` harmonic-1 gain assumed 1 for COMTRADE (NOT DETERMINED); (which
method the frequency-diagram view calls: `0x426510(n, idx, P)` from
`0x46f5a0`, fundamental at bin n = P — read in L9.65); primary/secondary/ADC/DC-removal flags of
`FUN_00426020` mapped to the GUI's existing options, not re-verified.
Calculator channel mode (implemented): `Channels` trait (= `CalcChannels`),
`Expr::parse_channels` (identifiers resolve to CHANNEL tokens; unresolved ones
are dropped from the token stream, 0x419126), `Expr::eval_at(idx)`; CHANNEL
atom scalar = sample, complex = phasor, flag bit 2 kept through arithmetic only
when both sides carry it, pointer cleared (so `rms:(a+b)` → 0x431); `rms: a0:
fault: fourie:` via `channels::OscChannels` + `signal::{rms_at,a0_at,fault_at,
harmonic_rms}`; `frequency:` = record frequency (0.0 in the `.fvb` context,
vtable 0x4084f0); `arg:` = 0 below the warm-up (one period; 0 in `.fvb`);
`3i0: 3u0:` = sum, scalar |·|; `i1: i2:` = (2A−B−C ± j√3(B−C))/6, scalar |·|
(✔ read: shared tail 0x41aa4b divides by 6 @0x4d1a00, stores |z| via
0x420610; common exit 0x41f227 sets flags 3 + truncates). GUI computed
channels use channel mode (analog + earlier computed channels by name).
Pinned by `tests/channel_mode.rs` (5, mock channel source).

Channel functions completed per **`docs/original/specs/calc-channels.md`**: value flags
(bit0 scalar / bit1 complex / bit2 channel) tracked exactly; name resolution =
trimmed COMTRADE id, exact-case first then case-insensitive, unknown → 0x162
"Имя канала задано неправильно." (CalcChannels never drops a token);
computed channel = scalar if valid at every sample, else Re(complex) if that
is, else 0x42e "Результат не действителен"; `ib: ulf: ull: ua: ub: uc: u1f:
u2f: u1l: u2l: uncn: pk: pf: pf3wm: pf2wm: zf: zk: chan: uniik: coeffpriv:
coefftt: coeffpowerpriv: coeffamppriv: grstriangle*: grssumcur: tormsumcur:
tormsubcur: tormmaxcur:` implemented, with the channel ratios k/r/s from
primary/secondary and the show-primary switch (`OscChannels`, samples × k).
Pinned by `tests/channel_mode.rs` (10).

`grspos a/b/c` implemented (0x41d7a7: a = −½+j√3/2, P/Q = e^{±j30°}, binary-
exponentiation integer powers of round(N), integer-rounded zero-sequence factor,
per-variant a/a² multipliers, forced flags 3 at exit 0x41f1f7, ✔ exit read).
Residual:
the computed-channel name guard (error 0x1d4, text unknown); the 0x46 retag
(only differs for primary-stored data); a computed channel's complex series is
not kept (only its scalar samples).

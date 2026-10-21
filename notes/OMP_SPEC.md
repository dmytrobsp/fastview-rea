# ОМП (fault location) — behavioural specification of the original

Distilled from the decompiled `FastView.exe` (ledger L7); behaviour in our own
words, addresses as evidence. `obj` = the ОМП engine object, `P` = samples per
period (`obj+0x4`), `Nk` = sample count (`obj+0x6c`), `In` = nominal positive-
sequence current (`obj+0x70`). `⊘` = MSVC complex division (`0x420ca0`):
divisor exactly 0 or NaN → NaN+jNaN. `|·|` = scaled hypot (`0x420840`).

## 1. Phasors and components

- Phasor (`0x477280`, read directly): `Σ_{j=0}^{P−1} x[k−P+j]·(sin θj + j·cos θj)`,
  `θj = 2πj/P`, × `2/P` (peak); 0 for `k < P`.
- Components (`0x497090`): `I1, I2, I0 = (Ia+Ib+Ic)/3` (each already /3),
  `U0 = (Ua+Ub+Uc)/3`.

## 2. Fault-type detection (grounded / double-ended, `0x498580`)

Hysteresis `H(set, reset, prev)`: set → 1, then reset → 0 (reset wins), else
hold; strict `>`; arrays start at 0. For k = 1..Nk−1, outputs[0] = 0:
- `s = H(|I1| > 4|I2|, |I1| < 3.6|I2|)`, `h = H(|I1| > √2·In, |I1| < 0.9·√2·In)`,
  `F3 = s ∧ h`.
- `a = H(4|I2| > |I1|, 0.9|I1| > 4|I2|)`, `n = H(|I2| > 6|I0|, 5.4|I0| > |I2|)`,
  `G = |I1| > 0.1·√2·In`; `F2 = a ∧ n ∧ G`; `E = a ∧ ¬n ∧ G`.
- Then debounce E, F2, F3 in place (§4).

Isolated (`0x497f10`, no hysteresis/debounce), k = 0..Nk−1: if
`|I1| ≤ 0.1·√2·In` → F2 = F3 = 0; else `F2 = 3|I2| > |I1|`,
`F3 = |I1| ≥ 3|I2| ∧ |I1| > 1.15·√2·In`. Phases:
`A = F3 ∨ (F2 ∧ (|Ia|>|Ib| ∨ |Ia|>|Ic|))`, B and C likewise.

## 3. Phase selection (grounded, `0x498be3`–`0x498f6c`)

`p_ab = H(|Ia|>|Ib|, 0.9|Ib|>|Ia|)`, `p_ac`, `p_bc` likewise (k = 1..Nk−1):
- `A = F3 ∨ F2∧(p_ab ∨ p_ac) ∨ E∧p_ab∧p_ac`
- `B = F3 ∨ F2∧(¬p_ab ∨ p_bc) ∨ E∧¬p_ab∧p_bc`
- `C = F3 ∨ F2∧(¬p_ac ∨ ¬p_bc) ∨ E∧¬p_ac∧¬p_bc`

## 4. Debounce (`0x499930`)

`T = trunc(P/4)`; `y[0] = 0`, `c = 0`; for i ≥ 1: `y = y[i−1]`;
`c = (c+1)·[x[i] ≠ y[i−1]]`; if `y[i−1]=1 ∧ x=0 ∧ c ≥ 0` → `y=0, c=−1`;
if `y[i−1]=0 ∧ x=1 ∧ c ≥ T` → `y=1, c=−1`. `x[0]` ignored.

## 5. Loop choice (k = P..Nk−1)

Grounded (`0x499e90`, later writes override): F3 → AB (`Ua−Ub`, `Ia−Ib`,
inst `ia−ib`); F2 → first of AB, BC, CA whose two phases are set (else
U = I = 1+0j, inst 1); E → first of A, B, C (`Uph`, `Iph`; else 1+0j), inst
`ia+ib+ic`; `Iref = Iloop` if F2 ∨ F3, else `I0` if E; none → U = I = Iref =
1+0j, inst 1.
Isolated (`0x499c80`): defaults 1+0j / inst 1; if F2 ∨ F3: AB, BC, CA tested in
turn, the last match wins (3-phase → CA), U = the line-voltage phasor of that
pair (`obj+0x1e8/1f4/200`, presumably Uab/Ubc/Uca); Iref not written.

## 6. Grounded distance (`0x49a670`), per sample k = P..Nk−1

State: `U = Uloop`, `Iph = Iloop`, `U0`, `I0`, `Iref`, `acc = 0`; `F = E[k]`.
For each section j (always enters section 0), per-km `Z1 = R1+jX1`,
`Z0 = R0+jX0`:
- `K = F ? Z0⊘Z1 − 1 : 0`; `Pp = F ? Σ_par (Rm+jXm)·Ip[k] ⊘ Z1 : 0`.
- `Iref' = Iref·e^{jθ}`; `I' = Iph + K·I0 + Pp`.
- `d = Im(U ⊘ Iref') / Im((Z1·I') ⊘ Iref')` (stored immediately).
- If `L_j ≥ d` stop. Else: `acc += L_j`; `U −= Z1·L_j·I'`;
  `U0 −= Z0·L_j·(I0 + Pp)`; then taps (only if F and the section has taps):
  `Zeq = 1 ⊘ Σ 1⊘(R0t + jX0t)`, `It = U0 ⊘ Zeq`, `I0 −= It`, `Iph += It`,
  `Iref = I0`.
- After the loop: if `0 > d` → `d = 0` (NaN kept). Result `d + acc` (km).
  When every section is passed, `acc` already includes the last length, so the
  result is `d_last + ΣL` (over-counts by `L_last`).

θ (`obj+0x380`): 0 initially. Mode 2 runs: grounded pass → window (§8) →
result D; if the LAST section has taps: recompute θ (`0x49adc0`), run the
grounded pass, window and result again (exactly once). θ recompute only if
`D > 0` and the upper median of E over the window is non-zero: per k,
`A_k` = I0 carried to distance D through the zero-sequence ladder
(`0x49b9e0`), `B_k` = zero-sequence network beyond D (`0x49bd40`, formula NOT
DETERMINED); `θ = arg(Mb − Mc·e^{jMa})` from the medians of
`arg B − arg A` (wrapped to (−π, π]), `|A|`, `|B|`.

## 7. Isolated distance (`0x49a430`), k = 0..Nk−1

If `F2[k] = F3[k] = 0` → distance 0 (and Δ 0). Else `Zacc = 0`, `acc = 0`;
per section with per-km `R, X` (`+0x0/+0x8`):
`Z' = Uloop⊘Iloop − Zacc`; `Δ = atan2(X, R) − arg Z'` (radians, no wrap);
`d = Δ > 0 ? Im Z'/X : Re Z'/R`; if `L_j ≥ d` stop, else `Zacc += (R+jX)·L_j`,
`acc += L_j`. End as §6 (clamp `0 > d`, result `d + acc`, same over-count).

## 8. Pipelines (driver `0x4a0ad6`–`0x4a14a3`)

Isolated: finder `0x499aa0` → loop `0x499c80` → distance `0x49a430` → window
`0x477db0` → result `0x49d330`. Grounded: finder `0x499b50` → `0x499e90` →
`0x49a670` → window → result (+ the θ repeat of §6). Double-ended: detection
`0x499000` → finder `0x499b50` → loop current `0x49a2f0` → distance
`0x49cf00` → window → result. The window is written to cursors 1/2.

## 9. Interval finders

Grounded/double-ended (`0x499b50`): phase code `c = A + 2B + 4C`; `e = P−1`;
repeat { `s` = first index in `[e+1, M−2]` with E ∨ F2 ∨ F3 (else `M−1`);
`e` = first index `> s` (up to `M`, exclusive) where `c ≠ c(s)` } until
`s + P ≥ M−1` or `e − s ≥ P`. Result `(s, e)` if `e − s ≥ P`, else `(P, M−1)`.
The **first** qualifying run wins. Isolated (`0x499aa0`): same, with
`s` = first F2 ∨ F3 and the run continuing while F2 ∨ F3.

## 10. Window chooser (`0x477db0`)

Candidates `[t, t+P−1]`, `t = lo … hi−P`, `k = t − lo`.
σ-score = population σ of the distances in the window + 0.001 − k·0.001/(10P);
μ-score = |mean of the instantaneous loop current| + 1e-6 − k·1e-6/(10P).
A search returns the smallest score strictly above a threshold τ (best starts
at 1e6; ties → earliest; none → `lo`). For i = 0..999: `a` = σ-search with τ =
raw σ of the previous `a` window (0 at first), `b` = μ-search with τ = raw
|mean| of the previous `b` window (0 at first); stop when `|a − b| ≤ 0.5·P`.
Reaching 1000 → `a` = σ-search with τ = −1 (global minimum). Else
`cursor1 = trunc(a + ceil((a − b)/2))`; `cursor2 = cursor1 + P − 1`.

## 11. Result (`0x49d330`)

`lo = max(0, min(c1,c2))`, `hi = min(max(c1,c2), last index)` (inclusive).
Distance = median (§ median rule) of dist[lo..hi]; `obj+0x3a0` = median of the
isolated Δ. Error = 2σ/mean·100 (population σ, mean over hi−lo+1). Phase flags
(and the E/F2/F3 flags used by the text) = upper median of the 0/1 flags
(`sorted[⌊n/2⌋]`).

Median (`0x477810`): sort a copy, mean of `sorted[floor h] … sorted[ceil h]`,
`h = (hi−lo)/2` (an even count averages the two middle values).

## 12. Location and text

Section (`0x49d200`): `r = d`; while `j < n` and `r > L_j`: `r −= L_j`, `j++`.
Support = 1 if no sections or `j ≥ n`, else `trunc(N_j·r/L_j) + 1`.
Text (`0x49d700`): s = 3016 "    КЗ отсутствует."; E → 3006 "    1-х фазное КЗ:
фазы ", F2 → 3007 "    2-х…", F3 → 3008 "    3-х…" (assignments, F3 wins); none →
done. Append "A"/"B"/"C" for each set phase. If `d > ΣL` or `d ≤ 0` append 3025
" на смежной линии." (NaN goes to the other branch). Else append
Format(3026 " на участке № %i, опоры %i - %i.", j+1, support−1, support) and
Format(3020 "    Расстояние = %.2f %sm.     Расстояние от нач. участка = %.2f
%sm.    Погрешность расчета = %.2f %%.", SI(d·1000), SI(r·1000), error).
SI prefix (`0x43d960`, |v|, strict): 0 → none; <1e-6 ×1e9 н; <1e-3 ×1e6 мк;
<1 ×1e3 м; >1e15 ÷1e15 П; >1e12 Т; >1e9 Г; >1e6 М; >1e3 ÷1e3 к; else none
(Cyrillic prefixes unless the UI-language global is 1 → Latin n µ m P T G M k).
No-data abort: message 349 "Данные для определения места повреждения не
введены." (no sections or Inom = 0).

## 13. Double-ended

Detection (`0x499000`), k = 1..M−1, both ends' components:
`a1`, `a2` = H(4|I2| > |I1|, 0.9|I1| > 4|I2|) per end (every sample). If
`a1 ∧ a2`: if `|I1₁| ≤ 0.1√2·In1` or `|I1₂| ≤ 0.1√2·In2` → no fault (n1/n2 not
updated); else n1, n2 = H(|I2| > 6|I0|, 5.4|I0| > |I2|) per end; F2 = n1 ∧ n2
else E. Otherwise F3 = |I1₁| > 1.1√2·In1 ∧ |I1₂| > 1.1√2·In2. Debounce; phase
selection as §3 on end-1 currents. `In2 = |obj+0x78|`.
Loop current (`0x49a2f0`, k ≥ P): F2 ∨ F3 → AB ia−ib, else BC, else CA, else 1;
E only → A ia, else B∧C ib−ic, else 1; none → unchanged.
Propagation per section, `ℓ` = min(L_j, remaining); Zα = (R0, X0)·ℓ path for
E (zero sequence), Zβ = (R1, X1)·ℓ for F2/F3 (negative sequence); taps
`Zeq = 1/Σ 1/Zt`; mutual `S = Σ_{Zm≠0} Zm·Im[k]/Zα_per_km`.
From end 1 (`0x49b210`, forward, stops when sections run out): E: `U −= Zα·ℓ·
(I + S)`, then if the section is fully crossed and Zeq ≠ 0: `I −= U/Zeq`;
F2/F3: `U −= Zβ·ℓ·I`. From end 2 (`0x49b600`, backward, no guard): E: if
Zeq ≠ 0 `I += U/Zeq` first, then `U += Zα·ℓ·(I + S)`; F2/F3: `U += Zβ·ℓ·I`.
E samples use U0/I0, F2/F3 samples use U2/I2 of each end.
Objective `F(x) = | |Uf1(x)| − |Uf2(L−x)| |`, `L = ΣL_j`. Search (`0x49cf00`,
samples with any flag; others unchanged): `x = 0`, `h = L/2`; if `h ≤ 0.01` →
0; loop: `σs = F(x) > F(x+0.01) ? +1 : −1`; `x' = x + σs·h`; `h' = |x'−x|/2`;
`x' < 0` → 0; `h' > 0.01` → continue from x'; else result x'.
Angle per sample: `arg(U/I)₁ − arg(U/I)₂` (U2/I2 for F2/F3, U0/I0 for E, not
wrapped). Direction check: median of the angle over the window in (−π/2, π/2)
(strict) → every distance set to 0 (→ " на смежной линии.", error NaN).

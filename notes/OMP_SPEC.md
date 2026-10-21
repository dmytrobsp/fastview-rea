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

## 8. Window, result, double-ended, output

Median (`0x477810`): sort a copy; mean of the elements between floor and ceil
of (e−s)/2 → an even count averages the two middle values.
Window choice (`0x499b50`/`0x499aa0` + `0x477db0`), result (`0x49d330`), error,
output text and the double-ended method: see ledger L7 summary; exact rules
pending.

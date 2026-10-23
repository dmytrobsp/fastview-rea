# Calculator channel functions — behaviour of the original (ledger L6)

Arg `e_i` = {flags F, complex Z, scalar s, channel}. Unless noted the result
goes to element 0, the channel pointer is cleared and the vector truncated.
"Forced 3" = shared tail `0x41f227` sets F = 3. "Channel check" = flag bit 2
required else 0x431. `/x` in coeff* uses `0x418b70` (x = 0 → 1e-6).
Channel ratios: k = d0/d8 if (+0x89 ∧ ¬+0x88), d8/d0 if (+0x88 ∧ ¬+0x89), else
1 (d0/d8 = COMTRADE primary/secondary, +0x88 = data primary, +0x89 = show
primary). r(e) = d0/d8 if e has a channel and +0x89 clear, else 1.
s(e) = k if e has a channel and ¬(+0x88 ∧ ¬+0x89), else 1.

## Name resolution / computed channels (`0x415780`, `0x42edf0`)
- Identifier = run of `iswalnum`/`_`, or `"…"`. Guard: names of the computed
  channel being built and all later ones (case-insensitive) → error 0x1d4.
- Lookup on the record's channels by the **channel id with trailing spaces
  trimmed** (`+0x68`): exact-case match first, else the first
  case-insensitive match; none → error 0x162 "Имя канала задано неправильно."
  (never "token omitted" with CalcChannels).
- Per channel: idx = 0..N−1 (N = sample count); scal = s if F bit0 else 0,
  cplx = Z if F bit1 else 0; all-bit0 ∧ all-bit1 → scalar+complex channel;
  all-bit0 → scalar channel; all-bit1 → scalar := Re(cplx); neither → 0x42e
  "Результат не действителен". No per-sample catch.
- 0x30–0x39 retag every later CHANNEL token to 0x46 (scalar from `0x426120`:
  same as the sample unless the data is primary).

## The unit of a formula channel (`0x415bd0`, ledger L9.70)
Run once after the samples. If the token where the evaluation stopped is a
`[...]`, its text (case kept) is the unit. Otherwise a dimension walk
re-reads the formula from the start, mirroring the evaluator's levels, and
the unit is the text of the `[...]` where it stops, else the unit it carried:
- `+ − * /`: the left operand's unit, even when empty; `^`: none; a unary
  sign and groups pass it through; numbers, `pi`, `null`: none; a channel:
  its canonical unit ("" in ADC mode; an earlier formula channel's own).
- No level loops on a `[...]`, so the walk ends at the first one met outside
  a group (`a[кВ]+b` → кВ); inside a group the unchecked `)` step swallows
  it (`(a[кВ])*b` → unit of a).
- Functions: sin cos tg ctg sqrt exp arg fault cosui fourie → none;
  `frequency:` → Hz; ib 3i0 i1 i2 → A; ulf ull ua ub uc 3u0 u1f u2f u1l u2l
  uncn uniik → V; pf pf3wm pf2wm → W; zf zk zl → Ohm (string 3060);
  coeffpowerpriv coeffamppriv → «ном»; tormsumcur → its second argument's;
  the rest (re im mod rms a0 chan coeffpriv coefftt grs* tormsubcur
  tormmaxcur absig pk) keep the first argument's. absig, frequency, uncn,
  pk and the two reductions are not consumed by the function level, so the
  walk stops at their `(`.
- Quick analysis takes its units the same way (its formulas come from the
  «Добавить…» builder `0x4129e0`, e.g. S0 = `Mod:(Pf: (…))[VA]`).
- Display (`+0x78`) replaces an exact «В» by V and «А» by A; the canonical
  (`+0x7c`) keeps the text.

## Functions
- `ib:(a,b,c)` = c − (a + b) (Z and s), F = AND. `ulf:(a,b)` = a − b.
  `ull:(a,b)` = −(a + b).
- `ua:/ub:/uc:(e0..e4)`: channel check e0–e2; q = e4.s/e3.s (e3.s = 0 → 1e-6),
  W = q·e2; ua = (2e0 + e1 + W)/3, ub = (−e0 + e1 + W)/3, uc = (−e0 − 2e1 + W)/3;
  F = e0 & e1 & e2.
- `u1f:/u2f:(a,b,c)`: Z = (2a − b − c ± j√3(b − c))/6, s = |Z|, forced 3.
- `u1l:/u2l:(a,b)`: Z = (2a + b ± j√3·b)/6, s = |Z|, forced 3.
- `uncn:(e0..e4)`: Z = e1 + e2 − e0 + (e3 − e4)/√3, s = |Z|, forced 3.
- `pk:(a)`: s = Re(a)/|a| (1.0 if |a| ≤ 0.001), F = 1.
- `pf:(u,i)`: Z = conj(u)·i, s = 0, F = 2. `pf3wm:` Z = e0·conj(e1) +
  e2·conj(e3) + e4·conj(e5), s = 0, F = e0&e2&e4&2. `pf2wm:` Z = e0·conj(e1) −
  e2·conj(e3), F = e0&e2&2.
- `zf:(e0..e10)`: channel check e0, e1; if e10.s > |Z(e1)|/k(e1): e0 / 0
  (IEEE/complex divide by zero), F = e0.F & 3; else K0 = ((e4.s + j e7.s)/
  (e5.s + j e8.s) − 1)/3, Km = ((e6.s + j e9.s)/(e5.s + j e8.s))/3,
  Z = e0 / (e1 + K0·e2 + Km·e3), s = e0.s/e1.s, F = e0 & e1 & 2.
- `zk:(e0..e8)`: as zf with ZA = e4∠e6°, ZB = e5∠e7°,
  Z = e0/(e1 + e2·ZA + e3·ZB), min-current gate on e8.
- `chan:(c)`: channel check, returns c, forced 3. `uniik:(a,b)`: channel check
  both, a + b, forced 3.
- `coeffpriv:(ch,c)` = ch·c.s/s(ch); `coefftt:(ch,a,b)` = ch·(a.s/b.s)/s(ch);
  `coeffpowerpriv:(ch,c,A,B)` = ch·c.s/(A.s/(√3·B.s))/s(ch);
  `coeffamppriv:(ch,c,base)` = ch·c.s/base.s/s(ch); forced 3.
- `grstriangle a/b/c:(e0,e1,e2,N)`: N = round(e3); 0 → e0/e1/e2; 1 →
  (e0−e1)/√3, (e1−e2)/√3, (e2−e0)/√3; 11 → (e0−e2)/√3, (e1−e0)/√3, (e2−e1)/√3;
  other → e0; then /s(e0); forced 3.
- `grssumcur:(e0..e23)` = Σ r(e_i)·e_i / r(e0); forced 3.
- `tormsumcur:(e0..e24)`: s = Σ_{1..24} |r(e_i)·Z(e_i)| / r(e1) / e0.s.
  `tormsubcur:(a,b,c)`: s = |r(b)Z(b) − r(a)Z(a)| / r(a) / c.s.
  `tormmaxcur:(e0..e23)`: s = max |r(e_i)·Z(e_i)| / r(e0). Z keeps arg 0's
  phasor; forced 3.
- `grspos a/b/c:` PARTIAL — symmetrical components /3, zero sequence × round(e4),
  each rotated by integer powers (round(e3)) of 30° phasors; exact constants and
  per-variant exponents NOT DETERMINED.

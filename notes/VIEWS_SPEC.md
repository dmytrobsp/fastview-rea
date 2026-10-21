# Analysis views — behaviour of the original (ledger L8)

Distilled from the decompiled `FastView.exe`; addresses as evidence.

## Быстрый анализ (CQAnPropSheet `0x48b970`) — a channel generator
Six tabs (DIALOG 536 symmetric, 537 zero sequence, 538 phase-phase Z, 543 phase
Z, 539 differential, 340 formula). On OK only the active tab is used
(`0x48b710` → `0x48c0e0`): it yields formula strings + channel names, the names
to toggle (XOR) on the vector diagram (flag 0x4) and the circle diagram (flag
0x8), a vector-diagram angle reference + flags, and (Iдиф, Iторм) pairs for the
differential diagram. They are appended as ordinary computed channels
(`0x4167f0`; a taken name gets "_2", "_3"…, format "_%d"; validated by
`0x415780`, all removed on failure). Channel names in formulas are quoted;
"," in numeric fields becomes "."; an empty 3Io argument becomes "0";
"Отключить" checkboxes drop the current / voltage group.

- Симметричные (`0x48c2b0`): currents — "Рассчитать Ib по Ia и Iс": "Ib" =
  `Ib:(Ia,Ic,0)`, "I1" = `I1:(Ia,Ib,Ic)`, "I2" = `I2:(…)` (no 3I0); else "I1",
  "I2", "3I0" = `I1/I2/3I0:(Ia,Ib,Ic)`. Voltages — phase: "U1" = `U1f`, "U2" =
  `U2f`, "3U0" = `3U0:(Ua,Ub,Uc)`; line: "U1" = `U1l:(Uab,Ubc)`, "U2" = `U2l`.
- Нулевая (`0x4887c0`): "3I0" = `3I0:(Ia,Ib,Ic)` (расчётный) or `Chan:(3Io)`
  (измеренный); "3U0" = `3U0:(Ua,Ub,Uc)`, `Uniik:(Uни,Uик)` or `Chan:(3Uo)`;
  both → "S0" = `Mod:(Pf:(3I0,3U0))` [VA]. 3I0/3U0 → vector diagram, S0 →
  circle diagram; angle reference "3U0" (with voltages); show values on;
  both groups → circle readout Re + jIm.
- Междуфазные (`0x487640`): optional "Ib"; phase mode "Uab" = `Ulf:(Ua,Ub)`,
  "Ubc", "Uca"; line mode "Uca" = `Ull:(Uab,Ubc)`; "Zab" = `Mod:(Zl:(Uab,Ia,Ib,
  Iтр))`, "Zbc", "Zca"; Z channels → circle diagram (phase mode only), readout
  |Z|∠.
- Фазные (`0x48a4c0`): "3I0" = `3I0:(Ia,Ib,Ic)`; "по удельным параметрам":
  "Za" = `Mod:(Zf:(Ua,Ia,3I0,3I0n,R0,R1,Rm,X0,X1,Xm,Iтр))` (Zb, Zc likewise);
  "по коэффициентам компенсации": `Mod:(Zk:(Ua,Ia,3I0,3I0n,K0,Km,Ф0,Фm,Iтр))`;
  → circle diagram, readout |Z|∠.
- Дифференциальные (`0x47ff60`): 2–24 arms × 1 or 3 phases; per arm reduced
  current "I? плечо k прив." by `CoeffPriv / CoeffTT / CoeffPowerPriv /
  CoeffAmpPriv`; with group compensation "…прив. пов." via `GrsPosl{a,b,c}`
  or `GrsTriangle{a,b,c}`; "I? диф." = `GrsSumCur`; "I? торм." =
  `TormSumCur` (default) / `TormSubCur(arm1, arm2, Kторм)` / `TormMaxCur`;
  pairs → differential diagram.
- Формула: the whole computed-channel list editor (flag byte 7: replace the
  list, restore on failure).

## Vector diagram (`0x490360`)
Channels with flag 0x4; RMS phasor at cursor 1 (`doc+0x284`); NaN → 0. Angle:
absolute (raw sine-referenced) or relative to a chosen channel, wrapped to
(−π, π]; degrees " %8.2fº" or "radians" = value/π " %8.2fπ". Lengths: two
groups (unit exactly "A" vs the rest), scale = ⌈⌈max/p⌉/5⌉·5p with p =
10^⌊log10 max⌋, length = R·|z|/scale; legend "U max"/"I max". "Только
направление": full radius. "Показывать значения": table of magnitude/angle.

## Frequency diagram (`0x46f5a0`)
Channels with flag 0x2. Spectrum `0x426510(k, idx, P)`: window N·P samples
ending at cursor 1, bin = √2·√(C²+S²)/W (RMS), bin 0 = |a0|, the record's
harmonic-1 gain on bin 1 (even when P > 1). NumPeriods P (default 1),
NumHarmonics K (default 10, ≤ Nyquist); x labels k·f/P Hz; bars normalised to
the channel's largest bin; labels % of bin P (or of true RMS with UseRMS) or
absolute; format <0.01 "0", <10 "%1.1f", <100 "%2.1f", else "%10.0f".

## Per-channel readout (`0x461440`)
Display mode: instantaneous `0x426020` / RMS `0x4272a0(idx,1)` / first harmonic
`0x426240(1,idx)` (the plotted series follows it). Value at cursor 1 and
Max/Min over the record follow the mode; СКЗ = one-period RMS at the cursor,
A1 = first-harmonic RMS at the cursor. Format " %-8g %s" with SI prefix.

## Circle diagram locus (`0x43a880`)
Channels with flag 0x8: per-sample cached complex phasor (Re, Im) ("Ohm" and
other units on separate scales); R/X channel pairs (DIALOG 554): (R, X)
instantaneous. Samples N..end, or [min(c1,c2), max(c1,c2)] (≥ N) with
"Диапазон между визирами"; one polyline; "+" markers at c1 (red) and c2
(blue); readout at c1: Re ± jIm "%.3f" or |·| "%.3f" ∠ "%.2fº".

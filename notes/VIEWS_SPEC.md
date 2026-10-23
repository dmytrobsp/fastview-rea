# Analysis views — behaviour of the original (ledger L8)

Distilled from the decompiled `FastView.exe`; addresses as evidence.

## Быстрый анализ (CQAnPropSheet `0x48b970`) — a channel generator
Six tabs (DIALOG 536 symmetric, 537 zero sequence, 538 phase-phase Z, 543 phase
Z, 539 differential, 340 formula). On OK only the active tab is used
(`0x48b710` → `0x48c0e0`): it yields formula strings + channel names, the names
to toggle (XOR) on the vector diagram (flag 0x4) and the circle diagram (flag
0x8), a vector-diagram angle reference + flags, and (Iдиф, Iторм) pairs for the
differential diagram. They are appended as ordinary computed channels
(`0x4167f0`; a taken name — any record channel or formula definition, ignoring case (L9.95) — gets "_2", "_3"…, format "_%d"; validated by
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

### The sheet (ledger L9.51)
Every generated formula is built by the «Добавить…» builder `0x4129e0` in
its quick mode (`Fn: (a, b)`, the builder's quoting, same-batch channels
named as they will be appended); the forms, names and order per page are in
ledger L9.73.
- 137 (`0x4167f0`): a stack `CMFCPropertySheet` (`0x48b970`) shown with
  DoModal; IDOK → generate (`0x414da0`, `0x415780`), anything else → nothing.
  Look = Outlook bar, 150 px, on the left (`SetLook(1,150)`), icons BITMAP 549
  (11 × 32 px, magenta key); caption string 159 «Быстрый анализ»;
  PSH_NOAPPLYNOW, no help: only OK renamed «Рассчитать» (164) and Cancel
  «Отмена» (3328) (`0x48be10`). Opens on «Редактор формул» every time.
- «Рассчитать»: every page's OnOK `0x48b710` → `0x48c0e0` once, asking only
  the ACTIVE page; then each page saves its fields. «Отмена» `0x48b770`
  also saves the fields (no generation).
- OK greying: OnActivatePage `0x48c210` enables OK, then the page's check
  (slot 0x1d0, also on every combo / edit change `0x476230`): 536 `0x48d5a0`
  (each group not switched off needs its combos: Ib unless calculated; Ua–Uc
  or Uab/Ubc by the radio), 537 `0x48a1f0`, 538 `0x488550` (Iтр non-empty +
  combos), 543 `0x48b360` (combos, Iтр, the active group's edits, the
  parallel-line edits when 3Ion is used); 539 and 340 always enabled. No
  page blocks switching; no messages of their own.
- Page dependencies: 536 «Рассчитать Ib» greys Ib (`0x487d70`); Фазные /
  Линейные swap Ua–Uc / Uab,Ubc (`0x487de0`); each group's «Отключить» greys
  its block and the OTHER group's «Отключить» (never both, `0x48c870` /
  `0x48cb60`). 537 Расчетный / Измеренный (`0x488f50`), the 3Uo radio
  (`0x4890b0`), «Отключить» pair as 536. 543 удельные / компенсации
  (`0x48a910`); parallel-line edits only when 3Ion ≠ «Не учитывать»
  (`0x48ab80`). 539: the «Приведение» radio enables its edit block
  (`0x4835b0`…); 1 phase greys Ib/Ic and the group radios (`0x482c10`);
  3 phases: «По последовательностям» shows 0…11 + «Удалить I0», «Цифровой
  треугольник» {0,1,11}, «Не учитывать» 0…11 greyed (`0x483ab0`);
  «Полуразность» only with 2 arms, else forced to «Полусумма» (`0x482b60`);
  Kторм enabled for Полусумма or Полуразность with 2 arms (`0x483da0`);
  «<<» / «>>» greyed at the ends (`0x4831b0`). The method radios
  (reduction, group compensation, braking) are page-wide; each of the 24
  arm slots (`0x47fb40`) keeps Ia/Ib/Ic, the eight edits, two group numbers
  («По последовательностям» 2127 and «Цифровой треугольник» 2129, two
  combos at one place) and «Удалить I0», written by each control's change
  handler and restored on «<<» / «>>» (`0x482ca0`); a changed arm count
  keeps every slot (ledger L9.79).
- Defaults (`0x411f40`): R/X/K «1.», Ф/Rm/Xm/Km/Фm «0.», Iтр «0.25», Kторм
  {1, 0.5} = 0.5, arms 2…24 = 2, phases {1,3} = 3, «Удалить I0» on; 536 / 538
  Фазные, 537 Расчетный + «из Ua, Ub, Uc», 543 удельные.
- Persistence per record: the shared OnInitDialog `0x48b690` loads each page
  from the document's settings tree (doc+0x300, written to the `.set`,
  L9.17) — categories SymmetricItemsPage, NullSequencePage,
  InterPhaseResistPage, PhaseResistPage, DiffCurrentsPage (no method key
  → «По мощности» / «Не учитывать»; all 24 slots set to the initial arm,
  then each `Leg_%d` over its own slot, a missing value keeping the
  initial one; the last arm is shown); FormulaEditorPage has stubs (its list is the computed channels). A
  missing key keeps the default.

## Vector diagram (`0x490360`)
Channels with flag 0x4; RMS phasor at cursor 1 (`doc+0x284`); NaN → 0. Angle:
absolute (raw sine-referenced) or relative to a chosen channel, wrapped to
(−π, π]; degrees " %8.2fº" or "radians" = value/π " %8.2fπ". Values are the
shown ones (the cache carries k, L9.69). Lengths: two groups (unit exactly
"A" vs the rest), scale = ⌈⌈max/p⌉/5⌉·5p with p = 10^(⌊log10 max⌋ − 1)
(not > 0 → 0), length = R·|z|/scale; header: a two-row box at the top right,
«U max» / «I max» and "%.1f V" / "%.1f A" (never an SI prefix; L9.69). 0° points up, positive angles counter-clockwise; angle labels
inside the circle; a legend row (centred while the window is strictly
wider, else from m — overflowing to the right, L9.93) or (ShowAnglesValue)
a values table at the bottom; the circle is the largest that clears the header and the band
(L9.72). "Только
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
A pasted, not yet merged channel (bit 14, `0x461a24`) reads its own
samples at t = T(В1) − offset·10⁶ µs: the first own sample i ≥ 1 with
ct(i) ≥ t, j = i − 1, linear interpolation, a sample past the values
reading 0 (so the line through samples 0 and 1 extended backwards before
the paste, a fall to 0 within one own period after it, then 0); СКЗ / A1
at own indices j and i over round(own rate / 50) own samples, combined
R(j) + (R(j) − R(i))·frac; Max / Min over every finite own sample (ledger
L9.77, L9.85).

## Circle diagram locus (`0x43a880`)
Channels with flag 0x8: per-sample cached complex phasor (Re, Im) ("Ohm" —
the canonical unit, "" in ADC mode — on the Ω scale, other units on the
power scale); R/X channel pairs (DIALOG 554): (R, X) instantaneous
(`0x426020`, 0 past the values or NaN), always the Ω scale. Samples N..len
inclusive, or [min(c1,c2), max(c1,c2)] with "Диапазон между визирами", both
bounds raised to N and capped at the sample count — so the default locus
ends at a (0, 0) point; a NaN or past-cache point is the origin, the
polyline never breaks. Draw order: the channels' polylines, their "+"
marks (±4) at c1 (200, 0, 0) and c2 (0, 0, 200) at the raw cursor index,
then the pairs' polylines (no marks). Readout table `0x43bbf0` at c1:
Re ± jIm "%.3f" (both «0.0» at |z| = 0) or |·| "%.3f" ∠ "%.2fº", an empty
cell for a non-finite value, a pair in its R channel's unit; layout in
ledger L9.80.

## Differential plane (`0x445e00`, loci `0x4457a0`, table `0x446460`)
Pairs at `doc+0x878` are (Iторм `BrakeCurr`, Iдиф `DiffCurr`); texts put
Iдиф first («Iдиф | Iторм»). Each enabled pair: x = Iторм, y = Iдиф
through `0x426020` (0 past the values or NaN), the Iдиф channel's
colour, the circle's range rule and marks (ledger L9.81). Table (on / off):
rows «Iдиф Iторм», cells Iдиф then Iторм `"%.3f %s%s"` each in its own
channel's unit, the circle table's layout.

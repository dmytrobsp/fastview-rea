# L8 — Analysis views  ·  ✅ (quick analysis, vector, loci, frequency, readout)

Spec: **`docs/original/specs/views.md`** (CQAnPropSheet `0x48b970`, vector `0x490360`,
frequency `0x46f5a0`, readout `0x461440`, circle locus `0x43a880`).

Key correction: the original's **"Быстрый анализ" is a channel generator**, not
a panel of values. On OK the active tab emits computed channels (formula
strings in the formula language) plus vector/circle diagram flag toggles, an
angle reference and differential pairs; the values are read on the
oscillogram and diagrams like any channel. Our earlier tabs showed value
tables computed by our own analysis code — replaced.

Implemented: `fvb_core::quick` — `symmetric`, `zero`, `phase_phase`, `phase_z`
(K0 / Km compensation via `Zf:`/`Zk:`), `differential` (reduction CoeffPriv/
TT/PowerPriv/AmpPriv, group compensation GrsPosl/GrsTriangle, GrsSumCur,
TormSumCur/TormSubCur/TormMaxCur), `unique_name` ("_2", "_3"…),
`evaluate_batch` (ordered channel-mode evaluation, all-or-nothing, the
scalar/complex rule; computed channels keep their complex series as the
phasor — the original's stored-phasor path). `zl:` added to the channel
functions (scalar/flag rule inferred from the shared divide helper as for
`zf:`). GUI: `quick_ui.rs` forms per tab (channels default to the role map),
OK → `App::apply_quick` (append, XOR flags, angle reference, show values,
circle readout, differential pairs). Pinned by `tests/quick.rs` (6) and a
`zl:` test.

Not verified / residual: exact channel naming of the differential tab and the
`CoeffTT` argument order; units of generated channels (only S0 = VA is set);
Vector diagram (done): analog + computed channels with the vector flag, RMS
phasor at the main cursor (computed channels: stored complex series, else
DFT), NaN → 0, angle reference selector (absolute = raw sine-referenced, or a
channel; wrapped to (−π, π]), groups by unit exactly "A", `nice_scale`, angle
texts " %8.2fº" / value/π " %8.2fπ", direction-only, solid pen for "V",
30° radial grid (`fvb_core::views`, unit-tested). Loci (done): circle
diagram draws every channel with the circle flag as its phasor series
(N..end or between the cursors), "+" markers at both cursors, readout at
cursor 1 (Re ± jIm / |·|∠); differential diagram draws the (Iторм, Iдиф)
pairs. Not modelled then: separate scales for "Ohm" vs other circle channels
(settled by L9.80 / L9.83), R/X channel pairs (DIALOG 554), the vector table layout.
Frequency diagram (done): `signal::spectrum` = `0x426510` (P-period window,
bins k·f/P, bin 0 = |a0|, harmonic-1 gain on bin 1 even for P > 1), periods /
harmonic count (≤ Nyquist) / percent (of bin P or of true RMS) settings, bars
normalised to the channel maximum, `views::freq_label`. Readout (done): the
display mode drives the plotted series, the value at the cursor and Max/Min;
СКЗ/A1 always one-period RMS / first harmonic (with the record gain);
computed channels read out like any channel; text = `views::readout_text`
(`" %-8g %s"` + SI prefix, `fmt_g` emulating C `%g`). Residual: the default of
the percent switch and the stat-visibility controls (settled by L9.12 and
L9.177); MSVC
printf tie rounding not modelled.

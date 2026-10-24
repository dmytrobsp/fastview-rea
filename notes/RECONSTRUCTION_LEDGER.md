# Reconstruction obligation ledger — behaviour mapped to the decompiled original

## 0 — HOW A FRESH SESSION RECOVERS AND CONTINUES (read this first)

**Mission.** The user wants the original Mekhanotronika **Fast View 5** reversed
and mapped so there is *tangible proof* that this Rust reconstruction is
**behaviourally identical**, not approximated from screenshots. **Proceed
AUTONOMOUSLY until completion.** Update THIS ledger at every checkpoint so any
later cold session can resume from it alone.

**Project.** `~/projects/fastview-rea` — clean-room, cross-platform (egui/Rust)
reconstruction. Two crates: `fvb-core` (engine), `fastview-gui` (egui app).
Keep ALL of these green at every commit:
- `cargo build --workspace`  (0 warnings)
- `cargo test --workspace`   (currently 402 pass)
- `cargo clippy --workspace` (0)
- `cargo build --workspace --target x86_64-pc-windows-gnu` (0 errors)

**State at handoff.** See the "Next in L9 (HANDOFF …)" paragraph at the end
of L9 for the current state. Work commits straight to `main` (no feature
branches), every commit keeps the gates green, and the publish guard
(`tools/publish_guard.py`, pre-commit / pre-push hooks, CI job) must pass —
see `CLAUDE.md`: never commit the original program or raw decompilation /
disassembly / resource dumps; only the repo's configured email.

**Read these first (context you won't have):**
- Memory notes (in `~/.claude/projects/-home-poniraq-projects-fastview-rea/memory/`):
  `ghidra-rea-setup.md`, `fastview-rea-ui-state.md`, `wayland-gui-capture.md`.
- `notes/UI_SPEC.md`, `notes/FINDINGS.md`, and the local-only RE dumps in
  `notes/ui/` (`menus.txt`, `dialogs.txt` = 99 decoded dialogs, `strings.txt`,
  `accelerators.txt`). `notes/ui/` and `extracted/` are **gitignored proprietary
  material — never commit them**.

**Reverse-engineering tooling (REA + Ghidra 12.1.3).** The original binary is
`extracted/stage1/FastView.exe`
(sha256 `6c503790d13d9d2d0b80711f27b4daba57b209462cfb626cc5a6f719c150d046`,
image base `0x400000`, 9129 procedures, PDB not loaded → names are `FUN_xxxx`,
use RTTI strings + behaviour). To drive it, call the REA MCP tools:
`open_binary` (path above) once, then `search_strings` (param `pattern`),
`xrefs`, `resolve_containing_procedure`, `procedure_pseudo_code` (param
`procedure`), `read_bytes` (`address`,`length`). Tool JSON is verbose — extract
only the essential logic. If REA/Ghidra is unavailable, see `ghidra-rea-setup`
memory (Flatpak overlay + CLI wrapper `~/projects/slack-rea/tools/rea-ghidra.sh`).

**Per-engine working loop (repeat until the table below is all ✅/⛔):**
1. Decompile the original function(s) for the engine (addresses in the table).
2. Distil its exact behaviour (constants, order, rounding, degrees/radians…).
3. Compare to the Rust module; **reconcile** — fix the Rust to match, add a test
   that pins the behaviour.
4. `cargo test` + `cargo clippy` green (+ Windows build for engine changes).
5. Commit (end message with the Co-Authored-By line from the session reminder).
6. **Update this ledger**: the entry's evidence (address), what you verified, and
   the verdict. Append new entries as `L<n>`.

**NEXT TASK (start here) — L9 UI-layer program (2026-10-21).** The user (a
proficient user of the original) reports the UI still *behaves and presents*
functionally differently — not pixel issues — and directed that the code
earlier dismissed as "MFC boilerplate" be reversed. The UI layer is now an
engine like any other: see **L9** below for the inventory method (every MFC
message map → handler addresses, `tools/msgmaps.py`), the scope split, the
spec file `notes/UI_BEHAVIOUR_SPEC.md` and the per-item reconciliation status.
Work L9 before the older residuals below.

Older residuals (lower impact; triaged 2026-10-23 — the `.osc` Save-As
(`0x44ca80` writes the record's own bytes, ours copies the file), R/X pairs
on the circle diagram (DIALOG 554, `Doc::rx_pairs`) and the parallel-line
entry in the ОМП UI (L9.11 / L9.12, `omp_params`) are done): OSC time
stamps (read in L9.123's report: L9.124); the frequency-correction variant
(`FUN_004267e0`, `rec+0x874` — see the L9 handoff); (the 401–543 dialogs:
L9.123); a GUI screenshot pass (Wayland
capture is NOT usable unasked on this Hyprland desktop — new windows map
on another workspace and grim captures the user's own screen; ask the user
for a screenshot of the reworked views instead). Done: L2 ✅, L3 🟡 (readers ✅),
L4 ✅, L5 ✅, L6 ✅, L7 ✅, L8 ✅.

**Scope reality.** "Every byte" literally = 9129 funcs of mostly MFC boilerplate;
the *provable* target is the behavioural engines (this ledger's table). `.sto`
reading is ⛔ (no sample file ships — map the header from `GetInfoFromStoHeader`
but it can't be byte-verified).

---

Goal: behavioural parity **proven by decompilation**, not approximated from
screenshots. For each behaviour we record the original function (address +
Ghidra evidence), what it does, how our Rust implements it, and the verdict
(MATCH / PARTIAL / GAP / INFEASIBLE). Reversing is driven by REA + Ghidra 12.1.3
over `extracted/stage1/FastView.exe`
(sha256 `6c503790d13d9d2d0b80711f27b4daba57b209462cfb626cc5a6f719c150d046`,
image base `0x400000`, 9129 procedures). Raw decompiler dumps stay local
(`.gitignore`); this file keeps the distilled mapping (addresses, token tables,
behaviour, verdicts).

Scope note: we reverse the **behaviourally-significant engines** (formula
evaluator, `.fvb` parser, geometry/region engine, signal/analysis math, ОМП,
COMTRADE) **and — since L9 — the UI layer** (message-map handlers, view input
and painting, ribbon construction, panes, dialogs). The earlier stance that the
MFC message-map plumbing was boilerplate was wrong: the handlers behind it are
where the UI's behaviour lives. Pixel-exact layout remains out of scope;
behaviour and presentation semantics are in scope.

Status legend: ✅ MATCH · 🟡 PARTIAL · ❌ GAP · ⛔ INFEASIBLE (needs absent input).

---

## L1 — Formula language token table  ·  `FUN_00401000` @ `0x401000`

Evidence: Ghidra `procedure_pseudo_code` of `0x401000` (the static token-table
initializer) + `read_bytes 0x4c4dc4` (the token-name block). It registers token
ids 0x00–0x45. The full operator/function vocabulary of the formula language
(used by `.fvb` characteristic parameters **and** the Редактор формул):

Operators: `,` `+` `-` `*`(MUL) `/` `^`(POWER) `(` `)`
Math fns: `sin: cos: tg:`(TAN) `ctg:`(COTAN) `pi` `sqrt: exp: re: im: mod: arg:`
Signal fns: `cosui: absig: rms: fourie: a0: fault: frequency:`
Quantity accessors: `ib: 3i0: i1: i2: ua: ub: uc: ulf: ull: 3u0: u1f: u2f: u1l: u2l:`
Power/impedance: `uncn: pk: pf: pf3wm: pf2wm: zf: zk: zl:`
Coefficients: `coeffpriv: coefftt: coeffpowerpriv: coeffamppriv:`
Diff-protection: `grsposlia/b/c: grstriangleia/b/c: grssumcur: tormsumcur: tormsubcur: tormmaxcur:`
Misc: `chan: uniik:` ; token categories `NUMBER VARIABLE CHANNEL DIMENSION NULL_VALUE`.

Our impl (`crates/fvb-core/src/expr.rs`): `+ - * / ^` and `sin cos tg ctg sqrt
mod` only (6 functions).

Verdict: 🟡 **PARTIAL** (math part ✅ — see L2).
- ✅ The **shipped `FastView.fvb`** uses only `tg sin cos sqrt mod ctg` + operators
  and evaluates identically under the decompiled evaluator semantics (L2).
- ✅ `pi exp: re: im: arg: cosui: absig:` + `null` now implemented per L2.
- Correction: id 0x1a is `ib:` (no `ia:` exists); 0x1e/0x1f are `ulf: ull:`.
- ✅ (done in L6; triaged 2026-10-23: every name of the token table has an
  evaluator case in `expr.rs`) The **Редактор формул / computed-channel**
  vocabulary is far larger. Was missing:
  `pi exp: re: im: arg: cosui: absig: rms: fourie: a0: fault: frequency:` and the
  quantity/power/impedance accessors (`ua: ib: zf: pf: pf2wm:` …) and
  diff-protection currents (`grs*: torm*:`). A user formula using these fails in
  our editor but works in the original.

Action: implement the math functions (`pi exp re im arg`) and the analysis
accessors that resolve via the channel-role map + existing signal math
(`rms: fourie: a0: ua: ib: 3i0: i1: i2: u1f: zf: zk: zl: pf: pk: cosui:`), each
backed by decompiling its evaluator case. The remainder (`fault: frequency:
pf2wm: pf3wm: grs*: torm*: coeff*: uncn: uniik: chan:`) need their own per-function
decompilation (pending, listed below).

---

## Engines to reverse (entry points identified)

| Engine | Original | Our module | Status |
|---|---|---|---|
| Formula token table | `FUN_00401000` @0x401000 | `expr.rs` | ✅ L1/L6 |
| Formula evaluator (tokenizer/grammar/values) | `FUN_00418c40`, `0x419950`..`0x41f960` | `expr.rs` | ✅ L2 (numeric) |
| `.fvb` loader / evaluation / settings | `FUN_0040c7e0`, `FUN_00409260`, `FUN_00407750` | `parse.rs`/`model.rs`/`geom.rs` | ✅ L5 |
| Geometry / region engine | `Figure` vtables `0x4c3d90..0x4c3dcc`, paint `0x43a4a0` | `geom.rs` | ✅ L4 |
| DFT / phasors / RMS | `FUN_00426fc0` & channel methods | `signal.rs`, `freqcorr.rs` | ✅ L6 (freq-channel variant ✅ L9.65) |
| Symmetric components / impedances | Calculator `i1: i2: 3i0: zl: zf:` | `signal.rs`/`analysis.rs` | ✅ L6 + L8 (no dedicated view: the quick-analysis sheet makes formula channels, L9.123) |
| ОМП single/double/k0 | `0x4a00b0` engine | `omp.rs` | ✅ L7 (parallel-line UI ✅ L9.11/L9.12) |
| COMTRADE reader/writer | `0x450a30`, `.cff` `0x44fe30`/`0x44d0f0`, `.osc` `0x4524b0` | `comtrade.rs` | 🟡 L3 (`.cfg` ✅ `.cff` ✅ `.osc` read ✅, `.osc` Save-As ✅ copies the record's bytes) |
| `.sto` native reader | `GetInfoFromStoHeader` | — | ⛔ no sample ships |
| Formula forms, sheet pages, misc dialogs | DIALOG 401–543 | `addformula`, `formula_dlg`, `chandlg`, `quick_ui`, `recdlg` | ✅ L9.123 (every used template mapped) |
| Analysis views | CQAnPropSheet `0x48b970`, vector `0x490360`, freq `0x46f5a0`, readout `0x461440` | `quick.rs`, `views.rs`, GUI | ✅ L8 |
| UI layer (message maps → handlers) | 34 maps, CFastViewView `0x45a970`, CMainFrame `0x4720a0`/`0x4728d0` | `fastview-gui` | 🔄 L9 (L9.1 context menu ✅) |

Next: `.fvb` parser re-verify (attribute defaults, missing attributes, parameter
/var evaluation order), DFT/signal (incl. the Calculator channel mode + `rms:
fourie: a0: …`), ОМП, COMTRADE.

---

## L4 — Geometry / region engine  ·  ✅ MATCH (world-space; pixel quantisation not modelled)

Evidence (subagent decompile of FastView.exe; scratch at the session scratchpad,
not committed). Each primitive is a `Figure` subclass — header `{vtable, +4 mode,
+8 combine op}`; vtable slots 0 dtor, 1 clone, 2 evaluate exprs, 3 build HRGN
`(CDC*, double scale, POINT* origin, RECT* clip|NULL)`:
PolygonF `0x4c3dcc` (eval `0x409860`, rgn `0x409c00`) · PolypointF `0x4c3d90`
(`0x40acb0`/`0x40ae40`) · Circle `0x4c3db8` (`0x40a010`/`0x40a170`) · EllipseF
`0x4c3da4` (`0x40a3a0`/`0x40a5d0`). All regions: BeginPath/(Polygon|Ellipse)/
EndPath/PathToRegion (no CreatePolygonRgn/CreateEllipticRgn imports).

Parser mapping (`FUN_0040c7e0`): `mode` == "on" → 1, **anything else incl.
absent → 0 (off)**; `type` "and"=1 "or"=2 **"xor"=3** "not"=4 = CombineRgn
RGN_AND/OR/XOR/DIFF; absent `type` leaves +8 **uninitialised**; polypoint
`closed` == "off" → false, **anything else incl. absent → true**; line
`type` = `_wtoi`.

Behaviour:
1. Lines: type 1 `y = kx + b`, type 2 `x = ky + b`, k is a plain slope. Other
   types → every vertex touching that line = (0,0).
2. Polygon vertex[i] = line[i−1] ∩ line[i] (vertex 0 = last ∩ first); no
   parallel guard (inf/NaN → `cvttsd2si` 0x80000000), no clamping.
3. Circle: true radius; ellipse `width` (X) / `height` (Y) are FULL axes; angle
   degrees, positive = CCW in world (stored −angle·pi/180, applied to the GDI
   Bézier path in device space).
4. **Combination is SEQUENTIAL in document order, per mode**: two passes (off,
   on); `rgn = first figure of that mode (its type ignored)`, then
   `CombineRgn(rgn, rgn, fig, fig.op)` for each later figure. No grouping.
   ← our `geom.rs` groups `(base ∪ or) ∩ ands ∖ nots` — WRONG in general.
5. Polypoint `closed="off"` is **still a filled region** combined like any
   other figure: paint closes it through a box around the client area (end
   segments extended; region lies left of p0→pn−1) — our `geom.rs` treats it as
   a polyline only. ← WRONG.
6. World→device `px = R(x·s)+ox, py = oy − R(y·s)` (round-half-up); circle/
   ellipse bbox floor/ceil. Degenerate (zero-area) region → replaced by a 1×1
   pixel rect (so a zero-radius figure still *starts* a pass).
7. Paint (`0x43a4a0`): operate region filled with alpha 50 + FrameRgn outline;
   reset region outline only, drawn only if window flag +0x489; characteristics
   drawn last→first; nothing drawn when `scale·extent ≥ 800·printScale`.

Library scan (original UTF-16 `FastView.fvb`): no later figure lacks `type`
(the uninitialised-op path is never hit); every `mode` is explicit; no `xor`;
all 374 polygon lines are type 1/2; three characteristics start a pass with an
`and` (Эллиптическая ×2, РНМ) — harmless under first-starts semantics; **all 58
polypoints are `closed="off"` in DIF characteristics** — the original paints
them as filled operate/reset regions, we drew bare polylines (visible on every
differential characteristic).

Our impl (reconciled): `model.rs` — `Mode` "on" else Off, `Combine::Xor`,
`LineKind::Invalid`; `parse.rs` — `closed` false only for "off". `geom.rs` —
per-mode sequential CombineRgn fold (first figure starts, its type ignored;
degenerate figures start the pass as empty), the original's per-type-pair line
intersection formulas and vertex order (vertex i = line[i−1] ∩ line[i]),
invalid-type vertices at the origin, `close_open_polypoint` = the `0x40ae40`
closing (end-line intersection test, far box = view ± 2 view-sizes, nearest
outward hit, backwards corner walk incl. the same-edge full-loop rule),
`evaluate` (no view = autoscale pass, direct close) vs `evaluate_in_view`
(paint pass). GUI: view-dependent characteristics re-evaluated against the last
plot bounds; autoscale uses the view-independent extent (no feedback loop).
Pinned by `tests/geometry.rs` (12 tests incl. sequential-vs-grouped,
first-type-ignored, xor, defaults, vertex order, same-edge corner walk, every
shipped DIF characteristic fills). Headless render of block 4 (transformer
ДЗТ/ДТО) shows operate regions above the restraint curves.

Residual / not modelled (honest):
- Integer-pixel GDI regions (round-half-up, floor/ceil bboxes, Bézier ellipse)
  vs our world-space polygons: edges may differ by < 1 pixel; the 1×1-pixel
  stand-in for degenerate figures is treated as empty.
- Parallel consecutive polygon lines: original → inf → `0x80000000` → GDI
  failure; we drop the vertex (graceful divergence on invalid input).
- Absent `type` on a non-first figure: undefined in the original (uninitialised
  heap); we union. Not reachable with the shipped library.
- `mode`/`type`/`closed` string compares: Ordinal_2948 case-sensitivity NOT
  DETERMINED; we compare case-insensitively.
- Paint details (presentation, not geometry): characteristics drawn last→first,
  operate fill alpha 50, reset outline only and gated by a window flag, nothing
  drawn when `scale·extent ≥ 800·printScale`, the 9-entry colour table
  `0x4c3030`. The GUI after this change was NOT screenshot-verified (the
  Wayland capture missed the window this session); the headless example render
  was checked instead.

---

## L5 — `.fvb` loader + characteristic evaluation + settings grid  ·  ✅ MATCH

Evidence (subagent decompile; MFC ordinals resolved against the bundled
mfc120u.dll 12.0.40649 export table: Ordinal_2948 = `CString::Compare`
(wcscmp), 2954 = `CompareNoCase`, 8594 = `MakeLower`):
- Loader `FUN_0040c7e0`: in-house XML tree (`FUN_004aa220`, nodes {tag,
  children, std::map<CString,CString> attrs}); loads **every `*.fvb` in the exe
  directory** (FindFirstFileW). Walked by structure — block/characteristic/line
  tag names never checked; characteristic children dispatched by Compare
  against parameter/var/polygon/polypoint/circle/ellipse, others ignored.
  `FUN_00408ab0`: exact-key lookup, **missing attribute = empty string**,
  stored as text (nothing evaluated at load). All value compares exact and
  case-sensitive; `dimension` only `Compare(.., L"DIF")` (0x4c3c28). Flag 0
  (CircleDiagramWnd 0x43608f/0x43641e/0x437691) keeps non-DIF, flag 1
  (DiffDiagramWnd 0x44227f/0x4424fe/0x443191) keeps DIF; empty blocks dropped.
- Evaluation `FUN_00409260` (only from `FUN_00437520`, then InvalidateRect):
  per characteristic: clear its map; each parameter's grid value text →
  `SetVariable` (`FUN_004083b0`: lowercase name, tokenize+evaluate, insert/
  overwrite); each var likewise; then each figure's evaluate. Single pass
  (defaults see earlier params; vars see params + earlier vars); last
  assignment wins. **One try/catch around the whole loop** (0x40948c → message
  0x40A to the error window): the first error abandons the rest of the block —
  later figures/characteristics keep previous values. Triggered by
  `BlockPropertyGrid::OnPropertyChanged` (0x407580), block selection (0x407ab8)
  and diagram init (0x436b30/0x442a10) — not by paint.
- Error text `"<formula>: <msg>"`; string table (read from the PE): 0x157
  "Синтаксическая ошибка в формуле", 0x42f "Переменная не найдена", 0x430
  "Неправильное число переменных", 0x431 "Аргумент функции должен быть
  каналом", 0x42e "Результат не действителен". An empty formula → 0x157.
- `BlockPropertyGrid` (`FUN_00407750`): property 0 = block selector; one group
  per characteristic, parameters as **string** properties (COleVariant BSTR =
  default text, no mask/min/max/format) re-evaluated as formulas. Persistence
  (`FUN_00408cf0` save / `FUN_00409030` load): settings tree section "Circle
  Chart" / "Diff Chart" → "Selected Block" → per characteristic `visible` and
  per parameter `value` text (backing file NOT DETERMINED).

Our impl: `parse.rs` rewritten as a structural DOM walk with exact compares
and lenient (`Expr::lenient`) expression storage; `model.rs` exact
`Dimension`/`Mode`/`Combine`, `_wtoi` line types, `Settings` = name → formula
text, `Characteristic::context` (single pass, `FormulaError` with the
original's message), `Library::for_dimension`, `Library::extend`; `geom.rs`
`FigureValues` + `evaluate_figures` (stop at first error, keep stale) +
`evaluate_block` (abort the block) + `build_geometry`; `ExprError` messages =
the original strings. GUI: separate circle/differential diagram state, each
with its own filtered library (fixes the two mixed blocks "Защита
электродвигателей/генераторов (с 2025 г.в.)" whose OHM and DIF characteristics
we drew on one plane); settings grid = groups per characteristic with text
values, committing re-evaluates the whole block and shows the error text; all
`*.fvb` beside the executable are loaded (bundled copy as fallback). Pinned by
`tests/loader.rs` (7) + 2 GUI tests.

Residual / honest:
- Persistence ✅ (behaviour): per diagram ("Circle Chart"/"Diff Chart") the
  selected block, each characteristic's visibility and parameter texts are
  saved on every change and restored (`persist.rs`, GUI test). Storage differs:
  a TSV file in the platform config dir (the original's settings-tree file is
  NOT DETERMINED), keyed by (diagram, block, characteristic) where the
  original's exact key nesting is ambiguous.
- Figure-granular staleness: the original updates a failing figure's fields up
  to the failing attribute; we keep the whole figure's previous values.
- Never-evaluated figures: original's zero-initialised fields → we treat as
  degenerate.
- ~~Our extra "Сбросить уставки" button and per-characteristic visibility list
  are conveniences~~ — superseded by L9.153 (the original's property grid
  with a tick box per characteristic; the button is gone).
- GUI change not screenshot-verified this session (Wayland capture missed).

---

## L6 — Signal engine (channel methods)  ·  ✅ (freq-channel DFT ✅ L9.65; 0x46 retag ❌)

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

Channel functions completed per **`notes/CALC_CHANNELS_SPEC.md`**: value flags
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
---

## L7 — ОМП (fault location)  ·  ✅ engine + GUI (parallel-line UI ✅ since L9.11/L9.12)

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
  secondary NOT DETERMINED (no ratio applied in ОМП code).
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
  (Takagi-type, θ from taps NOT DETERMINED); section walking with voltage drop
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
grounded/isolated section distance, θ, median) is kept in **`notes/OMP_SPEC.md`**.

Implemented (`omp.rs`, new per-sample engine; the old single-instant
functions remain only until the GUI moves over): `omp_phasors`, components,
grounded detection with hysteresis + `debounce`, phase selection, loop choice
(grounded/isolated), grounded and isolated per-sample distance with section
walking, voltage drops, taps, parallel-line term, clamp and the over-count
quirk, `median`. θ fixed at 0 (the tap re-estimation needs the unresolved
`0x49bd40`). Stage 2 (spec §8–§13): `fault_interval` (first qualifying run),
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

---

## L8 — Analysis views  ·  ✅ (quick analysis, vector, loci, frequency, readout)

Spec: **`notes/VIEWS_SPEC.md`** (CQAnPropSheet `0x48b970`, vector `0x490360`,
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
pairs. Not modelled: separate scales for "Ohm" vs other circle channels
(NOT DETERMINED), R/X channel pairs (DIALOG 554), the vector table layout.
Frequency diagram (done): `signal::spectrum` = `0x426510` (P-period window,
bins k·f/P, bin 0 = |a0|, harmonic-1 gain on bin 1 even for P > 1), periods /
harmonic count (≤ Nyquist) / percent (of bin P or of true RMS) settings, bars
normalised to the channel maximum, `views::freq_label`. Readout (done): the
display mode drives the plotted series, the value at the cursor and Max/Min;
СКЗ/A1 always one-period RMS / first harmonic (with the record gain);
computed channels read out like any channel; text = `views::readout_text`
(`" %-8g %s"` + SI prefix, `fmt_g` emulating C `%g`). Residual: the default of
the percent switch and the stat-visibility controls (NOT DETERMINED); MSVC
printf tie rounding not modelled.

---

## L9 — UI layer (message maps, view input/paint, ribbon, panes, dialogs)  ·  🔄 IN PROGRESS

**Why.** The UI had been reconstructed from help screenshots + PE resources
(menus, dialogs, strings, accelerators) while the handlers that give it its
behaviour were never decompiled. A proficient user of the original found
functional differences in behaviour and presentation.

**Inventory method (deterministic, repeatable).** `tools/msgmaps.py <exe>
<out.json>` (pure-stdlib PE parse): RTTI type descriptors → complete object
locators → vtables; `mov eax, imm32; ret` getters whose target is an
`AFX_MSGMAP {pfnGetBaseMap, lpEntries}` → the 24-byte `AFX_MSGMAP_ENTRY` tables;
getter ↔ class via the vtables that contain it. Result over FastView.exe: 165
RTTI classes, 106 vtables, **34 message maps / ~350 handlers**.
`tools/show.py [Class…]` prints a map with ids resolved via the local
`notes/ui/` dumps; `tools/vt.py` lists vtables (for OnDraw/OnUpdate etc. diff a
class's vtable against its MFC base). Key maps: CFastViewView 50 entries
(getter 0x45a970, vtable 0x4c9774), CMainFrame 25 (0x4720a0, ribbon built in
WM_CREATE 0x4728d0), CFastViewDoc 15 (0x44ade0), CircleDiagramWnd 25 /
CircleDiagPane 18, DiffDiagramWnd 23 / DiffDiagPane 16, VectorDiagramWnd 15 /
VectorDiagPane 10, FourieDiagramWnd 8, the CQAn* pages, ChannelsDlg,
FileFinder22Dlg. Classes with no getter found by the pattern (handlers to be
found via vtable/DDX): CCommentDlg, FileInfoDlg, CParamOscBar, CMsgCaptionBar,
the three diagram toolbars, WrongPlaceLocatorDlg (likely the unattributed map
at getter 0x4a7a41).

**First observation (read, 0x45d990 WM_MOUSEWHEEL).** Alt held → command
32800/32801 (vertical zoom in/out); MK_CONTROL → 32802/32803; MK_SHIFT →
horizontal scroll (position in 1/10000 of the record); plain wheel → vertical
scroll of the lane stack. Structural divergence noted before reversing: the
original shows diagrams as **docking panes** (CircleDiagPane, DiffDiagPane,
VectorDiagPane, CFramePane<FourieDiagramWnd>, each with its own toolbar) next
to the oscillogram; ours shows them as full-window tabs.

**Scopes (research pass, reports in the session scratchpad, distilled into
`notes/UI_BEHAVIOUR_SPEC.md`):** A view input (mouse/keys/scroll/cursor/hit
geometry) · B view painting (lanes, stats panel, axis, визиры, comments) ·
C view/doc COMMAND + CMDUI rules · D main frame: ribbon construction, panes,
status bar, app · E diagram windows/panes/toolbars UI · F dialogs and bars.

Status: research pass DONE (7 reports: A input, B paint, C commands, D frame/
ribbon/panes, E diagrams, F dialogs/bars, G ОМП UI), distilled into
**`notes/UI_BEHAVIOUR_SPEC.md`** (§1–8 behaviour, **§9 ranked reconciliation
backlog — work it top-down**, §10 unknowns; the scratchpad reports are
temporary). Reconciled so far: **L9.1**–**L9.22** below.

### L9.1 — Oscillogram context menu + channel selection  ·  ✅

User report (2026-10-21): the right-click menu only looked right near the top;
opened lower it became long and a few characters wide and stayed so until a
restart; its subject was never the top row and shifted as the mouse moved over
the menu. **Root causes (ours, not the original):** (1) the menu's subject was
recomputed every frame from `resp.hover_pos()`, which is `None` while the
pointer is over the popup (→ fallback to the view centre) and follows the
pointer elsewhere; (2) egui 0.29.1 gives every context menu ONE shared popup
`Area` whose remembered size is never dropped (`Areas::end_pass`) and which
lays out inside that size each frame — one frame with a near-empty menu (e.g.
a digital row's stats panel at the bottom) ratcheted the width down for the
rest of the process (egui's own doc on `AreaState::size`: "not persisted
between sessions, so that a bad … menu size won't be remembered forever").

Original behaviour (read): `WM_RBUTTONDOWN 0x45a8d0` = the left-button handler
`0x45dc10` (select-first: an unselected row becomes the only selection, a
selected row keeps the multi-selection; plain/Ctrl/Shift click = only/toggle/
range from the anchor, the anchor set only when a row becomes selected; a click
on the time-axis band deselects all). `WM_RBUTTONUP 0x45efa0` picks the menu by
region: axis band (y > client − 30, or − 75 with doc+0x870) → MENU 560; stats
panel (x < view+0x108) → MENU 561; plot → MENU 563 only when a comment is under
the pointer (relabelled by kind: strings 205–209/229/236–238; 33042 removed
unless a level line), otherwise **no menu** — the right button places визир 2.
Menu 561 commands act on the **selected** channels (`+0x100`): diagram flags /
ADC / invert / DC toggle-all (`FUN_00430340`, `FUN_0042f4b0`), display modes
per channel incl. group members (bits 7–9), hide, reset zoom (141), combine
(33035: ≥2 selected visible analog, same non-empty unit) / split (33037),
colour (33030 = CColorDialog from the first selected channel).

Ours now: the subject is latched at the right-click via `lanes::hit_test`; each
opening gets its own popup `Area` (fresh id per right-click, no text wrapping),
closed by an item, Escape or a press outside (that press is swallowed);
selection model `chansel::select_click` + `Doc::selection`/`sel_anchor`
(selected rows drawn on a gold background); MENU 560/561/563 in resource order
with the CMDUI enable/check rules; per-channel display mode (`ChanView::mode`,
ribbon toggles checked iff all selected agree); channel groups
(`ChanView::group`, replacing "overlay the previous lane"); comment lock flag;
155 «Момент времени» placement mode (next plot click places a moment); copy for
merge takes the selection; colour dialog. Pinned by `chansel` tests (4) and
headless egui tests driving `lanes_ui` (`lanes::menu_tests`, 5) — the sizing
test fails when the shared-area cause is reintroduced (176×526 px vs 317×408).

Residual (honest): «Абсолютное время» and «Разница между визирами» shown
disabled (not implemented); vector/circle unit eligibility of the diagram flags
not applied; level/area comment kinds (156/157), comment dragging and the
in-place level edit not done (33042 opens the editor); undo still snapshots
some view edits the original does not undo; per-channel display mode default =
instantaneous is **inferred** (not read); horizontal визиры get one ribbon
toggle instead of 32815/32816; the selected-row gradient is drawn flat.
### L9.2 — ОМП double-ended: end-2 currents negated  ·  ✅ (engine)

Found by the ОМП-UI reversing (scope G), verified by hand: after loading the
two stations' channels by name (station 1 via `esi+0xc` → Ua/Ub/Uc/Ia/Ib/Ic at
`+0x100/+0x10c/+0x118/+0x88/+0x94/+0xa0`; station 2 via `esi+0x38` → Ia/Ib/Ic
at `+0xb8/+0xc4/+0xd0`), `0x4a13a6`..`0x4a1409` passes station 2's three
current vectors to `0x4a2200`, which multiplies every element by −1.0
(`0x4d1890`; SSE pair `0x4d1a60` = (−1,−1)). `fvb_core::omp::double_ended`
now negates end-2 current samples first; `tests/omp_engine.rs` records end 2
in the standard into-the-line convention (the unfixed engine located an 18 km
fault at 1.18 km). Spec: `notes/OMP_SPEC.md` §13.

### L9.3 — Diagrams as docking panes beside the oscillogram  ·  🟡

Original (read, `0x474e10`, `0x475040`/`0x4750b0`, CMDUI `0x4750f0`; help
screenshot clip0014): four CDockablePanes created visible — Vector and Circle
tabbed, Differential, Frequency at the bottom — with captions (pin, close =
hide); the oscillogram never disappears; 32809/32808/140/32811 toggle a pane
and are checked while it is shown. Ours now: `App::panes` (`Panes`, all
visible, Vector the active tab), `Command::TogglePane`/`ActivatePane`; one
right-hand column (500 px, resizable) with the Векторная | Круговая |
Дифференциальная tab group (tabs at the bottom, caption + ✕) and Частотная
below it (200 px splitter), as in clip0014; the circle/diff pane puts the
block/settings grid above the plot behind a 200 px splitter (50 .. h − 150);
ribbon diagram buttons toggle with check. `View` keeps only Oscillogram /
Analysis / Omp. Tests: pane toggle/activation semantics + a headless render of
every arrangement at 1320×860 and 866×500.
Residual: no floating / re-docking / auto-hide pin (egui has no docking; the
saved Feature-Pack workspace is not modelled); the original's first-start
Differential is a separate pane on the right edge (exact split NOT
DETERMINED) — we follow the screenshot's single tab group; the vertical pane
toolbars (550/552/553) and pane contents are item 19/20+ of the backlog;
Быстрый анализ and ОМП still replace the central view (backlog 6, dialogs).

### L9.4 — Oscillogram view geometry, визиры, wheel, ruler, status  ·  🟡

Read (spec §3; checked by hand: `0x45c4f0` zoom steps, `0x45c430` fit,
`0x45ab70` initial update, `0x449170` визир block, `0x45d990` wheel,
`0x45dc10` press regions): sample-indexed horizontal scale (first sample,
power-of-two f ≤ 1, 25·f px/sample; initial f = stored `View/XScale`, else
**2 × fit**); fixed lanes (analog `YAnalogSize` 93, ×/÷1.3 within 70–500;
digital 20; no stretch) under a fixed bottom band (ruler 30 + difference band
45, on by default); per-lane min..max amplitude with 5 px margins; every
sample drawn, circles above 4 px/sample; full-width lane backgrounds (white /
cyan gradient / gold gradient when selected), 50 px vertical grid, panel
divider; визиры hidden at document open (all zero, lane −1), В1/В2/В3 1 px in
(255,100,100)/(100,100,255)/(0,160,0) with labels and **no shading**; ГВ1/ГВ2
per channel; press (not click) semantics: LMB places В1, RMB В2, Shift+LMB/RMB
ГВ1/ГВ2 in the analog lane under y, a shown визир within 3 px is grabbed and
dragged, **the plot never pans on drag**; wheel Alt = lane height, exactly
Ctrl = f ×2/÷2 anchored on В1>В2>В3, exactly Shift = one screen, else lanes
scroll 120 px/notch; ←/→ В1 (Ctrl В2), ↑/↓ ГВ1 (Ctrl ГВ2); cursor shapes; ruler
ticks every 25 px (10/5/7/5), labels every 100 px relative to the record start,
unit on the first label only, space-grouped µs/ms, `%.3f` s, samples k+1;
difference band labels and Δt dimension lines; status bar = six complete
strings (caption only while hidden); 32813/32814/139/32815/32816 toggles
(first show at ¼/½/¾); 32800–32803 ribbon zoom; `View` settings persisted;
ОМП puts В1 at the window start and В2 one period later (shown); inline rename
by double-clicking the name badge (message 397 on a duplicate).

Ours: `oscgeom.rs` (pure: fit/initial scale, x↔sample with ties up, visible
count, zoom steps, lane zoom, value↔y, ruler ticks/labels; 6 tests), `lanes.rs`
rewritten on it (own scrollbars, `lane_layout`, `Row` groups), `Doc`
(`first`, `xscale`, `vscroll`, `diff_band`, `v_shown`, `v3_cursor`, `hviz`,
`toggle_vizir`, `zoom_x`), `App` (`y_analog`, `xscale_pref`, `vizir_drag`,
`rename_edit`, `persist_view`), `persist::Store::view`. Headless egui tests:
fixed lane heights, place В1 + grab-drag without panning, wheel modifiers,
arrow keys, plus the L9.1 menu tests rebased on the new layout.

Residual (honest): stats panel content/width still ours (backlog 13:
LEFT_W 158, rows always shown, our fonts); trigger sample = record start
(COMTRADE has no `doc+0x214` writer); absolute-time ruler not implemented;
визир label merge/clamping and the В2=В1 pen not modelled; comment drag,
level/area kinds and the hover popup pending (backlog 14); digital
pixel-OR quirk approximated by drawing overlapping run rects; window zoom
(138/`ch+0x114`) not implemented (vzoom ignored); min..max and every-sample
drawing recomputed each frame (no cache); the free-text status message is
still shown at the right of the status bar (the original has none).

### L9.5 — Stats panel, SI units, cut-name hover  ·  ✅ (glyphs ours)

Read: `0x461440` stats text (name cut to 25 characters in its colour, framed;
value at В1 `-%-6g %s` / ` %-6g %s` on |v| in (69,85,199) after the label
column; rows Max/Min (bit 4), СКЗ (bit 6), A1 (bit 5) only when set, ` %-8g
%s`); `0x4606d0` widths (label column = widest cut name + 10; panel = label
column + value template, `": -0.00000000 A"` for SI units else
`": -0.0000000E-000 "`; 125 before measuring); `0x460290` lane need (name
row + 10 + one row per stat (Max/Min two) + 23 with status icons);
`FUN_0042f4b0` 32817–32819 toggle-all over all analog channels; **`0x43d960`
SI prefix only for V, A, VA, W, ОM (Cyrillic О), Ohm, OHM, var, F, H, GN, Hz,
m (exact case-sensitive `CString::Compare`, verified by hand), else the raw
value**; `0x469060` hover shows a cut name in full. Help screenshot clip0014
(cropped with `tools/pngcrop.py`) confirms the column layout and shows СКЗ +
A1 on, Max/Min off.
Ours: `fvb_core::views::{si_unit, si_scaled, stats_value_text}`, readout
`%-8g` sign rule; `StatRows` on `ChanView`/`Computed` (default СКЗ+A1 —
**inferred** from the screenshot), `Doc::toggle_stats_row`,
`Command::StatsRow` + three ribbon toggles; `lanes.rs` stats drawing, fitted
`view_left` (replaces the fixed 158 px), lane need, status-icon row (our own
glyphs, not bitmap 0x136), hover popup. Tests: SI/format cases, toggle-all +
lane growth, width fits the cut name.
Residual: egui has no Tahoma/bold (bold simulated by double drawing); (group
lanes' per-member blocks: done in L9.50).

### L9.6 — Comments: moments, level lines, areas  ·  🟡 (no .set persistence)

Read (spec §5: `0x43eb40` placement, `0x43ef50` release, `0x43efa0` drag,
`0x43f630` paint, `0x43df90` info box, `0x45efa0`/`0x45ce40` MENU 563):
three kinds with text / locked / text-hidden; ±10 px hit tests (moment by x,
level by its segment with parts left end / right end / body, area on its
border band only with side edges draggable); one-shot tools 155/157/156
(moment in one click; level on an analog lane and area on any lane dragged
out from the press, normalised on release); LMB on an element grabs it
(corrected in L9.60: the text flag plays no part), locked ones do not move; hover = selected (3 px (192,0,0),
info box with «Время:» / «Уровень: %.2f» / «Длительность: %d.%03d с»);
moment marks feed the difference band; MENU 563 labels per kind
(визир / линию уровня / область), «Изменить уровень» only for levels as an
in-place `%.2f` edit; double-click opens «Комментарии к диаграмме».
Ours: `comments.rs` (`Comment`, `CommentKind`, `hit`, `drag`, `normalise`,
`paint`, the item-mode dialog; 2 unit tests), `lanes.rs` placement / drag /
hover / level edit, three ribbon tools. Headless test: one-shot tools,
level/area drag-out, moment drag, lock, menu by kind.
Residual: (saving / loading comments in the `.set`: done in L9.17); (the moment info
box's absolute time: done in L9.47); bubble marks are
our own drawing; digital level-line snapping not modelled (levels are
analog-only).

### L9.7 — Channel value flags reach every consumer; window zoom; discrete visibility  ·  ✅

Read: **`0x426020`** (assembly; the decompiler lost the x87/SSE flow) — a
channel's value is `x·k`; with ADC the integer code; with remove-DC minus
`k·ch+0xf8`; with invert negated — and it is what the phasor DFT
(`FUN_00426fc0` calls it), RMS, harmonics, Calculator channels and ОМП
read. **`ch+0xf8` is ONE constant** (`0x428795`): the mean of the first
`N − N mod P` samples (whole periods from the start; of the ADC codes in ADC
mode). The original therefore flips an inverted channel's phasor on the
vector/circle diagrams and re-evaluates formula channels and ОМП (full
recompute after 32823/32824/32852 and primary/secondary; no undo).
Window zoom 138: `0x45e6b0` mode 8 turns the rectangle's y-span into lane
fractions a < b and applies `0x42afb0` to the channel and its group from the
current range (nested): window `[lo + (1−b)(hi−lo), hi − a(hi−lo)]`, kind =
display mode at zoom time; `0x42b640` draws in the window, adapted when the
mode changed (instantaneous window shown as RMS/harmonic → `[0, max|·|]`;
RMS/harmonic window shown instantaneous → `±1.5·hi`); equal ends reset; 141
resets the selected channels and groups. 32845/32846/32847 act on discrete
channels only (show / hide + deselect / hide unchanged).
Ours (were wrong: sliding DC per sample; diagrams/ОМП/computed read raw
samples; per-channel ×1.25/0.8 vzoom; «Показать/Скрыть все» on analog
channels with undo): `Doc::flagged` (record with flags applied,
`flag_series`, `whole_period_mean`), `Doc::recompute` (RMS, phasors, formula
channels re-evaluated in order) + `App::recompute_active` (re-runs ОМП when
shown); ОМП reads the flagged values; `oscgeom::{Window,
window_from_fractions, effective_range}`, `ChanView::window`, `RectZoom`
armed/dragging with the rubber line and up-arrow cursor, `Doc::apply_window_zoom`;
ribbon 138/141 and selection-based invert/DC toggles (`Command::ToggleFlag`,
checked iff all selected agree); `Doc::dig_visible` + `Command::Discrete`.
Tests: invert negates value, phasor and formula channel without undo; DC =
whole-period mean; discrete show/hide/unchanged; rectangle zoom window + 141.
Residual: the frequency diagram still reads raw samples (track D rewrites it
— switch it to `Doc::flagged` after the merge); ADC + primary scaling edge
(the original subtracts `k·dc` from an unscaled code: done in L9.57); undo for
merge operations only (paste) is now the only snapshotting command but the
Отмена/Возврат enable-by-count rule is pending (ribbon item 11).

### L9.8 — Document tabs, window title, start-up  ·  🟡

Read (spec §1.1/§1.5; help screenshot clip0014 sampled with
`tools/pngcrop.py`): OneNote-style tabs (slanted left edge, gradient
(255,255,146)→(255,220,120) for the first document, bold title = the file
name without extension, ✕ on the active tab only, ▾ list); window title
«Механотроника FastView - <doc>» (`0x45f7d0`); command line `-Dst <dir>`,
`-Find`, the file argument; DragAcceptFiles; minimum size 866×500
(`0x4728a0`). Ours: `main.rs` `doc_tabs`, `tab_color`, `startup_args`
(tested), dropped files, viewport title. Residual: (the auto-colour order:
read from mfc120u in L9.63 — the assumed OneNote palette was wrong); the 2 s
splash not implemented (its bitmap 0x1CE is a proprietary asset); single
instance done in L9.18 (`instance.rs`: loopback port + token file instead of
the stored HWND / WM_COPYDATA, the newest plain start wins, a file argument
is handed over and the start exits once accepted); (new tab groups: none
in the original either, tab reordering: L9.108); («Окна...» in the ▾
list: done in L9.64).

### L9.9 — «Проводник» as the original's modal finder  ·  🟡

Read (spec §7.5 and the scope-F finder sub-report: `0x46b9c0` scan thread,
`0x46bec0` by the last three characters, `0x46c080` COMTRADE header — station
= token 1, block = token 2 or «-», start time `.cfg` month first / `.cff` day
first, swapped when month > 12, +2000 below 100; `0x46c730` OSC «OSC
Осциллограф. Версия %i.%i» with the time from header bytes 6..11; `0x46cb20`
live filters, station/block exact or «Все», `С ≤ t ≤ По + 1 day`; sorting,
«Время» by time of day, every scan forces Дата ascending; `0x46e120` opens
every selected file; `0x46d4b0` deletes the file and its existing companions
(cfg → dat hdr ini inf set; cff, osc → set; sto → dst); `FileFinder`
persistence). Ours: `finder.rs` (header readers, `scan` on a worker with
cancel + join, `passes`, `companions`, the dialog with folder list,
«Выбрать», subfolders, combos, С/По, sortable columns, multi-select, open,
delete with confirmation), replacing our docked folder panel; `-Find` opens
it. Tests: header dates (cfg/cff order, swap, +2000, «-»), OSC header,
filters incl. the inclusive По day, scan depth + companions.
Residual: `.sto` entries not read (no sample `.sto`; `0x46c960` converts
it like the open does); (delete through the shell / recycle bin: L9.49;
the folder tree, «Выбрать», the date pickers: L9.116).

### L9.10 — Loading on a worker thread with the «Загрузка» dialog  ·  🟡

Read (spec §7.6): parsing always on a worker thread; the modal dialog only
when the `.dat` of a `.cfg` or the `.cff` exceeds 200 000 bytes, or the `.osc`
20 000; «Отмена» silently abandons the open. Ours: `App::load_comtrade_path`
spawns the parse, `poll_loading` adds the document when it arrives,
`loading_shown` applies the thresholds, `loading_window` shows an
indeterminate bar with «Отмена»; re-opening with another encoding stays
synchronous (`load_comtrade_now`). Residual: no 0..100 progress (the parsers
do not report it; the abandoned worker runs to completion and its result is
dropped).

### L9.11 — ОМП UI (track O, merged)  ·  ✅ (residuals)

Merged from worktree branch `worktree-agent-a7005e8361db7a509` (`f93a099`,
`5ecabbf`): `fvb_core::omp_params` (settings, `.omp` XML ver 2 with the
original's "lenght", OK validation 3187/3535/3068/3074/3071/3076, UTF-16LE
`.txt` report `0x49dbf0`), `omp_ui.rs` (Анализ toggle `0x449890`/`0x4a17a0`,
result bar under the ribbon following the active document with live text and
«Сохранить...» 32879, bar close = 32830 clears, the modal «Параметры ОМП» tree +
grid with toolbar and enable rules, channels by name per station, message
354, «Lкз» in m + digital «КЗ фазы А/B/C»); `View::Omp` removed. Merge fix: ОМП
reads `Doc::flagged` (L9.7); `recompute_active` re-runs it when shown.
Residual: (report times: L9.41), no
legacy `.omp` reader / `.set` embedding.

### L9.12 — Diagram panes (track D, merged)  ·  🟡

Merged from `worktree-agent-a7526b4faa28680ce` (`cafd051`): `diagram.rs`
(pure: extent `0x438b50`, grid cell LOGPIXELSX·scale/5 with integer-divided
cells and 1-digit-ceil units per cell, wheel `0x437ae0` exactly-Ctrl zoom /
exactly-Shift pan x / else pan y, pan limit `0x437b90`, toolbar steps ±2,
zoom reset past 800 px / refused at 400 px `0x43b570`, scrollbars
`0x437ca0`/`0x437f00` incl. the original's ×2 mismatch, vector 4/5/10 circles,
unit rules), `diagram_ui.rs` (painting on egui's painter: even-odd fill at
alpha 50, reverse order, two-scale edge labels, loci-only legend, readout
table, diff lower-left initial view, vertical toolbars 550/552/553, menus
387/388/355/351 via `popmenu.rs`, dialogs 554 (R/X and diff pairs) and 353,
frequency panels per channel with rotated labels and percent default, vector
arrowheads with «Углы в радианах» removed), unit eligibility in MENU 561 /
pane menus / attach (CMDUI `0x430650`: vector A/V/«ном»/none, circle
Ohm/W/VA/var). Merge fix: the frequency panels read `Doc::flagged` (L9.7).
Residual: frame/axes pens, edge-label/scale-box rects, dialog 554 combo
contents, frequency margins NOT DETERMINED; (per-document «* Chart»
persistence: done in L9.17), the themed splitter, bit 14 pending merge not modelled;
`egui_plot` dependency now unused (removed 2026-10-23).

### L9.13 — Dialogs and the parameters bar (track F, merged)  ·  🟡

Merged from `worktree-agent-a7cb4acef380f64fb` (`2818fb2`, `0fea977`,
`bf752d4`, `60831ef`). Read: parameters bar `0x456e90` (raw stamp
`dd/mm/yyyy, <time>` when local_code is empty; else `%d.%m.%Y, %H:%M:%S.` +
`_itow(frac)` + « (UTC)»/« (Местное время)»; the first date field is the month
unless rev is "2013", swapped when > 12, +2000 below 100); loader `0x450e70`
keeps the four stamp tokens and the 2013 `time_code,local_code` line;
code-page guess `0x459e60` (Cyrillic letter counts 1251 vs 866 over station +
recorder + names, tie → 866) and re-decode `0x4543e0`; ChannelsDlg `0x4349c0`
(vector column unit A/V/«ном»/"" case-sensitive; circle column CompareNoCase
Ohm/W/VA/var, not bit14 — **the spec's §10 note was inverted**). **Parser bug
fixed:** primary/secondary/PS are fields 11–13 of the analog line (we read
10–12). Ours: `chandlg.rs` (DIALOG 463 grid with header column toggles and
rename 397/398; DIALOG 477 coefficients with Shift fill), `recdlg.rs` (bar 142
on by default, DIALOG 392 file parameters, DIALOG 510 settings with four
encodings and default 1251, the record-mode .hdr editor 32805 with 866/1251
re-encode, per-channel primary/secondary `ChanView::primary`); our role window
relabelled as our extension. Merge reconciliation: F's
`Doc::recompute_signals` now wraps L9.7's `Doc::recompute` (flagged record,
whole-period DC) keeping the channel roles; `recompute_doc` re-runs ОМП via
`omp.shown`; discrete visibility unified as `Doc::digital_visible`;
`reopen_active` removed (a new encoding applies on the next open, as the
original).
Residual / NOT DETERMINED: (the `doc+0x1e4` correction candidates: read
and implemented in L9.65); digital groups `doc+0x878/+0x884`; bitmap 464 icons (ours);
whether 463 has OK; (`ch+0x89` default secondary: read in L9.57); `.set` persistence and the
document-modified flag are still not modelled.

### L9.14 — Ribbon contents, application menu, QAT, Стиль, help  ·  🟡

Read (spec §2, builder `0x472f30`..`0x474c76`, `0x475250`, `0x475800`): app
menu Открыть / Сохранить / Сохранить как / Проводник / Печать ▸ (Параметры
страницы, Предварительный просмотр, Печать) / Параметры файла / Комментарии /
Загрузка настроек / Настройки / Закрыть + «Последние документы» (10); QAT
Open/Save/Print only; Видимость columns [32845/46/47] [32802/03/138]
[32800/01/141] [32813/14/139] [32815/16] [155/157/156]; Вычисления
[32817/18/19] [32820/21/22] | [32823, 32824, filler] [32866, 32867, 32868] |
137; the unnamed " " panel for 142; Стиль menu 200 radio, persisted
`ApplicationLook` (default 215); help split button {Помощь, О программе...};
ribbon right-click = one «minimise» item, double-click on a tab minimises.
Ours: `ribbon.rs` rewritten to that order (our invented «Вся осцил.» removed;
demo / CSV / zip / .fvb / roles moved to «Дополнительно (FastView-REA)»),
Undo/Redo greyed by stack counts, Copy by a selected file channel, Paste by
the clipboard and no pending paste; `App::mru` + `Command::OpenPath`
(persisted `MRU/k`), `ApplicationLook` 215–218.
Residual: Сохранить and Сохранить как both open the save dialog (the
original's in-place save rule not modelled); page setup, print preview and
«Загрузка настроек» (.set) shown disabled; the five non-2007 looks (210–214) now
selectable and persisted (L9.18) with era system-colour palettes that are
inferred, not read from mfc120u.dll; Помощь disabled (no CHM viewer); icon-only buttons
carry text labels; keytips not modelled (read 2026-10-23: the program
sets keys on one element only — `SetKeys("p", "w")` at `0x473307` on
the application menu's Печать (57607); every other keytip would come from
MFC's ribbon keytip machinery in mfc120u, not read).

### L9.15 — Merging oscillograms («Объединение»)  ·  🟡

Read: `0x47bc00` by hand (sync by fault component: per channel the jump
`|x[k] − 2x[k−N] + x[k−2N]| / A(k−1)` with A the fault component
`FUN_00427670` on flagged values, window `[3N+1, last]` narrowed to `c ± N`
around В1 when there is room, peak = first maximum, success = peak ≥ 4×
window mean in both channels (3456, else 3507), offset = t₁(peak₁) −
t₂(peak₂) either way; 3260 / 3162 abort), with spec §7.8 for method 0
(`0x47ce10`: start-time difference, 219 when the records do not overlap),
method 2 (`0x47c440`: coarse `t₁(c) − t₂(c)` corrected by the mean
fundamental phase difference over ±1 period / (f·360), 3305 when the spread
exceeds 1°), «Объединить» (`0x47d990`: rate = the highest rounded to a
multiple of f_net, zero padding, `round(R·dur) − 4` samples, quadratic fit /
previous-sample hold), the bar CMergeBar (method, Канал 1 / Канал 2 enabled
for methods 1/2, the buttons, status text; hiding with pending channels =
Undo), paste (undo snapshot, ОМП cleared, deselect, "(2)" names (L9.20), pending),
manual drag of a pending lane (mode 6, hand cursor, offset clamped to one
period of overlap). Ours: `fvb_core::merge` (sync methods, `merged_rate`,
`quad_at`, `hold_at`, `merge`; 4 tests), `comtrade::stamp_seconds`,
`merge_ui.rs` (clipboard with times/rate/start, pending lanes refreshed at the
offset, bar, «Объединить» in place keeping view state and remapping comments
/ визиры by time; 2 tests), lanes drag. Replaces our sample-index paste.
Residual: system-clipboard format done in L9.20; «Объединить» undo
fixed in L9.19; (method 2, the ÷ `ch+0xf0` on merge: L9.111; pasted
discrete lanes: L9.71).

### L9.16 — Formula dialog «Добавить новую формулу» (DIALOG 359, track A2, merged)  ·  ✅ (residuals)

Read: tree built in `0x401c70` (string-table labels; each item's lParam = its
form's dialog id; groups expanded; «Формула»/400 selected at open `0x401c10`);
TVN_SELCHANGED `0x403260` → `0x403280` swaps the child form in placeholder
0x40b, a group (lParam 0) has no form and greys OK; form OnInitDialog
`0x411f40`/`0x411c70` fills channel lists with analog ids then computed
channels (3Io/3Ion lists start «Не учитывать», the 24 current lists «Не
использ.»; missing defaults → «Канал отсутствует» on top); edit defaults
«Ia + Ib», «1.», «0.», «0.25», groups 0–11 / 0/1/11, Kторм 1/0.5. OK
`0x403530`: DDX `0x403180` 396 for empty / leading digit, `0x4033a0` 396 for a
leading non-letter and 473 for a name equal (Compare) to a record channel or
formula; `0x4129e0` builds the formula (`0x412700` quotes non-identifier
names; numeric commas → dots; «Не учитывать»/«Не использ.»/"" → 0); the page
proposes «Канал%i» (max trailing number + 1, `0x486df0`). Ours:
`fvb_core::addformula` (57 leaves, `build`, `check_name`, `default_name`, …;
9 unit + 1 integration test evaluating all 57 on the demo record),
`formula_dlg.rs` (dialog, OK → `append_computed`; 5 headless tests), a
«Добавить…» button in the formula editor; `expr`: `frequency:` accepts no
argument. Deliberate: Im:(Zk) passes Km twice as the original (`0x413f69`).
Residual: channel unit from a trailing `[W]/[var]/[VA]` is inferred; what
`Frequency: ()` returns was not reversed (ours: line frequency); no tree icons;
the formula-editor page itself (list, Загрузить/Сохранить/↑/↓) not rebuilt;
(the 396 sequence: done in L9.48).

### L9.17 — Record settings file `.set` and the modified flag (track S, merged)  ·  ✅ (residuals)

Read: settings-tree library `0x48dc80`/`0x48dea0` (get-or-create), getters
`0x48da40`/`0x48db10`/`0x48dbb0` (string/`_wtoi`/`_wtof`, default only when
absent), setters `%i`/`%f`; loader `0x48e470` (skip BOM, UTF-16LE via
`0x4aa220`, errors swallowed; empty root → INI fallback, merges); writer
`0x48e1c0` (`"w, ccs=UTF-16LE"`, `<?xml version="1.0" encoding="utf-16"?>`,
`<settings>`, `<category name=…>`, sorted by name); path `0x44c710` (strip
.cfg/.osc/.sto/.zip/.cff, append .set; reader `0x44f5b0` falls back to path
minus 3 chars + `ini`). Sections: `Comments` (`0x43fcf0`: Areas/Levels/
Moments, text, locked 0, visible 1, kind fields, channels by name),
`Oscillogram/DiscretisationFrequensy`, `Codepage` (~~0 ANSI / 1 OEM~~ 1 ANSI / 0 OEM — L9.171; wins over
the guess), per-channel sections (`0x427d20`: ShowFlags = `ch+0x60`,
PrimaryMul, SecondaryMul, AliasName, NetFreq, CustomColorSet, colorCustom),
`CalculatedChannels` (`Channel<i>` = `name,formula`), `WrongPlaceFinding`
(`0x4a3050`), `Diff/Circle/Vector/Frequency Chart`, and **`View` per document**
(XScale 2×fit, RulerState 1, YAnalogSize 93, Sells 1 — not the registry;
`Properties Height` is). Written only by OnSaveDocument `0x44c940`; Save onto
the record's own path writes only the `.set`, Save As writes record + `.set`;
SaveModified `0x454fa0` = MFC Yes/No/Cancel (TRUE for .sto); 32860 `0x44fd10`
loads a settings file (no comments); 32805 OK always marks modified (spec §7.10
was wrong). Ours: `fvb_core::settree` (shared with `.omp`), `fvb_core::recset`,
GUI `setfile.rs` (read at open, write on save, 32860 enabled, modified flag via
explicit marks + a per-frame change detector, close/exit prompts); «Сохранить»
vs «Сохранить как» now differ. 11 tests. Residual: (the prompt wording:
read in L9.62); (`Selected Block`: the document's since L9.162);
`NetFreq`, `.cff` SET, `.zip`, `.sto` not modelled; comments store file name
vs alias NOT DETERMINED; (paste / an ОМП run do not mark modified: read
in L9.68).

### L9.18 — «Абсолютное время» (33007), band labels, 33033 toggle  ·  ✅

Read: ruler `0x464350` mode 3 — the start date token (`doc+0x24c`, getter
`0x4222a0`) split on `/` with the same month/day rule as the parameters bar
(Compare with "2013", swap when the "month" > 12, +2000 below 100) drawn
`%02d/%02d/%04d` day first at (5, H−20); every label = the start time token
(`doc+0x250`, `0x4222d0`: `_wtoi` h, `_wtoi` m, `_wtof` s) + (t − t_trig)
µs/1e6 as `%02d:%02d:%02d.%03d` (hours mod 24 — the `0x2aaaaaab` multiply;
ms = fraction·1000 rounded half up, can print 1000), no unit. Marker /
difference labels **`0x466170`** (callers `0x466a4e` marker, `0x46715c` |Δ|):
plain `%i` µs, `%i` ms rounded by **`0x411bf0`** (floor when < ½ away else
ceil), `%.3f` s, absolute time of day, samples k+1 (difference: the count);
an absolute difference = rounded ms → `%02d:%02d:%02d.%03d` with hours not
wrapped — **no unit and no grouping** (ours had both: fixed). Status bar
(`0x4947d0`, strings 240/241/202): V1/V2 time of day, the V difference
`%02d.%03d` = seconds within the minute + ms (`0x4951a3`: the minutes are
dropped). 33033 `0x4557d0` toggles `doc+0x870` + UpdateAllViews, no
modified mark; CMDUI `0x4557b0` SetCheck only (always enabled).
Ours: `TimeUnit::Absolute` (RulerState 3 persisted), `oscgeom::{round_half_up,
time_of_day_text, mark_label, diff_label, status_abs_diff}`, `fvb_core::
comtrade::{time_of_day, stamp_date_text}`, MENU 560 with all five radios and
«Разница между визирами» enabled (toggles `Doc::diff_band`). Tests:
`oscgeom::absolute_time_labels`, `app::cursor_status_absolute_time`. Closes
the L9.1 and L9.4 «Абсолютное время» residuals.
Also: the status bar's free-text pane is gone (the original has none,
`0x4729b0`); our own messages show as a 4 s toast above the bottom-right
corner (our extension). Residual: a start time without `:` makes the original read uninitialised
stack (ours: missing fields count 0); the ruler's `iStack_a0` label
condition in mode 3 not decoded.

### L9.19 — Undo/Redo: per-document record snapshots; «Объединить» side effects; Ctrl+S  ·  ✅

Read (research report, exhaustive scan of `call 0x454530`): three pushes —
paste `0x45ad50` (first), file-parameters OK with a changed rate `0x44a24a`
(then recompute, **silent ОМП run regardless of the banner**, SetModifiedFlag),
«Объединить» `0x47e834` (after the per-channel resampling, bit14 cleared and
the new time array; **before** `doc+0x228` := R and the nominal table, so the
first Отмена only reverts the rate and a second one the paste — inferred
from the read order). The earlier "callers" 0x42e554/0x450c36/0x452f15 call
`0x454960` (nominal-frequency table), not the push. Deques per document
(`doc+0x6ec` / `+0x700`), no limit, push clears redo, CMDUI by counts; the
snapshot is the record `doc+0x180…0x27f` (assignment `0x4546d0`); restore
only repaints (`0x45bf0f`). After «Объединить» (`0x47d990`): silent ОМП run
`0x4a00b0(1)`, SetModifiedFlag, bar hidden, MessageBeep; визиры, comments
and the view untouched. Accelerator table 128: **Ctrl+S = 57604 «Сохранить
как»**.
Ours were wrong: one global stack keyed by document index (Ctrl+Z undid
another tab; stale after a close), capped at 100, snapshotting chans +
comments + computed but not the record — **Undo after «Объединить» restored
pre-paste channel views over the merged record** (index mismatch); merge
remapped визиры/comments by time, reset the view, no modified mark / ОМП;
the rate change pushed nothing. Now: `Doc::undo` / `Doc::redo`,
`DocSnapshot` = record + chans + computed + pending + selection + digital
visibility; `push_undo` at the three sites in the original's positions;
`run_omp_silent`; merge marks modified and keeps indices (clamped to the new
length); ribbon greying from the active document; Ctrl+S → SaveAs. Test
`merge_ui::merge_is_undone_in_two_steps_and_history_is_per_document`.
Deliberate: restore also rebuilds our derived caches (flagged values,
RMS, phasors, pending lanes) and clamps визиры to the record; no beep; a
manual Ctrl+Z while the bar is up still hides our bar (the original leaves
it up, stale).

### L9.20 — «Mechanotronica FastView Clipboard Data»: copy/paste through the clipboard document  ·  🟡

Read (research report): the format is registered once at start-up
(`0x404f5e`); copy `0x45be28` puts an EMF picture of the view and the data
as UTF-16 XML (no BOM / declaration, CR LF, TAB, attributes by name) built
by `0x430af0` — root `channels` (`num_values` %g, `time_multiplier`,
`discr_frequency` %i, `times` µs %i low 32 bits, per-sample `net_frequency`,
`stationID`, `blockID`, `title`, `hdrFile`, `startTime`) and per selected
non-computed channel (`0x427f20`) name / dimension / type / desc2 / desc3 /
values_type / primary_mode / mul / offset / timeShift / the four
coefficients / normal_state / values_count / custom_color_set /
color_custom / record_start_* / duration_milli_sec / `data` `%f,`. Paste
`0x45ad02`: undo snapshot, insertion points, deselect, `num_values` by
`_wtoi` bounding `times` / `net_frequency`, root must be `channels`
(else no channels, rest still runs), flags 0x40e1 **and selected**, name +
"(%i)" **without a space**, the record comment rewritten as «Механотроника.
FastView, Объединенная осциллограмма…» (strings 3272/3221/3048/3218/3479/
3308/3297/3249/3050) with `Comments/Codepage` = 1, ОМП cleared, bar shown.
Copy CMDUI: a selected non-computed channel (digital counts).
Ours: `fvb_core::clipdata` (`ClipRecord::{to_text, parse}`, `merged_header`;
3 tests incl. the `1e+06` and 32-bit-time quirks), GUI `sysclip.rs`
(Windows: the registered format via `windows-sys`; elsewhere a
`clipboard.xml` in the settings folder stands in for the system clipboard,
so instances on one machine still merge; tests in memory), copy/paste in
`merge_ui.rs` through the document (so the `%f` precision loss happens as
in the original), pasted lanes selected, "(2)", the merged-record comment,
copy enabled by digital channels; the insertion point — pending lanes
shown in front of the first selected analog / digital channel
(`Computed::before`, `lane_layout`) and «Объединить» moving the merged
channels there with every stored index shifted (`Doc::move_appended`;
test `pasted_channels_go_before_the_first_selected_channel`). Residual: (EMF
picture: L9.30); an analog paste with no selected analog channel goes to our
computed section (whether `doc+0x1d8` counts computed channels is NOT
DETERMINED); `duration_milli_sec`
text and `time_multiplier` are inferred (0x44c1d0 / `doc+0x230` not read);
the clip's start date is read with the target record's revision rule; CR LF
placement inside the merged comment inferred from the concatenation count.

### L9.21 — Stats-panel drag of channels (backlog 48)  ·  ✅ (in-process)

Read (research report): the drag (`0x45ea70`) starts with LMB held in the
stats panel (no Ctrl / RMB) through `COleServerItem::DoDragDrop`, MFC's
2 px / 200 ms start rule; its payload (`0x42be30`) is a u32 count + the
selected analog-block indices — **no channel data; the backlog wording
"between views" was wrong**. Drop targets `0x4414c0` (kind = the diagram
flag bit): the view (1), frequency (2), vector (4), circle (8); the
differential diagram has none; drag-over always "move", no insertion mark.
OnDrop `0x4415b0` acts on the active document: on the view the analog lane
under y (x ignored), unless it is a dragged channel, is selected with them
(other selections stay) and 33035 runs, else message **1086**; on the
frequency / vector pane each channel not on it gets 35000+i (toggle → on,
recompute, SetModifiedFlag); the circle pane only non-pending Ohm / VA
channels (CompareNoCase).
Ours: `lanes::ChanDrag` via egui's `DragAndDrop` payload (2 px), move
cursor, `Act::DropCombine`, `App::pane_drop` / `drop_channels` on the
three panes. Tests `dragging_a_channel_onto_another_lane_combines_them`,
`dropping_on_diagram_panes_adds_the_channels`. Residual: a drag cannot
leave our window (no OLE "Embed Source" / cross-instance index drop — the
original's foreign drops are inert or misapplied anyway); the 200 ms
delay start: done in L9.61.

### L9.22 — Printing: dialog, page setup, paged output, preview (backlog 36)  ·  🟡

Read (research report; MFC ordinals resolved against the local
mfc120u.dll): 57607 → OnPreparePrinting `0x4053f0` (CPrintDialog with
template 347, the view's BOOL[5] — Осциллограмма on — kept also on Отмена);
57609 → `AFXPrintPreview` (stock preview, no dialog, opens on min(count, 2)
pages); OnBeginPrinting `0x4054b0`: margins 0.01 mm inside the printable
area, fit-to-width `r` = printable width (screen px) / view client width,
page height `P` = printable height / r, pages from packing the visible
lanes (`0x4600b0`: acc ≥ P − band → next page; band 30 / 75), then one page
each for the ticked Частотная / Векторная / Круговая / Дифференциальная;
slice `0x405970`: the screen painter in print mode — current horizontal
window, all lanes from the top, a lane that would reach the band moves to
the next page top − 1 after a ruler is drawn there (never split), the lane
background post-check leaves the moved lane's background behind (quirk),
no selected-lane highlight, визиры through all pages with labels on page 1,
the difference band on the last page only; HALFTONE stretch into R and a
1 px black frame. Header `0x405c20` above R: band (243,246,250), logo 341
50×42, three lines (strings 148–152: name in quotes, raw start date/time +
«Длительность: %g c.», station / recorder in quotes); footer `0x406590`:
page number only for jobs of ≥ 2 pages, «Отчет сформирован программой
FastView» + bold «ООО «НТЦ «Механотроника»» + underlined blue «www.mtrele.ru»
30 px apart (no bold/underline in preview). Diagram pages `0x441fe0`: the
pane's own painter into R at the printer/screen scale. Page setup
`0x448a90`: paper disabled, orientation + margins, «Print Parameters»
(Left/Right/Top/Bottom Margin 2000, Page Orientation 1).
Ours: `raster.rs` (headless egui pass → CPU raster of egui's own
tessellation), `pdf.rs`, `printout.rs` (`PageSetup` persisted under the
same keys, `Device`, `page_queue`, `osc_page_count`, `paged_tops`,
`Job`, slice painting through the shared `lanes::OscView` painters
extracted from `lanes_ui`, header/footer, diagram pages), `print_ui.rs`
(«Печать» with printer list + five checkboxes + «Параметры страницы» +
«Предварительный просмотр», page setup with a sample sheet, the preview in
place of the views with Печать / Увеличить / Уменьшить / Две страницы /
Следующая / Предыдущая / Закрыть), `spool.rs` (Windows: GDI printer DC with
the orientation in the DEVMODE, StretchDIBits per page; elsewhere a PDF to
CUPS `lp`), app menu Печать ▸ enabled, QAT/Ctrl+P = 57607; the old window
PNG export moved to «Дополнительно». Tests: pagination rules, queue and
setup persistence, a rendered page (frame, header, lanes), the dialog/
preview flow.
Residual: logo bitmap 341 not reproduced (slot left empty); A4 only (the
printer's paper and hardware margins only via GDI caps on Windows; CUPS
gets the full sheet); «Свойства...» and printer status fields empty; our
«Файл PDF…» entry is an extension; diagram print-only branches (pen widths
×scale, frequency tiling for the page aspect) not modelled; horizontal
визиры printed since L9.39, the comment under the pointer since L9.59; header/footer line pitch
T assumed 12 screen px.

### L9.23 — User-reported: crash at small window sizes; the bottom scroll bar  ·  ✅
Our bugs (not original behaviour), from the user's run of the release build.
(1) **Crash when the window is made too small**: a tiling window manager
ignores the 866×500 minimum, and `lanes::scrollbar` computed the thumb with
`clamp(16, len)` — `f32::clamp` panics when the bar is shorter than 16 px.
Further inverted clamps that a drag in a squeezed view reached (визир /
level drag `clamp(left, plot_right)` with the stats column wider than the
plot, `clamp(top, plot_bottom − 1)`), a one-sample pasted channel
(`merge_ui` resample `clamp(1, len − 1)`), and the diagram scroll bar
(`m < 0`) are guarded too. (4) **The bottom scroll bar was drawn wrongly**:
it was painted through the view painter clipped to the client above it, so
only its (invisible) hit area existed, it started under the plot instead of
the view's left edge, and it had no arrow buttons.
Read (`0x45d4a0`, `0x45d6d0`): the view uses its **window scroll bars**
(`CWnd::SetScrollInfo` / `GetScrollPos(SB_HORZ=0 / SB_VERT=1)`); H mask
`0x17` = RANGE|PAGE|POS|**DISABLENOSCROLL** (always present, disabled when
the whole record is visible), range 0..page+10000, page
`visible·10000/(N−visible)`, pos `first·10000/(N−visible)+1`; V mask `0x13`
= RANGE|PAGE|DISABLENOSCROLL, max = Σ lane heights, page = client H − A + 2,
both 0 when the lanes fit. Line steps: H one screen, V 100 px.
Ours: `frame_ui(ctx)` split out of `eframe::App::update` so the whole window
runs headless; `scrollbar` redrawn as a window scroll bar (own painter,
arrow buttons = line step, track = page, thumb drag only from the thumb,
disabled look with no thumb, squeezed arrows when short); H bar along the
whole view bottom (corner box blank when V is shown), V bar down the whole
client height beside the bottom band; the ruler ends at the plot's right;
thickness 17 px (default system metric — inferred, not read). Tests:
`window_tests::tiny_window_sizes_do_not_panic` (whole window, 21 layouts —
panes, preview, ОМП, nine dialogs — × sizes down to 1×1, hover + LMB/RMB
drags + Esc each), `the_horizontal_scroll_bar_is_painted_along_the_bottom_
and_steps_by_a_screen` (painted inside its clip, arrows/track step).
Residual: (arrow auto-repeat: L9.40); whether the V bar is
present (disabled) when the lanes fit is NOT DETERMINED (DISABLENOSCROLL
keeps an existing bar; the view's creation style was not read) — ours hides
it as before; the original's thumb-position quirk (pos max 10001) not
modelled (we map first ∈ [0, N−visible] linearly).

### L9.24 — User-reported: input fell through dialogs to the view  ·  ✅
Our bug: while the colour dialog (33030) or any other window was up, the
oscillogram underneath still took presses, RMB, drags, the wheel and the
arrow keys — beside the window, and for the stats-panel drag (raw pointer
state, no hover check) even through it; Ctrl shortcuts ran too.
Read (spec §7, `UI_BEHAVIOUR_SPEC` lines on 57664 / 32880 / 129 / 32807 /
32861 / 32862 / 32866): every dialog of the original is **modal**
(`DoModal`, incl. Проводник), so the main window gets no input at all while
one is up.
Ours: our dialogs are `egui::Window`s at `Order::Middle` (nested message
boxes / page setup at Foreground over a Middle backdrop of their own);
`dialogs::window_up` = a Middle layer was visible last frame →
`App::modal_up`; then `frame_ui` lays a transparent full-screen
`modal_backdrop` at `Order::PanelResizeLine` (above the panels, below the
windows) and skips `handle_keys`; the view's arrow keys and drops check
`modal_up`, the stats-panel drag needs the press on the view
(`is_pointer_button_down_on`). The ОМП dialog's own dimmed blocker became
the shared transparent backdrop (the original does not dim). Tests:
`the_view_ignores_input_while_a_dialog_is_up` (colour dialog: LMB / RMB /
drag / wheel / → beside it, on its colour plane and margin; the same input
reaches the view once it is closed), `the_view_ignores_input_beside_any_
dialog` (print, print+page setup, settings, Каналы, Коэффициенты, ОМП,
Параметры файла).
Residual: one frame of lag (the backdrop follows last frame's windows, as
egui's own hit test does); the original's beep / title flash on a click
outside a modal dialog is not modelled.

### L9.25 — User-reported: the «Файл ▾» application menu rendered wrongly  ·  ✅
Our bug: the two-column menu stretched across the whole window (a
separator in the unbounded left column took all offered width), pushing
«Последние документы» off-screen; «Печать ▸» popped out beyond the menu
with a wrapped, clipped header. Seen headlessly via the new
`window_dump` test (`FV_WINDOW_DUMP=<png> [FV_WINDOW_CLICK=x,y]
[FV_WINDOW_HOVER=x,y] cargo test window_dump`, `raster::render_input`).
Read (spec §2.1, strings 57605/57609/57607): main panel, separator between
every item, right column 300 px; «Печать» main part prints (57607), its
sub-items (header «Предварительный просмотр и печать документа», each
label + description) show in the right column (Office-2007 main panel).
Ours: fixed 220 / 300 px columns, separators between items, hovering
«Печать» (or our «Дополнительно») swaps the right column for its items,
any other item restores the MRU list; state stamped per pass because egui
0.29 returns no inner result while a menu stays open. Test:
`app_menu_is_two_columns_and_shows_print_items_on_the_right`.
Residual: item icons (32×32 images) not drawn; keytips not modelled.

### L9.26 — Diagram pane spilled over its tab strip  ·  ✅
Our bug (found in a headless `window_dump` render): in a short tab group
the pane's vertical toolbar (550 / 552 / 553) grew its side panel past the
slot, the canvas sized itself to that taller rect, and with the panel's
clip being the whole column both were painted over the Векторная /
Круговая / Дифференциальная tabs. Ours: buttons that do not fit are cut
off (a short toolbar), the tab group's content is clipped to its own rect.
Test: `diagram_pane_toolbar_stays_above_the_tab_strip` (860 / 700 / 560 px
windows). Residual: the original's toolbar overflow chevron not modelled.

### L9.27 — Ribbon: icon-only small buttons; panels collapse when narrow  ·  🟡
Our bug (headless `window_dump` render at the default 1320 px): the
«Видимость» group drew 140 px text buttons in six columns, so «Главная» was
~1400 px wide; past the window edge egui wrapped labels almost per
character and the ribbon body grew to ~300 px, while «Вычисления» and the
«Осциллограмма» panel were off-screen and unreachable; labels overlapped.
Read (spec §2.3): every «Видимость» command but Каналы, and 32817–32822 in
«Вычисления», are **small icon-only buttons** (label empty, the string is
the tooltip), three to a column, separators where listed; MFC ribbon
behaviour (framework): panels that do not fit collapse to one button that
drops the panel down; the body height is constant.
Ours: `mini_button` (24×24, tooltip = label, framed while checked) with
our own vector glyphs (`paint_mini`; the original's image list is
proprietary and not reproduced); fixed-height in-panel separators; small
text buttons sized to their label; no text wrapping in the ribbon; panel
widths measured when drawn expanded and `collapse_plan` collapses from the
right until the category fits — a collapsed panel is a button with its
name + ▾ opening the panel in a popup (closes on a click); body height
fixed at 102 px. Tests: `panels_collapse_from_the_right_until_the_
category_fits`, `narrow_ribbon_collapses_panels_and_keeps_its_height`.
Residual: MFC's intermediate reductions (large → small → icon-only before
collapsing) and its exact collapse order are framework internals not
read; collapsed buttons show no panel icon (0x1C/0x22/0x20/0x26); glyphs
are ours, not the original bitmaps.

### L9.28 — Idle controls were invisible on panels  ·  ✅
Our bug (headless render of «Быстрый анализ» and the dialogs): the theme
gave idle widgets the panel colour and no outline, so check boxes, radio
buttons, combo boxes and plain buttons showed only their labels in every
look. Ours: idle controls get a white interior, the lighter tab face for
buttons and a frame-coloured outline (as Windows controls). Test:
`theme::idle_controls_stand_out_from_the_panel` (all nine looks).

### L9.29 — Print preview pages centred  ·  🟡
Headless render: our preview drew the page(s) at the top-left of the grey
area; the stock MFC preview (57609 `AFXPrintPreview`) centres them. Ours:
centred horizontally and vertically at every zoom that fits.
Residual (not read in detail, framework behaviour): the original's
preview commands live in the ribbon's stock print-preview category and
MFC hides the docking panes during preview; ours shows a button bar under
the ribbon in place of the views.

### L9.30 — Copy's picture: the view's or the focused diagram's EMF  ·  🟡
Read (subagent report, `fv.asm` + Ghidra): copy 57634 (`0x45acb0` case
`0x45be28`) calls `0x405160`, which picks by keyboard focus: GetFocus, its
first untitled child (FindWindowExW, title ""); focus on the view or no such
child → the oscillogram picture; a DiagramWnd among the first two untitled
children (the focused pane's toolbar / diagram) → `0x441e60` (DiagramWnd
slot +0x170) reopens and empties the clipboard, sets only CF_ENHMETAFILE
and closes it, so the view's later SetClipboardData of the XML fails — **a
diagram copy leaves the picture alone**; other cases: XML only. Oscillogram
order: OpenClipboard(view), EmptyClipboard, SetClipboardData(14, EMF), the
registered XML, CloseClipboard; no other formats. The EMF:
`CreateEnhMetaFileW(NULL×4)` (screen reference, GDI-computed frame, no
description), MM_TEXT, **one BitBlt** of a memory DC = a raster. Content:
GetClientRect(view) w×h drawn by the **screen** Draw `0x45f950` (not the
print painter 0x405970): white background, all visible lanes at the
current scroll, selection highlight, stats panel, ruler, difference band,
визиры, comments; no scroll bars. Diagram: its window's client rect (no
toolbar / property grid) by the class Draw at scale 1 (circle / diff include
the 5 px splitter strip). DeleteEnhMetaFile right after SetClipboardData;
results unchecked; no messages.
Ours: `App::view_rects` (view rect + client without the bars) and
`pane_canvas` (each diagram window) recorded while drawing; a press in a
diagram window focuses its pane, a press in the view takes it back
(`track_copy_focus`, not while a dialog is up); `view_picture` runs
`lanes_ui` headless at the screen's pixels per point and crops off the bars
(menus / inline editors left out); `diagram_picture` reuses the print
diagram renderer at the canvas size; `SysClip::put(text, picture)` empties,
then sets the picture (Windows: an EMF with one StretchDIBits record via
`CreateEnhMetaFileW(NULL…)`, handed to the clipboard), then the XML; a
diagram copy sets the picture only (our file / memory clipboards drop the
document too). Test: `copy_pictures_the_view_or_the_focused_diagram`
(`FV_COPY_DUMP=<png>` saves the view picture).
Residual: off Windows the picture goes to the desktop clipboard as an image
(`arboard`, X11 / Wayland data-control / macOS; our extension, the handle
kept for the process — compile-checked only, not exercised: running it
would touch the user's real clipboard); on Windows the EMF + registered-format path is
now verified on a real Windows 11 machine (`sysclip::windows_tests::
system_clipboard_round_trip`, `--ignored`, cross-built and run over ssh); the record is EMR_STRETCHDIBITS rather than EMR_BITBLT
(same picture); focus on the property grid of the circle / differential
pane (the original's third case, XML only) is not modelled — a click there
leaves the focus unchanged (done in L9.58); our status-bar messages are ours.

### L9.31 — «Загрузка» progress and cancel polling  ·  🟡
Read (subagent report): the counters live in the document — position
`+0x82c`, total `+0x830`, per-sample step `+0x834`, cancel `+0x838`; the
dialog's 500 ms WM_TIMER `0x471870` sets PBM_SETPOS to
`((pos·100) mod 2³²)/T` (0 when T = 0; the control clamps at 100).
Phase 1 (data file memory-mapped, S bytes): `.cfg` ASCII `0x451800` T = 5·S,
pos = bytes consumed after each line (the parse fills 20 %); BINARY
`0x451be0` T = 5·S, pos = **T − remaining** after each record (≈80 % at the
first record — a slip, reproduced); binary32 `0x4520f0` never sets T (0 %
throughout); `.cff` nothing during the split, then the inner `.cfg` rule;
`.osc` `0x453010` T = 5 × the bytes after the header, pos = T − remaining
per 128-byte block. Phase 2 (`0x428490`, pos += step at `0x428809` per
sample of every analog channel, via the `.set` apply `0x44f5b0`): step =
⌊T/(N·C)⌋ (`.osc` ×5), N the first channel's samples, C all analog +
digital records — so the bar seldom ends at exactly 100 %. Cancel
(OnCancel `0x471850`: EndDialog, then flag = 1) is polled per line /
record, per channel in `0x44f5b0` and per sample in phase 2 (not in the
`.cff` split nor the `.osc` block loop); readers return normally and the
open fails silently (`0x44f391`). DIALOG 492 «Загрузка осциллограммы»:
progress bar 1095 (0..100) and «Отмена», no close box, no text.
Ours: `fvb_core::comtrade::Progress` (pos / total / cancel atomics,
`percent()` with the 32-bit wrap) threaded through `load_progress`,
`parse_cff_progress`, `parse_osc_progress` with the per-reader rules and the
phase-2 steps (cancel polled per 1024 samples); the GUI worker holds an
`Arc<Progress>`, the dialog (renamed, file label dropped) samples
`percent()` every 500 ms, «Отмена» sets the flag and drops the load. Tests:
`loading_progress_follows_the_original_counters`, `osc_loading_progress`.
Residual: our phase 2 only advances the counter (our derived arrays are
built on the UI thread when the document is added) and leaves out the
calculated channels F of the `.set` (applied later on the UI thread); the
original's UI freeze after «Отмена» until the next poll, its two step-0
`.osc` recompute passes and the «Неправильный формат файла» box for a
`.cff` cancelled during its split are not reproduced.

### L9.32 — User-reported: the scroll thumb lost the pointer  ·  ✅
Our bug (L9.23 code): the thumb drag only continued while the press point
lay inside the thumb, which it leaves as soon as the thumb moves — so a
drag stopped unless it happened to stay put. Ours now tracks like a window
scroll bar: the grab offset inside the thumb is kept for the whole drag and
the thumb follows the pointer's position along the bar wherever the
pointer goes. Test: `scroll_thumb_drag_survives_leaving_the_bar` (a drag
wandering 40 px above and below the bar). Residual: Windows' snap-back of
the thumb when the pointer strays far from the bar (system behaviour, not
the program's) is not modelled.

### L9.33 — User-reported: a vertically zoomed curve left its lane  ·  ✅
Read (subagent report): the analog painter `0x429490` (and the group member
painter `0x429c30`) selects a GDI clip region **(0, top + 5)–(W, top + h)**
(CreateRectRgn `0x4295a5`, SelectClipRgn `0x4295be`, removed at
`0x429bb4`) **only while the channel has a window zoom** (`ch+0x114`),
around the zero line, the polyline and the 4×4 marks; values are never
clamped or skipped (y = top + h − 5 − round-half-up((v − lo)(h − 10)/
(hi − lo))). Unzoomed ranges are the series' own min..max, so no clip is
needed. The bottom edge is top + h (the 5 px inset is not cut). Level-line
comments (`0x471020`, hit `0x470e4a`): t = (level − lo)/(hi − lo) with the
channel's current range only when lo ≤ level ≤ hi, else neither drawn nor
hit-testable; y truncates t·(h − 10). The rectangle zoom's end y is clamped
to [laneTop, laneTop + h − 1] (`0x45ec4e`). The stats readout ignores the
zoom; ГВ lines may leave the lane in the original too (`0x493110`, no clip).
Ours: the per-member lane clip when `ChanView::window` is set; level lines
via `oscgeom::level_y` (range check + truncation; a group lane uses the
channel's own range); the rubber band end clamp. Tests:
`a_rect_zoomed_curve_is_clipped_to_its_lane`, `level_lines_stay_in_their_
lane`. Residual: (round-half-up for value→y: done in L9.47); the clip under print / EMF map modes NOT DETERMINED;
the original's rect-zoom lane top sums preceding visible analog channels in
channel order (differs for group members — quirk not modelled).

### L9.34 — Units are normalised on load (`0x422b10`)  ·  🟡
User report: with real records («кА», «кВ», Cyrillic «А»/«В») the vector
diagram's «Каналы» list was empty, the «Каналы» dialog's vector column and
the channel menu's «на векторную диаграмму» were disabled.
Read (subagent reports): the channel constructor `0x422460` keeps the raw
token (`ch+0x78`, not trimmed — `0x477050` cuts at TAB/CR/LF/comma) and
`0x422b10` builds the canonical unit `ch+0x7c` and a multiplier `ch+0x98`:
"" for an empty or "NONE" unit; SI prefix in order мк/mk/u/µ ×1e-6, м/m
×1e-3, н/Н/n/N ×1e-9, к/К/k/K ×1e3, М/M ×1e6, Г/G ×1e9 (with the listed
one-letter, «/с», «ном» and ГН/ГЦ/ГР exceptions), a trailing «/с»/"/s"
stripped (per-second flag); then А/A→A, В/V→V, ВА/В·А/V·A/VA→VA, ВТ/W→W,
ВАР/VAR→var, Ом/Ω/OHM→Ohm, Ф/F, ГН/H, гр…/DEG→DEG, н/ном…/NOM→nom,
О.Е./P.U.…→p. u. (those three with the prefix undone), ГЦ/HZ→Hz, м/m→m,
с/s→s, См/S→S; no match → the raw unit, ×1, unknown flag. Literals are CP1251,
compared case-sensitively with the unit and an upper-cased copy. Samples are
`mult·a·raw + b` in the ASCII (`0x451904`) and BINARY (`0x451de0`) readers;
the writer inverts (`0x422250`). Every unit test reads `0x414c30` = the
canonical unit, or "" in ADC mode: vector A / V / «ном» / "" (CompareNoCase
in the CMDUI `0x430650` and the dialog paint/header; the dialog click
compares case-sensitively), circle Ohm / W / VA / var (no case); the readout
SI prefixes (`0x43d960`) are on the canonical units. On the sample records
every current and voltage channel is vector-eligible; none is
circle-eligible (no power / impedance unit) — **circle disabled is the
original's behaviour**; BINARY's «Напряжение 3U0» has a Latin "B" unit and
is ineligible in the original too.
Ours: `fvb_core::units::normalize` (tests); `AnalogChannel::{unit
(canonical), raw_unit, unit_mult}` set by the `.cfg` / `.osc` readers (the
`.osc` reader's own micro/milli table replaced), samples scaled in ASCII and
BINARY, the writer saves the raw unit and divides the multiplier out;
pasted channels take the clip's unit as canonical (×1); vector / circle
tests no-case on the canonical unit, "" in ADC mode (menu CMDUI, «Каналы»
column, diagram lists). Test `prefixed_cyrillic_units_reach_the_vector_
diagram` (the demo with «кВ»/«кА»/«B» units); `window_dump` takes
`FV_WINDOW_RECORD=<cfg>`.
Residual / NOT DETERMINED: whether `MakeUpper` folds Cyrillic (taken
ASCII-only: «МВт» stays unrecognised, «МВТ» is W ×1e6); whether binary32
applies the multiplier (its reader never reads `+0x98`; ours does not
scale it); the second «гр» spelling (string 3105); whether «ном» maps to
"nom" (then the vector test's string 162 is dead, as taken) or stays;
the original's dependence on the 1251 code page (other program encodings
break Cyrillic units there) is not modelled; what computed channels hold
in `+0x7c`.

### L9.35 — Diagram «Каналы» lists, «Показать все» / «Скрыть все»  ·  ✅
User report: «Скрыть все» hid channels for good on some diagrams.
Read (subagent report): one bit per channel and diagram (frequency bit1,
vector bit2, circle bit3 of `ch+0x60`) is both membership and "shown"; the
list, «Показать все» / «Скрыть все», 32849–32851 and the «Каналы» dialog
all write it. Vector MENU 355 (`0x48fc80`, list `0x492a00`): channels
0..`doc+0x1d8` passing the vector unit test, at position **3 + i** after
the separator, id 35000 + i, always enabled, check = bit2; «Показать все»
(`0x48f600`) sets bit2 on those, «Скрыть все» (`0x48f820`) clears all; on
attach (`0x48fda0`) bit2 is cleared on every ineligible channel, so a drawn
phasor is always listed. Circle MENU 387 (`0x43d220`): Ohm / VA channels,
not pending, at i + 3 before the separator; R|X pairs at n + 4 + j.
Differential MENU 388 (`0x446d70`): pairs only, at **4 + j** after the
separator. Frequency MENU 351 (`0x46f2e0`): analog, not pending, no unit
test, at 3 + i; the frequency painter tests **only bit1**, not the
oscillogram visibility. Every list click and both «…все» mark the document
modified and repaint.
Ours were wrong: raw-unit tests made the vector list empty while the
channels were drawn (flags defaulted on), so after «Скрыть все» nothing
could bring them back (L9.34 fixes the test; the vector flag is now cleared
on attach for ineligible units); vector / frequency items sat before the
separator, differential pairs one slot early; no modified mark; the
frequency diagram also required lane visibility. Now as read. Test
(above) hides and restores through «Показать все» and a tick.

### L9.36 — User questions answered from the binary (no change)  ·  ✅
- «Осциллограмма» (142) toggles only the parameters bar: CMainFrame COMMAND
  142 → `0x4751d0` calls `ShowPane(!IsVisible())` on the pane at frame
  `+0x79c0` and `RecalcLayout`; that object is constructed with vtable
  `0x4cb3f4` = RTTI **CParamOscBar** (`0x4721e6`); CMDUI `0x475180` checks
  it while that bar's window exists and is visible. The view is never touched.
- Undo / Redo are disabled until something is undoable: CMDUI `0x45bf60`
  (CFastViewView range 57632–57644, byte table `0x45c10c`) enables 57643
  iff the undo deque size `doc+0x6fc` ≠ 0 (`0x45c0bd`) and 57644 iff the
  redo size `doc+0x710` ≠ 0 (`0x45c0da`); the push `0x454530` has exactly
  three callers (paste `0x45ad50`, «Параметры файла» with a changed rate
  `0x44a24a`, «Объединить» `0x47e834`) — zooms, visibility, flags, comments
  are not undoable in the original.
- Circle-diagram eligibility for current / voltage channels: none (L9.34)
  — the original requires an Ohm / W / VA / var unit; the differential
  diagram takes Iдиф | Iторм pairs from its own «Каналы ▸ Создать», not
  per-channel flags, so the «Каналы» dialog has no column for it.

### L9.37 — Print preview as MFC's ribbon category  ·  🟡
Read (`vcredist_vs13_x86.exe` → carved WiX cabinets → `mfc120u` / `mfc120rus`,
local only): `CMFCRibbonBar`'s preview category (`mfc120u` `0x100fcfc2`):
panel 17092 «Печать» {0xE304}, 17093 «Масштаб» {0xE305, 0xE306, 0xE301},
17094 «Просмотр» {0xE302, 0xE303, separator, 0xE300}; category 17091
«&Просмотр для печати»; labels 58116 «&Печать», 58117 «Увеличить», 58118
«Уменьшить», 61505 / 61506 «&Одна страница» / «Две с&траницы» (the mode
it switches to), 58114 / 58115 «Следующая / Предыдущая страница», 58112
«&Закрыть»; status 61507 «Стр. %u» / 61508 «Страницы %u-%u». In preview MFC
shows only that category and its dock manager hides the panes.
Ours: the ribbon shows the single tab «Просмотр для печати» with those
panels (large buttons, our glyphs; two-word labels wrap at the space) while
a preview is open; the buttons queue a `PreviewCmd` the preview applies with
the existing enable rules; our button bar is gone; the parameters and
merge bars are hidden; the first status pane shows the page text (the
original's status bar has no message pane, so MFC's message-bar text lands
in pane 0). Big ribbon buttons now size to their label. Test
`preview_category_commands_and_page_text`.
Residual: the panel/button layout is read from mfc120u's call sequence but
the large/small sizing and images (proprietary, not reproduced) are not;
the QAT and app button stay as in MFC [I]; MDI tabs stay visible [I].

### L9.38 — Ribbon labels legible in the dark look  ·  ✅
Our bug (headless render of «Черный стиль»): egui fades a disabled
widget's painter towards the panel colour, so the dark look's light labels
vanished on its mid-grey body (Копировать / Отмена / Возврат), and checked
big buttons put white labels on the light-yellow checked fill. Ours: the
ribbon's own buttons paint without the fade, disabled = the text colour at
half strength, checked labels dark. Test `ribbon_labels_contrast_with_
their_background_in_every_look`. (The looks' palettes remain our
approximations of the MFC visual managers.)

### L9.39 — Horizontal визиры on printed pages  ·  ✅
Read: the print slice painter `0x405970` draws through the screen Draw
(`vt+0x1b8` = `0x45f950`, call at `0x405b56`), which calls the визир painter
`0x493110` and the comment painter `0x43f630` unconditionally (`0x45fa29`,
`0x45fa5e`; only the difference band is conditional); `0x493110` has no
map-mode test — a ГВ is drawn when shown (`+0x48`) and above the bottom band
(`0x494179`–`0x4941a2`). Ours: printed slices draw shown ГВ lines and
labels on their paged lanes (L9.22 residual closed). Test
`horizontal_vizirs_are_printed`.

### L9.40 — Scroll bar arrows and track act on the press and repeat  ·  🟡
Window scroll bar behaviour (system, not program code): a press on an
arrow or the track scrolls at once and repeats while held; the track stops
once the thumb reaches the pointer. Ours (was: one step per click): the
step on the press, then repeats after 0.4 s every 0.05 s — delays assumed
[I] (the system's are set by the double-click time); the track repeat
checks the pointer is still beyond the thumb. Test
`held_scroll_arrow_and_track_repeat`. (L9.23 residual closed.)

### L9.41 — ОМП report: start / trigger times and the document line  ·  🟡
Read (subagent, `0x49dbf0` disassembly): the header literals match ours
exactly (strings 128, 3102, 3112–3114, 3480, 3103, 3298 «Начало записи:  »,
3279 «Момент запуска: »); «Начало записи» = `doc+0x24c + ", " + doc+0x250`
(`0x4048c0`), «Момент запуска» = `doc+0x254 + ", " + doc+0x258`
(`0x496360`) — the record's raw date and time tokens, no re-ordering, no
UTC / local conversion (empty tokens still give ", "); station `doc+0x244`
(`0x404860`), recorder `doc+0x248` (`0x404890`); «Осциллограмма:» is
`doc+0x24`, by MFC 12's CDocument layout `m_strPathName` [I] (`0x454fa0`
also tests its last four characters for ".sto").
Ours were empty times and the file name; now the stamp tokens joined by
", " and the full path (the name for an unsaved record). Test in
`omp_ui` (the demo's start / trigger lines). (L9.11 residual closed.)
Residual: the path-vs-title reading of `doc+0x24` is inferred.

### L9.42 — Визир lines and labels as `0x493110` draws them  ·  ✅
Read (subagent, objdump of `0x493110`–`0x49449a`): see spec §3.5 — the
column-search x with the no-leftW fallback quirk, truncation; lines only
for x > leftW; В2 in В3's green pen when x2 = x1; all lines then labels;
labels on opaque cells at the DC top, clamped sideways by 13 px per side,
merged with one space in the orders «В1 В2 [В3]», «В1 В3 [В2]», «В2 В3»
(first member's colour and x), clamped labels unmerged; ГВ lines while
above the band, ГВ2 green on ГВ1's (clamped) y, ГВ labels **only when out
of view** at W/2. 0x493110 has no print logic (print reaches it through
Draw).
Ours were: rounded x, labels only for on-screen визиры centred at the line
with padding, no merging, no clamping, no pen special case, ГВ labels always
drawn mid-plot just above the line. Now `oscgeom::vizir_x` /
`vizir_labels` (pure, tested), `OscView::vizirs` and `lanes::paint_hvizirs`
(screen and print); (the hit test: L9.44). Tests `vizir_x_is_the_drawn_column_with_the_
fallback_quirk`, `labels_clamp_sideways_and_merge_with_a_space`,
`vizir_labels_follow_the_original`.
Residual: the ГВ label font / background colour are the DC's (caller's)
[ND] — ours the row font on white.

### L9.43 — Lane min / max computed once, not per frame  ·  ✅
Measured (release, `frame_bench`): a 500 000-sample × 8 record took
48.9 ms of CPU per frame — every lane's min / max (and the stats Max/Min)
re-scanned all samples each frame; the original keeps them at
`+0xa8`/`+0xb0`, recomputed when the values change. Ours: the unscaled
series' [finite min, finite max, min, max] cached per (channel, display
mode, `Doc::series_gen`), the generation bumped whenever the derived series
are rebuilt (`Doc::recompute`, load); the primary/secondary scale applied
on use. 7.0 ms per frame after; drawing every sample when zoomed out is
next (the original draws columns — research running). `frame_bench`
(`FV_FRAME_BENCH=<cfg>`) and test `range_cache_follows_value_changes`.

### L9.44 — Визир hit test, cursor, press order and the difference band  ·  ✅
Read (subagent, objdump): the shared hit routine `0x495800` (callers
`0x45e37c` in button-down `0x45dc10`, `0x45f6bc` in WM_SETCURSOR
`0x45f660`) tests V1 > V2 > V3 at **trunc(leftW + (s − first)·25·f)** —
not the painter's column search, so a визир the painter drops (on
`first`) is still grabbed at leftW..leftW+2 — with |Δ| < 3 on integer
pixels, then ГВ1 > ГВ2 at their drawn y (`0x4944a0`), then the comment
test `0x43fab0`, and only then the pending lane `0x45f3d0`; R-down forwards
to L-down without MK_LBUTTON, so no drag. Cursor: no leftW / band gate;
HAND for a merge drag / pending lane first, then SIZEWE (V, comment hit 9),
SIZENS (ГВ, hit 10), UPARROW in rectangle zoom. Difference band
`0x466630`: x = trunc((s − first)·25·f) + leftW; labels V1 V2 V3 then
moments by sample, drawn when x > leftW, dropped when the cell ±3 px
overlaps a placed one; dimension lines between consecutive samples of a
map (moments + shown визиры), drawn when x_a > leftW and x_b − x_a > 16,
no right-edge test.
Ours were: hit against the drawn x (L9.42, inferred), float ±3, pending
lane before comments, cursor gated to the plot with the resize cursor over
the hand; band at rounded x sorted by x with a right-edge cut. Now as read.
Test `a_vizir_on_the_first_sample_is_grabbed_at_the_plot_edge`.
Residual: comment hit codes 9 vs 10 — done in L9.60 (10 = a level line's
body → SIZENS).

### L9.45 — Waveform drawing: sample count, mark order, digital decimation rules  ·  ✅ (wired L9.46)
Read (subagent): the analog painters `0x429490` / `0x429c30` never
decimate — one aliased Polyline over offsets 0..min(⌈W/k⌉, len − first) at
x = L + rhu(25·i·f), Ellipse marks (4×4) **after** the line when f > 0.16
(fill = the DC's brush, ND); only the digital painter `0x42a0f0`
decimates: k ≥ 0.5 one Rectangle per run at truncated x (right edge
trunc(i·k) + 1, +4 at the data end; a run starting at the last visible
offset is not drawn), k < 0.5 1-px columns j = 1..P−1 (P = min(rhu(M·k),
W − 4), r = count / P) OR-ed into runs plus red ticks for mixed columns;
mid = top + h/2; pen RGB(200,0,0), fill RGB(253,219,102) when selected.
Ours: analog count `oscgeom::drawn_last` (was ⌊W/k⌋ + 1, one point at R),
marks after the line; with no marks each pixel column's vertices collapse
to first / min / max / last (`collapse_columns` — the same image, our
rendering economy for long records). `oscgeom::digital_shapes` implements
the digital rules (tested); wired into the lanes in L9.46.
Tests in `oscgeom::draw_tests`.

### L9.46 — Digital lanes drawn by the decimating painter  ·  ✅ (bit14 branch ✅ L9.71)
Read (`0x42a0f0`, re-checked by hand): lane mid = top + h/2 (h = 20 →
top + 10); a 1-px pen RGB(200,0,0) and the fill brush (channel `+0x110`,
RGB(253,219,102) when selected) are selected once; runs are GDI
`Rectangle`s mid−6..mid+6; the zoomed-out (25f < 0.5) transition ticks
are `MoveTo(x, mid−6)` / `LineTo(x, mid+6)` with the same red pen at
x = leftW + j for every column j = 1..P−1 that mixes on and off samples.
Columns are visited with a column counter that starts at ⌈25f⌉ = 1 and
steps by 1, so run edges are leftW + j exactly.
Ours: `OscView::row` (screen and print slices — both paint through it)
now draws `oscgeom::digital_shapes` (runs + 1-px red ticks) instead of
the old per-sample run list, which drew every sample at x_of (rounded,
not truncated), had no column OR-ing and no ticks; `digital_runs` and
the unused `OscView::last` removed. New exact test (401 samples,
k = 0.25: one run columns 2..6, ticks at 2 and 5). `window_dump` gains
`FV_WINDOW_XSCALE=f` to look at zoomed-out lanes; rendered the demo at
f = 0.01 / 1 and a real 18-channel record, and a print page.
Residual: the painter's **bit14 branch** (a pasted, not yet merged
digital channel drawn against its own time array — x from the time
interpolation, count ⌈W/(25f·R/r)⌉ + 5 scaled 11/10, icon 1 at
(leftW−3, top+h−19)) is not modelled — our pasted channels become
pending computed lanes (see the merge entries); the icon 0 drawn at
(leftW+1, top+h−19) for bit14 with kind 0 likewise. Our run outline is
egui's centred stroke on the rectangle edge (GDI draws it inside the
right / bottom exclusive edge): ≤ 1 px difference.

### L9.47 — Comment texts as slot 9; the moment's absolute time; value→y rounding  ·  ✅
Read (RTTI `MomentItem` / `LevelItem` / `AreaItem`, vtables `0x4c7100` /
`0x4c70d4` / `0x4c70a8`; slot 9 = `0x47a4e0` / `0x470f30` / `0x403fa0`):
slot 9 is both the info box text (`0x43df90`) and the level / area label
(their painters `0x471020` / `0x4040c0` call it; the moment painter does
not) — the comment text, then a **blank line** and the kind line:
«Уровень: %.2f» (text alone when the level's channel is gone,
`0x42edf0`); «Длительность: %d.%03d с» of |Δµs| with **truncated** s and
ms; for a moment «Время: » + absolute + «\nОт начала ОСЦ: %d.%03d с».
The absolute time: start date via `0x455800` (`dd/mm/yyyy`, "" when it
does not scan) + " " + the raw start time (`doc+0x250`), cut at the first
`.`; the left part through `VarDateFromStr` (user locale); `_wtoi` of the
first three characters after the `.` is **added to the µs offset** (the
start's milliseconds count as µs — they effectively vanish); whole seconds
move the COleDateTime (`0x47abb0`), `Format()` with no pattern
(`0x47aa50`, `VarBstrFromDate`: date alone at midnight, time alone on day
0, «Invalid DateTime» on failure) + "." + `%03d` of the sum's ms. When
`VarDateFromStr` fails the **whole text is empty** (the box shrinks to its
padding). Value→y in the analog painters rounds half up (L9.33's read).
Ours: `fvb_core::comtrade::moment_time_text` (+ `ole_date`), `Comment::
info(us, abs)` for the box and both labels (`comments::Geo` carries the µs
and absolute providers; µs = `oscgeom::micros` of the time from the first
sample), `seconds_ms` on whole µs truncated; `oscgeom::value_y` uses
`round_half_up`. Tests `moment_time`, `info_texts_as_slot_9`;
`window_dump` gains `FV_WINDOW_STATE=comments` (rendered the hovered
moment and the labels).
Residual / ASSUMED: the user locale is taken as ru-RU / uk-UA (day-first
parse, `dd.MM.yyyy H:mm:ss`); other locales and OLE's lenient spellings
are not modelled; the ±3615897-day span check and the date-0 / time-0
edge (`0:00:00`) not exercised.

### L9.48 — «Добавить новую формулу» OK: the 396 sequence  ·  ✅
Read: OnOK `0x403530` calls `UpdateData(TRUE)` and **ignores its result**;
the DDX `0x403180` reads the name control (0x40a) and, for an empty text
or a leading `iswdigit`, shows 396 (`AfxMessageBox`, MB_OK) and calls
ordinal 4567 = `CDataExchange::Fail` (identified in the carved mfc120u:
SetFocus + EM_SETSEL, then the user exception) **before** its DDX_Text
updates the member `+0x1c4` — so the member keeps the last name that
passed. OnOK then checks that member with `0x4033a0` (leading non-letter
→ 396, a record / formula name → 473, both MB_ICONERROR; either stops
here), builds the formula, and calls `CDialog::OnOK` (ordinal 10896),
whose second `UpdateData` repeats the DDX 396 and keeps the dialog open.
So an empty / digit-led name gives 396 twice (or 396 then 473 / 396
when the stale member is bad); a valid name over a taken one gives 473.
Ours: `af::ddx_name_ok`, `AddFormulaDlg::{member, alerts}` (a queue of
message boxes), `accept` in that order. Test `name_is_checked` (four
cases incl. 396 → 473 over a taken member).
Residual: the message box icons (exclamation vs error) are not drawn;
`iswdigit` taken as ASCII digits.

### L9.49 — The finder's delete: one shell delete, recycle bin unless Shift  ·  ✅ (Windows compile-checked)
Read (subagent report, Shift / flags re-checked by hand at `0x46df84`):
`0x46d4b0` does nothing with no selection (LVM_GETSELECTEDCOUNT); walks
the selection (LVM_GETNEXTITEM, LVNI_SELECTED); for a name ending
(case-insensitively) in .osc / .cfg / .cff / .sto adds the file itself
unconditionally and each companion that **opens** (CreateFileW
GENERIC_READ, FILE_SHARE_READ, OPEN_EXISTING — a companion locked for
writing elsewhere is skipped) — base = path minus its last 3 characters;
the list is double-NUL terminated; then ONE `SHFileOperationW`: hwnd
NULL, FO_DELETE, fFlags `FOF_ALLOWUNDO` (0x40) unless the Shift byte of
`GetKeyboardState` is ≥ 2 (then 0 = permanent); no FOF_NOCONFIRMATION /
SILENT / NOERRORUI — **the program asks nothing, the shell does**. Non-zero
return → nothing; zero (aborted flag unchecked) → erase the rows' records
and a full rescan (`0x46b9c0`). Inferred latent bug: the record erases use
unadjusted indices (several selected rows can run past the shrunk vector);
the files are gone by then. (The spec had Shift backwards — fixed.)
Ours: `finder::shell_delete` — Windows: that exact `SHFileOperationW`
call; elsewhere a stand-in prompt (there is no shell prompt), then
`trash::delete_all` or, with Shift, `remove_file`; rescan on success.
Test `shift_delete_removes_the_companions` (permanent path; the trash
path is not exercised in tests). Residual: the Windows call is only
compile-checked (its shell dialogs cannot run over ssh); our companion
test is "exists as a file" (any case of the extension), not "opens for
shared reading"; the record-erase bug is not reproduced.

### L9.50 — Group lanes: a full block per member, one-line stats  ·  ✅
Read (subagent report; the height re-checked by hand at `0x46045f`, the
line builder `0x4683c0` and label 3449 «СКЗ:» by hand): a group's visible
members (`0x455930`, doc+0x174 name → member set) are painted in channel
order, member k at top + 5 + k·S with **its own** S = rowH + 33 (+ hS when
it has any of bits 4/5/6): the full header (grey frame, cut name in its
colour, value at В1); no Max/Min/СКЗ/A1 rows but one DrawText line (font
`view+0xbc`, RGB(0,0,127), x 5 → leftW, top + 8 + k·S + hS, height hS,
single-line centred) built by `0x4683c0`: `Max:` `Min:` `СКЗ:` `A1:`
items joined by `|`, each the SI-scaled value `%-8g` trimmed, cut to 2
decimals with trailing zeros stripped when it has a `.` and no `e`, then
the prefixed unit cut to 5; its status icons at top − 21 + (k+1)·S. The
lane height `0x460290`: non-owners 0; the owner N × (rowH + 33 + (owner
has bits 4/5/6 ? hS : 0)), at least `view+0x104` — no +10 / per-row / +23
terms (the owner's flags size it, each member's flags paint it).
Ours: `views::group_stats_line`, `lanes::{group_stride, chan_icons,
GROUP_STATS_H}`, the group branch of `OscView::row`, `lane_layout`'s
group need (was members × 49 and a name + value line per member). Tests
`group_line`, `group_lane_height_is_members_times_the_owners_stride`;
`window_dump` gains `FV_WINDOW_STATE=group` (rendered with a real record).
Residual: hS scaled like our fonts (13 px); whether "none" units are
blanked in the line NOT DETERMINED (kept); СКЗ / A1 read at В1 without
the bit-14 interpolation (as the original; ours has none either).

### L9.51 — «Быстрый анализ» is a modal sheet, not a view  ·  🟡 (stage A)
Read (subagent report; the sheet in VIEWS_SPEC «The sheet»): 137 shows a
modal `CMFCPropertySheet` with a 150 px Outlook bar, caption 159, only
«Рассчитать» / «Отмена», always opened on «Редактор формул»; only the
active page generates; Cancel generates nothing.
Ours (stage A): `Command::QuickAnalysis` (enabled iff analog channels;
the ribbon button is a plain command now) opens `quick_sheet_window`:
the six pages in a 150 px left bar (our glyphs, not bitmap 549), the page,
«Рассчитать» (`quick_calc`: the active page's channels, nothing for the
formula editor) and «Отмена» / Esc / ×; `View::Analysis`, the central
tab strip, our «Каналы…» button and `Command::Show` removed; the form is
filled with the role defaults on opening. Test
`quick_analysis_opens_the_sheet_on_the_formula_editor`; rendered.
Residual (stage B, all read — VIEWS_SPEC): the six page layouts and their
enable / disable dependencies; OK greying per page; per-record
persistence in the `.set` (load on open, save on both buttons); the
formula page as the original's list editor (DIALOG 340: list + Загрузить /
Сохранить / ↑ / ↓ / Добавить / Удалить / Удалить все) working on a copy that
«Рассчитать» applies (ours edits the record's list live); Enter as the
default button.

### L9.52 — Sheet pages 536 / 538, OK greying, the page defaults  ·  ✅ (stage B1)
Read: VIEWS_SPEC «The sheet» (subagent; `0x487d70`, `0x487de0`,
`0x48c870`, `0x48cb60`, `0x48d5a0`, `0x488550`, defaults `0x411f40`).
Ours: `symmetric_page` draws 536 and 538 as the templates group them —
«Токи» (Ia / Ib / Ic, «Рассчитать Ib по Ia и Iс *» greying Ib, the
footnote; 538 «Iтр, А:» «- ток точной работы») and «Напряжения» (both
radios, Ua–Uc and Uab / Ubc always shown, the other set greyed); on 536
each «Отключить» greys its block and the other group's check;
`QuickForms::ok_enabled` greys «Рассчитать» (536: each live group's combos,
Ib unless calculated, voltages by the radio; 538: Iтр + all). Defaults
now as `0x411f40` (Iтр 0.25, R/X/K «1.», Ф / Rm / Xm / Km / Фm «0.»).
Test `calculate_needs_the_pages_channels`; rendered both pages.
Residual: pixel placement follows the template's grouping, not its dialog
units; «нет канала» / an empty combo counts as not chosen (how CB state
maps to the check NOT DETERMINED); our pages share one set of fields
(the original keeps each page's own); the other pages, persistence and
the formula list editor remain (L9.51).

### L9.53 — Sheet pages 537 / 543 and their OK checks  ·  ✅ (stage B2)
Read: `0x48a1f0` by hand (BM_GETCHECK of the radios / «Отключить» checks,
then each needed combo's selection: a live current group needs Ia–Ic when
«Расчетный», 3Io when «Измеренный»; a live voltage group Ua–Uc, Uни / Uик
or 3Uo by its radio); the rest per VIEWS_SPEC «The sheet».
Ours: `zero_page` (separate «Отключить» checks `zero_i_off` / `zero_u_off`
— the radio no longer carries "off" — greying each other as on 536; the
radios enable only their combos), `phase_page` (удельные / компенсации
edits, «3Ion:» with «Не учитывать» first and its Xm / Rm or Km / Фm edits
only with a channel; «Не учитывать» generates the "0" argument);
`ok_enabled` for 537 and 543; 3Io / 3Uo / 3Ion preselect 3I0 / 3U0 / 3I0n
by name. Test `calculate_needs_the_pages_channels` extended; rendered.
Residual: as L9.52 (grouping not dialog units; shared fields across
pages); 539, persistence and the formula list editor remain.

### L9.54 — Sheet page 539 «Дифференциальные токи»  ·  ✅ (stage B3)
Read: VIEWS_SPEC «The sheet» (subagent; `0x4835b0`…, `0x482c10`,
`0x483ab0`, `0x482b60`, `0x483da0`, `0x4831b0`).
Ours: `differential_page` — «Количество плеч:» 2…24 and «Количество фаз:»
{1, 3} combos; per arm «Приведение плеч» (4 radios; each block's own edits,
only the chosen block enabled — `ArmForm` now keeps every block's values
instead of three shared p1–p3) and «Учет группы соединений» (greyed with
1 phase; «Группа:» 0…11 / {0, 1, 11} / greyed, «Удалить I0» only By
sequences); «<<» «Текущее плечо:» «>>» over one arm group «Плечо %d»;
«Расчет тока торможения» with «Полуразность токов» only for 2 arms (else
reset to Полусумма) and the «Kторм:» {1, 0.5} combo for Полусумма /
2-arm Полуразность. One phase generates no group compensation. Rendered.
Residual: (defaults read 2026-10-23: the pages' shared OnInitDialog
`0x48b690` calls `0x411f40`, which sets every one of ids 1050–1058 a page
has to «1.» (GetDlgItem guarded) — all of 539's reduction edits; the
first radio of the group in template order is «По коэффициенту
приведения» (the load forces «По мощности» when none is set — L9.79) — both as
ours); what a newly added arm stores before it is first shown is still
inferred («1.», Kприв); grouping not dialog units.

### L9.55 — Sheet pages keep their own fields and persist in the `.set`  ·  ✅ (stage B4)
Read (subagent; DIALOG 539's edit ids mapped by hand from the template:
R0 Kприв, R1 Kттбаз, R2 Kтт (ТТ), R3 Kтт (мощность), R4 Kтт (ном. ток),
R5 Sном, R6 Iном, X0 Uном): a page created (first shown) gets-or-creates
its category under doc+0x300 (`0x48dc80`) and loads it (`0x48b690` → vslot
0x1d4); every created page saves (vslot 0x1d8) on BOTH «Рассчитать» and
«Отмена»; no SetModifiedFlag (the tree reaches disk with the `.set`).
Combos: a child `<KEY channelName="text"/>` (flag 1, created on load too)
or a flat attribute (539's counts / Kторм / per-leg); load applies a
non-empty name the list has (case-insensitive), selecting the FIRST item
that starts with it; edits raw text, non-empty only; checks `%i`, missing
→ untouched, forced first radio when none is set. Keys per page — 536
SymmetricItemsPage IA…UBC + currOffState voltOffState calcIBState
phaseVoltState lineVoltState; 537 NullSequencePage IA…UC 3I0_0 UNI UIK
3U0 + calcCurrState measuredCurrState calcUABCState calcUniUicState
measuredVoltState currOffState voltOffState; 538 InterPhaseResistPage
itr (lowercase) + combos + phase/line/calcIB; 543 PhaseResistPage R0 R1
X0 X1 K0 F0 RM XM KM FM ITR (uppercase) + combos + 3I0n + specificState
compensateState; 539 DiffCurrentsPage legOption / groupConnOption /
brakeCurrOption children, legCount phaseCount brakeCoeff (item texts),
Leg_0… (IA IB IC seqGroup triangleGroup R0–R6 X0 I0State) loaded into
the SAME controls one after another (a missing value keeps the previous
arm's — WRONG, see L9.79: each leg loads over its own initial slot),
higher Leg_ nodes never removed; FormulaEditorPage stubs (only
the empty category).
Ours: `App::quick` is one `QuickForms` per page (`page_index`); 
`quick_show_page` / `quick_save_pages`, `load_page` / `save_page`
(`PAGE_KEYS`); the formula page creates its empty category. Tests
`symmetric_page_round_trips_through_the_set`,
`differential_page_legs_inherit_missing_values`, the sheet test.
Residual: our change detector compares the whole tree (L9.17), so page
edits surface as modified at close like any tree change — whether the
original's detector sees these categories NOT DETERMINED; per-arm option
radios saved from arm 1 (the original saves after re-selecting arm 1 —
which state the option nodes then hold is inferred); 539's seqGroup /
triangleGroup share our one group field; the `I0State`-missing flag at
page+0x5b0 not modelled.

### L9.56 — «Редактор формул», the sheet's formula list page  ·  ✅ (stage B5)
Read (subagent report): DIALOG 340 (`0x485e30`) copies the record's
formula list; the list view shows name / formula (25 % / 70 %, full-row
select, grid lines, no sorting). `UpdateData(TRUE)` with the list changed
(after every button and kill-active) deletes at once each record channel
of the list no longer among the rows; kill-active (`0x486d20`) also writes
the copy into the record's definitions (no recompute). In-place edit only
by a double click (`0x486850`) on the selection mark's row, the column
under the pointer; Enter AND Esc commit (`0x486bf0`), as does a click
elsewhere; column 0 is checked (`0x486a50`): a leading letter (396), not
a record channel nor another row (473, case-sensitive — confirming a
computed channel's own name is refused). Buttons never greyed:
«Загрузить» (`0x414ee0`, *.frm, string 3165: the rows' channels deleted
first, the file's `name,formula,` lines replace the list, record
modified), «Сохранить» (`0x415230`, `name,formula,` CR LF, CP1251), ↑ / ↓
(`0x487210` / `0x4873a0`), «Добавить» (`0x486df0`: DIALOG 359, «Канал%i»
over the rows, the row appended and selected), «Удалить» (`0x487170`, no
confirmation), «Удалить все» (`0x487530`). «Рассчитать» on this page:
kill-active, then `0x415780` over the list — an existing channel of that
name (exact, else case-insensitive) updated in place (flags, colour
kept), a new one inserted after the analog channels; no diagram flags;
on failure the backups are re-applied (created channels stay). «Отмена»
drops the copy but undoes no deletion.
Ours: `formula_page` (`FormulaPage`, `fp_*`), `fvb_core::quick::{parse_frm,
write_frm}`; DIALOG 359's OK appends a row (no longer a channel); the old
ad-hoc editor (`formula_editor_ui`, `add_computed`) removed. Tests
`formula_page_edits_the_record_list`, `frm_round_trip`, the DIALOG 359
test; rendered (`FV_WINDOW_STATE=analysis` adds two formula channels).
Residual: kill-active does not rewrite our merged channel list (the
original's definition list changes without a recompute — ours keeps the
old channel until «Рассчитать»); on an evaluation error ours restores the
record's formula channels as before (the original re-applies its backups
and leaves created channels); `.frm` read in the program encoding,
written CP1251; column hit by pointer x without the scroll offset not
reproduced; ↑ / ↓ show arrows, not BITMAP 475's icons.

### L9.57 — The display factor k both ways: «P» records shown secondary  ·  ✅
Read by hand: the per-channel recompute `0x428490` sets k = `ch+0xf0`:
`+0x89` (show primary, set by 32867) with `+0x88` clear (the file stores
secondary) → `+0xd0 / +0xd8` (primary / secondary); `+0x89` clear with
`+0x88` set → `+0xd8 / +0xd0`; else 1. The COMTRADE channel constructor
`0x422460` clears both bytes (`0x4225a6`) and sets `+0x88` from the P/S
token (compare with "P", `0x4228b4`) — so `+0x89` starts clear and a «P»
record is shown in SECONDARY values, divided by its ratio. The value
accessor `0x426020` (asm): x·k; with ADC the integer code instead; with
remove-DC minus k·`ch+0xf8`, where `0x428795` stores the mean of the ADC
codes in ADC mode (of the raw values otherwise); invert ×(−1).
Ours: `app::display_k` (both cases; the oscillogram's `primary_scale` and
the «П» status icon use it — they only handled «S» shown primary, so the
user's «P» records (e.g. ratio 2000 : 1) were shown in primary values
under «Вторичные знач.», disagreeing with the original and with our own
diagrams, whose `fvb_core::channels` already had both cases);
`flag_series` subtracts k·mean code in ADC mode. Test
`display_factor_both_ways_and_adc_dc`; rendered a user «P» record (IA
454 mA secondary with «П», was ≈ 908 A).
Residual: a zero coefficient (division) is guarded to 1 (the original
divides); the `.set`'s per-channel show flags restore `+0x89` as before.

### L9.58 — Copy with a property grid focused: the document, no picture  ·  ✅
Read by hand (asm): the copy case `0x45be28` opens (OpenClipboard(view))
and empties the clipboard, then calls `0x405160`, then writes the XML.
`0x405160`: focus = FromHandle(GetFocus()); child = the focus window's
first untitled child (FindWindowExW(focus, NULL, NULL, "")). Focus = the
view, or no such child → the view's picture. Else the child is
dynamic-cast to DiagramWnd (`0x4ae484`); if not one, the next untitled
child after it is tried; a DiagramWnd → EmptyClipboard, CloseClipboard,
its slot +0x170 (the diagram picture, §L9.30); **neither → return with
nothing set**, so the caller's XML lands alone. The circle / differential
property grid takes the focus on a click (MFC property grid) and its
untitled children are its scroll bar / header, never a DiagramWnd → the
third case. A window with no untitled child (e.g. a plain edit) gets the
view's picture.
Ours: `app::CopyFocus { View, Diagram(pane), Grid }`; the grids' rects
are recorded while drawing (`pane_grid`), a press there sets `Grid`
(`track_copy_focus`), and copy then puts the document with no picture
(a focus on a hidden pane / grid falls back to the view). Test
`copy_pictures_the_view_or_the_focused_diagram` (grid case added).
Residual: other focus targets with untitled children (e.g. the merge
bar's combo boxes, if a drop-down style one has the focus) are not
modelled — only the view, the diagram windows and the grids move our
copy focus.

### L9.59 — The comment under the pointer: sticky, printed, drawn twice  ·  ✅
Read by hand (asm): the print slice `0x405970` calls the view's Draw
through its vtable (+0x1b8 = `0x45f950`), which calls the comment painter
`0x43f630` unconditionally — no print test. The painter draws every
element through slot +0x18 (each one flags itself selected when it is the
manager's selected element, `0x404110` for the area), then the selected
one (`doc+0x854`) **once more** through +0x18 and its info box through
+0x14 — so a selected area's AlphaBlend fill (unconditional, `0x404332`)
is laid twice. `doc+0x854` is written by the mouse move `0x45ea70`
(`0x45ef09`): with no drag mode, and only when x ≥ the stats-panel width,
the hit test `0x43ea60` re-picks it (repaint on change); there is no y
test and the view's map has no WM_MOUSELEAVE (only WM_MOUSEHOVER for the
stats popup), so the selection **stays** while the pointer is over the
stats panel or outside the view — on screen, in print and in copy's
picture. Delete (`0x43f5f0`) erases the element and resets the selection.
Ours: `Doc::comment_hot`, re-picked in `lanes_ui` only when the pointer
moves to a new spot of the client area right of the stats panel with no
drag (`App::view_pointer` stands in for WM_MOUSEMOVE), kept otherwise;
`comments::paint` draws all, then the selected one again plus its info
box; the printout passes `comment_hot` and now places level lines as the
screen does (`g::level_y` on the line's own channel range, nothing
outside it — it used the lane's range unchecked); delete clears it. Test
`the_comment_under_the_pointer_sticks_until_the_next_plot_move`; rendered
(an area selected: dark border, denser fill, info box).
Residual: the cursor shape over a comment still uses a live hit test
(WM_SETCURSOR `0x45f660`, L9.44's open item); a scroll / zoom under a
still pointer keeps the old selection (as the original, which only
re-picks on WM_MOUSEMOVE) — not separately tested.

### L9.60 — Comment cursor codes, the lock, and what a press does under a selection  ·  ✅
Read by hand (asm): `0x43fab0` (called by WM_SETCURSOR `0x45f703` and
button-down `0x45e3bf`) walks the elements; for each whose ±10 px test
(slot +0x8) hits **and that is not locked** (`+8`) it asks the part test
(slot +0xc: x only, the pointer ±10 px turned into a sample range, rounded
to the nearer of floor / ceil): moment 2 if its sample is in range;
level (`0x4715c0`) none outside from..to, 2 at `from`, 3 at `to`, else 4;
area (`0x404460`) 2 at `from`, 3 at `to`, else **none** (the top / bottom
border band has no part). The first part found is stored at `doc+0x85c`
and returned as **10 for part 4, else 9**. Cursor (`0x45f660`): codes
1–3 / 9 or modes 1–3 → IDC_SIZEWE, 4–5 / 10 or modes 4–5 → IDC_SIZENS,
modes 7 / 8 → IDC_UPARROW, a pending lane or mode 6 → IDC_HAND first
(no cursor from the comment drag mode itself — the live code decides).
Button-down: LMB with code 9 / 10 → that mode; the drag `0x43efa0` moves
the **selected** element (`+0x854`) — nothing when none, x < leftW or it
is locked; no test of the text flag (+9; the L9.6 claim "an element
whose text is shown" was wrong). Then the pending lane; then, with a
tool armed, LMB only and `0x43eb40` places only when nothing is selected
(`0x43ec35`) — any other press does nothing; then Shift → H визир; mode 7
→ rectangle; and the визир placement only when **nothing is selected**
(`0x45e65d`). The selection is the hover pick (L9.59), which takes locked
elements too: a locked element highlights, opens MENU 563, takes a
double-click (`0x465890`: x ≥ leftW and a selection) and blocks V
placement, but gives no resize cursor and does not move.
Ours: `part_at` in `lanes_ui` (first unlocked hit with a part ≠ 0); the
press chain in the original's order with the drag on `comment_hot`, the
armed-tool branch swallowing other presses, V placement gated on no
selection; the cursor N–S for part 4; the context menu and double-click
on `comment_hot`. Test `comment_cursor_codes_lock_and_visir_suppression`
(and the harness keeps the cursor).
Residual: our part test is in pixels (|Δx| ≤ 10) rather than the sample
range rounded to floor / ceil — the same at ordinary zoom, may differ by a
sample when samples are wider than 10 px.

### L9.61 — The stats-panel drag starts by MFC's rule; a self-drop is inert  ·  ✅
Read by hand (asm): the mouse move `0x45ea70` in mode 0 / 7 (`0x45ed4c`):
no inline edit, MK_LBUTTON, x < leftW (the current point — the press may
have been anywhere in the view), and neither MK_RBUTTON nor MK_CONTROL →
the selected (`+0x100`) analog channels are collected; if any,
`COleServerItem::DoDragDrop(client rect, point, no link, copy|move, start
rect NULL)` (`0x45ee88`). MFC (`COleDataSource::DoDragDrop` +
`COleDropSource::OnBeginDrag`, known library behaviour, not re-read from
mfc120u): a NULL start rect → the empty rect at the cursor inflated by
DragMinDist 2 → [x−2, x+2) × [y−2, y+2); with capture it waits until a
mouse message's point is outside it, or DragDelay 200 ms pass with the
button held; a button up / down or Esc cancels (no drag; a later move
calls DoDragDrop again). OnDrop on the view (`0x441709`): the analog lane
under y (heights `0x460290` over the shown channels, scroll subtracted);
**if it is one of the dragged channels nothing happens** (`0x4417b7`), else
the dragged ones and it are selected and 33035 is sent, or message 1086
when `0x468c80` refuses — so the handoff's "a self-drop shows 1086" was
wrong: a slow click that turns into a drag and lands on its own lane is
silent.
Ours: `App::stats_drag_arm` (anchor, time, channels) set by a pointer
move over the panel with LMB held, no Ctrl / RMB / drag mode / inline
edit; the payload is set when the pointer leaves the floored [−2, 2)
box or 200 ms pass (a repaint is requested for the timeout); release,
RMB or Esc disarm. Test `a_stats_drag_starts_after_2_px_or_200_ms`
(timeout, early release, the asymmetric box, a silent self-drop).
Residual: the MFC values come from the library defaults (DragMinDist /
DragDelay can be changed in WIN.INI's [windows]); while the original
waits it swallows the view's mouse messages (no hover pick meanwhile) —
ours keeps tracking.

### L9.62 — Message boxes: MFC's prompt, caption and icons; the inline rename's commit  ·  ✅
Read (local only: `extracted/mfc/mfc120u.dll` / `mfc120rus.dll`, copied
from the vcredist carve so they outlive /tmp; string tables with
`tools/pestr.py`, ordinals from the export table, disassembly by hand):
FastView.exe has no string in MFC's AFX range except a few window-menu
prompts, so `AFX_IDP_ASK_TO_SAVE` 61699 comes from mfc120rus: «Сохранить
изменения в %1?» (ours already). No `0xf103` anywhere in the EXE: the
question is `CDocument::SaveModified`'s (the override `0x454fa0` only
skips it for `.sto`) — `%1` = `AfxGetFileTitle(path)`, MB_YESNOCANCEL, and
`CWinApp::DoMessageBox` adds MB_ICONQUESTION. Every message box is
`AfxMessageBox` (no MessageBoxW import; `#2261` by id, `#2262` by text) →
caption `m_pszAppName` = `AFX_IDS_APP_TITLE` 57344 «Механотроника
FastView»; with no icon flag MFC adds the exclamation to MB_OK. Call sites:
by id 396 MB_OK `0x403213` (the formula dialog's DDX), 396 / 473
MB_ICONHAND `0x4033ed` / `0x403519` (its member check) and `0x486a9d` /
`0x486bd3` (the formula page), 397 MB_OK `0x435f2f` / `0x4660ab`, 1086
MB_OK `0x441843`; by text MB_OK ×5, MB_ICONHAND ×2 (`0x4677dc`,
`0x47d915`), OK-Cancel + information once (`0x44c03f`). The inline rename's
commit `0x465df0`: GetWindowText as is; for every other channel of the
vector (file and computed, bit 13) compares with `#2948` =
`CString::Compare` (`wcscmp`, case-sensitive) against its name (+0x68)
and its encoded name decoded (`0x4292e0`: +0x64 through `#8758`
OemToAnsi for the default code page) → 397 and close; else `0x425a30`
sets it (no trim, no empty test); then SetModifiedFlag on **every**
commit (`0x466062`), UpdateAllViews and the panes.
Ours: `app::APP_TITLE` on every message box (three said «FastView»);
`dialogs::MsgIcon` + `message_body` (our drawing of the exclamation /
error / question icons) on the alert, the ОМП message, the save prompt
(question), the formula dialog (per message: DDX exclamation, member
check error) and the formula page (error); the inline rename: message
box 397 instead of a status-bar text, the text as typed, modified on
every commit. Test `inline_rename_commits_as_typed_or_refuses_with_397`
(+ the formula dialog's icons in `name_is_checked`); rendered the alert
(`FV_WINDOW_STATE=alert`).
Residual: `%1` keeps the extension (GetFileTitle drops it when Explorer
hides known extensions and `.cfg` is registered — machine-dependent);
the text-variant boxes other than those above are not mapped to ours one
by one (ours default to the exclamation); on a non-Russian Windows MFC's
prompt would come from mfc120enu / the English base.

### L9.63 — Document tab colours: mfc120u's list and pick rule  ·  ✅
Read by hand in the carved mfc120u (local only): no colour of the assumed
"OneNote" palette occurs in the DLL at all. The tab control's auto-colour
list (`0x1006a975`, array at `this+0x1bc`, only when auto colour is on):
with more than 8 bits per pixel 16 colours in order — (197,212,242),
(255,220,120), (190,206,161), (240,160,161), (188,168,225), (156,193,182),
(247,184,134), (217,173,194), (165,194,215), (179,166,190), (234,214,163),
(246,250,125), (181,233,157), (95,195,207), (193,131,131), (202,202,213)
(a different list at ≤ 8 bpp, not read). The pick (`0x1006a29c(iTab)`,
for a tab whose colour is still −1): idx = iTab mod 16; if idx equals the
static "last" (`0x102b0588`) → idx = (++counter `0x102b0584`) mod 16; if
the tab left of it has colours[idx] → again ++counter; then last = idx and
the tab keeps that colour. Both statics are zero-initialised (.bss) and
shared by every tab control of the process. So the first document gets
index 1 — (255,220,120), exactly the bottom colour sampled from the help
screenshot (L9.8) — and the next ones 2, 3, … while tabs are appended.
Ours: `app::TAB_COLORS`, `TabAuto::assign` (the rule, the statics in
`App::tab_auto`), `Doc::tab_color` given when the tab is first laid out,
in tab order. Test `tab_auto_colours_follow_mfc`; rendered (first tab
yellow).
Residual: the ≤ 8 bpp list not modelled; when MFC calls the pick (at
insert vs at layout) was not traced — ours at first layout, in order,
which gives the same colours for tabs appended one by one or together.

### L9.64 — «Окна...»: MFC's windows dialog  ·  🟡
Read by hand: `OnCreate` (`0x47296b`) calls `EnableWindowsDialog(131,
132 «&Окна...», always, no help button)`. The dialog is MFC's
`CMFCWindowsManagerDialog` (mfc120u; template 16927 «Окна» of mfc120rus,
316×169 DLU: «Выбрать о&кно:», a sorted owner-drawn extended-selection
list, buttons Активировать (default) / ОК / Сохранить / Закрыть окна /
Каскадом / Горизонтально / Вертикально / Свернуть / Справка). Constructor
`0x101aa325` (template 0x421f, frame, help flag; "MDI actions" +0x114 = 1);
OnInitDialog `0x101aa7c7`: fill, then hide the four arrangement buttons
only when "MDI actions" was cleared, select, update, Справка shown only
with the help flag (off here). Fill `0x101aaafc`: every MDI child the frame
lists, its window text added (LB_ADDSTRING, so sorted); a child whose
slot +0x1f0 holds goes on one list, a child without WS_SYSMENU (or without
a system menu) clears "MDI actions", a child whose SC_CLOSE is disabled
goes on the "cannot close" list. No code in mfc120u removes WS_SYSMENU
from tabbed children (no such mask found), so "MDI actions" stays on.
Selection `0x101aaa4e`: all deselected, the active child (WM_MDIGETACTIVE)
selected. Buttons `0x101aa897` (n = selected count): Закрыть окна n > 0 and
no selected window on the "cannot close" list (the walk stops at the
first one), Сохранить when a selected window is off the slot-+0x1f0 list,
Каскадом / Горизонтально / Вертикально "MDI actions" and n ≥ 2, Свернуть
"MDI actions" and n > 0, Активировать n = 1. Message map: Активировать and
LBN_DBLCLK `0x101aa495` (exactly one selected: restore if minimised,
WM_MDIACTIVATE, EndDialog(IDOK)); Сохранить `0x101aa5aa` (each selected
window's active document saved through its vtable +0xec, then refill /
reselect / update); Закрыть окна `0x101aa666` (WM_CLOSE to each selected
window from the last row up — each runs SaveModified — then refill);
LBN_SELCHANGE → update; ОК is not in the map (the dialog's OnOK).
Ours: `windows_dlg.rs` — «Окна...» after a separator at the end of the ▾
list; the dialog with the list (titles sorted ignoring case, full-width
rows, click / Ctrl / Shift selection, double-click activates), the
button states, Активировать (activate + close), ОК (close), Сохранить
(each selected document's Save in list order, then the active one
selected), Закрыть окна (one document at a time, last row first, each
through our close with its question — «Отмена» keeps that one and the
next follows — then the active one selected); documents carry a serial
so the queue survives the index shifts. Tests
`extended_selection_and_button_states`, `close_windows_one_by_one`;
rendered (`FV_WINDOW_STATE=windows`).
Residual (honest): slot +0x1f0 of the child frame not identified — ours
treats every document as savable; the four arrangement buttons are
enabled as read but do nothing in our tabbed view (what WM_MDICASCADE /
TILE / minimise do to tabbed children NOT DETERMINED); ОК assumed to be
`CDialog::OnOK` (the vtable slot not checked); LBS_SORT's locale
collation approximated by a lower-case compare; the dialog is our size in
pixels, not converted from its DLUs exactly.

### L9.65 — Correction of computed values by a measured frequency  ·  ✅
Read (subagent report, key points re-checked by hand: the gate at
`0x4262ac`, the constants and window in `0x4267e0`, the candidate test
`0x42aee0` and the selection tail of `0x42ebd0`, the compares `#2949` =
`CStringA::Compare` (`_mbscmp`) and `0x42deb0` → `wcscmp`). «Частота для
коррекции расчётных значений» is `rec+0x874` (0 none, k the k-th
candidate). Candidates (`0x42ebd0` over the channel vector, `0x42aee0`):
analog channels whose ph (`+0x70`) or ccbm (`+0x74`) is exactly «system»
or whose canonical unit is Гц / ГЦ / Герц / ГЕРЦ / Hz / HZ / Herz / HERZ.
The rebuild ends with: selection 0 or past the list and a candidate
exists → **1**. It runs at `.cfg` load (`0x45127d`, so such a record opens
corrected), on every channel insert / remove (`0x42e653`, `0x42e92b`,
`0x42eaba`, `0x42ebb7`, `0x42edba` — formula channels, ОМП results,
paste) — so «Не корректировать» does not survive the next channel change —
but not at `.osc` load. The gate in every method: analog channel, exactly
one rate segment (`rec+0x1f0`), a candidate list, selection > 0, and the
channel is not the frequency channel itself. Then f = the frequency
channel's stored sample at the window's newest index (no ratio / flags),
50.0 when 0; fs = the rate (`ch+0x108` or the record's); `M = P·fs/f`,
`K = ⌊M⌋`, `τ = P/f − K/fs`; 0 while idx ≤ K; trapezoid weights `dt/2`,
`dt`…, `dt/2 + τ − τ²/2dt`, `τ²/2dt` (exact over P/f for the linear
interpolant). Phasor / harmonic `0x4267e0`: `(√2·f/P)·Σ w_j·x[idx−j]·
(−sin jθ + i cos jθ)`, θ = 2π·n'·f·dt (n' = n, or n/P from the frequency
diagram's `0x426510`) — the same sine reference at the newest sample as
the normal DFT; no `rec+0x278` gain; fourie's n = 0 goes through it too
(√2·|mean|). rms `0x427450`: `sqrt((f/P)·Σ w_j x²)` (0 past the frequency
channel's end); a0 `0x426df0`: `(f/P)·Σ w_j x`; fault `0x427830`: the
signed `x[idx] − 2·x̃(1/f) + x̃(2/f)` with linearly interpolated samples
one and two measured periods back (no RMS). The per-channel cache
`0x428490` fills the phasor cache with the corrected fundamental (stored
phasors, flag 0x2000, win) and the display cache through rms / fourie.
The file dialog (`0x4499e0`) lists «Не корректировать» + the candidates'
names; a changed choice is stored, the document marked modified and every
channel recomputed (`0x44a2c9`). The frequency diagram calls
`0x426510(n = h·P, idx, P)` (`0x46f5a0`).
Ours: `fvb_core::freqcorr` (`candidates`, `refresh_selection`, `of` (the
gate), `FreqCorr::{harmonic, phasor, rms, a0, fault}`, dispatchers
`sliding_phasors / sliding_rms / phasor_at / rms_at / fundamental_at /
spectrum`); `Oscillogram::{segments, freq_sel}` and `AnalogChannel::ccbm`
(the `.cfg` loader rebuilds; `.osc` counts rate changes between blocks as
segments — inferred; a merged record has one); `Analysis::compute_with`,
`OscChannels::new_with`, `quick::evaluate_batch_with` take the correction
from the record as stored while working on the flagged copy; the RMS
cache, the vector / circle phasors of computed channels, the frequency
diagram and the stats panel use it; `Doc::channels_changed` after every
channel add / remove (formula page, quick analysis, ОМП results and clear,
paste and merge, `.set` load); the dialog's combo works. Tests
`fvb-core/tests/freq_corr.rs` (4: a constant integrates exactly for any
f and P, an off-nominal sine's magnitude / angle / RMS / fault, warm-up
and the 50 Hz fallbacks, candidates / load selection / rebuild / gate)
and `frequency_correction_on_load_off_and_back`; rendered the dialog on a
record with a «Гц» channel (selected by default).
Residual: computed channels are not candidates (the original's vector
holds them, so a formula channel in Гц would be one — done L9.78); (`frequency:` is
`rec+0x1fc[idx]`, the nominal frequency, and does not read `rec+0x874` —
unchanged, as ours); merge synchronisation (`0x427670` / phasors in
`merge.rs`) keeps the normal path; a0 / fault past the frequency channel's
end take 50 (the original reads out of bounds); our guard returns 0 for a
window under one sample or a non-finite f (the original computes on);
the meaning of mfc #4434 on the dialog member `+0x110` when the
selection is 1 not resolved.

### L9.66 — Diagram pages: the integer scale and the painters' print branches  ·  🟡
Read (subagent report; the scale and font re-checked by hand at
`0x442012`…`0x4420b1`): a diagram page (`0x405820` → slot 0x16c =
`0x441fe0`) is painted first, into the whole margin rect R, then the
header and footer. Scale `s = ⌊LOGPIXELSY(printer) / LOGPIXELSY(screen)⌋`
(two GetDeviceCaps(90), integer division — 6 at 600 dpi, 1 at 150), font
Arial `⌊52·s/5⌋` (screen Tahoma 13). The preview draws into a memory DC
whose `m_bPrinting` is off. Painters (slot +0x188, (dc, rect, s, font,
doc)): **vector** `0x490360` — circles and radials solid (100,100,100),
outer (50,50,50), width ⌊s/3⌋; circle count `⌊R/50⌋` with R in device
pixels (so a page nearly always has 10); phasor pen s wide (PS_DOT for
non-V, which GDI draws solid when wider than 1); arrowhead min(18·s, L/2)
long, a fifth of that each side (screen too). **Circle / differential**
`0x43b570` / `0x445e00` — grid (150,150,150) ⌊s/2⌋, frame (50,50,50) and
axes black ⌊2s/3⌋; locus pen s; cursor "+" arms ±4·s (screen too) with
function-static pens, so 1 px on paper; on a real print (not the preview)
four white FillSolidRect(x, y, w, h) from the PAGE origin around the frame
inflated by ⌊2s/3⌋ + 1 — (0,0,R,T), (0,0,L,B), (R,0,L,B), (0,B,L+R,T) —
crop overflowing characteristics / loci before the legend. The cell is
`⌊96·s/5⌋` device px (≈ 19.2 points, ours keeps 19).
Ours: `printout::diagram_scale`, the page rendered at R/s points with s
pixels per point; `diagram_ui::with_print_scale` (thread-local s and, on a
real print, R's page offset) read by the painters: `font()` 0.8× in print,
`dev_pen` widths, the vector print pens / circle count / solid phasors, the
plane print pens and the masks; screen: arrowhead min(18, L/2) / L/5,
phasor pen 1 px, "+" arms 4. Test `diagram_pages_use_the_integer_scale`;
rendered the job (`FV_PRINT_DUMP`): 5 solid circles at 150 dpi, the
differential page's overflow cropped at the frame.
Residual: (the frequency diagram: L9.67); the cell's print size (≤ 1 %); unscaled print leftovers (angle
label offsets, the header box offsets, the 5 px top allowance, the 120 px
thresholds) and the vector header box itself (its fill (254,250,220)) not
modelled; the circle zoom reset under print updating the live window;
the differential's first-paint "push to quadrant I" during a print; the
GDI+ fill's offset band on paper.

### L9.67 — The frequency diagram as read: screen layout and print tiling  ·  ✅
Read (subagent reports, the dead grid pen re-checked by hand: its handle
is never pushed to SelectObject; only the axis pen and the per-bin bar
pens are used): the painter `0x46f5a0` in drawing order — white fill; B =
R.bottom − 20·s; `panelW = (W − 12s)/n`, `slot = panelW/(K+1)`; pass 1 per
channel: swatch (10·s, black pen s, channel brush) at
`c·panelW + 6s + (panelW − wName − 10s)/2`, `B + 3`, the name at +16·s, x
labels `k·f/P` in `[c·panelW + 12s + k(slot−1), +slot]` at `B − 18s` when
they fit; pass 2: v_k = `0x426510(k, pos, P)` k = 0..K, normalised to the
largest (bin 0 included; 0 → 1), the label norm bin P or rms(P);
`U = B − 30s − h − top` (≤ 0: nothing), `t = round½↑((1 − v/max)·U)`, bar
(slot, or 10·s centred) from `top + 12s + h + t` to `B − 18s`, GradientFill
colour→white then the outline; the value label 6·s above, horizontal when
⌊6w/5⌋ ≤ slot else rotated Tahoma 13·s at `barLeft − 2`,
`top + 6s + max(t + h, wr)`; then separators `9s + i·panelW` (top + 12s + h
to bottom − 6s) and the baseline `B − 18s − 1`. Print (`0x46f8b5`): rows
1..10, `cols = ⌈n/rows⌉`, cell areas `⌊(⌊W/c⌋)²·a⌋` / `⌊(⌊H/r⌋)²/a⌋` with
`a` = the screen pane's height / panel width, max of the minimum (ties
keep fewer rows), f = 1 or √(A1/A2); row-major from the top; inset e, the
screen slot / bar (unscaled 12 and 10) rescaled to `panelW − 2e`; U =
`H·f/rows − 30s − h`, offset `⌊(1 − f)·H/rows⌋`; names per row at the
row's bottom; x labels' rects inverted with several rows (none drawn);
n separators over the whole height; a baseline per row `rem` below the
bars when rows do not divide H.
Ours: `diagram::freq_geom`, `FreqTiling` (`screen_tiling`,
`print_tiling`, `cell`, `slot_left`, `label_left`, `row_bottom`, `u`,
`bar_offset`, `swatch_x`); `frequency_ui` rewritten in device pixels at
the print scale (it drew a dotted grid, bars normalised differently, its
own margins and slots). Tests `frequency_geometry`,
`frequency_print_tiling`; rendered the screen pane and a 3 × 2 printed page.
Residual: the font quirk (after a rotated label the next width test uses
the rotated font — same metrics on screen); separators past R and the x
labels' clipping on a real print (our page image is clipped to R); a hidden
pane's division by zero (ours: one row, slots from the page); GDI's
outline / gradient pixel rules approximated.

### L9.68 — Paste and an ОМП run do not mark the record modified  ·  ✅
Read (by hand): the document's SetModifiedFlag is vtable slot `+0x6c` of
CFastViewDoc (`0x4c8a20` → `0x45a540`: stores the argument at
`doc+0x48`; no direct call of it anywhere). The edit-command range handler
`0x45acb0` (57632–57644, switch on id − 57634: 57634 copy `0x45be28`,
57637 paste `0x45ad02`, 57643 / 57644 undo / redo `0x45befd` /
`0x45bf07`) contains no `+0x6c` call, and none of the functions it calls
directly reaches one (a direct-call walk to depth 5 over the listing,
functions split at call targets and int3 padding; the walk does find the
known marks of «Объединить» `0x47e8d6` and ОМП «Параметры» OK
`0x4543b6`). Likewise the ОМП run `0x4a00b0` and its toggle 32806
`0x449890` (clear `0x4a17a0`) reach none. So paste, copy, undo / redo
and an ОМП run or clear leave the modified flag as it was.
Ours: paste already left it (pending channels are outside the change
print), but the run's phase lanes «КЗ фазы А/B/C» (digital channels) were
in the per-frame change print, so a run marked the record modified — now
skipped in the print (they are still written on save). Test
`omp_ui::paste_and_a_run_leave_the_document_unmodified`.
Residual: virtual / indirect calls on the walk are not followed (none of
the calls in these handlers is a document-vtable call other than the
ones listed); whether the original's saved `.set` holds sections for the
phase lanes after a run (ours writes them) NOT DETERMINED.

### L9.69 — Diagrams in shown values (k); the vector header as read  ·  ✅
Read (subagent report; by hand: the header strings, the format choice
`0x491331`–`0x4913b6`, the `− 1` before the pow at `0x4910fc` /
`0x491141`): no diagram painter reads the display factor k (`ch+0xf0`);
it is already in what they read — the phasor cache filled by `0x428490`
(stored complex × k; the DFT `0x426fc0` / the corrected `0x4267e0` over
`0x426020`, which returns x·k) and every instantaneous read through
`0x426020` (circle R/X pairs, differential pairs, the frequency spectrum
`0x426510`). So a «P» record shown «Вторичные» is drawn in secondary
values on every diagram, like its readouts. Vector header
(`0x4910c3`–`0x4917fc`): per group (unit exactly "A" → I, the rest → U)
the largest |phasor| rounded with `p = 10^(⌊log10 max⌋ − 1)`,
`⌈⌈max/p⌉/5⌉·5p` (not > 0 → 0); text `"%.1f %sV"` / `"%.1f %sA"` when the
value is > 1 or 0, else `"%w.(w−2)f"` with `w = |⌊log10 v⌋| + 3`; the
prefix comes from `0x43d960` called with the unit "U" / "I", which it
never prefixes, so the header always reads plain V / A. Layout: a box
(254,250,220) under the window pen (screen (150,150,150) 1 px, print
s/2 (100,100,100)) at the top right, `off = max(0, W − L − Wv − m − 10)`,
m = 6·s, h = ⌊3·cy/2⌋ (cy = height of "Ay"), label column L = width
"U max" + 2m, value column Wv = the widest of "0.0000" and both texts +
2m; a vertical divider after L and a horizontal one at h; «U max» /
«I max» and the values at (+m, cy/4) in each cell, colour (20,20,20);
both rows always drawn ("0.0 V" for an empty group).
Ours had all three wrong: the diagrams read the stored phasors / samples
without k (a user «P» record: I max 1000 A over readouts of 354 mA),
`p` one decade too large (1234 → 5000, now 1500) and the header
«U max = 500 кV    I max = 1000 A» at the top left (prefixed, invented
text). Now `Doc::shown_k` multiplies the vector / circle phasors, the
R/X and differential samples and the frequency spectrum;
`views::nice_scale` / `vector_header_value`; `diagram_ui::vector_header`.
Tests `diagrams_carry_the_display_factor`, `nice_scales`; rendered the
user's «P» record (U max 60.0 V, I max 0.40 A).
Residual: the circle's placement under the header and the «Показывать
значения» table layout are still ours [I]; a zero scale with non-zero
lengths cannot occur (ours draws no length; the original divides 0/0);
the computed channels' own k (INFERRED 1).

### L9.70 — The unit of a formula channel: the dimension walk  ·  ✅
Read (subagent report; by hand: the two DIMENSION (id 0x44) tests in
`0x415bd0` around the walk call `0x415d10`, the literals `0x4c4d84`…
`0x4c4d90` «ном» / W / V / Hz and `0x4c3498` A, and that quick analysis
builds its formulas through `0x4129e0`, e.g. form 414's
`Mod:(Pf: (%s, %s))[VA]` reached from the page at `0x4889de`): the unit is
computed once per formula after the samples — the `[...]` where the
evaluation stopped, else the dimension walk (rules in
`CALC_CHANNELS_SPEC.md` «The unit of a formula channel»), else the unit it
carried. Quick analysis has no unit of its own.
Ours took the unit only from a trailing `[...]` (`addformula::unit_of`),
quick analysis passed its own (S0 = "VA" with a formula without `[VA]`),
so «Ua−Ub» had no unit (no SI prefix in the readouts, no vector group),
`Rms:(Ia)` none, `Frequency: ()` none. Now `expr::formula_unit` (tokens keep
the bracket text; `Walk` mirrors the levels; `unit_rule` is the table),
`Channels::unit` (`OscChannels`: canonical units of the record and of the
earlier computed channels), and the batch is `(name, formula)` — every unit
comes from the walk; S0's text is the builder's. Tests
`tests/formula_units.rs` (4: operators, brackets where the walk stops,
the function table, an earlier formula channel's unit).
Residual: the «В» → V / «А» → A display substitution is not split from the
canonical (ours keeps one string: an explicit `[В]` reads raw); the ADC-mode
"" of a channel atom (ours: the record's unit); the other quick-analysis
formulas are still our texts (spacing, quoting) rather than the builder's
— same values and units, different editor text; zero samples (step 1
position) NOT DETERMINED. **Frequency candidates (L9.65 residual, read):**
a formula channel is analog with empty ph / ccbm, so it is a candidate
exactly when its unit is one of the eight Hz spellings (`Frequency: ()`,
`[Гц]`, `fch*2`…); inserting one runs the rebuild, which turns a selection
of 0 into 1 — not modelled (our candidates are file channels only). Merge
sync under correction (read): method 1 «по аварийной составляющей» calls
the `fault:` dispatcher `0x427670`, which under the gate returns the
corrected signed value; method 2 reads the corrected phasor cache; pasted
channels use the base record's `rec+0x874` (not modelled; ours keep the
normal path).

### L9.71 — Pasted (bit 14) lanes drawn from their own samples; the digital base bar  ·  ✅
Read (subagent report; by hand: the analog icon `0x429501`–`0x429532`,
the analog count `0x42960f`–`0x429692`, the digital count and the 11/10
`0x42ab69`–`0x42abdf`, the whole digital run loop `0x42ac42`–`0x42ae32`
incl. its "on" test (value ≠ 0, NaN on, **past the values off** — the
report said on), the icon 1 draw `0x42ae4a`, the time getters `0x454b40` /
`0x428b40` and the base bar `0x463e9a` with `0x4653b0`): the lane loop
dispatches on the kind (`0x429380`: `+0x5c` 0 → analog `0x429490`, else
digital `0x42a0f0`), so a pasted channel keeps its kind and row height
(digital 20). For bit 14 the first drawn sample k is the first own sample
at or after the view's left edge (d = offset − (T(s) − T(0)) < 0 → first i
with ct(i) ≥ −d, else 0); x of a sample = its own time + offset placed in
the record's times by a forward scan, ((x − T(j))/(T(j+1) − T(j)) + j − s)
·25f rounded half up; times past an array's end extrapolate by
⌊(i − len + 1)·10⁶/rate⌉. Analog: samples k..min(N, ⌈W/(25f·R/r)⌉ + k),
one polyline, marks above 4 px per sample, icon 0 at (leftW + 2,
top + h − 21) under the trace. Digital: count = min(⌈W/(25f·R/r)⌉ + 5,
N − s) (unsigned: N < s no clamp), samples k..k + ⌊11·count/10⌋, no column
OR-ing; a run opens at an on sample (a lone last sample: x..x+4) and closes
at the next off sample's x or at the last visited one (+4 at the channel's
last sample); icon 1 at (leftW − 3, top + h − 19) on top. The digital base
bar of every digital lane (`0x463e9a`, the lane background pass) runs from
leftW to leftW + trunc(25·N·f·R/r) — the channel's count, **not** the
scroll position (zoomed in it spans the whole plot) — plus the paste
offset in px (R·offset·25f half up) for bit 14.
Ours drew a pending channel resampled onto the record grid as an analog
lane (a digital one as a 0/1 analog lane of y_analog height), no icons,
and the base bar to the last visible sample. Now `oscgeom::{time_us,
PastedTimes (first, xs, analog_end, digital_end, offset_px),
pasted_digital_runs, base_bar_len}`, `Doc::{pending_of, pending_digital}`,
`OscView::{pasted_analog, pasted_digital}`, `paste_icon` (our drawing of
image list 489's two marks), pending lanes' range from their own samples;
`FV_WINDOW_STATE=paste` renders a paste. Tests `pasted_times_and_positions`,
`pasted_digital_rectangles`, `pasted_lanes_keep_their_kind`; rendered.
Residual: T(0) — the original compares an offset time with absolute record
times (ours relative; equal for records starting at 0); the stats
readout of a pending lane (`0x461a24` with the offset) and the cursor
mapping `0x463e9d` not read (ours: the resampled series); the icon art is
ours; a subagent claimed the paste handler clears every channel's
selection after inserting, against L9.20's "pasted lanes selected" — not
re-checked.

### L9.72 — The vector diagram as read: circle, labels, arrows, legend / values table  ·  ✅
Read (subagent report; by hand: the four angle labels' strings and
DrawText flags — 0x120 for «0º», 0x128 (DT_BOTTOM) for «180º» — so 0° is
at the top; the reference phasor's atan2 at `0x491ec5`; the circle loop
over the channel vector with flag 0x4): paint order white, bottom band,
header (L9.69), circles, radials, labels, then per phasor its arrow and
its table row. Bottom band: the values table when «Показывать значения»
is on and (count + 6)·h < height — b = bottom − 2m − count·(m + cy),
columns name (max name + ⌊4cy/5⌋ + 5m) / magnitude / angle (each
width("-0.0000000") + 4m), centred (else from m), a (150,150,150) grid,
a swatch (cy/3 inset, black s pen) and the name per row h apart, the
magnitude `"%.2f %s%s"` (or `%w.(w−2)f` at or below 1) with `0x43d960`'s
prefix on the canonical unit, the angle `" %8.2fº"` / `" %8.2fπ"`;
otherwise a one-row legend at b = bottom − (m + cy) (squares ⌊4cy/5⌋ at
b + 2, names after m). Circle: centre x the middle; R = the smallest of
the circle sitting on b − m that clears the header box's lower-left
corner by m, W/2 − m and (b − top − 2m)/2; centre y min(Y − R,
(b − top)/2 + m + cy); n circles k·R/n (outer solid (150,150,150), inner
PS_DOT (200,200,200)); 12 radials every 30° in the inner pen, ends
rounded half up; labels inside the circle: «0º» at (cx + 2, cy − R)
top-left, «180º» bottom-anchored at (cx, cy + R − 2), «90º» at
(cx − R + 7, cy), «−90º» right-aligned at (cx + R − 2, cy) (radians: 0, π,
π/2, −π/2). A phasor: 0° up, positive angles counter-clockwise; length
R·|z|/scale rounded half up (R with «Только направление», 0 with a zero
scale); head hl = min(18·s, ⌊L/2⌋), half-width ⌊hl/5⌋, the four points
turned and rounded half up; shaft s wide (solid for V, PS_DOT else); the
head filled and outlined in the channel colour.
Ours put 0° to the right, the circle in the middle of a box under a 22 px
strip, no angle labels, no legend, the values as text lines at the top
left and an unoutlined head. Now `diagram::vector_layout` (pure, tested),
`views::vector_value_text`, `diagram_ui::vector_header_off`; the painter
rewritten in that order; `FV_WINDOW_STATE=vecvalues` shows the table.
Tests `vector_layout_clears_the_header`, `nice_scales` (value texts);
rendered the user's «P» record with and without the table.
Residual: nothing drawn without cursor 1 (`doc+0x280`) — ours always has
one; the magnitude format chooses w from the unscaled value (INFERRED);
the per-second «/с» unit suffix (`0x42bc50`) not modelled; GDI pixel
rules (PS_DOT pattern, Ellipse) approximated.

### L9.73 — Quick analysis builds its formulas through the «Добавить…» builder  ·  ✅
Read (subagent report; by hand: the templates `Mod:(GrsSumCur: (`,
`Ulf: (%s, %s)`, `TormSumCur: (%s, `, `I1: (%s, %s, %s)`, `Chan: (%s)`,
`Uniik: (%s, %s)`, `CoeffPriv: (%s, %s)` and `Mod:(Pf: (%s, %s))[VA]` in
the binary, the latter used by both `0x4129e0` sites): every page of the
sheet builds each channel with `0x4129e0` in a quick mode (helper byte set
for pages 536–539 / 543), reading its own controls and hidden statics the
page fills first — among them `0x48bff0(name)`, the name a channel of the
batch will be appended under. Texts are therefore `Fn: (a, b)`, channel
fields quoted unless a letter and alphanumerics, other fields as typed
(an empty one stays empty), 3Io / 3Uo / 3Ion «Не учитывать» (and 3Io
absent) → 0. Per page: 536 calc Ib → Ib 401 `Ib: (Ia, Ic, 0)`, I1 / I2
800 / 801 naming uniq(Ib), no 3I0; else I1, I2, 3I0 = 403 / 404 / 402;
U1, U2, 3U0 = 410 / 411 / 407 or U1, U2 = 408 / 409. 537 3I0 = 402 or 515
`Chan: (…)`, 3U0 = 407, 517 `Uniik: (…)` or 516 `Chan: (…)`, S0 = 518
`Mod:(Pf: (uniq 3I0, uniq 3U0))[VA]`. 538 Ib 401 when computed; phase
voltages Uab / Ubc / Uca = 449 / 511 / 512 (`Ulf: (Ua, Ub)`, `(Ub, Uc)`,
`(Uc, Ua)`), Z = `Mod:(Zl: (U, I1, I2, Iтр))` with U = uniq(Uab…) written
into non-channel fields (never quoted); line voltages Uca = 450 and Zab /
Zbc on the line channels. 543 3I0 = 402, Za–Zc = `Mod:(Zf: …)` /
`Mod:(Zk: …)` with uniq(3I0); no calc-Ib. 539 the reduction and group
methods are the page's radios as shown, applied to every arm; order:
every arm's reductions (501–504), then every arm's turned currents
(`Mod:(GrsPoslI?: (a, b, c, group, 0|1))` / `Mod:(GrsTriangleI?: (a, b,
c, group))`), then each phase's `Mod:(GrsSumCur: (24 slots))`, then each
phase's braking (523 `TormSubCur: (s1, s2, K)`, 524 `TormMaxCur`, 525
`TormSumCur: (K, 24 slots)`), slots = uniq(arm names), unused 0; one phase
is named «Ia».
Ours wrote `Fn:("a","b")` everywhere, referenced same-batch channels by
their plain names (with «Рассчитать Ib» on a record that has an «Ib», I1 /
I2 / Z used the **measured** Ib instead of the computed «Ib_2»), wrote
`GrsPosla:` / `GrsTrianglea:` — not functions of the language, so **group
compensation failed** — and `GrsSumCur` without `Mod:`, interleaved the
differential channels per arm, emitted Ib on the phase page and passed an
empty field as 0. Now `quick.rs` builds the builder's texts (`c`, `num`,
`call`, `uniq`) with the record's names passed in; the differential page
takes the shown arm's methods. Tests `tests/quick.rs` (6, exact texts,
uniq, group compensation evaluating).
Residual: our page UI still keeps the reduction / group radios per arm
(the original's are page-wide; we take the shown arm's — done L9.79); `0x48bff0`'s rule
taken as "free → as is, else _2, _3…" (VIEWS_SPEC), not re-read; missing
page controls' edge cases NOT DETERMINED.

### L9.74 — Units are shown in Russian («В», «мА», «Ом», «Гц»)  ·  ✅
Read by hand: the unit text every readout uses is `0x42bc50` (callers:
the per-channel readout `0x461ba7`, the group line `0x4695c5`, the vector
table `0x4926ca`, the status bar `0x49557b` …, the circle / differential
readouts): the canonical unit (`0x414c30`), returned as is when the
normaliser kept a raw unknown unit (`ch+0x155`, set at `0x423f7c`), else
`0x42b740(unit)` + the per-second suffix (`ch+0x154`). `0x42b740`:
Replace("DEG", «°» string 3105); when the language global `0x4ed17c` is 0
— it is never written (only compared), so always — Replace over 23
ordered pairs built once at `0x4ed280`: OHM→Ом, var→вар, VAr→ВАр, Ohm→Ом,
nom→ном, VA→ВА, Hz→Гц, mk→мк, A→А, V→В, W→Вт, F→Ф, H→Гн, m→м, n→н, K→К,
k→к, M→М, G→Г, s→с, S→См, «p. u.»→«o. e.», µ→мк (loop `0x42bbc1`–
`0x42bc0e`); then Replace("Ohm", «Ω» string 3546). The SI prefix comes
from `0x43d960` on the canonical unit (Cyrillic prefixes). So a readout
reads «57.92 В», «354.15 мА», «1.5 кВА», «50 Гц». The vector header is
not one of these (its format has a literal V / A, L9.69).
Ours printed the canonical Latin unit after a Cyrillic prefix («мA»,
«кV»). Now `units::ru` (the table) and `units::is_canonical`;
`views::si_scaled` shows recognised units in Russian, so every readout,
the status bar, the vector table and the plane readouts follow. Tests
`russian_display_units`, the views tests' expectations.
Residual: the per-second suffix («/с») is not kept on our channels;
computed channels' bracket units (e.g. `[kV]`) are not translated (the
original translates every unit without `+0x155`; ours only the canonical
ones). — Both done in L9.76.

### L9.75 — Circle / differential planes: labels, opaque text, whole-unit extents  ·  ✅
Read (subagent report; by hand: the formats `" R: %g %s%s, %g %s%s"`,
`" X: …"`, `" %g %s%s, %g %s%s"` at `0x4c68e8`…`0x4c6940`, `" %g %S%s"`
`0x4c76d4`, strings 161 « Iдиф: %g %s%s», 162 «ном», 163 « Iторм: %g
%s%s», «Ω» `0x4c69cc` and the translator above): the circle plane's edge
labels pass the Ω value to `0x43d960` with the unit «Ω» — never prefixed —
and the power value with "W" (prefixed); units "Ohm" / "W" / "var" through
`0x42b740` → «Ом» / «Вт» / «вар»; the differential plane's are « Iторм: %g
ном» left / right and « Iдиф: %g ном» top / bottom (never prefixed); the
scale texts likewise. Nothing sets a transparent background before them
(only the legends do, undone by RestoreDC), so labels paint opaque white
(INFERRED from the DC default). Characteristic extents come from integer
regions built at scale 1 (`0x4387d0`): whole units outward, a sub-unit
figure a 1×1 box. Confirmed as ours: the draw order, cell = LOGPIXELSX·s/5,
cells = half the smaller side / cell, pens grid (230,230,230) / frame
(200,200,200) / axes (150,150,150), label anchors, the scale box.
Ours printed «R: 1.5 кΩ, 300 W» (Ω prefixed, Latin units), bare numbers
on the differential plane, transparent labels and real-valued extents.
Now `diagram_ui::plane_ui` labels / scale text / opaque cells,
`diagram::region_extent` (test `characteristic_extent_in_whole_units`);
rendered both planes.
Residual: the extent's exclusive-edge +1 (taken as +0) and the Ω / power
split of characteristic extents (ours: one plane dimension per block);
the legend's unclamped start x and swatch pen; GDI's frame / scale-box
edge pixels.

### L9.76 — The unit text: «/с» kept, computed channels' units translated  ·  ✅
Read by hand: `0x42bc50` — when `ch+0x155` (unrecognised) is set the
canonical unit (`0x414c30`: `ch+0x7c`, "" in ADC mode `+0x60 & 0x1000`)
as is; else the suffix «/с» (`0x4c5eac`; "/s" `0x4c5ea4` only for a
non-zero language global) when `ch+0x154`, appended to the canonical unit,
the whole through `0x42b740`. The flags are set only by the file
normaliser (`0x422b10`, one caller: the `.cfg` channel line `0x422ad8`;
«/с» found → `+0x154` at `0x422e90`, unrecognised → `+0x155` with `+0x154`
cleared at `0x423f5f`/`0x423f7c`). A formula channel is made by
`0x42e940` → `0x4242e0`, which clears both (`0x424504`, a word store) and
stores the walk's unit (`0x425d70`) in `+0x78` and `+0x7c` alike, then
replaces an exact «В» / «А» (strings 3122 / 3106) by V / A in `+0x78`
only. So a computed channel's unit is always translated (`[kV]` → «кВ»,
no prefix: the prefix test `0x43d960` is on the Latin list), and a file
«кВ/с» reads «кВ/с».
Ours dropped «/с» and showed computed channels' non-canonical units as
typed. Now `units::Shown` (canonical unit + shown text; `file`,
`computed`), `AnalogChannel::per_second` / `shown_unit`, and every readout
formatter (`views::si_scaled`, `readout_text`, `stats_value_text`,
`group_stats_line`, `vector_value_text`) takes it — stats panel, status
bar, vector table, circle readout table. Tests `shown_unit_texts`, the
views readout test.
Residual: the ADC-mode stats text (ours «отсч.», the original's ""
through this path — the stats panel's own ADC branch not re-read); the
«Каналы» dialog's unit column not checked against `0x42bc50`.

### L9.77 — A pending lane's readout; paste deselects first  ·  ✅
Read by hand: the stats readout `0x461440` at `0x461a24` — for a bit-14
channel t = `0x454b40`(В1) − `doc+0x6d8`·10⁶; i from 0 while the own time
`0x428b40`(i) < t; j = i − 1; the value v(j) + (v(i) − v(j))·(t −
ct(j)) / (ct(i) − ct(j)) with v(m) = 0 for m past the values
(`0x461b13`–`0x461b5d`). `0x428b40` past its own times returns the last
time + (i − len + 1)·10⁶/rate **unrounded** (`0x428bb0`–`0x428bf6`; the
32-bit count wraps, so j = −1 is 2³² − len samples on), own rate `+0x108`
else `rec+0x228`; with no own times the record's `0x454b40`, which rounds.
So the readout is the first value (×(1 − ε)) before the paste — WRONG, the scan starts at own index 1, see L9.85 — falls to 0
within one own period after its last sample, then 0. Paste `0x45ad02`
clears `+0x100` of every channel (loop `0x45ae90`–`0x45aec1`, after the
insertion points are found, before inserting); a pasted channel is created
selected (`0x424e02` in `0x424cb0`); nothing in the handler (depth 3)
clears a selection afterwards — L9.20 stands, the L9.71 doubt was wrong.
`0x463e9d` (named "cursor mapping" in L9.71) is the digital base bar's
paste shift, done in L9.71.
Ours read the record-grid resampling, NaN (nothing shown) outside the
paste, and rounded the own extrapolation. Now
`oscgeom::PastedTimes::value_at`, `own_time_us` (the record's
`time_us` keeps rounding; no own times → the record's rate), and
`Doc::pasted_times`; the stats panel's value of a pending analog lane uses
it. Test `pasted_value_at_the_cursor`.
Residual: Max / Min / СКЗ / A1 of a pending lane (ours over the
resampled series) not read; T(0) as L9.71.

### L9.78 — A formula channel in Hz is a frequency candidate  ·  ✅
Read by hand: a formula channel is inserted into the channel vector at
index `rec+0x58` (the count after the analog channels, `0x42ea36`; the
counter incremented after, `0x42eabf`) and the candidates are rebuilt at
once (`0x42eaba` → `0x42ebd0`). Its ph / ccbm are empty, so its canonical
unit decides (L9.70: `Frequency: ()` → Hz, `[Гц]` as typed). So the list
is the file's candidates, then formula channels in creation order; adding
one to a record without candidates turns the correction on with it
(selection 0 → 1); the stored samples are read as the frequency.
Ours had file channels only. Now `Oscillogram::extra_freq` (name +
samples), kept by `Doc::channels_changed` from the formula channels with a
frequency unit; `freqcorr::{candidate_names, is_freq_unit}`, `of` reaches
past the file's candidates; the «Параметры файла» combo lists them. Test
`a_formula_channel_in_hz_is_a_frequency_candidate`.
Residual: pasted (bit 14) channels are inserted before the first selected
channel of their kind, so a pasted Hz channel's place in the list is not
modelled (ours: not a candidate); the formula frequency channel's own
readouts are not excluded from the correction (the original's gate skips
the frequency channel itself).

### L9.79 — Page 539: methods page-wide, values per arm  ·  ✅
Read (subagent report; by hand: the restore `0x482ca0` sets the caption
2086, the eight edits, the combos and the «Удалить I0» check from the
arm's byte `+0x5b0` (`0x482f31`) and never a radio (2088 is fetched and
dropped); the load forces 2082 «По мощности» with no key set
(`0x4846e5`); the generation `0x47ff60` reads radios 2080–2083 and
2087 / 2088 once): the page keeps 24 slots per arm field (`0x47fb40`:
channels `+0xd0/+0x130/+0x190`, edits `+0x1f0`…`+0x490`, the sequence
group `+0x4f0` (2127), the triangle group `+0x550` (2129), I0 bytes
`+0x5b0`), written by each control's change handler; the reduction /
group / braking radios and Kторм are page-wide. Generation: per arm its
own values, the group number of the chosen method, I0 from the arm's byte.
Load `0x4841c0`: options, counts, `0x4857a0` copies the controls' initial
state (edits «1.», groups «0», channels preselected «Ia» / «Ib» / «Ic» by
`0x411c70`) into all 24 slots, then each `Leg_%d` over its own slot (a
missing value keeps the initial one); the last arm ends up shown. Save
`0x484330`: the options from the radios, both group keys per leg. A
changed arm count (`0x482b60` → `0x482a40`) keeps every slot.
Ours kept the methods per arm (a newly shown arm silently switched every
arm to Kприв, no group compensation), one group number per arm (a «5»
carried into the triangle), dropped arms on a lower count, inherited the
previous leg's values on load, showed arm 1 and left new arms' channels
empty. Now `QuickForms::{dif_arms, dif_reduction, dif_group}`, 24
`arms`, `ArmForm::{seq_group, tri_group}`, `initial_arm`, `load_diff` /
`save_diff` as read, `quick_defaults` preselects the arm channels. Tests
`differential_page_legs_load_over_initial_arms`,
`differential_methods_are_page_wide`.
Residual: `0x411c70`'s match rule taken as the case-insensitive exact
name used for 3I0 (not re-read for «Ia»); a hand-edited `.set` with two
method keys set (two radios checked → two reductions, INFERRED) not
modelled; which «Группа:» combo shows under «Не учитывать» (`0x4831b0`
not re-read; ours the sequence one, greyed).

### L9.80 — The circle diagram's loci and readout table as read  ·  ✅
Read (subagent report on `0x43a880` / `0x43bbf0` with `0x43b570`,
`0x43ba80`; by hand: the static marker pens `0xc8` / `0xc80000`
(`0x43a8cc`, `0x43a90b`), the bounds raised to N and capped at
`0x431020` (the record's sample count) itself (`0x43a9ed`–`0x43aa41`,
count = end − start + 1), the NaN / past-cache test giving (0, 0)
(`0x43ab37`–`0x43ab96`), the phasor cache `+0x38` sized to the samples
(`0x4287c1`)): the loci run N..=len, so the default range ends with a
stroke back to the origin; a NaN never breaks the line; the cursor marks
use the raw cursor; R/X pairs (instantaneous, 0 past the values / NaN,
the Ω scale, the R channel's unit) get no marks and are drawn after the
marks; the scale test is on the canonical unit ("" in ADC mode → power).
Table: no header; name column = widest name + 30m + ⌊4cy/5⌋; two value
columns of "-0.0000000" + 24m (both reserved in mode 1); centred when it
fits, else 6m in; top at the plot's bottom; rows ⌊3cy/2⌋; swatch a
Rectangle (black m pen, colour fill) (⌊cy/3⌋, ⌊cy/3⌋)–(⌊cy/3⌋ + cy − 1,
⌊3cy/2⌋ − ⌊cy/3⌋); name at ⌊cy/2⌋ + cy, values col1 + 6m, text ⌊cy/4⌋
down; rules top / under each row / both edges / after the names /
(mode 2) between the values, pen (150,150,150) 1 px, print (100,100,100)
⌊m/2⌋; no rows → nothing; mode 1 |z| = 0 → «0.0+j0.0», a non-finite part
→ empty cells.
Ours stopped at len − 1 and broke the line at NaN, clamped the cursors,
marked R/X pairs, used pure red / blue, let the between-cursors start fall
under N, showed pairs in "Ohm", and sized the table to its content
without vertical rules. Now `circle_range`, `finite_or_origin`,
`Locus::markers`, the three-pass draw, `circle_cells` / `circle_table`;
`FV_WINDOW_STATE=circletable` renders it (rendered on DOC_1999). Tests
`circle_range_and_points_as_read`, `diagrams_carry_the_display_factor`
(the end point).
Residual: the differential plane's painter and table (not `0x43a880` /
`0x43bbf0`) keep ours; a computed channel without a unit stays on the Ω
scale (ours; the original's "" → power, INFERRED); cy is our font's row
height, not GetTextExtent("Ay"); the table's text colour / background
mode come from the caller's DC (not traced); NaN cursor marks are skipped
(not read).

### L9.81 — The differential plane as read; its pairs were stored swapped  ·  ✅
Read (subagent report on the pane painter `0x445e00`, loci `0x4457a0`,
table `0x446460`, reserve `0x446320`; by hand: the `.set` reader
`0x4211b6`–`0x42123f` — `BrakeCurr` into `+0`, `DiffCurr` into `+4` of
the 12-byte pairs at `doc+0x878`; the loci take x from the `+0` channel
and y from `+4`, both through `0x426020`, the pen in the `+4` channel's
colour (`0x445a55`–`0x445ac4`); the Каналы submenu `0x446d70` builds
«`+4` | `+0`»): the plane uses the circle's range (N..=len, bounds ≥ N,
capped at the count — the end point at the origin), 0 for a NaN or
past-the-values coordinate (the line never breaks), all polylines then
the marks (static (200,0,0) / (0,0,200), raw cursors), one scale. Table:
on / off (`+0x48a`), rows «Iдиф Iторм» (`+4` then `+0`, INFERRED from the
join order), the circle table's geometry and rules with both value
columns ruled, cells Iдиф then Iторм `"%.3f %s%s"` each in its own
channel's prefix and unit (`0x43d960`, `0x42bc50`), an exact 0 «0.0», a
non-finite value empties both cells; nothing with no rows.
Ours read `BrakeCurr` into the pair's first name, which we use as Iдиф —
a `.set` pair was drawn with its axes exchanged, coloured by Iторм, and
quick analysis's pairs were written back swapped; the table was one
"%.3f; %.3f" cell without units; the line broke at NaN; the range and
marks were the old ones. Now `setfile` swaps at the boundary (our
`ChanPair` stays in text order Iдиф, Iторм; `recset::PairRec` keeps the
file's), `plane_loci` shares `circle_range` and the 0-for-bad rule,
`Locus::pair_units`, `circle_cells` / `circle_table` for both planes,
`shown_by_name`; `FV_WINDOW_STATE=difftable` renders it (rendered on
BINARY). Test: the `.set` round trip asserts `BrakeCurr="Ib"
DiffCurr="Ia"` for (Iдиф Ia, Iторм Ib).
Residual: the legend `0x443b90` (label order, swatch) not read; where
the original's quick analysis puts its pair names NOT DETERMINED (ours
Iдиф = "I? диф."); text colour / background mode from the caller's DC.

### L9.82 — The plane legends as read  ·  ✅
Read (subagent report on `0x438210` (circle) and `0x443b90`
(differential), the same routine apart from the items; by hand: the pen
`CreatePen(PS_SOLID, m, 0)` (`0x4384a5`–`0x4384b6`), the start as a signed
halving with no clamp (`0x438432`–`0x43843a`), the text colour 0x323232
(`0x438447`), and the circle pairs' reader `0x420f80` — `ActiveResist`
into `+0`, `ReactiveResist` into `+4`, so the R/X pairs are not swapped
as the differential ones were): items are the flagged channels then the
enabled pairs with both channels found (circle: «R X» in R's colour;
differential: «Iдиф Iторм» in Iдиф's); the strip below the plot's bottom
b (raised by 20m by the caller); start = left + (W − total)/2 truncated,
never clamped; per item a Rectangle with a black m pen and the colour's
brush at (x, b + 3) — 3 device px — 10m square, the text at (x + 16m, b),
transparent, (50, 50, 50); x += text width + 22m.
Ours outlined the swatch in its own colour, clamped the start at the left
edge, drew the text 1 px low and scaled the 3 px gap in print. Now
`legend` as read; `FV_WINDOW_STATE=circlelegend` renders it (rendered on
DOC_1999).
Residual: the text alignment taken as the DC default (top-left).

### L9.83 — Formula channels on the circle diagram by their unit  ·  ✅
Read (L9.80 / L9.70 together): the circle pane lists, attaches and
«Показать все»s channels by the canonical unit "Ohm" / "VA" (`0x43d220`,
`0x436b30`, `0x437150`) and scales a locus by "Ohm" vs anything else
(`0x43a880`); a formula channel's canonical unit is its walked unit
(`0x425d70`), "" when the walk finds none — not listed, power scale.
Ours still treated a unitless computed channel as an Ohm one in the list,
the Каналы menu and the scale (from before L9.70, when our formula
channels had no units). Now the same unit rules for every channel. Test
`unitless_formula_channels_are_not_circle_channels`.


### L9.84 — Printing on the CUPS printer's paper  ·  🟡
Read (L9.22): the original lays its pages out on the printer DC's paper —
its size and printable area from GDI caps (our Windows path does the
same). Ours sent every CUPS job as A4. Now `spool::cups_paper` takes the
starred `PageSize` of `lpoptions -p <printer> -l` (A3, A4, A5, B5, Letter,
Legal; others and a missing `lpoptions` → A4) and the job is laid out on
that sheet, oriented by the page setup. Test `cups_paper_from_lpoptions`.
Residual: (CUPS hardware margins: L9.110); the PDF-file entry stays A4
(the preview and the page-setup paper: L9.94).


### L9.85 — A pending lane's statistics; the readout scan starts at own index 1  ·  ✅
Read (subagent report; by hand: `0x461a41` pushes 1 and sets the counter
to 1 before the first `0x428b40` call — so i ≥ 1 and j = i − 1 ≥ 0, and
L9.77's "j = −1 wraps" was a misread of the stack offsets; the СКЗ block
`0x4623d7`–`0x46244d` computes R(j) + (R(j) − R(i))·frac): for a bit-14
channel the value is v(j) + (v(i) − v(j))·frac, so before the paste the
line through own samples 0 and 1 extended backwards; СКЗ (`0x4272a0(idx,
1)`) and A1 (`0x426240(1, idx)`) are taken at own indices j and i and
combined with the **mirrored** sign; their window is round(own rate / 50)
own samples (50 fixed, `0x4d1718`; the record's P without an own rate),
samples past the values or NaN read 0 (`0x426020`), 0 before a full
window, A1 × `rec+0x278` uncorrected; under correction fs = the own rate,
f at the own index; Max / Min are the cached extremes over every finite
own sample of the display series (`0x428490`, filled by the paste
constructor `0x4254b9`), cursor-independent.
Ours started the scan at 0 (the first value before the paste), read СКЗ /
A1 / Max / Min from the record-grid resampling at the raw cursor with the
record's window, and left the harmonic-1 gain off computed lanes' A1. Now
`PastedTimes::{bracket, stat_at, own_period}`, `value_at` on `bracket`,
`lane_stats` for pending analog lanes; computed lanes' uncorrected A1 ×
`harmonic1_gain`. Test `pasted_value_at_the_cursor` (backward line,
mirrored statistic, own period).
Residual: the display mode of a pasted channel (ours none — whether the
UI can set one is NOT DETERMINED).

### L9.86 — Frequency candidates in vector order; pasted ones; no self-correction  ·  ✅
Read (subagent report; by hand as cited): `0x42aee0` has no bit-14 test,
so a pasted analog channel whose ph / ccbm (from the clipboard's desc2 /
desc3, `0x424f8b`–`0x424fe1`) is «system» or whose unit is a frequency
unit is a candidate; paste inserts before the first selected analog
channel, else at the end of the analog block (`0x45ad77`–`0x45ae64`),
and the insert `0x42e5c0` rebuilds the list at once; the list follows the
vector (`0x42ec10`–`0x42eca6`) and the selection stays a number — an
earlier insertion silently changes the frequency channel; the gate
compares the list entry's element address with the channel
(`0x4262b6`–`0x4262c6`, `0x4272ec`–`0x4272fc`), so a formula / pasted
frequency channel reads itself uncorrected.
Ours listed the file's candidates, then formula channels, no pasted ones,
and corrected a formula frequency channel by itself. Now
`comtrade::ExtraFreq { name, samples, pos, id }`, `freqcorr::{Cand,
ordered, selected_extra}`, `Doc::channels_changed` adds pasted analog
candidates at their insertion point, `Doc::computed_corr` (the stats
panel and the diagrams' computed phasors). Test
`candidates_in_vector_order`.
Residual: an implicit selection change (insert / paste moving the
candidate list) — ours recomputes everything, the original keeps stale
caches (read in L9.95); a pasted channel's ph / ccbm from the copy side
(desc2 / desc3) INFERRED.


### L9.87 — Page 539: each stored method key checks its radio; 1 phase shows the sequence combo  ·  ✅
Read (subagent report): the load `0x4844a0` calls CheckDlgButton for each
`legOption` key present (value ≠ 0) on its own, «По мощности» only when
none reads non-zero, and runs each checked radio's edit enabler in the
order power `0x483830`, nominal current `0x483970`, cast `0x4835b0`, TT
`0x4836f0` — the last one's edits stay enabled; the generation `0x47ff60`
tests 2080…2083 as four independent ifs (cast, TT, power, nominal), each
emitting its reduction lines for every arm. `0x483ab0`: with 1 phase the
sequence combo 2127 shows (greyed) whatever the radio; the triangle combo
2129 only with 3 phases and its radio. (`0x4831b0` only greys «<<» / «>>»
— as ours.)
Ours kept the first stored key and showed the triangle combo greyed in 1
phase with its radio stored. Now `QuickForms::{dif_also, dif_methods,
dif_edit_method}`, `quick::Arm::reductions` (a list, methods in the
generation's order), the load / save per key, a radio click clearing the
others. Test `two_stored_methods_both_reduce`.
Residual: ~~two methods give two «… прив.» lines per name — resolved by the
appender with «_2» (L9.95)~~ corrected in L9.167: the name list has one
«прив.» set per arm whatever the methods, the formulas one per checked
method, and `0x4167f0` pairs them by index — names slide (the tail past
the list NOT DETERMINED); ours names the extra lines «_2».


### L9.88 — Circle scales: characteristics split by an exact «Ohm»; no Ω locus, no characteristics  ·  ✅
Read (subagent reports; by hand: the dimension compare `0x438987` is
`#2948` = CString::Compare → MSVCR120 `wcscmp`, case-sensitive, against
string 3060 «Ohm»; the shipped FastView.fvb writes `dimension="OHM"` 33
times; the draw guard `0x43a564`–`0x43a5a0` (draw only if 800·m >
max(Ω, power characteristic maxima) × the passed ppu, a NaN skipping);
the pane passes the same Ω ppu to the reset `0x43b72f` and the painter
`0x43b7d9`): `0x4387d0` puts a characteristic's extent into the Ω maximum
only when its dimension text is exactly «Ohm», else into the power
maximum; `0x438b50` adds the loci (Ω / other by unit) and computes
upc = nice(max / (z·cells)) and ppu = z·cells·cell / (z·cells·upc) with
no guard — a zero extent gives ppu = +∞; the zoom resets to 1 when the
larger characteristic maximum × ppu_Ω > 800·m (every paint while it
holds); characteristics are always drawn at the Ω ppu. So with the
shipped library they are drawn only when an Ω locus (or R/X pair) sets a
scale they fit (< 800 px); with only VA / W loci or none they are not
drawn and the zoom snaps to 1; the «Вт» upc (labels, scale box) includes
the characteristic extent. Extents (`0x4389a2`–`0x438a44`): max |·| of the
integer box, no +1; polygons / polypoints round vertices to the nearest
unit (ties up), circles outward; an empty box (width **or** height 0) is
1×1 at the first vertex rounded up.
Ours put every characteristic into the Ω scale (so they set the Ω scale
themselves and were always drawn), rounded the extent box outward and
took an empty box only when both spans were under 1. Now
`Characteristic::dimension_text`, the split in `plane_ui` (circle only;
the differential plane keeps one maximum, `0x443f20`), `zoom_resets`,
`region_extent` to the nearest unit. Tests `characteristic_extent_in_whole_units`,
`circle_wheel_pans_and_zooms_without_egui_plot` (no Ω locus: no zoom).
**User-visible:** the circle diagram shows its characteristics only with
an impedance (Ohm) locus or R/X pair large enough to hold them — as the
original.
Residual: (per-figure extents: L9.99); loci at an infinite scale land at
INT_MIN in GDI — nothing visible, as ours skipping them (L9.99).


### L9.89 — Level lines: a body drag re-homes; unshown levels are kept  ·  🟡
Read (subagent report): placement `0x43eb40` needs a visible analog lane
(kind `+0x5c` == 0), so no level is placed on a digital lane — as ours;
the body drag `0x43efa0` (mode 4) reassigns the level's channel name to
the analog lane under the pointer (`0x43f46e`) and takes its value there
(`0x4716d0`), over a digital lane nothing; the `.set` reader appends every
`Level_%d` without a kind or existence check (`0x43ff8a`–`0x440004`) and
the saver writes it back (`0x470b20`); the paint `0x471020` finds the
channel by name (visible only), takes the fraction of the value in the
display window, snaps it to 1 / 0 at 0.5 for a digital channel, y = top +
h − 5 − (h − 10)·fraction; the hit test `0x470d20` and the info anchor
`0x471430` use the unsnapped fraction.
Ours kept the value when the body was dragged over another lane and
dropped levels on digital / unknown channels on load (lost on re-save).
Now `Comment::drag` part 4 re-homes; `Doc::kept_levels` holds the levels
we cannot show and `store_settings` writes them back. Tests `drag_and_lock`,
the `.set` round trip (a kept level saved back).
Residual: (kept levels drawn, hit and kept in place, areas kept: L9.96).


### L9.90 — Ribbon keytips as MFC shows them for this ribbon  ·  🟡
Read (subagent report on mfc120u; by hand in FastView: the application
button's text `"\nf"` at `0x4cacac` (pushed `0x472fb8`), the Help split
button's `"\na"` at `0x4cacb8` (`0x474a97`), strings 276 «Вставить» + `v`,
277 «Вырезать» + `x`, 278 «Копировать» + `c`): MFC assigns no keytip from
a label — only keys after a `\n` in the element's text (SetText
`0x10063ee1`), the QAT numbered 1…9 (RebuildKeys `0x1012bc08`), and in
popup menus the letter after `&`; the text is upper-cased in a box of the
larger of the keys and «O» plus 10 × 2. ~~Alt held 200 ms (timer 0xec1a)
shows the tab level; Alt released focuses the ribbon (showing them) or,
focused, hands the focus back~~ (MFC's, never reached: L9.155); F10 toggles (not with Shift / Ctrl); losing
the focus removes them; typing compares upper-cased Latin keys (A–Z give
the Latin letter whatever the layout, `0x100add31`) in list order — a
one-key match activates, a longer keytip's first letter narrows; Esc goes
back a level (out at the top), Space and an unmatched key leave. For this
ribbon: the tab level holds F (application button), 1 / 2 / 3 (Открыть,
Сохранить, Печать), A (Help); no tab has keys, so the category level
(Вставить «V», Копировать «C») is unreachable; the application menu's
rows take their `&` letters (Cyrillic — INFERRED; untypable) and Печать «P»
/ «W».
Ours had none. Now `keytips.rs` (`Keytips`, `Level`, `Target`, `tips`,
the Alt / F10 / Esc / typing state machine), `App::keytips_input` (before
the shortcuts; the keyboard is the keytips' while shown),
`keytips_paint`, the application menu opened by «F» through egui's bar
state (`kt_menu`; a root shows for its button's response id).
`FV_WINDOW_STATE=keytips` / `keytipsmenu` render them. Tests
`alt_tap_hold_and_f10`, `typing_activates_descends_and_backs_out`,
`keytips_open_the_application_menu`.
Residual: (placement and the box: L9.97; Alt+key held, the Recent
Documents' keys, «A»: L9.115).


### L9.91 — Ribbon panels reduce in MFC's steps  ·  🟡
Read (subagent report on mfc120u): `CMFCRibbonCategory::RecalcLayout`
(`0x10106b37`) resets every panel to its first layout when the width
changes; when even the sum of 2 + (collapsed width + 2) does not fit, all
panels collapse at once; otherwise, while the panels overflow, the panel
with the most steps left takes one step (ties: the rightmost); FastView
sets no collapse order; still overflowing, scroll arrows (13 px wide, 50 px a click — L9.100). A panel's
layouts (`RecalcWidths` `0x10125189`, `Repos` `0x101227d5`): galleries
shrink; from the right the first run of min(3, n) large elements goes to
small-with-text; three small-with-text in one column with a small image go
icon-only; a large element joins a short small column to its right; every
remaining small-with-text goes icon-only; last, the collapsed button — no
collapse step when the first width is at most the collapsed button's, or
1.5 × it with a single element. Applied to «Главная» (INFERRED from the
elements, widths ours): Объединение full → Вставить + three icons →
collapsed; Диаграммы four large → Частотная + three small → three icons →
collapsed; Видимость full → collapsed; Вычисления full → its right small
column icon-only → all icon-only → collapsed; " " (one button) usually no
collapse.
Ours collapsed whole panels from the right at a fixed 82 px. Now
`ribbon_group` takes its layout count and draws the step, an invisible
measuring pass (once per tab) records every layout's width,
`reduction_plan` is the category rule, `small_button_mode` the icon-only
small button, the collapsed button sized to its title. Tests
`panels_reduce_by_most_steps_left_rightmost_first`,
`narrow_ribbon_collapses_panels_and_keeps_its_height` (the order).
Residual: (the hand-applied steps checked against the generic Repos, the
small sizes and the fit test: L9.100); the collapsed button's width is
ours (title + 16, ≥ 48); no scroll arrows; the «ОМП» / preview panels
have one layout.

(L9.64 residual resolved by the same report: FastView uses the stock
`CMDIChildWndEx`, whose slot `+0x1f0` returns FALSE (`0x1005eafa`) and
`+0x1ec` TRUE (`0x1005eb63`) — every window listed and savable, as ours.
The arrangement buttons: Каскадом / Горизонтально / Вертикально minimise
every unselected child, restore the selected ones and send WM_MDICASCADE /
WM_MDITILE (MDITILE_HORIZONTAL / VERTICAL); Свернуть minimises the
selected ones; under MDI tabs their visible effect is NOT DETERMINED, ours
still does nothing.)


### L9.92 — Diagram panes float, re-dock and auto-hide  ·  🟡
Read (subagent report on mfc120u and FastView; by hand: the four
`CreateEx` calls at `0x474e81` / `0x474ef8` / `0x474f7b` / `0x474fea`
push pane style 0xF (float, close, resize, auto-hide), tabs 0x20, window
styles 0x56004140 (right) / 0x56008240 (Частотная, bottom), ids −2 / −3 /
−5 / −4): a caption double-click (`0x100886dd`) floats a docked pane — a
grouped one with its whole group — at its recent floating rect
(`RectRecentFloat`, first the create rect, 500 × 500 / 200 × 200, at the
frame client's top-left) and redocks a floating one; a caption drag
detaches past SM_CXDRAG (4 px) once the pointer has left the pane
(`0x10086f3a`), the mini-frame following the pointer, smart docking
(DT_SMART) offering the edges and the panes' sides; the pin toggles
auto-hide (`0x100871c7`): a strip button per pane on its edge, a 400 ms
hover (timer 0xec00) or a click slides it out in 12 steps (`0x10088459`),
a 700 ms check (`0x10087a4a`) slides it back when the pointer is outside
it and its button (not with a menu up); close hides (the ribbon toggle
shows it where it was); the Feature Pack saves the layout (Workspace).
Ours had one fixed column. Now `docking.rs` (`Unit` group / Частотная,
`Place` docked / floating / auto-hide, `Dockable` with the recent float
rect, `double_click`, `drag_detach`, `pin`, `auto_hide_frame`),
`views.rs` (`diagram_panes` with the strips, the column, floating
windows, slid-out areas and the drag back onto the unit's home marker;
captions with pin and close; the double-click by the 500 ms system
time). `FV_WINDOW_STATE=floating` / `autohide`. Tests
`double_click_floats_and_redocks`,
`a_drag_detaches_past_the_threshold_outside_the_pane`,
`auto_hide_hover_slide_and_check`,
`diagram_panes_float_redock_and_auto_hide`.
Residual: (smart-docking drops on every edge, beside and into the other
unit: L9.101; a tab dragged out: L9.102); Частотная stays in the right column (the
original's create docks it at the frame's bottom — ours follows the
user's screenshot, L9.3); the layout saved between runs: L9.98; Ctrl+pin
and the animation-off switch not modelled.

### L9.93 — The vector legend starts in its window when it does not fit  ·  ✅
Read by hand: `0x490d2f`–`0x490d4f` centres the one-row legend by a
truncating halving only while the window is strictly wider than the row
(each item text width + 2m + the square), else starts it at m = 6·s
(`[esp+0x3c]`, set at `0x49062c`) in the window's own coordinates.
Ours took that m as an absolute screen x, so on the user's BINARY record
(long names) the row began under the lanes and only «…е фазы С» showed
in the pane; equal widths centred. Now `diagram::vector_legend_x`. Test
`vector_legend_starts_m_from_the_window_when_it_does_not_fit`; rendered
the BINARY record. (Found by the real-records render pass.)
Residual: in print the original's m is in the page DC's coordinates
(whether the printed cell has its own origin NOT DETERMINED; ours m from
the cell).

### L9.94 — Page setup and preview on the application printer's paper  ·  🟡
Read by hand: «Параметры страницы» `0x448a90` builds a CPageSetupDialog
(#886) with flags 0x260a — margins, in 0.01 mm, paper greyed, the
RETURNDEFAULT pass first, then the paint hook `0x4488a0` — so its first
DoModal (#4062) fetches the default printer's DEVMODE, writes the stored
orientation into it (`dmFields |= DM_ORIENTATION`, `+0x4c`), shows the
dialog with the stored margins; on OK it keeps orientation and margins
and hands the DEVNAMES / DEVMODE to the application (#12964, INFERRED to
be `CWinApp::SelectPrinter`); the greyed «Размер» is that printer's
form. MFC's preview renders on the application printer's DC (INFERRED
from the stock `AFXPrintPreview`, L9.22).
Ours showed «A4» and laid the preview out on A4 whatever the printer.
Now `spool::{Paper, paper}` (CUPS: the starred `PageSize` choice;
Windows: the DEVMODE form name and the DC's PHYSICALWIDTH / HEIGHT),
`PageSetup::oriented`; the preview and the page-setup sample / «Размер»
use the application printer's paper (the last «Печать» choice, else the
default; A4 for none or the PDF entry). Test `cups_paper_from_lpoptions`
(names); rendered the page setup; the Windows path run on the VM
(ignored `system_printers_paper`: Microsoft Print to PDF → «A4», 210 × 297).
Residual: the preview's printable area (hardware margins) is the whole
sheet; the dialog's OK does not write our orientation into the printer's
own defaults (ours keeps it in the page setup); the standard dialog's
«Принтер...» button not modelled.

### L9.95 — Appended names: «_2» ignoring case; what a frequency change refreshes  ·  🟡
Read (subagent report; by hand: the format `"_%d"` at `0x4d040c`;
`0x48bed0` walks the channel vector `doc+0x184` (stride 0x160) and then
the definition names `doc+0x194`, comparing with #2954 = `_wcsicmp`
(mfc120u `0x1005f756`; #2948 Compare is `wcscmp`); InitInstance sets
`Russian_Russia.1251` at `0x4484e5`): a new formula channel's name is
free unless it equals any record channel's (analog or digital) or any
formula definition's name ignoring case (Cyrillic folds under that
locale — CRT INFERRED); else the first free «name_2», «name_3», …. The
command `0x4167f0` uniq's the vector / circle flag names and the pairs
against the state before the batch (`0x41699d`, `0x416baa`, `0x416bd3`)
and then each generated line in order, each name joining the list before
the next (`0x416c9d`–`0x416ead`): page 539's second «Ia плечо 1 прив.»
becomes «Ia плечо 1 прив._2»; no skip, no message; every later reference
is the first line's. `0x415780` then re-evaluates the whole definition
list.
Ours compared case-sensitively, did not count digital channels, and
mapped a repeated generated name to its last line. Now
`quick::taken_names`, `unique_name` ignoring case, `apply_quick` maps a
repeated name to its first line. Test `unique_names` (within a batch,
case, a digital name).
Also read (same report): the frequency-channel choice in «Параметры
файла» (`0x44a2c9`) ends in `0x42e540`, which refreshes every channel's
cache (`0x428490`) and re-evaluates every formula — as ours. Its only two
callers are that dialog's; insert / paste (`0x42e5c0` → the candidate
rebuild `0x42ebd0`) refresh nothing else, so when they move the selection
the stats panel's СКЗ / A1 (`0x4272a0`, `0x426240` read `rec+0x874`
live) and the frequency diagram show the new correction at once, while
the vector phasors, RMS / harmonic lane series and formula channels'
samples keep the old one until the dialog, a «Рассчитать» (formulas
only) or a flag toggle (that channel only) refreshes them.
Residual: (the staleness after an implicit change, the live statistics
and «Рассчитать» re-evaluating every formula: L9.103).

### L9.96 — Levels on digital lanes; unshown items kept in place; y truncated whole  ·  ✅
Read (subagent report; by hand: `0x471210`–`0x471232` truncates
(top + h − 5) − (h − 10)·f as one double; `0x47111e`–`0x47112e` snaps f to
(t > 0.5) for kind 1; the t = −1 sentinel skips the paint): `0x42b640`
returns the channel's cached window (`ch+0xb0` / `+0xa8`; the zoom branch
needs an analog channel), and for a digital channel `0x428490` fills it
from the raw samples — both ends the first sample, a later 0 sets the low
end to 0, a later 1 the high end to 1 — so a mixed channel's window is
[0, 1], a constant one [c, c]. The paint finds the level's channel by
name among all visible channels; lo ≤ v ≤ hi or nothing; y =
trunc(top + h − 5 − (h − 10)·f) with the digital snap (0.5 → 0); the hit
test `0x470d20` and the info anchor `0x471430` use the unsnapped t (a 0.3
level on a 0 / 1 lane is drawn at the bottom but grabbed at 30 %); a
constant digital channel with v at its value gives t = NaN — drawn at the
bottom, never hit. The `.set` reader `0x43fcf0` appends every Area, Level
and Moment unconditionally (an area's channels are kept as names), so
items on channels the record lacks stay in their list place and are
saved back.
Ours truncated only t·(h − 10) (1 px low whenever it was fractional, on
every lane), left levels on digital lanes undrawn and unhit in a side
list written after the others, and dropped areas naming an unknown
channel. Now `CommentKind::Level { chan: ChanKey }`, `CommentKind::Kept`
(an unshown `.set` item in its place), `oscgeom::{LevelY, level_y,
digital_window}`, `lanes::level_place` (screen and print), `Doc::kept_levels`
removed. Tests `level_lines_stay_in_their_lane`,
`digital_windows_from_raw_samples`, the `.set` round trip (a kept level and
area in place); `FV_WINDOW_STATE=comments` now also shows a level on the
first digital lane (rendered: drawn at the lane's bottom).
Residual: (levels placed / re-homed on formula channels: L9.104); whether `0x4040c0` draws an
area on an unknown channel NOT DETERMINED (ours: not drawn).

### L9.97 — Keytips placed per element; the Office 2007 box; menu rows with large images  ·  🟡
Read (subagent report on mfc120u; by hand: `CMFCRibbonButton::GetKeyTipRect`
`0x101033e0` — the element rect at `+0x74`, left = R.left + 10 or, for
the menu part's key, R.right − w/2; top = R.bottom − h/2 before the
per-kind cases; FastView's application-menu buttons are built with small
and large image index n (`0x473079`: Открыть 1 / 1, Сохранить 2 / 2, …);
the four styles' `RIBBON_KEYTIP_BACK` sampled): the application button's
box at its left + 10, centred on its middle; a QAT button's centred
across it with its top at the button's middle; a tab-row button (Help) at
left + 10, centred on its bottom edge; the main panel's items — large
image, text always on the right (`0x1011cf6f`) — end their box 4 px right
of the 32 px image and 4 px above the row's bottom, the menu part's key
centred on the row's right edge; the keytip window is moved into the work
area (`0x1011bbcb`). The Office 2007 manager (`0x10194ae0`) draws a 15 × 15
bitmap stretched with 2 px corners: a 1 px border (118, 118, 118; Aqua
134) with clipped corners round a vertical gradient — Blue (251, 252,
253) → (208, 222, 241), Black / Silver (253, 253, 254) → (231, 231, 242),
Aqua (254, 254, 254) → (208, 214, 226) — the text the style's tooltip ink
(76, 76, 76), a disabled element's window at alpha 128 (`0x10182927`); the
other looks the tooltip colours (COLOR_INFOBK, COLOR_INFOTEXT border).
Ours placed every button's box at its bottom centre, menu rows' at their
left middle, in a tooltip-like box; our menu rows had no image column.
Now `keytips::{tip_origin, into_screen, office2007_box}`, `keytips_paint`
per look, and the application menu's rows (`main_entry`) a 36 px row with
a 32 px image (our drawings where we have one) and the text on its right
— the column kept tight so it fits the 500 px minimum window. Test
`keytip_boxes_sit_per_element_type`; rendered `keytips` / `keytipsmenu`
and the menu at 866 × 500.
Residual: the main panel's row height and image margins (ours 36 / 2 px)
not read; the disabled text's blend colour (ours: half-way to white)
NOT DETERMINED; tab and collapsed-panel keytip rules not needed (no
level reaches them); the original's 32 px images are its own (ours drawn,
four rows without one).

### L9.98 — The workspace: pane layout and window placement saved on exit  ·  🟡
Read (subagent report on mfc120u and FastView; by hand: `SaveState`
`0x101a9425` returns at once unless `+0xfc`, which the CWinAppEx
constructor sets to 1 (`0x101a8a64`); nothing in FastView writes the app
object's `+0xfc` (`0x4ed074`)): the frame's load (`CFrameImpl::OnLoadFrame`
`0x100a0a80` → `LoadState` `0x101a8eea`) restores the window placement
and, when the saved version keys are there, the docking state; its close
(`0x100a079f` → `OnClosingMainFrame` `0x101a9717` → `SaveState`) writes
every pane through the docking manager (`DockingPaneAndPaneDividers`,
`IsVisible`, `RectRecentFloat`, `RectRecentDocked`, `IsFloating`, …), the
`WindowPlacement` and FastView's `SaveCustomState` (`0x448da0`) under
HKCU\Software\НТЦ Механотроника\Механотроника FastView\Workspace;
documents and MDI tab groups are not saved (no SaveMDIState import).
FastView's frame (`0x4728d0`): `SetDockingMode(DT_SMART)` (immediate),
`EnableAutoHidePanes(0xF000)` (all four edges dockable), each pane
`EnableDocking(0xF000)`.
Ours saved no pane state and no window placement. Now
`docking::{store, restore, parse_rect}` (`Workspace/<unit>/Place`,
`RectRecentFloat`), `App::{save_workspace, restore_workspace}` (each
pane's `IsVisible`, the group's shown tab, `WindowPlacement` "l t r b
maximised" — the normal rect kept while maximised), saved when the window
close is accepted (`handle_exit_request`), the placement applied by
`main` before the window is built. Test `workspace_round_trip`.
Residual: docked sizes (`RectRecentDocked`, MRUWidth — ours has a fixed
column) and the docked order are not saved; the original keeps them in
the registry, ours in our settings file (`persist`); a floating rect is
taken as saved even if the screen changed (egui keeps windows on the
screen).

### L9.99 — Characteristic extents per figure, folded as integer regions  ·  ✅
Read (subagent report; the L9.88 / L9.75 reads by hand): `0x4387d0` makes,
per visible characteristic and per mode, one integer HRGN per figure at
scale 1, origin 0 (x → R(x), y → −R(y), R nearest with ties up), folds
them with CombineRgn by each figure's op like the painter, and takes
max |·| of `GetRgnBox`. Polygon `0x409c00` / polypoint `0x40ae40`: the
rounded vertices; circle `0x40a170`: Ellipse(⌊cx − r⌋, ⌊−(cy + r)⌋,
⌈cx + r⌉, ⌈r − cy⌉) — outward; ellipse `0x40a5d0`: the axis-aligned
Ellipse(⌊−a/2⌋, ⌊−b/2⌋, ⌈a/2⌉, ⌈b/2⌉) path turned, rounded and moved to
(R(cx), R(−cy)). A figure whose region is NULL or whose box has no width
or no height becomes a 1 × 1 CreateRectRgn that takes part in the fold:
a polygon's at its first point (⌈x⌉, −⌈y⌉) — vertex 0 is the last line ∩
the first — or (0, 0) with none; a circle's at (⌈cx⌉, ⌈cy⌉), an ellipse's
at (R(cx), R(cy)), neither scaled nor flipped. Loci at an infinite scale
(`0x43a880` has no guard): every vertex becomes INT_MIN — nothing visible.
Ours rounded the combined world box of both modes with the polygon rule
(circles could come out a unit short, r 2.3 → 2 instead of 3), applied
the fallback to that box at its start, and never let an AND / DIFF fold
shrink the box. Now `geom::integer_extent` (per figure, folded with the
same `combine`, cached in `CharState::extent`); `diagram::region_extent`
removed. Test `characteristic_extent_in_whole_units` (fvb-core: polygon,
outward circle, sub-unit and zero-height fallbacks, an emptying DIFF, an
ellipse).
Residual: whether GDI's box of an Ellipse path includes its right /
bottom edge NOT DETERMINED (ours: the inscribed polygon's box); our
ellipse ring is sampled (180 points) where GDI flattens the Bézier path.

### L9.100 — Ribbon: Repos generically checked; small button sizes; the fit test  ·  🟡
Read (subagent report on mfc120u; by hand the keytip rect start `0x101033e0`
only): FastView adds no button groups (ordinals 669 / 670 / 1259 not
imported), so every panel keeps align-by-column. An element starts large
when it has a large image, else small with text when it has text
(`0x10062cd9`); a step goes large → small-with-text → icon-only (needs a
small image, `0x10062bb0`); always-large or no-small-image elements never
compact. Repos (`0x101227d5`, margins 4 / 2, columns of height H = panel −
caption − 2) applies, until the flow fits, the first of: a gallery
shrinks; right to left the first run of min(3, N) large compactable
elements goes small-with-text; right to left the first column of three
small-with-text goes icon-only; from index 1 (N ≥ 3) the first large
element whose right neighbour is small-with-text in a short column goes
small; every small-with-text from that index goes icon-only; else the
panel collapses. RecalcWidths (`0x10125189`) tries widths 16 px apart,
recording each narrower state, and stops at the collapsed width (1.5 × it
with one element), then appends the collapsed button. Sizes: icon-only
(image + 6)² = 22 × 22 (`0x10101d6a`); small with text text + image + 10
wide, max(16, image, text) + 6 tall (`0x10101f92`). The category
(`0x10106fcb`): panels from left + 4, each its width + 8, 2 px apart; they
fit while Σ(width + 8 + 2) ≤ the category − 8; all collapse when the
category − 8 ≤ 2 + Σ(collapsed + 2); only then 13 px scroll arrows
(13 × DPI, not 50 — 50 px is one click's scroll, auto-repeating) at the
ends.
The rules run by hand over «Главная»'s elements give exactly L9.91's
sequences (Объединение full → icons → collapsed; Диаграммы three to small
→ icons → collapsed; Видимость full → collapsed; Вычисления the right
column → all icons → collapsed; « » no collapse), so the hand-coded steps
stay. Ours sized small buttons 19 px tall, at least 90 px wide with text
and icon-only 24 px, and tested Σ(w + 2) + 2 against the whole width. Now
22 × 22 icon-only, text + 26 × 22 with text, the fit and collapse tests
as read. Test `panels_reduce_by_most_steps_left_rightmost_first`;
rendered at 1320 / 1000 / 866 px.
Residual: large-button and collapsed-button sizes not read; the scroll
arrows never show at our 866 px minimum (all-collapsed «Главная» is far
narrower) — not modelled; the 16 px skip of near-equal states not
modelled (our measured states differ by more).

### L9.101 — Smart docking: edge markers, the five-way group, drops beside and into a pane  ·  🟡
Read (two subagent reports on mfc120u; by hand: SaveState's gate only):
FastView's frame sets DT_SMART (`0x1008fad2`, immediate) and the VS2008
look (Office 2007 manager, `0x101950e0`); dragging a floating pane shows
four edge markers 16 px in from the frame client's edges, centred along
them (36 × 35 / 35 × 36, `0x101347e0`), and over a docked pane — or the
document area — a 110 × 110 five-way group centred on it (`0x101350f5`,
`0x1013598a`; no centre over the documents). The first marker hit (centre
first) is hot (`0x10136b50`); only then the preview shows and the floating
window stops following the pointer. Preview (`CalcExpectedDockedRect`
`0x1008c45d`): an edge strip as thick as the floating window (outer: the
frame's dock area, inner: next to the documents); beside a pane
min(the floating size, half the pane) (`0x100d35e8`); into a pane the pane
less a bottom tab strip with a 40 px tab 10 px in (`0x100962d3`). Drop
(`0x100d6794` → `0x1008a27f`): an edge docks the pane at its floating size
capped at half the area (`0x10088f61`, `AdjustRectToClientArea`
`0x1008c750`, 50 %); a side arrow splits the target (`DockToWindow`
`0x100891c3`, the same share, a 4 px divider); the centre attaches it as
the active tab (`AttachToTabWnd` `0x1008a390`); no hot marker — it stays
floating where it was. FastView's creates: the right panes 500 wide,
Частотная 200 tall (on the bottom edge in the original).
Ours docked only back at a unit's home marker. Now `docking::{Edge, Spot,
Drop, edge_markers, five_way, hit_marker, drop_on, dock_extent, leave}`
(a unit beside another takes its container — keeping its size — when that
one leaves, which keeps the mirrored place for a redock; a tabbed-in unit
goes along), `Dockable::{spot, extent, gen}` (saved in the workspace:
`Spot`, `RecentDockedSize`), and `views.rs`: containers on any edge
(edge-marker docks outermost), the beside split, the combined tab group
(Частотная as a tab, in front after its drop), auto-hide strips and
slide-outs on the unit's edge, the markers / preview during a caption
drag of a docked (after detaching) or floating unit. `FV_WINDOW_STATE=
dockleft|dockbottom|docktab|dockmarkers`. Tests
`smart_dock_markers_and_drops`, `smart_docking_drops_on_the_markers`
(Частотная into the group's tabs, the group with it to the left edge),
`workspace_round_trip`; rendered all four.
Residual: the marker bitmaps are ours (drawn boxes and arrows, MFC's
17117…17127 are its own); (a tab dragged out: L9.102; reordered: L9.105); the dock rows of several panes on one edge
are separate containers; (Ctrl held: L9.128);
Частотная's default stays below the group, and Дифференциальная in the
group's tabs (the user's screenshot, L9.3 — the original's creates dock
Частотная on the bottom edge and Дифференциальная on the inner side of
the Векторная | Круговая group, `0x474e10`; the screenshot is likely a
saved workspace).

### L9.102 — A tab dragged out of a tabbed group floats  ·  🟡
Read (subagent report on mfc120u): a press on a tab arms detach mode and
captures the mouse with the grab offset (`0x10068d0b`); moving inside the
strip over another tab swaps them (reordering, `0x100690b4`); moving out
of the strip calls DetachTab (`0x10068657`), which needs SM_CXDRAG /
SM_CYDRAG (4 px) from the press; `CTabbedPane::FloatTab` (`0x1013a259`,
more than one tab only) removes the tab and floats that pane at its
recent floating rect, the drag going on as a mini-frame drag.
Ours could not take a tab out. Now the combined group's strip
(`group_body`): pulling the Частотная tab out past 4 px floats Частотная
under the pointer (keeping its tab place for a redock) and the smart-
docking drag goes on; pulling the host's own tab out hands the container
to the other unit. `App::pane_tab_rects`. Test
`smart_docking_drops_on_the_markers` (the tab pulled out, released over
nothing: floating, the group still docked).
Residual: our group's three panes are one unit — a group tab pulled out
of a group without Частотная does nothing, and one pulled out of a
combined group takes the whole group; (tab reordering: L9.105); the
float lands under the pointer rather than at the recent floating rect.

### L9.103 — A frequency change by insert / paste refreshes no cache; statistics live  ·  ✅
Read (L9.95's report; by hand: `0x42e540` is called only at `0x44a25d` /
`0x44a30d` in «Параметры файла»; none of the six callers of the
candidate rebuild `0x42ebd0` — insert `0x42e5c0`, `0x42e810`, `0x42e940`,
`0x42ead0`, `0x42ecf0`, `0x450e70` — refreshes the channel caches; only
`0x42e810` / `0x42e940` update their own channel): when an inserted or
pasted channel moves the frequency selection, the statistics panel's
СКЗ / A1 (`0x4272a0`, `0x426240`, reading `rec+0x874` when painted) and
the frequency diagram take the new correction at once; the vector
phasors, RMS / harmonic lane series and formula channels keep the old
one until the dialog's full recompute; a «Рассчитать» re-evaluates the
whole formula list (`0x415780`).
Ours recomputed everything whenever the rebuild changed the selection,
read the statistics from the caches and evaluated only the new formula
lines. Now `channels_changed` only rebuilds the candidates;
`Doc::{stat_rms, stat_a1}` (live, current correction) feed the
statistics panel; `Doc::reevaluate_formulas` runs before each appended
batch (and in the full recompute). Tests
`frequency_correction_on_load_off_and_back` (stale phasors, live
statistics after the rebuild; fresh after the recompute),
`a_stale_formula_channel_is_refreshed_by_the_next_calculation`.
Residual: the frequency diagram's own cache behaviour beyond its live
spectrum not re-read; the per-channel flag commands that refresh one
channel's cache (`0x428490`'s other callers) follow our full per-channel
recompute (INFERRED equivalent).

### L9.104 — Level lines on formula and pasted analog lanes  ·  ✅
Read (L9.89's report): the placement `0x43eb40` and the body drag's re-home
(`0x43f46e`) test the lane's channel kind (`ch+0x5c` == 0, analog), not
whether it came from the file — formula channels and pasted analog ones
sit in the same vector with kind 0. Ours placed and re-homed levels on the
record's analog lanes only. Now `lanes::{analog_kind, level_lane_at}`; the
drag passes a value over every analog-kind lane; `Comment::drag` part 4
re-homes onto any lane with a value. Test `a_level_line_on_a_formula_channel`.

### L9.105 — Diagram tabs reorder by dragging along the strip  ·  ✅
Read (L9.101's report): a press on a tab activates it and arms detach mode
(`0x10068d0b`); moving inside the strip over another tab — with more than
one tab and tab swapping on — swaps it with the active (dragged) one and
moves the cursor to keep the grab offset (`0x100690b4`); the tabbed pane's
order is part of the saved workspace (INFERRED from the docking state).
Ours had a fixed order. Now `App::tab_order` (saved as
`Workspace/TabOrder`), `docking::swap_tabs`; the strip draws in that order
with an id per pane (so the drag follows the moved tab), a press or drag
brings the tab to the front, and a swap needs the pointer past the other
tab's middle (no back-and-forth between unequal widths; MFC moves the
cursor instead). Test `a_tab_dragged_over_another_swaps_with_it`.

### L9.106 — A gap audit of the message maps; four small fixes  ·  ✅
Read (subagent audit of all 354 entries of `index/msgmaps.json` against
the ledger and specs: 79 cited by address, 122 by id, 59 in the other
notes, 94 nowhere — almost all of those already described in the spec's
prose without their address; by hand the four below): the finder's
NM_DBLCLK `0x46e330` runs OnOK when LVM_GETSELECTEDCOUNT > 0 — every
selected file opens; the rename edit commits on Enter and cancels on Esc
at key-up (`0x470ae0`, `0x465df0(1 / 0)`), its WM_KILLFOCUS `0x470ac0`
does nothing, and the view's PreTranslateMessage `0x466100` commits it on
a left / right / middle press aimed elsewhere (MFC walks only the target's
ancestors, so a press on the ribbon or a diagram pane leaves it open);
the circle menu's channel items `0x437080` are checked when the flag is
set **and** the channel is not pending (bit 14); the vector toolbar's
33012 / 33013 (`0x48f490` / `0x48f4f0`) are enabled with a document and
checked only with one and the flag. The pair dialog's Добавить
`0x421aa0` stores (second, first) in diff mode — as ours after the L9.81
boundary swap (no change).
Ours opened only the double-clicked row, committed the rename on any
focus loss (and took the focus back every frame), checked pending
channels in the circle menu and showed the vector checks without a
document. Now `finder` (the selection on a double-click), `rename_ui`
(focus once, the press-in-view commit), `menu_chans`, the vector toolbar.
Test `inline_rename_commits_as_typed_or_refuses_with_397` (a press in the
view commits; idle frames keep it open).
Residual: Enter / Esc act at key-down in ours (egui's single-line edit
drops the focus on the press); the splitter's cursor is resource 30981
(AFX_IDC_VSPLITBAR, INFERRED; ours egui's vertical resize); F1 / 57667 /
57669–57671 (MFC's help handlers) not modelled — the CHM is not shipped;
the audit's address citations for §6 / §7 / VIEWS_SPEC beyond §7.7 not
added.

### L9.107 — The four diagram panes dock as units of their own; tab show / hide  ·  ✅
Read (by hand: `0x474e10` creates four `CDockablePane`s — Векторная,
Круговая attached to it as a tab (vtable +0x350), Дифференциальная and
Частотная docked on their own; `0x475040` toggles with `ShowPane(!visible,
FALSE, FALSE)`; subagent report on mfc120u: `CDockablePane::ShowPane`
`0x1008977d` → `CBasePane::ShowPane` `0x100619d3` →
`CBaseTabbedPane::ShowTab` `0x100661f7` → `CMFCBaseTabCtrl::ShowTab`
`0x10067f1f`): a re-shown tab becomes the front one only when no tab of
its group was visible (only bActivate, which FastView never passes, brings
it forward otherwise, `0x10089968`); hiding the front tab activates the
nearest visible tab on its right, else on its left (`0x10067fae`); hiding
the last hides the tabbed pane; the tab control hides its strip with one
visible tab (`HideSingleTab(TRUE)` `0x1013a212`); the tabbed pane's
caption follows the active tab (`0x10066b00`).
Ours had two docking units (the three-pane group, Частотная), a re-shown
pane brought to the front, a strip over a single visible tab, and the
circle / differential planes sharing one state (only one could show).
Now `docking::{Unit = Pane, Docks, Spot::{Beside(unit, side), Tab(unit)}}`
— `host` (a docked tab follows its host), `members`, `parent` (a group
beside another splits its slot, nested; loops and hidden parents make a
container of its own), `drop_on` (tabbing in brings the group's tabs),
`leave` (a beside group takes the container), `float_tab` (the group's
own pane hands the group to the next tab); `Panes::{stamp, activate,
front}` (each group shows its most recently activated visible tab);
`TogglePane` as read; `views.rs` renders each group (strip only with two
visible tabs), containers as nested slots, the five-way over any docked
group; each circle / differential pane brings its plane's state in
(`switch_diagram`, also for the printout's picture, which used the wrong
plane's characteristics); workspace keys per pane (`Workspace/<Pane>/…`,
`Activated`), an L9.98 two-unit workspace read into the four.
`FV_WINDOW_STATE=docksplit`. Tests `a_tab_pulled_out_floats_alone`,
`beside_slots_nest_and_collapse`, `a_two_unit_workspace_reads_into_four`,
`group_tabs_dock_as_panes_of_their_own`,
`diagram_panes_start_visible_and_toggle_beside_the_oscillogram` (now as
read); the L9.92–L9.105 docking tests unchanged in intent.
Residual: the default stays the user's screenshot (one group, Частотная
below it — L9.3); several containers on one edge are still separate rows;
a slot's children split in pane order (MFC keeps the split tree it built);
the caption double-click / pin / drag act on the whole group (as read in
L9.92) — the animation switch still not modelled (Ctrl: L9.128).

### L9.108 — Document tabs: MDI tabbed groups as FastView sets them; tabs reorder by dragging  ·  ✅
Read (by hand): the frame's OnCreate builds a `CMDITabInfo` (MFC
constructor `0x100b29f6`: location top, style 3, close button, no icons,
no tooltips, no auto colour, no document menu, tab swap on, flat frame,
no active-tab close, the manager's border size) and changes style to 4
(OneNote), no icons, auto colour, document menu and the close button on
the active tab (`0x47290a`–`0x472933`), then calls ordinal #4367
(`0x100bb6ad`, which forwards to the MDI client area's tabbed-groups
enable — its neighbour `0x100bb6bc` is the plain `EnableMDITabs`) with
TRUE: MDI tabbed groups, tab swap on. The frame's vtable (`0x4cbd00`)
against `CMDIFrameWndEx`'s overrides slots 0–4, 12, 66, 69, 97, 120, 123,
126–131, 134, 135 — not slot 117 (`0x100bc637`, `OnShowMDITabContextMenu`:
returns FALSE unless the allowed items carry the can-be-docked bit 0x10,
then offers only the «dockable» item −106): a tab dropped outside its
strip or right-clicked offers no new tab group, so FastView never splits
the document area into groups.
Ours had fixed tab order. Now `doc_tabs`: an id per document (its
serial), a press or drag brings the tab to the front, a drag over another
tab past its middle swaps them (`0x100690b4`); out of the strip nothing
happens. `App::doc_tab_rects`. Test
`a_document_tab_dragged_over_another_swaps_with_it`.
Residual: MFC moves the cursor to keep the grab offset after a swap
(ours swaps past the middle instead, as the pane tabs, L9.105); the tab
border size from the visual manager not compared.

### L9.109 — Edge-docked blocks in the docking manager's order  ·  ✅
Read (subagent report on mfc120u; the 4 px static by hand from its
report's address): a drop on a frame-edge marker (`DockPane` `0x10088cdc`
→ `0x1008bc77`, markers 0–3 outer, 4–7 the document area's arrows,
`CheckAlignment` `0x100a8695`) calls `DockToFrameWindow` `0x10088f61`,
which always makes a new block (a default pane divider with its
container, `0x1008860c`) — never a row of a `CDockSite`, never an existing
block; an outer drop inserts the block before every pane block of the
manager's list (`AddPane` `0x1008b521`, just inside the toolbar rows,
status bar and auto-hide sites), an inner one appends it (`0x10061335`;
the start-up docking too, `0x1008be35`); `AdjustDockingLayout`
`0x1008cd55` lays the list out in order, each block keeping its thickness
along its edge and spanning the space the earlier ones left; the divider
is 4 px on the block's inner side (`0x102a10a8`, ordinal 8529, not
imported by FastView).
Ours sorted the containers outer first, otherwise by pane. Now
`Dockable::order` (an outer drop below every order, an inner one above;
a group taking a container takes its order; saved as
`Workspace/<Pane>/DockOrder`, an older workspace ordered by its outer
flag); `diagram_panes` lays the containers out in that order.
`FV_WINDOW_STATE=dockbottom` is now a real outer drop (Частотная spans
the frame's width under the group's column). Test
`edge_blocks_in_docking_order`.
Residual: the divider is egui's resize grip (not a 4 px bar); its drag
limits NOT DETERMINED; groups docked beside one another inside a block
(`DockToWindow`) keep our nested split (L9.107).

### L9.110 — Printing inside the printer's hardware margins (CUPS PPD, GDI caps in preview)  ·  ✅
Read (L9.22 / L9.94): the original lays every page out on the printer
DC, whose origin and extent are the printable area (HORZRES / VERTRES at
PHYSICALOFFSET); MFC's preview shows the pages on that DC.
Ours laid CUPS jobs and the preview out on the whole sheet. Now
`spool::Paper::margins` (portrait l, t, r, b mm): on CUPS the PPD's
`*ImageableArea` of the starred `PageSize` (`ppd_margins`, the PPD from
`/etc/cups/ppd/<printer>.ppd`, else the scheduler's public
`/printers/<name>.ppd` on 127.0.0.1:631), on Windows PHYSICALOFFSET and
HORZRES / VERTRES; `Device::printable` (the job on the printable area,
`offset` its place on the sheet), `PageSetup::oriented_margins`
(landscape turns them a quarter counter-clockwise — INFERRED);
`pdf::Sheet::at` places each page there, `Canvas::on_sheet` shows the
preview pages on the whole sheet. Test `cups_margins_from_the_ppd`.
Residual: the landscape turn of the margins is INFERRED (GDI's own
landscape caps on Windows printing are taken from the DC as before); the
Windows margins path run on the VM in L9.113 (Print to PDF: none); the
PDF-file entry stays the whole A4 sheet (no printer).

### L9.111 — Merge: method 2 as read, the period and cursor rules, flags baked in, the discrete resampling  ·  ✅
Read (subagent report on `0x47c440`…`0x47ce0f`, `0x47d990`, `0x426020`;
by hand: the fixed 50.0 at `0x4d1718` and the rate test in both methods
(`0x47bd43`, `0x47c562`–`0x47c5ad`), the cursor's rounding `0x47c555`,
the median helper `0x477810` (0.5 at `0x4d1648`, sort, floor / ceil)):
a channel's samples per period are its own rate ÷ 50 (rounded) when it
has one (pasted), else the record's integer rate div trunc(f) — in both
methods; the cursor in channel 2 is round-half-up(r₂·(c/R − offset)).
Method 2: refused when 1.5·N + 1 exceeds either cursor (3259) or a cursor
is past the last sample (3162 — no period after it needed); Δ = t₁(c) −
t₂(c₂); in [c − N₁, c + N₁) (to the last sample near the end) the
one-period window whose |P₁|·|P₂| (P₂ at the first sample with t₂ + Δ ≥
t₁, at most the last but one) has the least non-zero population spread
(`0x4789a0`); over it the phase differences to P₂ interpolated linearly
at t₁ − Δ, wrapped once into ±180°; offset = Δ − median/(f·360); the
population spread > 1° → 3305, else 3456. «Объединить» resamples every
channel as the accessor reads it (ADC code, minus the DC, inverted, ×k,
NaN as 0) and divides analog by k (`0x47e598` / `0x47e748`) — the flags
stay set, so their transforms are baked in (an inverted channel shows
un-inverted after the merge); a discrete channel takes, at grid time t,
the first sample from index 1 at or after t, or the next one when that is
half a grid step or more away (`0x47e5b8`–`0x47e6a9`; not a
previous-sample hold).
Ours took the mean / max−min over c ± N with channel 2 by scaled index,
N from f, a full period required after the cursor, raw samples merged
under the kept flags, discrete channels held. Now `merge::{period_samples,
cursor_in, sync_by_prefault (as read), hold_at (the rule)}`, NaN pasted
samples read 0, `App::merge_records` feeds `flagged / k`. Tests
`prefault_sync_corrects_by_the_phase` (limits, a fault outside the
quiet window), `period_samples_and_the_pasted_cursor`,
`merge_rate_and_resampling`.
Residual: (the padding, the sub-sample shift and the pick's d without
the shift: L9.120); (pasted channels' own flags: L9.136); whether the paste deselects the pasted
channels NOT DETERMINED; (pasted discrete lanes before the merge: L9.71).

### L9.112 — Print details: device-pixel offsets on paper, the header name, the preview's diagram scale  ·  ✅
Read (subagent report on the diagram painters and `0x405c20` /
`0x406590`; the earlier L9.66 / L9.67 / L9.93 print branches confirmed):
the scale s comes from the attribute DC — the printer, in the preview
too (`0x442012`…`0x44204a`); a few offsets stay in device pixels on paper
— the vector angle labels +2 / −2 / +7 / −2 (`0x491bfc`–`0x491c55`), its
legend swatch +2 (`0x490e13`), the plane's top +5 (`0x43b679`) and the
120 px plot / table thresholds (`0x43b61f`, `0x43b62e`, `0x43b6b2`); the
header's first line names the path after its last '\' with exactly four
characters cut off ("" with no path), each of the three lines a
single-line DrawText clipped to its own 0.9T rect.
Ours scaled those offsets by s on paper (6× at 600 dpi), named the file
by its stem (or the document's name), clipped the header as one block,
and drew preview diagrams at s = 1 (110 dpi). Now `diagram_ui::dpx`,
`printout::header_name`, per-line clips, `Job::attrib_dpi` (the preview
renders diagram pages at the application printer's s — 300 dpi PDF / CUPS
→ 3 — and shows them at the preview's resolution). Test
`header_name_cuts_four_characters`.
Residual: the print look keyed to s ≠ 1 rather than to printing (a
printer under 192 dpi gets the screen pens, layout and the 5 px splitter
strip — ours prints the print look at s = 1); header T on page 1 / in the
preview is the printer's stock font's «A» height (device-dependent, ours
12); the company name measured in the heavy font on paper; the print
dialog's status / type / place / comment fields and «Свойства...» (the
common print dialog fills them from the spooler — ours empty); a print
resetting the live circle zoom / consuming the differential's first-paint
push not reproduced.

### L9.113 — The print dialog's printer fields from the spooler  ·  ✅
Read (subagent report): DIALOG 347 is the common print dialog's template
(cmb4 the printer list, psh2 «Свойства...», stc12 / 11 / 14 / 13 the
values of Состояние / Тип / Место / Комментарий; «Печать в файл» present
but hidden); CFastViewPrintDialog handles only 2173 / 2174. comdlg32 fills
the fields from the printer's spooler data (INFERRED: status, driver,
port, comment) and «Свойства...» opens the driver's document properties.
Ours left the four fields empty. Now `spool::details` — Windows:
PRINTER_INFO_2 (status 0 «Готов», paused «Приостановлен», error «Ошибка» —
the wording INFERRED, comdlg's own strings not read), the driver, the
port, the comment (run on the VM: «Microsoft Print to PDF» → «Готов»,
«Microsoft Print To PDF», «PORTPROMPT:», ""); CUPS (`cups_details`):
`lpstat -p` idle / now printing / disabled, `lpoptions -p`'s
printer-make-and-model, printer-location, printer-info; read once per
printer chosen. Test `cups_printer_details`.
Residual: «Свойства...» stays disabled (the driver dialog and a job
DEVMODE kept across it not modelled); comdlg's queued-jobs count in the
status not shown.

### L9.114 — Ribbon sizes as MFC lays them out: large and collapsed buttons, menu rows, disabled ink  ·  ✅
Read (subagent report on mfc120u; by hand: the original's labels, string
158 «Дифферен циальная», 565 «Осцилло грамма», 159 «Быстрый анализ»): a
large button (`GetRegularSize` `0x10101ab9`, margin (5, 1)) is max(image
+ 10, text + 5) wide and image + 1 + max(two lines, image + 1) tall (66
with 32 px images); its bottom text (`OnCalcTextSize` `0x10100a07`) is one
line without a space, else broken at the space minimising max(line 1,
line 2 + arrow), the width rounded down to even; the image at the top + 3,
the lines from + 38. A collapsed panel's button (`0x10120c93`, MFC's own)
has a 16 px image, always counts the 9 px arrow, is max(28, T + 5) + 2
wide and the panel's whole height; Office 2007 draws a 38 × 38 frame from
the top + 3, the icon at + 10, the text from + 40 (`0x10193c7b`). The
application menu's rows (`Repos` `0x1011cea4`) are max(text, 34) + 10 =
44 px with the 32 px image at the left + 3 and the text from + 38 with an
end ellipsis (`0x101012f1`), separators from 4 + 32 px. Disabled text in
the Office 2007 looks is the style's fixed TextDisabled (141, 141, 141;
Aqua 159, 141, 154 — `0x10193a84`), not a blend.
Ours wrapped labels at the first space and hyphenated two by hand («Диффе-
ренциальная», «Осцилло-грамма»), drew 64 px buttons with a 28 px icon,
collapsed buttons sized to their title with a «▾» and no image, 36 px menu
rows, and half-strength disabled ink. Now `ribbon::{bottom_text,
large_width}` and the original's labels, `LARGE_H` 66, the collapsed
button and `collapsed_width` as read (so the untitled single-button panel
can now collapse in a very narrow window — L9.91's «never» was ours),
`main_entry` 44 px rows, `Palette::disabled`. Tests
`large_button_text_breaks_at_the_balanced_space`,
`narrow_ribbon_collapses_panels_and_keeps_its_height` (the first step
found by width); rendered at 1320 / 866 and the menu.
Residual: (the Black palette: L9.117); the line height is ours (12 px; MFC's from the
font); whether Repos stretches large buttons to the column NOT
DETERMINED; the collapsed button's 16 px image is ours (a framed square).

### L9.115 — Keytips: recent documents, disabled elements, Help's drop-down, Alt held  ·  ✅
Read (subagent report on mfc120u): the recent documents are created as
«&N name» for 1–9, «1&0 name» for the tenth, the plain name beyond
(`0x1011cb1e`–`0x1011cb5f`), and a menu element whose keys are shorter
than 2 takes the letter after its first «&» (`GetKeyTipSize`
`0x10064ea3`) — keys 1…9, 0; `OnKey` (`0x101036b6`) returns FALSE for a
disabled element and ProcessKey leaves keytip mode only on TRUE
(`0x100fef15`) — its key does nothing and the keytips stay; the Help
split button has a menu and no separate menu key, so «A» opens its
drop-down (Помощь / О программе…) and posts Home (CMainFrame handles 57667
with MFC's help finder, so it is enabled); a letter with Alt held arrives
as WM_SYSKEYDOWN, which the ribbon passes on — no keytip fires.
Ours had no recent-document keys, activated (and left) on a disabled
element, did nothing on «A», and fired letters typed with Alt held. Now
`keytips::Target::Recent`, `Keytips::key(c, avail)` (absent targets
unmatched, disabled ones inert), the Help button's own popup opened by
«A», letters ignored while Alt is down; the QAT's Сохранить / Печать are
disabled without a document (no handler then — MFC's auto-disable).
Test `typing_activates_descends_and_backs_out` (disabled, recent);
rendered `keytipsmenu`.
Residual: the Home highlight in the opened Help menu not shown; our
Помощь stays disabled (no CHM shipped — the original's opens the help
file); the recent rows' keytip place is ours; what removes hold-shown
keytips after an Alt+key combination NOT DETERMINED.

### L9.116 — «Проводник»: the shell folder tree, «Выбрать», My Documents, date pickers  ·  ✅
Read (subagent report on FileFinder22Dlg; marks there): the folder pane
is MFC's `CMFCShellTreeCtrl` (control 2167; constructor #747 at
`0x46b071`), left at its defaults — folders only (SHCONTF_FOLDERS
`0x10132ec6`, hidden ones out), one Desktop root expanded (`0x10132fa6`),
children read on expanding; OnInitDialog selects the scanned folder
(`SelectPath` `0x46b7de`); the dialog handles no tree notification —
a click only moves the tree's selection; «Выбрать» (`0x46b8f0`) takes
GetItemPath of the selection (nothing on a virtual node), compares it
case-sensitively with Path and only when it differs sets Path, refreshes
«Папка:» and rescans; the scan always uses Path; the default Path is My
Documents (`0x46b0c0`, CSIDL_PERSONAL) before the profile's; «Папка:» is
a static with a static edge; the date pickers (1077 / 1078) are long-date
drop-down calendars without a «none» check.
Ours listed one level of the current folder (read every frame, hidden
ones included, with «..»), changed the scanned folder on every click,
started in the current directory, showed «Папка: …» as one label and
three day / month / year spinners. Now `finder::{Node, FolderTree,
fs_children, documents_dir}` (the Desktop root with the computer — drives
on Windows, `/` elsewhere —, the user's folder and the desktop's folders;
children read once on expanding; opened to Path, all or nothing),
«Выбрать» as read, the framed «Папка:» field, `date_picker` (the long
date «23 октября 2026 г.», a month calendar, Monday first; a day click
keeps the time of day; the pickers start at today, UTC). Tests
`folder_tree_opens_to_the_start_folder`, `long_dates_and_weekdays`;
rendered.
Residual: the shell namespace is ours (no Libraries / Network / Recycle
bin, no shell icons or context menu); the local time offset of «today»
not taken; the spec's `<` corrected to `≤` (L9.9's ≤ was right); `.sto`
entries stay ⛔ (the converter).

### L9.117 — The Office 2007 looks' colours from MFC's style data  ·  ✅
Read (subagent report on the four `*_IDX_OFFICE2007_STYLE` XMLs and style
PNGs in mfc120u; the values READ, their mapping to our fields INFERRED
unless named): RIBBON/PANEL/TextNormal (labels) Blue (21, 66, 139), Black
(0, 0, 0), Silver (76, 83, 92), Aqua (0, 21, 110); RIBBON/CATEGORY/TAB
TextNormal (inactive tabs) the same but Black white; BARS/DEFAULT (the
band) (191, 219, 255) | (83, 83, 83) | (208, 212, 221) | (196, 202, 217);
the category body's top (229, 237, 248) | (215, 219, 224) | (242, 244,
248) | (195, 202, 217); MAINWND/MainClientArea (164, 195, 235) | (70, 70,
70) | (186, 193, 202) | (181, 189, 211); the status bar (STATUSBAR_BACK,
BARS/STATUSBAR/TextNormal) Blue (186, 212, 247) / (21, 66, 139), Black
(47, 48, 48) / white, Silver (199, 201, 206) / (35, 38, 42), Aqua (209,
215, 229) / (0, 21, 110); the hot button (255, 237, 186) → (255, 218,
115), border (219, 206, 153) — Aqua's blue-grey (198, 210, 231) →
(183, 193, 213), border (143, 153, 181). So Black is a light ribbon with
black labels in a dark frame, tab row and status bar, and Aqua is
blue-grey, not teal.
Ours had a dark Black ribbon with light labels (dark egui visuals) and a
teal Aqua, among smaller shifts. Now the four palettes as read (panel
separators blended from their alpha over the body — INFERRED),
`Palette::{tab_text, status_bg, status_text}`, `ribbon_tab` with the
inactive colour, the status bar's own fill and text, light visuals for
every look, the read disabled grey for Black too. Rendered all four;
the real-records pass then showed the selected pane tab's text nearly
invisible (egui draws selected text in the selection stroke, now the
style's light hover border) — the selection stroke is the label colour,
pinned by `ribbon_labels_contrast_with_their_background_in_every_look`.
Residual: the panels' translucent caption band and inner white line, the
category body's gradient (ours flat), the application button's orb, the
caption bar and the main frame border are not drawn from the style
images; the pre-2007 looks' palettes stay ours (INFERRED system colours).

### L9.118 — The print look keyed to s ≠ 1; the splitter strip at s = 1  ·  ✅
Read (L9.112's report): the vector pens (`0x490495`) and the plane grid
pens (`0x439486`) test `s == 1` and make the screen pens then; the
frequency painter's print layout flag (`0x46f63b`) is s ≠ 1 and is the
only flag behind its tiling, names per row and missing x labels; the
masks alone test `m_bPrinting`; and at s == 1 the circle / differential
painters, after RestoreDC, paint the 5 px splitter strip (`0x438110`:
PatBlt at the DC's (0, 0), as wide as the live diagram window).
Ours chose the print look whenever printing. Now `diagram_ui::print_look`
(the scale when s ≠ 1) for those pens and the frequency tiling / axis
(fonts and masks still by printing), and `render_page` paints the strip
on circle / differential pages at s = 1 (colour INFERRED the panel fill).
Test `a_circle_page_at_scale_one_carries_the_splitter_strip`.
Residual: the readout table's and the vector header's pens left on
printing (not named by the report); the company name's heavy-font width
on paper (ours measures the regular one); the header's T on page 1 / in
the preview (the printer's stock «A», device-dependent).

### L9.119 — Ribbon body and panels drawn as the Office 2007 styles do  ·  ✅
Read (L9.117's report on the style images): the category body
(RIBBON_CATEGORY_BACK) is a gradient — top, a dip 18 px down, a light
bottom (Blue (229, 237, 248) → (199, 216, 237) → (231, 242, 255); Black
(215, 219, 224) → (181, 188, 198) → (211, 211, 211); Silver (242, 244,
248) → (213, 220, 231) → (249, 249, 249); Aqua flat); a panel's body is
transparent with a white inner line at alpha ≈ 130 (RIBBON_PANEL_BACK_T)
and a 15 px caption band (RIBBON_PANEL_BACK_B: Blue (152, 188, 232) →
(133, 169, 216) at alpha 92, Black grey 159 → 137 and Silver (222, 226,
238) → (196, 201, 210) at ≈ 186, Aqua opaque (106, 112, 128) → (163,
174, 199)) under the caption in RIBBON/PANEL/CAPTION/TextNormal ((62,
106, 170) | white | (83, 84, 89) | white).
Ours filled the body flat, gave panels no band and the caption the label
colour at 80 %. Now `Palette::{body, caption_band, caption_text}`,
`ribbon::gradient_mesh` (one mesh — the old 24 strips left anti-aliased
seams, visible on Black's dark band), the body gradient and each panel's
band and inner line painted behind their content. Rendered all four.
Residual: the band's rounded corners and the panel's outer edge image
(our divider line) are approximations; the application button's orb and
the caption bar still ours.

### L9.120 — «Объединить»: the zero padding and the sub-sample shift as read  ·  ✅
Read (subagent follow-up on `0x47d990`, `0x428c20`): with the pasted
record `offset` after the document, the later side — the pasted channels
when the offset is positive, the base ones when negative — gets
ceil(r·|offset|) real zero samples at its own rate r in front (times
round to µs, `0x47dd20`–`0x47ddd9`), and those channels are shifted by
−(1/R − |fmod(offset, 1/R)|) (`0x47e014`–`0x47e066`, `0x47e29c`; a whole
grid step early when the offset is a multiple of 1/R, up to 1/r off when
r ≠ R — the original's arithmetic); every channel shorter than the
longest padded span gets zeros at its back; the grid starts at the padded
common start, round(R·span) − 4 samples, stored as whole µs; the zeros
take part in the fit and the pick and are read through the accessor
(a DC-removed channel's padding reads −mean); the fit's and the pick's j
is the first sample from index 1 whose time plus the shift reaches t (no
cap for the fit — a sample past the end reads 0 at an extrapolated time;
the pick capped at the grid count), the fit evaluated at t − shift over
the unshifted times, the pick measuring d without the shift.
Ours placed the pasted channel exactly at t + offset with 0 outside, the
fit clamped to the channel's ends, and the pick measured with the shift
(L9.111). Now `merge::merge` pads, shifts and spans as read (`Lane`,
`pad`, `fit_at`, `pick_at`; `base_pads` the accessor's reading of zero
per base channel from `merge_records`). Tests `merge_rate_and_resampling`
(the fit past the end, the pick with a shift),
`merge_pads_and_shifts_the_later_record`.
Residual: (the merged record's track rec+0x1fc: L9.132; channels' own +0x50 not
modelled); `0x478410` assumed an exact 3 × 3 solve; (pasted channels'
own flags: L9.136).

### L9.121 — The header's line pitch on the first page: the printer's stock font  ·  ✅
Read (L9.112's report, `0x405c20`): the header measures «A» with
GetTextExtentPoint32 in whatever font the DC holds (`0x405ce1`) before it
creates its own Arial and never restores the old one; so T is the
printer DC's stock font's «A» height on the first printed page (and on
every preview page, the preview DC restoring per page — INFERRED), the
header's Arial 12 cell on later pages.
Ours used 12 screen px always. Now `Device::stock_t` (Windows: the «A»
height of a fresh printer DC ÷ k; run on the VM — Microsoft Print to PDF
gives 16 screen px; CUPS: none → 12), the header's T taken from it on
page 1 and in the preview (`print_preview_open` copies the application
printer's).
Residual: the footer keeps 12 (as read); a driver resetting DC
attributes at StartPage NOT DETERMINED.

### L9.122 — «Свойства...» in the print dialog (Windows)  ·  🟡
Read (L9.113): the dialog is the common print dialog, whose psh2
«Свойства...» opens the driver's document properties; the DEVMODE edited
there is the one the job prints with (INFERRED from comdlg32's
behaviour).
Ours had the button disabled. Now `spool::properties` — Windows:
DocumentPropertiesW with DM_IN_PROMPT (seeded with the last choice) and,
on OK, the DEVMODE kept per printer and used as is for that printer's
DC (`devmode`); elsewhere none (the button disabled off Windows).
Residual: the driver dialog itself not exercised (the VM is reached
over ssh only; the code path compiles and the stored DEVMODE path is the
existing one); the dialog has no owner window (ours passes none); the
page setup's orientation is not merged into a DEVMODE chosen there.

### L9.123 — DIALOG 401–543 mapped: formula forms, sheet pages, dialogs; the tree's icons  ·  ✅
Read (subagent report; an immediate-operand scan of `.text` for 400–559):
the range is not "channel-role dialogs" — the tree builder `0x401c70`
pushes exactly the 57 AddFormula form ids (equal to
`addformula::TREE` / `form()`); 463 ChannelsDlg, 477 СMultiplierDlg, 492
LoadingDlg, 510 CProgSettingdDlg, 536–539 / 543 the quick-analysis pages,
509 CProgrBarDlg (the `.sto` conversion's modeless bar — created hidden
and, by an inverted null check in its callback `0x4709e0`, never shown);
412, 416–421, 427–430, 433–435, 451, 455, 459–461 (older form variants),
480 (the old finder) and 535 have no code reference. Every used template
has its reconstruction. The tree's image list (`0x401c9d`–`0x401d1a`:
bitmap 395, 16 × 16) gives each top-level branch one image for its groups
and leaves — Формула 0, Анализ сигналов 5, Токи 2, Напряжения 4,
Мощности 3, Сопротивления 1, Дифференциальная защита 6. Also settled by
the same pass of reports: the symmetric components have no view of their
own — the quick-analysis sheet makes formula channels read out like any
other (L8), so the L6 row is closed.
Ours drew the tree without icons. Now `formula_dlg::branch_icon` (our own
glyphs, one per branch). The spec's form range corrected.
Residual: the images are ours (bitmap 395 is the original's artwork);
DIALOG 509 stays with the ⛔ `.sto` reader (the original never shows it).

### L9.124 — `.osc` start and trigger stamps, the trigger sample, the finder's dates  ·  ✅
Read (subagent report; by hand: `0x44a690` builds the trigger SYSTEMTIME —
year byte 0x0B + 2000, month 0x0A, day 9, hour 8, minute 7, second 6,
milliseconds 10 × byte 5 — after `0x44a570` byte-swaps the info block's
i16s at 2 / 0x0C / 0x0E and BCD-decodes bytes 5..0x0B): the OSC reader
(`0x452d42`–`0x452e57`) fills the same four stamp fields the `.cfg`
loader does — the trigger from the info block, the start (pretrig × N − 1)
periods of ⌊10⁷ / R⌋ × 100 ns before it (pretrig the i16 at 0x0E, N the
samples per block — with no pre-trigger blocks the start lands a period
after the trigger), formatted `%i/%02i/%4i` month first and
`%i:%02i:%02i.%03i`; `doc+0x214` (the trigger sample) = pretrig × N
(`0x452d89`), 0 from the document constructor and set back to 0 by the
`.cfg` / `.cff` writers (`0x44d6c5`, `0x44daaa`); the finder (`0x46c730`)
takes the version from block 0 and its date / time from the info block
at 128 · H, BCD-decoded, without milliseconds.
Ours left `.osc` stamps empty (the parameters bar, the print header,
«Абсолютное время», the clipboard header, merge method 0 and a `.cfg`
Save-As all saw defaults) and the finder read block 0's raw bytes. Now
`comtrade::{osc_trigger, osc_stamps}`, `Oscillogram::trigger_sample`
(reset by a COMTRADE Save-As in `save_doc_to`), `finder::read_osc_header`
on the info block. Tests `osc_stamps_from_the_info_block`,
`osc_header_and_filters` (the layout corrected).
Residual: R is our first block's rate (the original's `rec+0x228` after
all blocks — equal unless the rate changes); invalid BCD leaves stamps
empty (the original reads an uninitialised value); the trigger sample's
uses — the dotted trigger line, ruler / readouts / samples mode relative
to it, the difference band — in L9.125.

### L9.125 — The trigger sample in the view: the dotted line, labels and readouts from it  ·  ✅
Read (L9.123's report pass): `doc+0x214` ≠ 0 draws a dotted trigger line
(`0x464219`, (35, 194, 87)); the ruler (`0x46453b` / `0x464e44`), the
difference band (`0x466792`) and the status bar (`0x49484d`) count from it
— times relative to the trigger's, samples k − trigger + 1.
Ours counted from the record start (no trigger existed before L9.124).
Now `OscView::label_at` (ruler and band labels), `cursor_status` from the
trigger, the dotted line in `vizirs`. By hand afterwards: the ruler's
tick value is t(k) − t(trigger) in µs for every unit
(`0x4649cf`–`0x464a0d`) and the absolute label adds it to the start's
time of day (`0x464dc0`–`0x464e26`) — «Абсолютное время» is
trigger-relative too (a quirk with an `.osc` trigger); ours the same.
`FV_WINDOW_STATE=trigger`. Test `cursor_status_counts_from_the_trigger`;
rendered.
Residual: COMTRADE has no trigger (always 0, as read).

### L9.126 — Merge synchronisation under the frequency correction  ·  ✅
Read (subagent report; L9.65 for the gate): the `fault:` dispatcher
`0x427670` takes the corrected path for an analog channel when its record
has one rate segment, a candidate list, `rec+0x874` > 0 and the candidate
is not the channel itself — a pasted channel sits in the base document,
so its gate and frequency channel are the base record's; the corrected
fault `0x427830` uses fs = the channel's own rate and f = the frequency
channel's sample at the channel's own index (50 when 0). Method 1's
second difference stays plain (`0x426020`) while its denominator A =
`0x427670(k − 1)` is corrected; method 2's phasors come from the cache
`0x428490` fills, corrected under the gate, for both channels.
Ours fed the plain fault component and DFT to both methods. Now
`merge::SyncChan::corr` — the base channel `freqcorr::of(..).for_chan`,
the pasted one the same frequency channel with its own rate — used by
`jump` (A) and `sync_by_prefault` (the phasors). Test
`prefault_sync_under_the_correction` (agrees with the DFT at 50 Hz).
Residual: whether our pasted samples equal what `0x426020` returns for a
pasted channel (its flags / k) NOT DETERMINED.

### L9.127 — The Office 2007 looks' round application button  ·  ✅
Read (by hand, the ribbon set-up `0x472f30`): the application button
gets image 570 (`0x472fa7`), the keys «f» (`0x472fb8`) and is installed
with `SetApplicationButton(…, (45, 45))` (`0x472fef`) — the Office 2007
round button, 45 × 45 at the top left over the quick-access row and the
tab row; L9.117's report gives its centre per style (RIBBON_BTN_MAIN:
Blue (215, 221, 229) / hot (247, 222, 89) / pressed (205, 126, 14);
Black and Silver (223, 223, 223) with the same hot / pressed; Aqua
(209, 217, 232) / (146, 200, 248) / (215, 229, 250)).
Ours showed a logo and a «Файл ▾» text button in the quick-access row.
Now, for the Office 2007 looks, `Palette::app_orb` and `paint_orb` (a
round button with a darker rim and a gloss, our logo inside — image 570
is the original's artwork) over both rows; its upper part anchors the
menu, a click on its lower part opens it (`kt_menu`); the older looks
keep the text button (their visual managers' button NOT DETERMINED).
Test `the_round_application_button_opens_from_its_lower_half`; rendered.
Residual: the orb's look is drawn (the style's bitmap not reproduced).

### L9.128 — Smart docking: nothing docks while Ctrl is held  ·  ✅
Read (by hand, mfc120u): CDockingManager's vtable slot 17
(`0x1008c83e`, OnMoveMiniFrame) returns TRUE at once when
GetKeyState(VK_CONTROL) is negative (`0x1008c860`) — no marker update;
the expected-rect calculation skips its work under the same test
(`0x1008c49b`, no preview); the drop (`0x1008a27f`) skips docking when
GetAsyncKeyState(VK_CONTROL) is negative (`0x1008a2cd`). So a pane
dragged with Ctrl held just moves and stays floating where it is let go.
Ours docked on whatever marker was under the pointer. Now
`dock_drag_frame` takes no hot marker while Ctrl is down (no preview, no
drop; the window follows the pointer). Test
`smart_docking_drops_on_the_markers` (a Ctrl-held drag onto the group's
centre stays floating, moved).
Residual: the marker that was hot when Ctrl went down stays lit in MFC
(markers not updated — INFERRED from the early return); ours unlights it.

### L9.129 — The update (CMDUI) rules as read: zoom limits, no-document greying, stats / modes, Save on .sto, the comment check  ·  ✅
Read (subagent audit of every ON_UPDATE_COMMAND_UI entry in
`index/msgmaps.json`; by hand `0x45c8c0`, `0x45d160`, `0x42f4b0`):
32800 / 32801 enabled while YAnalogSize < 500 / > 70, 32802 / 32803 while
f < 1.0 / f > the fit scale (`0x45c430`); MFC's auto-disable greys every
view / document command with no document (`m_bAutoMenuEnable` stays 1 —
frame+0x74, never written); `0x42fc60` serves 32817–32824: the stats rows
count every analog channel (computed and pasted too; disabled with none),
32820–32824 every selected channel of any kind with no group expansion
(disabled with no selection, checked iff all have the bit); 57603 greyed
when the path ends in ".sto" (`0x475750`); copy 57634 needs a selected
non-computed channel (digital and pasted count, `0x45bf60`); 32867 /
32868 checked from the first analog channel, neither with none
(`0x42ffb0`); 33041 checks while the comment is SHOWN (SetCheck(+9),
text 203 «Скрыть комментарий» when set — the spec had it inverted).
Ours: zoom always enabled, «Видимость» / «Вычисления» enabled with no
document (and Вставить), stats rows always enabled and checked over file
channels only, modes always enabled and counted with groups, Save on
.sto enabled, menu 561's copy analog-only, «Вторичные» checked with no
analog channel, 33041 checked while hidden. Now `App::{zoom_enabled,
can_save}`, `Doc::{zoom_x_enabled, stats_row_state, can_copy,
primary_shown}`, `mode_state` as read, the ribbon / QAT / app menu /
menus 561 and 563 use them. Test `update_rules_as_read`.
Residual: computed channels have no display mode of their own (the
original's 32820–32822 set bits 7–9 on them too — ours counts them as
instantaneous); digital channels' mode bits NOT DETERMINED (not counted);
invert / DC on digital, computed and pasted channels (32823 / 32824 count
them) — next; a path-less document (only our demo) keeps the comment
tools.

### L9.130 — Keyboard and wheel as read: Ctrl+C / Ctrl+V through egui-winit, MDI keys, preview keys, визир keys, per-message wheel  ·  ✅
Read (subagent audit of RT_ACCELERATOR 128 / 30997, every PreTranslate /
WM_KEYDOWN / WM_MOUSEWHEEL / button handler; by hand `0x45f490`,
`0x492f20`, the clamp at `0x45f52f`): table 128 is the only accelerator
table (LoadFrame 128, `0x4722b6`); CMDIFrameWnd::PreTranslateMessage
(mfc120u `0x1024c143` / `0x1024c16b`) translates it, then
TranslateMDISysAccel (Ctrl+F4 / Ctrl+F6 / Ctrl+Tab — the key set
INFERRED from Win32); print preview loads MFC's 30997 instead (PgDn
58114, PgUp 58115, Esc 58112, `0x102474d9`); the view's WM_KEYDOWN moves
визиры with the setter that also SHOWS them; no other window handles keys;
the wheel handlers act once per WM_MOUSEWHEEL, "exactly Ctrl / Shift" =
wParam with no button; Ctrl+Shift+LMB in the stats panel skips the
deselect-all (`0x45dce3`) and adds the range.
Ours: **Ctrl+C / Ctrl+V never fired in the real app** — egui-winit 0.29
turns them into `Event::Copy` / `Event::Paste` (the latter only with text
on the OS clipboard) and sends no key event; the headless tests injected
Key::C. Also no MDI keys, preview PgUp / PgDn, Ctrl+Shift matched the
table, disabled commands ran by shortcut, an arrow key moved a hidden
визир without showing it and acted with a diagram focused or Alt held,
the wheel coalesced a frame's notches and forced ≥ 120 px, Ctrl+Shift+LMB
cleared. Now `App::handle_keys` (`Event::Copy` / `Event::Paste`, Ctrl+V's
key-up when no paste was announced, Ctrl alone, `shortcut_enabled`, the
MDI keys), `handle_keys` skipped in preview, `preview_ui` PgUp / PgDn,
lanes' key block gated on `CopyFocus::View` and !Alt with `MoveV` showing,
`lanes::wheel_deltas` (per event: Line ×120, Page ×360, points as they
come) for the view and the planes, `SelClick::CtrlShift`. Tests
`shortcuts_as_egui_winit_sends_them`, `preview_category_commands_and_page_text`,
`arrow_keys_move_v1_and_ctrl_v2_by_one_sample`,
`wheel_modifiers_zoom_time_resize_lanes_and_scroll`,
`ctrl_shift_click_adds_the_range`.
Residual: Ctrl+V with an empty OS clipboard fires on the key-up (and not
at all if Ctrl is let go first); Ctrl+Insert / Shift+Insert copy / paste
on Windows through egui-winit (the original has no such entries); → caps
at N − 1 (the original at N, a sample past the data); (↑ / ↓ on a ГВ
never placed: L9.139);
MDI next / previous in tab order (WM_MDINEXT's order under MDI tabs ND);
the Shift page not quantised to the 1/10000 scroll grid.

### L9.131 — Message boxes as read: posted exceptions, the 219 box, the int rate field, a queue  ·  ✅
Read (subagent audit of every AfxMessageBox call — the only two message-box
imports, mfc #2261 / #2262; by hand `0x475680`, `0x4756e0`, `0x47d915`,
`0x44a230`): the readers / writers / block and computed-channel
evaluation / ОМП / .set load catch their exception and post it to the
frame (`0x475680`, message 0x40a), whose handler calls its ReportError(0,
0) — an MB_OK box (MFC adds the exclamation) with the exception's text,
"<file> - <message>" for file errors; «Синхронизация» by start time with
no overlap shows 219 with MB_ICONHAND and leaves the bar's text; DIALOG
392's sample rate goes through DDX_Text of an **int** (mfc #3761 — the
ordinal order of the int& / double& overloads, #3766 being 0x414's
double): text that does not start with a number gives «Введите целое
число.» (scanf's rule: L9.148); modal boxes and
posted exceptions come one after another. The other boxes (396, 473, 398,
397, 1086, the ОМП checks, SaveModified) already matched.
Ours: open / save errors and block / computed-channel evaluation errors
in the status bar, 219 as the bar's text, «Введите число.» with
fractions accepted, one alert slot that a second message overwrote. Now
`DialogState::{alerts (a queue of text + icon), push_alert,
push_alert_icon}` drawn one at a time; `finish_load`, `report_save`,
`recompute_block`, the quick batch push boxes; `synchronise` method 0's
NoOverlap is an error box; the rate field parses an i32. Tests
`a_failed_open_is_reported_in_a_box`, `no_overlap_is_a_box_and_boxes_queue`,
`bad_setting_reports_the_formula_and_keeps_old_geometry`.
Residual: 239 «Выбранные каналы имеют разную частоту» on 33035 (needs a
per-channel rate; reachable only after a paste); (DIALOG 353: L9.133); the print
failure's wording (ours «Ошибка печати: …»; MFC's 61702 INFERRED); the
.sto close warning 190 (.sto ⛔); (MessageBeep: L9.138); the
exception types each catch takes ND; 0 / negative rates in DIALOG 392 ND
(ours refuses them).

### L9.132 — The record's nominal-frequency track and rate segments as read  ·  ✅
Read (subagent report; by hand `0x454960`, `0x453db0`, the merge's
`0x47e4b5`–`0x47e84b`, the reader `0x414e90`): rec+0x1fc holds a value per
sample built by `0x454960` — resized to the sample count (old values
kept, new ones 0), then from sample 0 each sample gets its segment's rate
÷ X · f with X = round(R/f)·round(f) truncated (R = rec+0x228), moving to
the next segment after the sample whose index is the segment's `end` (so
the first sample of a new run still has the old value) and stopping when
the segments run out. Segments rec+0x1f0 = {rate, end}: a `.cfg`'s rate
lines verbatim; for OSC block 0 sets R (snapped), a block with another
rate pushes {current, its first sample}, then {R, total} unless the last
run already ends there with R (the final run carries R). rec+0x228: a
`.cfg`'s `0x453db0` of the mean rate, snapped to the standard table only
with several rate lines and only at exactly 50 / 60 Hz (else the
round·round fallback), from the second sample's time when the rate is 0;
OSC the snapped first rate; «Параметры осциллограммы» sets it and rebuilds
the track over the old segments (`0x42e540`); «Объединить» keeps the
base's segments, sets R rounded to an integer and rebuilds the track over
the grid (0 past the base's last `end`). Readers: `frequency:` (0 past the
end), the clipboard's `net_frequency` and `discr_frequency` (= rec+0x228).
Ours: `frequency:` = the line frequency everywhere, the segment count
only (the merge reset it to 1, so a merged two-rate record lost the
correction gate's refusal), the snap tables at any f near 50 / 60, the
clipboard wrote the line frequency and the first rate. Now
`Oscillogram::{rate_segs, rate_r, freq_track, segments(),
rebuild_freq_track}`, `comtrade::{int_rate, nominal_track,
comtrade_rate}`, built by both loaders, `retime` and `merge`;
`OscChannels::frequency` reads the track; `clip_record` writes it and
rate_r; DIALOG 392 shows / compares rec+0x228. Tests
`nominal_track_as_read`, `merge_pads_and_shifts_the_later_record`.
Residual: (channels' own frequency vectors and the ОМП tracked phasor:
L9.137); our `sample_rate` stays the first rate where the original's
signal code uses rec+0x228 (they differ only for rates that are not
multiples of f and multi-rate `.cfg`s); a rate-0 `.cfg`'s .dat
timestamps are not read (R stays 0, the track NaN); the `.set`
DiscretisationFrequency (rec+0x228 without a rebuild) not modelled.

### L9.133 — «Настройки частотной диаграммы»: the int exchange and its ranges  ·  ✅
Read (by hand, DoDataExchange `0x46ee70`; the profile keys at `0x4ca468`
/ `0x4ca480`): 0x401 → NumPeriods through DDX_Text of an int (mfc #3761)
and DDV_MinMaxInt (#3641) [1, N div (R div trunc f)] — N the samples of
the first analog channel (`0x431020`), R rec+0x228; then 0x3fe →
NumHarmonics, an int in [1, trunc(R / f · 0.5) · the periods just read];
the two checkboxes; on save pane+0x98 / +0x9c and the `Frequency Chart`
profile. MFC's messages: «Введите целое число.» / «Введите целое число
от 1 до N.», the first failure only, the dialog stays.
Ours: two DragValues clamped to 1..=50 / 1..=200 silently. Now text edits
and `diagram_ui::freq_dialog_exchange`; a failure queues the message and
keeps the dialog. Tests `frequency_dialog_exchange_as_read`,
`frequency_settings_dialog_edits_a_copy_until_ok`.
Residual: MFC moves the focus to the failing edit (ours does not); a
record whose R div trunc(f) is 0 (the original divides by zero) takes
1 in ours.

### L9.134 — Digital channels carry the value flags: invert drawn, baked by the merge  ·  ✅
Read (subagent report on `0x428490`, `0x42a0f0`, `0x426020`, `0x47e6a9`;
by hand the toggle `0x42f4b0` — every selected row, no kind test — and
`0x428490`'s ch+0x20 fill): the digital painter draws the display cache
ch+0x2c, not the raw samples; `0x428490`'s digital branch fills it with a
copy of the samples, or with invert 1 where the sample is 0 and 0
elsewhere; remove-DC and the mode bits (0x80 by default from the
constructors' 0xe1) are set by 32820–32824 on digital channels too but
read by nothing else (only the menus' checks); the stats panel draws the
invert icon beside the name (`0x462e88`); «Объединить» picks digital
values through `0x426020` (inverted baked in) and keeps the flag, so the
merged channel shows the raw record again. Also confirmed (L9.7 stands):
`0x428490` fills ch+0x20 with `0x426020(i)`, the vector the ОМП engine
copies — ОМП reads flagged values.
Ours: digital channels had no flags; 32823 / 32824 / 32820–32822 ignored
a digital selection. Now `DigFlags {invert, remove_dc, mode}` per digital
channel (`Doc::{dig_flags, dig, dig_mut, dig_on}`, in the undo snapshot,
shifted by `move_appended`), counted and set by `flag_state` /
`toggle_flag_selected` / `mode_state` / `set_mode_selected`
(`selected_digital`), the lane and its value drawn through `dig_on`, the
invert icon at (leftW − 18, y + 2), the merge baking it. Test
`digital_channels_carry_the_value_flags`.
Residual: (the merge's padding of an inverted digital channel: L9.140); a
formula naming a digital channel (the original's lookup has no kind test
— INFERRED; L9.145); computed and pending channels' flags (L9.135 /
L9.136).

### L9.135 — Computed channels carry invert, remove-DC and the display mode  ·  ✅
Read (subagent report on `0x428490`, `0x429490`, `0x461440`, `0x41faa0`,
`0x426240`, `0x4272a0`; the toggle `0x42f4b0` / CMDUI `0x42fc60` by hand,
L9.129): a computed channel (bit 13) goes through the same cache as a file
channel — its DC is the whole-period mean of its values from the start,
the accessor subtracts it and negates with invert, the display cache
ch+0x2c follows the mode (RMS `0x4272a0`, first harmonic `0x426240` over
the flagged values), the lane, the value at В1 and Max / Min read that
cache, СКЗ / A1 the flagged values; its stored complex series is
multiplied by −1 with invert (`0x42896f`; DC does not touch it), and the
vector / circle diagrams and later formulas (CHANNEL atom through
`0x426020`, its complex through the cache) read them so; the status icons
(DC, invert, RMS, harmonic) have no bit-13 gate, only the ratio icon
(`0x462b15`).
Ours: computed channels had no flags and were always instantaneous.
Now `Computed::{raw, raw_phasors, invert, remove_dc, mode, display,
set_values, shown, shown_series}` and `Doc::reflag_computed` (run after
every evaluation, in order, so later formulas read flagged values; the
evaluation sites store raw values through `set_values`), the toggles /
checks / mode set include computed channels, the lane / value / Max / Min
/ range read the display cache, the status icons. Test
`computed_channels_carry_the_value_flags`.
Residual: pending (pasted) lanes' flags — next; ADC (0x1000) on computed
channels ND (32852 skips them); whether the `.set` keeps a computed
channel's flags ND (ours does not save them); k (ch+0xf0) of a computed
channel taken as 1.

### L9.136 — Pending (pasted) lanes carry their flags; «Объединить» bakes them and keeps them  ·  ✅
Read (subagent report on the paste constructor `0x424cb0`, `0x428490`,
the painters, the stats bit-14 branch `0x461a24`, the merge `0x47d990`):
a pasted channel gets flags 0x40e1 (instantaneous, СКЗ / A1 rows, no
invert / DC) and its cache built (`0x4254b9`); 32820–32824 act on it like
any channel and rebuild its cache, so invert / DC / the mode show on the
pending lane before the merge — the lane and the value at В1 from the
display cache over its own samples, Max / Min of that cache, СКЗ / A1 over
its flagged own samples, the DC over its own period round(own rate / 50);
«Объединить» reads every pending channel through `0x426020` (`0x47e3fb`;
digital `0x47e6a9`), divides by k, clears only bit 14 (`0x47e55e`) — so
the flags stay on the merged channel and apply a second time (an inverted
pasted channel shows un-inverted after the merge, like a base channel).
Ours: pending lanes had no flags (the toggles skipped them), drew and
read their raw own samples, and the merge took them raw. Now
`Computed::{pending_own, own_shown, own_series}` built by
`reflag_computed` for pending lanes (at the paste and on every toggle),
the pasted painters / readout / range read them, the toggles and mode
include pending lanes, `merge_records` feeds the flagged own samples and
gives the merged `ChanView` / `DigFlags` the lane's flags; pending lanes
show the status icons. Test `pending_lanes_carry_their_flags_into_the_merge`.
Residual: a pasted channel's k (ch+0xf0) taken as 1; the DC of NaN own
samples taken over them as 0 (the mean `0x428795` over raw values with
NaN ND).

### L9.137 — Channels' own frequency vectors and the ОМП's frequency-tracked phasor  ·  ✅
Read (subagent reports; by hand the merge's `0x47e4b5`–`0x47e4f2` and
`0x428490`'s ch+0x20 fill): a pasted channel's ch+0x50 is its record's
`net_frequency` (`0x424cb0` at `0x425002`, as many as `num_values` by
`_wtoi`, zero-filled); «Объединить» gives every merged analog channel one
— at the fit's source sample j its own value, else the record's track
rec+0x1fc[j], else 0. The ОМП phasor `0x477280` (called per slot per
sample by `0x497090`): the fixed path (ours, window [i−N, i−1]) unless the
slot's channel has a non-empty ch+0x50 (`0x42f250`, per slot, by the name
in the station's properties) — then it integrates backwards from i over
one measured period: Δt from the record's µs times (`0x454b40`), the term
(−sin θ·x·Δt, cos θ·x·Δt) at the phase before the step, phase += Δt·f(s −
1) with f its own value / the track / 0, the last sample re-weighted by
(1 − phase before) / f(s_L), the sum × 2 / mean(1/f). So after a merge
(every channel then has one) — the double-ended method's normal case —
the ОМП runs tracked. The parallel-line I0 looks up the station's own
"I0" key (never set): fixed, as ours.
Ours: no own frequency anywhere, the fixed phasor always. Now
`AnalogChannel::own_freq` (the paste, `merge`), `omp::{omp_phasors_tracked,
Slot, PhasorFn}`, `SingleEnded` / `DoubleEnded::phasor`, and the ОМП run
choosing tracked per slot by its channel's vector. Tests
`tracked_phasor_at_a_constant_frequency` (at a constant f with exact steps
the same magnitude as the fixed window ending at i, 2π/N behind — the
rotation cancels in the ОМП's ratios when every channel is tracked),
`merge_pads_and_shifts_the_later_record`.
Residual: (the ОМП's name lookup: L9.138); the record's µs times taken as
round(t·1e6) (the loaders' rounding of rec+0x208 ND); whether the merge
pads ch+0x50 with the values (ours indexes the padded lane) ND; the
`.set` NetFreq round trip (`0x427bc0` / `0x427af0`) not modelled.

### L9.138 — The ОМП's channel lookup ignores case; the merge's beep  ·  ✅
Read (by hand `0x42edf0`, the ОМП engine's lookup; `0x42f250` per the
L9.137 report; `0x47e910`): every channel whose name matches by
CompareNoCase (mfc #2954) is a candidate, one matching by Compare (#2948)
is preferred, else the first; none → the engine's error 354 (0x162); the
ОМП's tracked-mode choice looks the same way. «Объединить» ends with
MessageBeep(MB_OK).
Ours: exact names only; no beep. Now `omp_ui::find_chan` (file channels,
then computed ones; Unicode case folding — the app runs in the Russian
locale, L9.68) for the samples and the own-frequency choice;
`merge_records` beeps on Windows (`windows-sys` Diagnostics_Debug /
WindowsAndMessaging). Test
`channel_lookup_ignores_case_and_prefers_the_exact_name`.
Residual: the beep not run on the VM (no audio check possible); other
platforms silent.

### L9.139 — ↑ / ↓ on a horizontal визир never placed  ·  ✅
Read (by hand, the view's WM_KEYDOWN `0x45f55e`–`0x45f5ed`): ↑ takes the
ГВ's y from `0x4944a0` minus 1 and clamps it at 0 (`cmovs`), ↓ plus 1;
`0x495b40` then sets the ГВ (shown) from that y. A ГВ never placed is on
lane −1, for which `0x4944a0` gives −1 — so either key puts it at y = 0,
the view's top, on the lane there.
Ours: nothing happened for a ГВ never placed, and ↑ was not clamped.
Now the key block takes y ∓ 1 (↑ clamped at the view's top) or the top
for one never placed. Test `arrow_keys_move_v1_and_ctrl_v2_by_one_sample`.

### L9.140 — The merge's padding read through the accessor for every channel  ·  ✅
Read (L9.120: the zeros are real samples of each channel and the fit /
pick read them through `0x426020`; L9.134 / L9.136: digital and pending
channels' flags go through the accessor too): an inverted digital
channel's padding reads 1, a pending analog channel's −(0 − DC) negated
— front and back padding alike.
Ours padded base digital and all pasted channels with a raw 0 / off
(only base analog channels had their accessor value, L9.120). Now
`merge::merge(.., dig_pads)` and `Pasted::pad` (from the pending lane's
flags in `merge_records`), used for the front and the trailing pads.
Test `merge_pads_and_shifts_the_later_record`.

### L9.141 — A `.cfg` record's times as the readers build them; a trailing ^Z line  ·  ✅
Read (by hand: the ASCII reader `0x451800`, BINARY `0x451be0`, binary32
`0x4520f0`, the tokenizer `0x476d60` / `0x4771f0` / `0x477050`, the
duration `0x44c0b0`): the `.dat` is memory-mapped; the ASCII reader
takes each row's second token (the timestamp, as an integer) × the
`.cfg`'s multiplier rec+0x230, rounded to whole µs, as that row's time —
whatever the rate lines say; BINARY / binary32 give record k the time
round(k · 1e6 / R × multiplier) with the integer rate R = rec+0x228, the
record's timestamp × multiplier only when R is 0 (then R from the
second time, L9.132). The tokenizer skips tab / LF / CR / space and ends
a token at tab / LF / CR / comma; after the two leading tokens a row
whose cursor reached the end is dropped (`0x45189b`) — a trailing DOS
EOF (^Z) line adds no row. The duration is the first non-pasted
channel's last time minus its first.
Ours built every `.cfg` record's times from the rate lines (piecewise for
several), and read a trailing ^Z line as an extra row of zeros (DOC_1999:
11001 samples, «Длительность 11 с.»; now 11000 and 10.999 с, as
WIN_1999 already showed). Now `Oscillogram::time_mult`, the ASCII times
from the timestamps, the binary times from R (timestamps at R = 0), a
row needing something after its timestamp. Test
`record_times_as_the_readers_build_them`; real-records render pass
(BINARY / WIN_1999 unchanged, DOC_1999 as above).
Residual: a mid-file short row shifts the original's stream of tokens
(ours drops just that row's values — ND which rows such files have).

### L9.142 — N and the correction's rate from rec+0x228; the `.set`'s DiscretisationFrequensy  ·  ✅
Read (subagent report; by hand the RMS's N at `0x427315`–`0x427351`):
every signal function (a0 `0x426c90`, fault `0x427670`, DFT `0x426fc0`,
RMS `0x4272a0`, harmonic `0x426240`, the cache / DC mean `0x428490`)
takes N = round(own rate / 50) for a channel with an own rate (ch+0x108),
else rec+0x228 div trunc(f) — the integer rate, never the first rate line
(rec+0x218 is not read); the correction's P is rec+0x228 too
(`0x4267e0`). The `.set` writes `DiscretisationFrequensy` = rec+0x228
(`%i`) and on load stores it into rec+0x228 only (`0x44f72c`): nothing
retimed, the track not rebuilt.
Ours took N and the correction's rate from the first rate line (1234 Hz
at 50 Hz: 24 instead of 25; 1000 Hz at 60 Hz: 16 instead of 17; a
two-rate `.cfg`: the first line instead of the snapped mean), the merge
sync and the formula dialog's harmonic list likewise; the `.set` wrote
the rounded first rate and retimed the record on load. Now
`Oscillogram::int_rate_hz` (rec+0x228, the first rate when 0) behind
`samples_per_cycle`, `freqcorr::of`, the merge sync's R and the formula
dialog; the `.set` writes / reads `rate_r` only. Tests
`record_times_as_the_readers_build_them` (N 32 / 25 / 17),
`reads_a_file_in_the_original_layout_and_the_legacy_ini`.
Residual: (the per-channel `.set` keys: L9.143).

### L9.143 — The `.set` per channel: every channel's whole ShowFlags, NetFreq, computed flags after the evaluation  ·  ✅
Read (subagent report on the save `0x44cbc0`, the writer `0x427bc0`, the
reader `0x427d20` / `0x427af0`, the load `0x44f5b0`): every channel of
the vector — analog, computed (inserted after the analog ones) and
digital — gets a section with `ShowFlags` = the whole ch+0x60 (no mask),
the multipliers, `AliasName`, the colour keys and, for an analog channel
with an own frequency vector, `NetFreq` (`%g` joined by ","); on load the
file channels' sections are read first, then `CalculatedChannels` is
evaluated, then the new computed channels' sections are read — their
invert / DC / mode applied after every formula ran, the caches rebuilt
without re-evaluating; `NetFreq`, when not empty, resizes the vector to
the channel's count and fills it from the `_wtof` pieces.
Ours stored a computed channel's visibility / diagrams / stats rows only
and a digital channel's visibility only (their invert / DC / mode were
lost), and no `NetFreq`. Now `computed_flags` / `digital_flags` carry the
mode bits (0x80 by default), DC and invert, read back into `Computed` /
`DigFlags` (`reflag_computed` after the evaluation, not re-evaluating);
`recset::{write_net_freq, read_net_freq}` for analog channels. Tests
`settings_survive_save_and_reopen`, `net_freq_round_trip`.
Residual: the multipliers / alias / colour of digital and computed
channels written as ours (1, "", the computed colour) — the original
writes their fields (ND what they hold); `colorCustom` written as 0 when
not custom (the original writes ch+0x144 always).

### L9.144 — Context menus: MFC's separators and check column, Paste with pending channels, no vector / frequency menu without a document, «Цвет канала» from black, the circle list without pending channels  ·  ✅
Read (subagent audit of every popup — LoadMenuW for 387 / 388 / 561 / 563
/ 351 / 355, 560 by the same handler, the context-menu manager's
ShowPopupMenu; by hand MFC's copy into the popup `0x100e2fdd`–
`0x100e3010`): a separator is dropped when it comes first, follows
another one or comes last (per submenu); 561's Paste (`0x45bf88`) is
disabled while any channel is pending; the vector (`0x48fc80`) and
frequency (`0x46f2e0`) windows show no menu without a document; «Цвет
канала» (`0x4310b0`) always opens the colour dialog, from the first
selected channel's colour or black; the circle's «Каналы» (`0x43d2a5`)
leaves pending channels out. Every label, order, id and check rule of the
seven menus otherwise matched ours.
Ours drew every separator (typical records: the circle / differential /
«Отсчет углов» lists ended in one), egui check boxes and radio buttons
(an empty box for an unchecked item — MFC draws a tick / bullet in a left
column only when checked, INFERRED from the stock drawing), enabled 561's
Paste with pending channels, opened the vector / frequency menus without
a document, did nothing for «Цвет канала» with no selection, listed
pending channels (unchecked) on the circle. Now `popmenu::{shown_seps,
mark_item, glyph}` used by the popups and by the view's menus
(`menu_item` / `menu_check` / `menu_radio`), and the four behaviours.
`FV_WINDOW_BUTTON=right` added to `window_dump` (a synthetic right click
there does not open the menus yet — the tests do). Tests
`separators_as_mfc_copies_them`, `menu_rows_tick_only_checked_items`.
Residual: (keyboard in the popups: L9.146); the inline rename edit
has no system edit menu; the submenu arrow drawn as text; whether
computed channels block 33035's combine in the original ND.

### L9.145 — Formulas may name digital channels  ·  ✅
Read (by hand: the lookup `0x42edf0` walks the whole channel vector with
no kind test — analog, then computed, then digital; the accessors'
digital branches: the value `0x4260db` (x·k, inverted 1 where 0), RMS
`0x4272ae`, a0 `0x426c9b`, harmonic `0x426274`, fault `0x42767e` return
early, the phasor `0x428ac4` gives 0 + 0j): a formula naming a digital
channel reads its 1 / 0 state (inverted with the flag) and 0 from every
period function.
Ours did not resolve digital names (the identifier was dropped). Now
`OscChannels` appends the digital channels after the computed ones
(`digital_from`), their period functions and phasor 0, and `Doc::flagged`
carries the digital invert. Test `formulas_read_digital_channels`.
Residual: k (ch+0xf0) of a digital channel taken as 1.

### L9.146 — Keyboard in the diagram windows' popup menus  ·  ✅
Read (INFERRED from the stock Win32 / CMFCPopupMenu behaviour — FastView
shows every context menu through the context-menu manager's
ShowPopupMenu, L9.144): a popup opened by the mouse has no item
highlighted; ↓ / ↑ move over the enabled items and submenus (separators
and disabled items skipped, wrapping), → or Enter opens a submenu on its
first item, ← or Esc leaves it, Enter picks, Esc closes.
Ours: the diagram popups were mouse-only. Now `PopMenu::{hot, sub_hot,
in_sub}`, `popmenu::step`, `mark_item_hot`. Test
`popups_follow_the_keyboard`.
Then the view's own menus (561 / 563 / 560) too: `lanes::MenuKeys` counts
the rows each frame (`menu_row`), ↓ / ↑ / Enter as above, the view's
визир keys held off while a menu is open. Test
`view_menus_follow_the_keyboard`.

### L9.147 — The status bar as read: the H panes' formats, the panes without a document, no preview page text  ·  ✅
Read (subagent report; by hand the strings 3171 / 3173 «Гор. визир N: %4g
%s», 3429 «Разница г. визиров: %g %s%s», 3426 / 3430): a
CMFCRibbonStatusBar (`0x4729b0`) with six static panes and the extended
«НТЦ Механотроника», nothing else (no message pane); the setter
`0x472e10` caches the six texts and nothing ever resets them — before
the first document they read the creation texts (pane 2 «Разница в.
визиров:», 3426), after the last one closes the last document's; the H
panes print the stored value as `%4g` (unscaled, padded to 4) and the
plain Russian unit (none in ADC mode); the H difference is SI-prefixed
`%g` + " " + prefix + unit, in ADC mode rounded half up with no unit; ms
values are whole µs rounded half up (`0x411bf0`); in absolute mode a
start time without ':' leaves the shown V panes empty; prompts and the
preview's page number go to the frame's message bar = the ribbon status
bar's window text, which nothing shows (no WM_SETTEXT handler in its
maps, SetInformation never called — INFERRED from that).
Ours SI-scaled and padded the H panes («57.92    кВ»), showed «отсч.»,
reset to the captions (pane 2 3430) with no document, rounded ms away
from zero, printed the preview's page in pane 0 (L9.37's guess). Now
`cursor_status` → `Option`, `App::status_panes` kept across documents,
the H formats as read, the ms rounding, the blank absolute panes; the
preview's page text is no longer shown. Tests
`cursor_status_horizontal_panes`, `cursor_status_absolute_time`.
Residual: the pane widths and the extended element's colour ND; the
setter's "no lane at y" path (value 0, empty unit) ND whether reached.

### L9.148 — Dialog data exchange as MFC 12 does it; DIALOG 477 / 554's OK; Enter and Esc  ·  ✅
Read (subagent audit of every DoDataExchange — slot 66, 14 overriding
classes — and the DDX / DDV exports; by hand MFC's int helper
`0x101e0e79`: GetWindowTextW of 64 characters, then `_snwscanf_s` "%d"):
an int field takes leading blanks, a sign and digits and ignores the rest
("50.5" → 50, "12 Гц" → 12); only text with no leading number fails
(61712); a double shows `%.*g` at 15 digits; DIALOG 392 has no DDV;
DIALOG 477 (coefficients): a commit dirties only a text that differs and
writes it into every selected cell of both columns, OK with nothing
dirty is Cancel, otherwise the values are stored and the caller marks
the document modified; the cell edit is a plain CEdit (Enter = IDOK,
Esc = IDCANCEL of the whole dialog); DIALOG 554 (pairs): every OK marks
the document modified; the dialogs' default buttons (Enter) and Cancel
(Esc) as IsDialogMessage gives them.
Ours parsed ints strictly, showed «Частота сети» with `{}`, made 477
dirty on any commit, filled only the edited column, marked modified only
when values changed, cancelled just the cell on Esc, did not mark
modified on 554's OK, and had no Enter / Esc in 353 / 554 / 510 / 392.
Now `dialogs::ddx_int` (392, 353), `views::fmt_gp` (`%.15g`), the 477 and
554 rules, `App::dialog_keys` (not while a message box is up; a box takes
its keys from its second frame, consuming them, so the key that raised
it is not also its answer). Tests `ddx_int_as_scanf`,
`frequency_dialog_exchange_as_read`, `coefficient_bulk_entry_fills_the_shift_range`,
`coefficients_ok_without_edits_changes_nothing`,
`dialogs_take_enter_and_escape`.
Residual: ours still refuses a 0 / negative rate in 392 (deliberate —
what follows such a rate is ND); whether the edited cell of 477 is
always selected ND. Then CDataExchange::Fail's focus: after the message
box the refused field (392's rate, 353's periods / harmonics) takes the
focus with its text selected (`dialogs::ddx_edit`,
`freq_exchange_field`); asserted in `dialogs_take_enter_and_escape`.

### L9.149 — MFC's pane menu on a diagram pane's caption  ·  ✅
Read (subagent report on the frame's OnCreate `0x4728d0` — no
EnablePaneMenu, the dock-site menu flag `0x102abeb0` left 0 by the Office
2007 managers, so no caption menu button and no pane-list menu —, and on
CDockablePane / CPane::OnContextMenu → OnShowControlBarMenu `0x100ce72c`):
a right-click on a pane's caption — docked, a floating mini-frame's, a
slid-out auto-hidden one's — opens MFC's pane menu at the point:
«Плавающее» (17039), «Закрепленное» (17040), «В закладке» (17088),
«Автоматически скрывать» (17041), «Скрыть» (17042); with FastView's pane
style 0xF: auto-hidden greys Плавающее / Закрепленное / Скрыть and checks
Автоматически скрывать, floating greys Автоматически скрывать and
checks Плавающее, docked checks Закрепленное; В закладке is always
greyed (CanBeTabbedDocument is never set). Плавающее on a docked group
floats its active tab alone at its recent rect (`0x100ceb39`–
`0x100cec00`), unlike the caption double-click; Закрепленное docks a
floating one back; Автоматически скрывать toggles auto-hide; Скрыть is
the close button. No menu anywhere else (strips, the parameters bar,
empty dock areas, document tabs; the diagram windows' own menus inside).
Ours had no caption menu. Now `CaptionAct::menu`, `App::pane_menu` and
`pane_menu_frame` (a `PopMenu`, the slide-out held while it is up, the
view's визир keys held off while any popup is up). Test
`the_pane_menu_floats_and_docks`.
Residual: a right-click on a group's tab strip (INFERRED to reach the
group's menu — not wired); Shift+F10 / the Menu key; the `&` mnemonics
of the MFC strings not drawn.

### L9.150 — The caption row: the ribbon's, inside the window's frame  ·  ✅
Read (subagent report; FastView `0x4721f0` constructs its
CMFCRibbonBarEx with bReplaceFrameCaption = 1; mfc120u
`CFrameImpl::IsOwnerDrawCaption` `0x100a05aa`, FastView's Office 2007
manager's slot 127 `0x475d80` = CanDrawImage() && !DwmIsCompositionEnabled;
`CMFCRibbonBar::RecalcLayout` `0x100f98be` extends the DWM frame by the
caption height when composition is on, for every look; FastView's only
caption override, slot 181 `0x471f30`, forces the glass title text
black): on Windows with composition (always on since 8) the caption is
DWM's frame with the system's minimise / maximise / close, and the
ribbon paints its own caption row in it — the application button, the
quick-access toolbar and the window title («Механотроника FastView -
<doc>», `0x45f7d0`) centred on its width in the free space between the
toolbar and the buttons (Aqua: left), black, one line with an ellipsis;
the style's owner-drawn caption, border and button images appear only
without composition (XP, or Vista / 7 Basic).
Ours kept the OS title bar above our own quick-access row (two rows),
with «Стиль» and «?» in that row. Now the window has no system
decorations (unless `FV_SYSTEM_TITLEBAR` is set) and row 1 is the
caption row: the title centred in the free space, which drags the window
(double-click maximises / restores), the three system buttons drawn as
Windows 10 / 11 draw them (close red when hot), 5 px resize edges (12 px
corners) while not maximised; «Стиль» / «?» on the right of the tab row
(spec §2.5). `App::own_caption`, `ribbon::caption_buttons`,
`App::resize_edges`; `FV_WINDOW_CAPTION=1` renders it. Rendered Office
2007 Blue and Windows 2000.
The Aqua look left-aligns the title (its TextCenter off — the glass path
taking it too is INFERRED).
Residual: the QAT's glass frame
(RIBBON_CAPTION_QA_GLASS), the DWM drop shadow / Aero snap of a real
frame, the maximised offset (SM_CYFRAME / 2 + 2) not reproduced; the
title's glow on glass; on a tiling window manager the buttons may do
nothing (the WM owns the window) — `FV_SYSTEM_TITLEBAR` restores the
system frame.

### L9.151 — «Быстрый анализ»: fresh pages on every open, defaults by name, K0 «0.», Enter  ·  ✅
Read (subagent audit of the sheet `0x48b970` and every CQAn* page — the
slots 110–119 are output getters, «Рассчитать» `0x4167f0` already matched
(L9.51–L9.56, L9.69 / L9.70); by hand the defaults `0x412688`–`0x4126e2`):
the pages are created on every DoModal — defaults first (`0x411f40`),
then the `.set`; each combo is filled with the record's channels and
preselected by its channel's name ignoring case (CB_FINDSTRINGEXACT:
Ia…Uc, Uab / Ubc, 3I0 / 3U0), a name the record lacks inserting «Канал
отсутствует» at the top (3Ion «Не учитывать»), the unnamed Uни / Uик on
the first channel; K0 / Ф0 / Km «0.» (`0x4c3e98`), R / X «1.»; an edit
counts by its text's length (blanks fill it, `0x48b360`); the combo's
selected text is what the `.set` stores, the placeholder too; Enter is
IDOK = «Рассчитать» (INFERRED from the dialog manager).
Ours kept the pages across opens (even across records — a stale name
kept «Рассчитать» enabled), picked the phases by our detection with blank
combos when missing, left Uab / Ubc / Uни / Uик empty, defaulted K0 to
«1.», trimmed blanks, and had no Enter. Now `Command::QuickAnalysis`
resets `App::quick`, `quick_defaults` as read (the placeholder listed in
the combo while selected), K0 «0.», the length rule, Enter. Test
`quick_analysis_pages_start_from_defaults`; VIEWS_SPEC's defaults line
corrected.
Residual: the combo lists also hold formula-editor rows without a
channel in the original (ours file and computed channels); whether
doc+0x1d8 counts computed channels ND.

### L9.152 — The pair lists: rows in «Каналы», stale pairs purged; the hand-over restores a minimised window  ·  ✅
Read (subagent gap sweep over every message-map handler — 192 uncited,
~183 already covered in prose; by hand `0x4554e0`): DIALOG 463 lists,
between the analog and the discrete channels, one row per differential
pair (`doc+0x878`, «Iторм | Iдиф», " | " `0x4c65d0`) and one per R/X pair
(`doc+0x884`, «R | X») (`0x433b00`–`0x433d6a`); column 5 flips a
differential pair's enabled byte, column 4 an R/X pair's (`0x4349c0`),
the header toggles them with the channels, then the diagrams are rebound
(`0x44c090`) and the document marked modified; the circle / differential
painters and 33027 purge every pair naming a channel the record no
longer has (analog and computed names, Compare — case-sensitive;
`0x4554e0` R/X, `0x455210` differential); WM_COPYDATA `0x475530` restores
an iconic frame before bringing it forward.
Ours had no pair rows (the spec mislabelled the two lists «digital
groups»), kept stale pairs in the menus and the `.set`, and only focused
the window. Now `chandlg::{PairRow, DlgRow, dlg_rows, pair_name,
pair_cell, toggle_pair_cell}` (the header toggle over them),
`Doc::purge_pairs` (the plane paint and 33027), `Minimized(false)` before
`Focus`. Tests `channels_dialog_lists_the_pairs`, `stale_pairs_are_purged`.
Spec §7.1 corrected.
Residual (from the sweep): the circle / differential parameter grid is
ours (the original: one MFC property grid — «Тип блока» row, a group row
per characteristic with a framed colour box ticked when visible and
toggled by a click, `0x407dc0` / `0x4075c0`); (the grid's splitter
height `Properties Height` is now kept per diagram in our settings — read
as the panel's default, written when it changes); 32806 with no document stays
enabled-looking in the original (INFERRED; ours greys it).

### L9.153 — The circle / differential parameter grid as the original's property grid  ·  ✅
Read (gap sweep, by hand `0x4076fd`): the panel under the circle and the
differential planes is one MFC property grid (`BlockPropertyGrid`) with
no header, text colour (40, 40, 40) and the name column a third of the
width. Property 0 is «Тип блока» (string 3483, loaded at `0x4076fd` in
`0x407640`), a combo of the library's blocks (`0x407cc0` rebuilds the
grid on a change); then one group per characteristic (`0x407750`), its
parameters as text properties under it. The group row's draw override
(`0x407dc0`) paints a 12 × 12 box near the right edge, framed with the
characteristic's palette colour, with a blue tick while the
characteristic is shown; a click in that row's value area (`0x4075c0`)
flips the visibility and repaints the plane.
Ours was an egui list (colour dot + checkbox per characteristic), a
separate collapsible settings grid with «Характеристики» / «Уставки»
headings and a «Сбросить уставки» button the original does not have.
Now `char_side_panel` draws the one grid: the «Тип блока» row with the
block combo, a banded bold group row per characteristic with its expand
box and the framed tick box (a value-area click toggles and persists the
visibility), the parameter rows as frameless edits; leaving an edit
re-evaluates the block. The reset button is gone.
Residual: (the expand state and the clicks: L9.156).

### L9.154 — Dialog keys and defaults from the vtable sweep: «Каналы» has no OK, pairs open on the first channel, the finder's default button, Enter in a formula form  ·  ✅
Read (vtable-override sweep over the dialog / quick-analysis classes; by
hand `0x436030`, `0x4216b0`, `0x401c40`, `0x403530`, the DIALOG 463 / 554
templates): DIALOG 463 holds only the list (id 1013) — no OK; its OnOK
`0x436030` is a bare jump to the rename commit `0x435c80` (nothing
without a running rename), OnCancel is MFC's, so Enter only commits a
rename and Esc closes the dialog, a running rename dropped (only a click
elsewhere commits). DIALOG 554: both combos are filled (kind-0 channels,
unsorted) and set to their first item (`0x4216b0`, CB_SETCURSEL 0), so
«Добавить» is usable at once; «Добавить» sits at the right under the
combos, «Удалить» at the right under the list, OK / Отмена at the bottom
right. The finder (DIALOG 558): «Открыть файлы» is IDOK and never greyed
— OnOK `0x46e120` stops the scan, opens every selected file and closes
(nothing selected: just closes); OnCancel `0x46e2b0`; the finder only
enables / disables its list, so «Удалить файлы» is never greyed either
(an empty delete: NOT DETERMINED, ours does nothing). «Добавить новую
формулу»: the form templates lack DS_CONTROL, so Enter / Esc in a form
field reach the form — FormulaWnd::OnOK `0x401c40` calls the parent's
OnOK and then EndDialog(IDOK) whatever it did, OnCancel `0x485d10` ends
it with IDCANCEL; the parent's OnOK `0x403530` returns after a refused
member check before the formula member (`+0x1c8`) is built, and builds it
before `CDialog::OnOK`'s second DDX; the caller `0x486df0` appends a row
from the two members on any IDOK. So a taken name → 473, the dialog
closes, a row with that name and an empty formula; an empty or digit-led
name → 396 twice, a row with the proposal and the built formula.
Already covered (no change): the quick-analysis sheet and pages (no
OnSetActive / OnApply / timers; nothing recalculates while the modal
sheet is up), the coefficient, print, ОМП and converter overrides.
Ours had an OK in «Каналы» (Esc cancelled only a rename), empty pair
combos, both finder buttons greyed until a selection and no Enter / Esc,
and Enter in a form field stayed open on a refusal. Now
`channels_window` (no OK; Esc via `dialog_keys` closes), `CreatePairs`
seeds both combos with the first channel and the template's button
places, `finder_window` (Enter = open the selection and close, Esc =
close, neither button greyed), `AddFormulaDlg::{formula, form_focus,
accept_from_form}` (the alerts move to the app queue as the dialog
closes). Tests `channels_enter_commits_esc_closes`,
`pair_dialog_opens_on_the_first_channel`, `enter_in_the_form_always_closes`.
Residual: whether doc+0x1d8 counts computed channels for the pair
combos (as L9.151); the sheet's OnActivatePage `0x48c210` skips the base
(the side bar would not follow a keyboard page switch — ours has none).

### L9.155 — Alt alone never reaches the ribbon: keytips by F10 only  ·  ✅
Read (vtable sweep of CFastViewApp; by hand `0x448e80`): the
application's PreTranslateMessage override discards every WM_SYSKEYDOWN
and WM_SYSKEYUP whose key is VK_MENU (returns TRUE) and passes the rest
to the base. MFC's ribbon Alt handling (the 200 ms hold timer 0xec1a, the
tap that focuses the ribbon, the release that hands the focus back) sits
in CMDIFrameWndEx::PreTranslateMessage (`0x100baa9b` → `0x100ff240` /
`0x100ff383`), which runs only after the application's filter — so an
Alt tap or hold shows nothing; F10 (another key) still toggles the
keytips; Alt+letter / Alt+F4 pass (other keys); the Alt state seen by
the wheel (L1, `0x45d990`) is the key state, unaffected.
Ours showed the keytips on an Alt hold and toggled them on a tap (L9.90
read MFC's code without the app's filter). Now `Keytips` has no Alt path
(`alt`, the hold timer and `by_hold` gone; `f10` toggles);
`keytips_input` no longer looks at Alt. Test `f10_toggles` replaces
`alt_tap_hold_and_f10`.

### L9.156 — The parameter grid's clicks: the document marked modified, double-clicks dropped, groups re-expanded on a rebuild  ·  ✅
Read (vtable sweep of BlockPropertyGrid / BlockProperty; by hand
`0x407c40`, `0x4075c0`): a click whose hit-test is the value area calls
`0x407c40`, which flips the row's characteristic (when it has one), then
SetModifiedFlag(TRUE) on the active frame's document — also for «Тип
блока»'s value, which has no characteristic — repaints the plane and
saves the block state (DiagramWnd slot 94; into the document's `.set`: L9.162). A
committed parameter edit (`0x407580`) recomputes and saves without the
modified mark. The grid's WM_LBUTTONDBLCLK handler `0x407640` drops a
double-click on any BlockProperty row (every group and «Тип блока»: no
expand / collapse, no second toggle) and passes one elsewhere on as a
plain button-down (no MFC double-click edit). The rebuild `0x407750`
(at init and on a block change, `0x407cc0`) selects «Тип блока», deletes
the other rows, adds each characteristic as a plain group and ends with
ExpandAll(TRUE) (`0x407aaa`) — every group starts expanded, nothing
keeps a collapse. Keys are MFC's (Enter on a group toggles it, ± / arrows
navigate; F4 or typing edits) — ours has no row selection (residual).
Ours did not mark the document, toggled twice on a double-click (egui
reports both clicks) and kept a collapse across block changes. Now
`char_side_panel`: a value-area click (and a click on the block combo)
sets `set.modified`; group-row clicks act only when not the second of a
double-click; a block change clears the groups' open state. Test
`grid_group_click_toggles_once_and_marks_modified`.

### L9.157 — Deleting channels sweeps the review list  ·  ✅
Read (vtable sweep: AreaItem `0x4044c0` / LevelItem `0x471740` validity
slots, MomentItem `0x43dd60`; by hand `0x42ecf0`, `0x440800`, `0x4044c0`,
the callers): the remove-channel-by-name routine `0x42ecf0` ends by
sweeping the review list at `doc+0x840` (`0x440800`): every element whose
validity slot answers false is erased and the hot element (`doc+0x854`)
reset. A level is valid while its channel resolves by name, an area while
both of its channels do (the lookup `0x42edf0` throws otherwise); a moment
always. Callers: the ОМП clear `0x4a17a0` (four calls), the formula
page's DDX `0x4863a0`, `0x4156e0` — every formula channel, called by the
`.set` reader at `0x44f732` (32860; at an open there are none yet) and by
«Загрузить» at `0x414fb5` —, and the quick analysis's «Рассчитать»
`0x4167f0` at `0x416f1b`, which deletes the channels it has just added
when the recompute `0x415780` fails.
Ours kept every element, and ours binds them by row index — so a deleted
computed or digital channel left the levels / areas after it pointing at
the wrong channel. Now `Doc::removing_channels` (names before, the
removal, re-bound by name after; a level / area whose channel went, and
every element kept from a `.set` naming a missing channel, dropped;
`comment_hot` cleared; nothing when no channel went) wraps `fp_update`,
`fp_load_rows`, `omp_clear` and the 32860 formula drop. Test
`removing_a_channel_sweeps_the_review_list`.
Residual: the quick analysis's rollback of its new channels when the
recompute fails (`0x416ed6`–`0x416f2b`) is not modelled.

### L9.158 — The open and Save As dialogs as `CDocManagerEx::DoPromptFileName`  ·  ✅
Read (vtable sweep: CDocManagerEx overrides only DoPromptFileName, slot
15; by hand `0x447e30`, strings 328 / 394 / 399 / 569): opening always
uses string 328 — six rows: «Осциллограммы (*.cfg, *.cff, *.osc,
*.sto)», COMTRADE (*.cfg), COMTRADE'13 (*.cff), OSC2 (*.osc),
Configurator-MT (*.sto), «Все файлы (*.*)» (no `.dat`). Save As picks by
the record path's last four characters: `.osc` → 394 (COMTRADE *.cfg +
OSC2 *.osc), `.zip` → 399 (ZIP only), anything else — `.cfg`, `.cff`,
`.sto` — → 569 (COMTRADE *.cfg + COMTRADE'13 *.cff). The proposed name
is the record's folder and its title without extension (`.osc` / `.sto`
swapped for `.cfg` first, then stripped — the strip INFERRED from the
title helper's last-dot search); overwrite prompt, read-only hidden, the
title argument ignored (the system's default title, INFERRED). After OK
the chosen filter's extension is applied: one of the list typed but not
the filter's is replaced, any other name gets it appended ("x.txt" →
"x.txt.cfg"). ZIP: OnOpenDocument `0x44ee70` never compares `.zip` and
ZipArchive is imported for create / add / close only — a `.zip` cannot
be opened, so 399 and the archive writer `0x44db10` (`<base>.osc` +
`<base>.set`, or a temporary «<zip>.files» folder with `<base>.cfg`) are
unreachable from the UI (INFERRED).
Ours offered one «Осциллограммы» row with `.dat` and no `.sto` when
opening, and in Save As «COMTRADE» + «COMTRADE'13» (+ OSC for an `.osc`),
"name.cfg" with no folder, and refused a name without a known extension.
Now `open_comtrade` lists 328's rows (`.sto` opens to our reader's error,
⛔ as before); `save_filters` (394 / 569), the record's folder and title,
`save_target` (a listed extension kept, else appended). The dialog here
does not report the chosen filter: a listed extension is kept as typed
and the first filter's is appended otherwise. Test
`save_as_filters_and_extension`.

### L9.159 — A file already open is brought forward; a right-click on a document tab activates it; dropped files  ·  ✅ (drop position residual)
Read (vtable sweep of CFastViewApp / CMainFrame / CDocManagerEx /
COleDropTargetEx): no own document template and CDocManagerEx overrides
only DoPromptFileName, so every open (dialog, MRU, a drop, the shell /
WM_COPYDATA hand-over) is MFC's OpenDocumentFile — a file already open
(AfxComparePath) just activates its frame, nothing reloads (INFERRED from
MFC, the stock path). CMainFrame keeps OnShowMDITabContextMenu (slot 117,
`0x100bc637`) and has no WM_CONTEXTMENU entry: the frame's OnContextMenu
(mfc `0x100bc085`–`0x100bc1b0`) activates the tab under the point, then
the default shows no menu (it offers «&В закладке» only with the can-dock
bit, not set here); a right-click on the strip's empty part does nothing.
COleDropTargetEx accepts only FastView's private clipboard format
(registered at `0x404f59`): OnDragEnter `0x441ad0` = OnDragOver `0x441b00`
(always "move"), OnDrop returns FALSE without that format
(`0x441620`–`0x441652`) — so a file dragged from the shell over the view
or the frequency / vector / circle panes shows "move" and does nothing;
elsewhere on the frame OLE falls back to WM_DROPFILES (DragAcceptFiles)
→ CFrameWnd::OnDropFiles opens each file (INFERRED).
Ours loaded a second copy of an open file and ignored a right-click on a
tab. Now `load_comtrade_path` activates a document whose path is the same
file (`same_file`: absolute, case ignored on Windows) or ignores one
still loading; a tab's `secondary_clicked` activates it. Tests
`opening_an_open_file_activates_it`,
`a_right_click_on_a_document_tab_activates_it`.
Residual: the drop position — egui reports dropped files without where
they landed, so ours opens a file dropped anywhere (the original ignores
drops over the view and those three panes).

### L9.160 — DIALOG 353 laid out as its template  ·  ✅
Read (the DIALOG 353 template, 226 × 222 DLU; vtable sweep of
FourieDiagramSettingsDlg): top to bottom «Расчет значений» (12,12 204×54)
with «Число обрабатываемых периодов:» / edit 1025 (y 30) above «Число
гармоник:» (right-aligned at x 84) / edit 1022 (y 48); «Значения
величин» (12,72) with «Абсолютные величины» 1024 (y 88) above «В
процентах (нормализовано)» 1023 (y 106); «Нормализация» 1061 (12,132)
with «По основной гармонике» 1026 and «По среднеквадратичному значению»
1027; OK (114,200) / Отмена (168,200) at the bottom right. The
command-routing override `0x46f070` re-enables 1026 / 1027 / 1061 on any
click while 1023 is checked; OnInitDialog `0x46ee20` disables only 1026 /
1027 (the frame stays enabled until the first click — ours greys the
whole group at once, a cosmetic residual).
Ours had «Значения величин» first (percent above absolute), then
«Нормализация», then «Расчет значений» (harmonics above periods), OK /
Отмена at the left. Now `freq_dialog_ui` follows the template.
`FV_WINDOW_STATE=freqdlg` renders it.

### L9.161 — A wheel over the circle / differential pane's toolbar reaches the plane  ·  ✅
Read (vtable sweep, slot 72 of CircleDiagPane / DiffDiagPane /
VectorDiagPane; by hand `0x436220`): the pane's window procedure runs the
base, then posts every WM_MOUSEWHEEL (same wParam / lParam) on to its
diagram window (`pane+0x370`). The pane toolbars (C*DiagToolBar) have no
wheel handler (their only overrides: OnUpdateCmdUI routed to the pane,
`0x471a00`, and AllowShowOnList FALSE, `0x404ba0`), so Windows hands a
wheel over a toolbar to the pane, which forwards it (INFERRED for the
system's parent hand-off) — the plane pans / zooms as if the pointer were
on it; the property grid scrolls itself. (The frequency pane's `0x475b30`
is the frame pane's own procedure, L9.107.)
Ours took the wheel only over the plane. Now `pane_toolbar` records the
circle / differential toolbars' rects (`DiagState::toolbar`) and the
plane takes the wheel over them as well. Test
`a_wheel_over_the_pane_toolbar_reaches_the_plane`.

### L9.162 — The circle / differential block state is the record's (`.set`), «Нет» first  ·  ✅
Read (background report; by hand `0x408cf0`, `0x436b30`, strings at
`0x4c3ba4` / `0x4c3bc0`, string 3306): save `0x408cf0` and load
`0x409030` reach the active document through the main frame and work on
`doc+0x300` — the tree the `.set` reader `0x44f5b0` loads — and do
nothing with no document. The save get-or-creates `Circle Chart` (flag 1)
/ `Diff Chart` (0), **empties it** (keys and subtrees, `0x40f7e0` /
`0x40f830` — SightsOnly / ShowOffCharacteristics / ViewChannelType go
until their toggles write them again), writes `SelectedBlock` = the
block's name and under `Selected Block` one child per characteristic
(`visible` `%i`) with one child per parameter (`value`, the grid's text);
only the current block; then SetModifiedFlag(TRUE) (`+0x6c`). Saved after
a visibility click (`0x407c40`), a parameter commit (`0x407580`, after
the evaluation) and a block change (`0x407cc0` → rebuild → evaluate →
save); a refresh never saves. The refresh (`0x436b30` / `0x442a10`, slot
90, called from the frame's refresh-all `0x475420` on a view's
activation, 32860, the pair rebind, …): no document → nothing; reads
`SelectedBlock` (empty → «Нет», string 3306); a different name than the
current block's rebuilds by that name (an unknown one → block 0); only a
document other than the one last bound (`this+0x98`, never cleared)
loads the block's values (`visible` default 1, `value` default the
library's text) and evaluates. The library loader `0x40c7e0` puts an
empty «Нет» block first and ends on it, so the diagrams start on «Нет»;
32864 ends on «Нет» too. The registry keeps only `Properties Height`.
Ours kept the block state program-wide (`characteristics.tsv`: the
selected block, every characteristic's visibility and values, saved on
every evaluation), never read `SelectedBlock`, had no «Нет» and started on
the stored or first block. Now `plane_library` (+ «Нет» first),
`App::save_block` (`recset::write_block`: the section emptied, then the
block; the document marked modified; no document — nothing), the
refresh `refresh_blocks` / `refresh_live_block` with `diag_bound` (from
`track_settings` on a document's activation, after 32860, and when a
diagram is first shown), `char_side_panel` saving on the three edits;
`persist::Store` lost `selected` / `visible` / `values`. Test
`block_state_is_the_documents`.
Residual: «Тип блока» showing a saved name the library lacks (ours shows
«Нет»); (the circle refresh's ShowFlags sweep: L9.163); the refresh's other callers (open / close, ~25 sites) not all
identified — ours refreshes on activation and 32860.

### L9.163 — The circle refresh takes W / var channels off the circle diagram  ·  ✅
Read (by hand `0x436d77`–`0x436e64`, string 3060 «Ohm», the literal
"VA" at `0x4c5e38`, import #2954 = CompareNoCase `0x1005f756` →
`_wcsicmp`; the rebind's one caller `0x434a9f`): every circle refresh
walks the record's channels and, for each with ShowFlags bit 3 (on the
circle diagram) whose canonical unit compares unequal, ignoring case, to
both "Ohm" and "VA", clears bit 3. The «Каналы» column 4 lets Ohm, W, VA
and var be ticked (`0x434e81`), but every «Каналы» click ends in the
refresh-all (`0x43502b`, the header's `0x435a60`; L9.166), so such a tick
is cleared by the same click; other refresh-alls (a view's activation,
32860) clear one set elsewhere (a `.set`). Ours kept them. Now
`chandlg::circle_refresh_flags`, run by `refresh_blocks` (the activation
and 32860) and after every «Каналы» click.
Test `the_circle_refresh_keeps_ohm_and_va_only`.

### L9.164 — A channel drag scrolls the view at its top and bottom edges  ·  ✅ (left / right ND)
Read (vtable sweep of CFastViewView, CView slots 91 / 92; by hand
`0x468e40`, `0x468f00`): the view overrides OnScroll and OnScrollBy.
OnScroll always allows the scroll; of the vertical code only a line-up
becomes SB_LINEUP — every other code, "no vertical scroll" (0xFF)
included, becomes SB_LINEDOWN —; OnScrollBy sends that as WM_VSCROLL
(the 100 px line of `0x45d6d0`), updates all views, runs the frame's
refresh-all `0x475420` and repaints. MFC calls OnScroll from
CView::OnDragScroll during an OLE drag over the view (the stats panel's
channel drag, L9.21): an 11 px inset of the client rect
(DD_DEFSCROLLINSET), a 50 ms delay, then a step every 50 ms; moving from
one inset to another keeps the timer (INFERRED from MFC). Ours did not
scroll. Now `lanes_ui` steps `vscroll` by a line while a `ChanDrag` rests
in the top (up) or bottom (down) 11 px (`App::drag_scroll`). Test
`a_channel_drag_scrolls_at_the_top_and_bottom_edges`.
Residual: the left / right insets (they would scroll down, by the
mapping) depend on whether the view window has scroll-bar styles — not
determined, ours does not scroll there; the refresh-all at each step is
not run.

**Status and what is left: `notes/COMPLETION_PLAN.md`** (workstreams W1–W6, the progress log, `tools/coverage.py`).

### L9.165 — The start-up pane layout, seen running on the VM  ·  ✅
Run (the original on the Windows VM with no saved workspace, COMPLETION_PLAN
W4.1) and read (by hand `0x474e10`): the panes are created with alignment
styles 0x56004140 (right) / 0x56008240 (Частотная, bottom) and docked in
this order: Векторная (DockPane), Круговая attached to it as a tab
(`+0x350`), Дифференциальная (DockPane), Частотная (DockPane). In the
docking manager's order (L9.109) each later block sits inside the space
the earlier ones left: Векторная | Круговая the outermost right column,
Дифференциальная the column inside it (both 500 wide), Частотная 200 tall
under the oscillogram only — what the capture shows (the differential
diagram on «Нет», L9.162). L9.3's one right-hand group of three with
Частотная beneath was a saved workspace in the user's screenshot.
Ours started from that screenshot. Now `Docks::default` is the start-up
docking (Векторная order 0, its tab Круговая, Дифференциальная order 1,
Частотная bottom order 2); the docking tests start from
`Docks::one_group()`. Test `the_start_up_layout`.

### L9.166 — «Каналы» as read to the last function: group hides, визиры on dissolve, the header's counts, names  ·  ✅
Read (COMPLETION_PLAN W1.1, the eleven uncited functions of the area; by
hand `0x434b1c`–`0x434c71`, `0x43502b`, `0x435a60`):
- WM_DRAWITEM `0x433db0`: rows filled white from x 25, the selection never
  drawn; every name black at x + 26 (top-aligned); a channel whose display
  name differs from its decoded file name (`+0x68` vs `+0x64`, `0x4292e0`)
  gets " (original)" after it in (150,150,150); bitmap 464's images (0
  waveform, 1 spectrum, 2 vector, 3 circle, 4 differential characteristic,
  5 ticked, 6 empty, 7 greyed box, 8 analog, 9 discrete) — row icon at
  x + 5, cells at the column's left + 8; not-applicable cells are image 7;
  column 4 also needs "not bit14"; discrete rows are image 7 in columns
  2–5; the column-5 header is image 4.
- A column-1 hide (`0x434b1c`–`0x434c71`) collects the clicked channel's
  group members by original name (`0x455930`, `0x42f020`), dissolves the
  group and clears visible and selected on every member.
- Dissolving a group `0x455df0` (the hide, 32845–32848 `0x4301ff`,
  Совместить `0x467796`, Разъединить `0x467e30`) resets a horizontal визир
  on a member to "never placed" (hidden, value 0, channel −1).
- `0x42f020` finds a channel by its original file name (exact, else first
  match ignoring case; error 354 otherwise) — groups follow file names.
- The header `0x435080`: column 5 only the differential pairs (all on
  unless all are); columns 1–4 "set < applicable" with the original's
  counts — 2: set = bit1 and not bit14, applicable = every kind-0 row
  (computed and pending included), writes the non-pending ones; 3: set =
  bit2 on any row, applicable = rows passing the vector test; 4: set =
  bit3 and not bit14 plus the R/X pairs on, applicable = Ohm / W / VA / var
  rows regardless of bit14 plus the R/X pairs, writes the non-pending ones
  and every R/X pair; column 1 writes every row, deselects on hide, no
  dissolve. Its tail — and every cell click's (`0x43502b`) — always runs:
  views updated, the refresh-all `0x475420`, the document modified.
- Explained away (library code, no behaviour of their own): `0x431970`
  (a channel record's copy constructor), `0x4588c0` (map insert with a
  hint), `0x4593e0` / `0x458a60` / `0x4330d0` (tree node inserts, "map/set
  too long"), `0x432020` / `0x432270` (tree erase with rebalancing).
Ours hid only the clicked member, never touched the ГВ on a dissolve,
toggled headers by the cells' all-or-nothing rule through the cell path
(dissolving groups), refreshed the circle flags only after a pair click,
greyed computed names, showed no original name and drew a dash and a step
wave. Now `Doc::dissolve_groups` (used by the column-1 hide,
`hide_selected`, `combine_selected`, `split_selected`), `toggle_column`
with the original's counts, `circle_refresh_flags` after every click,
column 4 not for pending channels, `Doc::original_name` drawn grey after
the name, the greyed box and the differential-characteristic glyph.
Tests `hiding_dissolves_the_group_and_deselects` (extended),
`header_clicks_use_the_originals_counts`,
`a_renamed_channel_keeps_its_original_name`.
Residual: row icons and the text's exact places; live column widths;
column 2 for non-pending computed channels (ours has no such flag); the
circle refresh's channel range (the report says file analog channels only,
doc+0x1d8 — the same open count as L9.151; ours also clears computed ones).

### L9.167 — The quick-analysis pages to the last function: the getters, loaders and savers cited; ↑ / ↓ icons  ·  ✅
Read (COMPLETION_PLAN W1.2, sixteen uncited functions): every CQAn* page
has the same virtual layout — slots 110–115 the outputs the generator
`0x48c0e0` reads from the active page (110 formulas, 111 names, 112 / 113
vector / circle flag names, 114 the angle reference, 115 the differential
pairs), 116 the OK check, 117 the `.set` load (from the pages' OnInitDialog
`0x48b690`, after the defaults `0x411f40`), 118 the save (OnOK `0x48b710`,
OnCancel `0x48b770`) — none has a button of its own. `0x4167f0` pairs
names and formulas by index (count = formulas, no bounds check,
`0x416c61`–`0x416c9d`), uniques every name, pair and flag name against the
record before the batch (`0x48bff0`, `0x416baa` / `0x416bd3`) and pushes
pairs without a dedup (`0x417950`). Cited as already described:
- names: 536 `0x48c590` (VIEWS_SPEC «Симметричные»), 539 `0x481d60` (L9.73:
  per arm «Ia плечо k прив.» (+ Ib / Ic), then «… прив. пов.» with
  «Не учитывать» off, «Ia диф.», «Ia торм.» — one «прив.» set whatever the
  reduction radios), 539 pairs `0x483f40` (L9.152);
- loaders / savers: 536 `0x48ce30` / `0x48d290`, 538 `0x487f50` /
  `0x4882c0`, 543 `0x48ac90` / `0x48b060` (L9.55: checks applied as value
  ≠ 0; a radio pair with no key forces its first radio; then the page's
  dependency refreshers and the OK check); 539 `0x484920`
  (groupConnOption), `0x484cb0` (brakeCurrOption), `0x484730`
  (legOption), `0x485040` / `0x485550` (one `Leg_%d`) — L9.79 / L9.87;
- the formula editor's OnInitDialog `0x4860a0` (L9.56): columns 25 % / 70 %
  of the list's width, grid lines, full-row select, bitmap 475 cut into
  16 × 16 icons (transparent C0C0C0) — icon 6 (a filled up-triangle) on ↑
  1018, icon 5 on ↓ 1019; no defaults, no `.set` load (its slots 117 / 118
  are the empty stub `0x403760`);
- `0x432730` is not the editor's: it moves the channel records down one
  when one is erased (the remove-by-name `0x42ecf0`, L9.157, and
  `0x4574f0`).
No message box in any of them. Ours matched names, orders, pairs, keys and
forced radios; it drew «↑» / «↓» as text. Now `formula_page::tri_button`
paints the filled triangles. The L9.87 residual is corrected (above).
Residual: page 537's loader / saver and eight more slot 111–114 getters
are unread (four skimmed match VIEWS_SPEC) — plan W1.2b lists them.

### L9.168 — The parameters bar's close button  ·  ✅
Run (the first VM capture shows a ✕ at the bar's top right) and read (by
hand `0x474db2`–`0x474dea`): CMainFrame creates CParamOscBar (id 810) with
style 0x54000000 and the last Create argument TRUE — MFC's caption bar in
message-bar mode, which draws a close button at the right; a click on it
hides the bar (MFC's caption-bar close, INFERRED), the same state 142
toggles. The other caption bar created just before (`0x474d8d`, hidden
style 0x44000000) is the merge bar (L9.15). Ours had no close button. Now
`param_bar_ui` draws the 16 × 16 ✕ (a frame on hover) and a click hides
the bar. Test `the_parameters_bar_closes_from_its_button`.
Residual: the message bar's own frame / margins as the visual manager
draws them (W4.5).

### L9.169 — The `.omp` file is a settings document; the ОМП toolbar as TOOLBAR 475  ·  ✅
Read (COMPLETION_PLAN W1.4; by hand `0x4a91c0`, the category name at
`0x4d1298` "omp"): «Сохранить» (32877, `0x4a91c0`) builds a fresh settings
tree, get-or-creates the category `omp` (`0x48dc80`), fills it with the
writer `0x4a2620` (clears the node, `ver` "2", `type`, then each station
through `0x4a2740`: `obj_name In Ia … Uc`, `Section{k}`, `child{n}` lines
first) and writes it with the `.set` serializer `0x48e1c0` — `<settings>`,
every node a `category` named by its key. «Загрузить» (32878, `0x4a9010`)
reads with `0x48e470`; with a category it takes `omp` and runs the reader
`0x4a3050` (each station through `0x4a38c0`: `Section{k}` for k up to the
child count, get-or-create — a gap gives an all-empty section, `child{j}`
likewise, a type other than "parLine" a tap); with none, the legacy reader
`0x4a45f0`; never a message. The same reader / writer serve
`WrongPlaceFinding` (the `.set`). The dialog: OnInitDialog `0x4a7c50`
(toolbar 10 px in, tree W/3 − 22 wide, the grid at W/3 with a border, tree
icons from bitmap 476 with magenta transparent, the stored item re-selected),
the toolbar `0x4a7e40` (TOOLBAR 475: combo 1075 for 32870, 32871–32874,
separator, 32876, 32875, separator, 32878, 32877; text only on Участок /
Параллельная линия / Отпайка). The element classes: Station's constructor
`0x4a5880`, Section's `0x4a5330` (defaults and masks as spec §8.4), Section's
"move down" `0x4a57f0` (swap within the same kind's list). Explained away:
`0x411360` (map node erase), `0x4a6350` (vector copy-assignment).
Ours wrote and expected an `<omp>` root with element tags — neither side
could read the other's files — reported load errors, and had a separator
after the combo, «＋ » prefixes and text Загрузить / Сохранить. Now
`recset::omp_file_bytes` / `omp_from_file_bytes` (the settings document,
silent), the toolbar as TOOLBAR 475 with the command strings as tooltips
(egui glyphs for the icons). Tests `omp_file_layout_matches_the_original_writer`,
`omp_file_round_trips`.
Residual: the tree icons (bitmap 476) and the dialog's geometry; reader
gaps (an all-empty section for a missing `Section{k}`); the legacy
readers; the grid's "modified" marking and the initial selection.

### L9.170 — Channel colours by phase, name and place  ·  ✅
Read (COMPLETION_PLAN W1.3; by hand `0x42b520`, the literals of
`0x42b0c0`, the table `0x4c2fe0`): the colour getter `0x42b520` (22 call
sites in the lanes, the four diagrams, legends and tables) returns the
custom colour (ch+0x144) when its flag (ch+0x140) is set; else, for an
analog channel, `0x42b0c0` classes it — the phase field lowered and
compared exactly (a / а → 1, b / б → 2, c / с → 3, ab / аб, bc / бс, ca / са
→ 5 / 6 / 7), else the first of the narrow cp1251 substrings in the
lowered name: uab / uаб, ubc / uбс, uca / uса → 5 / 6 / 7; ua, uа, ia, iа →
1; ub, uб, ib, iб → 2; uc, uс, ic, iс → 3; uo, uо, un, uп, u0, io, iо, in,
iп, i0 → 4 (so a name with "in" left over is class 4); classes 1–4 are
(192,192,0), (0,114,41), (255,0,0), (0,0,255); everything else — classes
5–7, unclassed, digital — takes the first colour of pair (place in the
channel vector `doc+0x184`, `0x42b5f0`) mod 10 of the table `0x4c2fe0`;
recomputed on every call. Seen on the VM: WIN_1999's IA / UA yellow, IB /
UB green, IC red. Also cited (W1.3, already described under their
callers): the differential grid / labels / scale box `0x444630` (L9.75),
its scale `0x4441b0` (as the circle's `0x438b50`, L9.88; the cursor range
from min(c1, c2), not raised to N), its characteristics `0x4453e0` (as
`0x43a4a0`), its scroll bars `0x443720` (DISABLENOSCROLL), its pan limit
`0x443620` (client −6, +5, −20 always). Explained away: `0x410140`,
`0x4103a0`, `0x42d220`, `0x42d460`, `0x48e910`, `0x48ebe0` (settings-tree
map inserts / erases), `0x43d750` (pair vector assignment), `0x4094c0` (a
characteristic's copy constructor).
Ours cycled a guessed 8-colour list by analog index, coloured computed
channels with the characteristic palette and every digital lane one grey
blue. Now `channel_color` / `phase_class` / `table_color`: file channels at
load, computed and pending ones at creation (by their place after the
analog channels), digital lanes by their place after both. Test
`channel_colours_by_phase_name_and_place`.
Residual: ours stores a computed channel's colour at creation (the
original recomputes it from the current place); the second colour of
each pair is unused here; the scroll bars kept disabled after zooming
back out, the pan limit with the table on, the differential scale over a
cursor range inside the first period and the print scale box's pen
(W1.3's list) are not yet changed.

### L9.171 — Channel defaults on no diagram; `Codepage` 1 = ANSI; the `.set` as the original writes it, byte for byte  ·  ✅
Run (COMPLETION_PLAN W4.2: WIN_1999 opened in the original on the VM and
saved through Save As — with the user Format and the system locale ru-RU;
under an English Format the original stores names through
CP_THREAD_ACP and turns Cyrillic into «?», mfc #1527 → WideCharToMultiByte)
and read (by hand `0x422536`, `0x48fda0`–`0x4900cb`, `0x459e60`,
`0x450d56`):
- A new channel's ShowFlags is the constructor's 0xE1 (`0x422536`: visible,
  the A1 / СКЗ rows, instant values) — on no diagram: the record's new
  `.set` says 225 for every analog and the discrete channel, and the
  vector / frequency diagrams show nothing until channels are put on them.
- The vector window's refresh (`0x48fda0`, part of the refresh-all) clears
  the vector flag of every channel failing the vector test (`0x4900cb`), as
  the circle refresh does for its flag (L9.163).
- `Codepage` in `Oscillogram` is the loader's guess (`0x450d56` stores
  `0x459e60`'s result): 1 when the text has more Cyrillic letters as it is
  than converted from OEM — 1 = ANSI (1251), 0 = OEM (866), a tie 0; the
  CP1251 record got 1. L9.17's "0 ANSI / 1 OEM" was inverted — ours read
  the original's files' 1 as 866.
- The sections the reads get or create are saved even empty: «Frequency
  Chart», «Vector Chart», «View», and «Selected Block» under both charts.
Ours put every channel on the frequency diagram, A / V ones on the vector
diagram and the first Ohm one on the circle diagram, wrote discrete flags
without the 0x60 bits, inverted `Codepage` and dropped the empty sections.
Now `ChanView::new` starts on no diagram; `circle_refresh_flags` also
clears the vector flag of ineligible channels; `merged_flags` defaults to
0xE1; `cp_code` / `cp_of` 1 = 1251; `store_settings` creates the empty
sections. Our `.set` for WIN_1999 is now byte-identical to the original's.
Tests updated (`eligibility_by_unit_as_the_original_reads_it`,
`prefixed_cyrillic_units_reach_the_vector_diagram`,
`menu_rows_tick_only_checked_items`, the code-page test).
Residual: (the `.cfg` / `.dat` writer: L9.172; the save path: L9.173).

### L9.172 — The `.cfg` / `.dat` writer byte for byte; `%g` with three exponent digits  ·  ✅
Run (COMPLETION_PLAN W4.2: WIN_1999, DOC_1999 and BINARY saved through
Save As by the original on the VM and by ours with `FV_SAVE_AS` — all nine
files now byte-identical) and read (background report on `0x44dfa0`,
`0x425530`, `0x44e680`, `0x44e920`, `0x422300`; by hand the min / max
quirk seen at runtime):
- The writer is a narrow stream with default formatting (doubles `%g`, MSVC
  12's three exponent digits — also on screen: «-3.79074e-005»), CR LF.
  Line 1: the station, `,device` when not empty, `,rev` when the revision
  token read was not empty. Analog lines `n,name,ph,ccbm,unit,a,b,skew,
  min,max` (+ `,primary,secondary,P|S` with a revision): min / max rounded
  (`0x411bf0`) from the channel's actual values as (v − b) / a — the unit
  prefix not undone, so a kA channel's are 1000× its codes; primary /
  secondary the tokens as read. Discrete lines `n,name,,,state` (bare
  `n,name,state` without a revision) with the normal state read. Then the
  frequency, the rate table as read, the stamps, the format line as read
  and the multiplier (with a revision); never a time-code line.
- The data: exactly "ASCII" writes rows `n,µs` (the stored µs) then
  `, value` per channel (round((v − b) / (mult · a)), discrete 1 / 0);
  anything else binary records — int32 number, int32 µs / multiplier,
  int16 values (int32 for revision "2013"), discrete bits in LSB-first
  words.
- The texts stay the bytes read: station, recorder and names are shown
  through the record's code page (L9.171), so they go back unchanged —
  DOC_1999's CP866 names stay CP866, its CP1251 units CP1251.
Ours wrote UTF-8, LF, shortest-exact numbers, ±32768, `%g`-less tokens, the
digital state 0 with its phase, ",1999" always, one rate line, "ASCII", the
2013 time-code line, and always an ASCII `.dat`. Now `comtrade::write(osc,
names)` as above (`rev_text`, `format`, `skew`, `primary_tok` /
`secondary_tok`, `normal` kept by the readers), `write_cff` on its bytes,
`fmt_gp` with three exponent digits. Test `cfg_dat_writer_as_the_original`;
the integration tests follow (the 2013 test now checks no time-code line).
Residual: an OSC2 record saved as COMTRADE (its revision / format tokens
NOT DETERMINED, ours "1999" / "ASCII"); the `.cff` writer `0x44d0f0` beyond
its use of these bytes; pending channels in the binary writer (the cfg
omits them, the binary data writes them — INFERRED).

### L9.173 — The save path: the record path never updated, the `.hdr` and trigger on every save, the lowered proposal  ·  ✅
Run (the VM: Ctrl+S — accelerator 57604, Save As — proposes «win_1999»
for WIN_1999.cfg; accepting it rewrote WIN_1999.cfg / .dat and wrote
win_1999.set) and read (background report, `0x44c940`, `0x44d720`,
`0x447e30`): OnSaveDocument dispatches on the target's lowered last four
characters (.cfg `0x44d720`, .osc `0x44ca80`, .zip `0x44db10`, .cff
`0x44d0f0`) and clears the modified flag. In `0x44d720` the same-file test
(`0x44d772`) compares the record path `doc+0x260` — the opened path, set by
the loader (`0x450ee4`) — with the target by `wcscmp`, case-sensitively;
the same file skips the `.cfg` / `.dat` but still writes the `.hdr` (with a
record comment) and the `.set`; the trigger sample is zeroed either way
(`0x44daaa`); and `doc+0x260` is never updated (the compare at `0x44da96`
discards its result) — after a Save As every save writes the record again.
DoPromptFileName lowers the passed name in place (three MakeLower calls)
before cutting folder and title (CommonInclude's GetFilePath_ /
GetFileTitle_ keep case), so the proposal is lowercase.
Ours compared MFC's path (updated by a Save As), wrote the `.hdr` and
zeroed the trigger only when the record was written, and kept the name's
case. Now `Doc::rec_path` (set at the open, never moved) drives the
same-file test and the `.osc` copy; the `.hdr` is written on every `.cfg`
save, the trigger zeroed on every `.cfg` / `.cff` save; the proposal is
lowered on Windows (elsewhere a lowered folder may not exist). Test
`the_record_path_is_never_updated`.

### L9.174 — The planes' pan limit counts without the readout table; the scale box's pen  ·  ✅
Read (W1.3 report; the pan limits `0x443620` / `0x437b90`, the grid
`0x444630`): the pan limit is round(⌊span / cell⌋ · z · 0.5) over the client
− 6 px, top + 5, bottom − 20 always — even with the readout table shown,
whose height only the painter's plot rect loses; the scale box is
outlined with the axes pen (black ⌊2s/3⌋ on paper). Ours took the limit
from the table-shortened plot rect (the wheel / toolbar pans refused early
and the differential's lower-left push stopped short with the table on)
and printed the box in the grid grey. Now `PlaneGeom::pan` carries the
span for every limit (the scroll bars, the toolbar pans), and the box takes
`axs`.
Residual (W1.3b): the scroll bars disabled rather than hidden after zooming
back to 1 (VM check first); the differential scale over a cursor range
inside the first period.

Next in L9 (HANDOFF 2026-10-24, night 6): **public at
github.com/dmytrobsp/fastview-rea** (release v0.1.0 = the 2026-10-22 state;
CI artifacts since then: linux-x86_64, windows-x86_64, macos-universal,
each one flat archive). `main` = HEAD, 405 tests.
**Night 6 (L9.128–L9.173)**, audits as background agents (message boxes,
CMDUI, keys / wheel, context menus, dialog DDX, status bar, docking-pane
menus, the caption) and follow-up reads (frequency track, flags on
non-file channels, the ОМП tracked phasor, `.set` per channel, N): L9.128
Ctrl held → no docking; L9.129 CMDUI rules; L9.130 **Ctrl+C / Ctrl+V
never fired in the real app** (egui-winit sends `Event::Copy` /
`Event::Paste`) + MDI / preview / визир keys, per-message wheel; L9.131
posted exceptions as boxes, an alert queue; L9.132 the nominal-frequency
track; L9.133 DIALOG 353's DDV; L9.134–L9.136 invert / DC / modes on
digital, computed and pending channels; L9.137 own frequency vectors and
the ОМП tracked phasor; L9.138 ОМП lookup ignores case, the merge's beep;
L9.139 ↑ / ↓ place a never-placed ГВ; L9.140 the merge's padding via the
accessor; L9.141 **record times from the readers** (ASCII timestamps,
binary k·1e6/R; a trailing ^Z line is no row — DOC_1999 is 11000
samples); L9.142 **N from rec+0x228**; L9.143 `.set` per channel (whole
ShowFlags, NetFreq); L9.144 / L9.146 context menus (MFC separators, check
column, keyboard); L9.145 formulas name digital channels; L9.147 status
bar formats / persistence; L9.148 DDX as MFC 12 (scanf "%d"), Enter /
Esc in dialogs; L9.149 MFC's pane menu on captions; L9.150 **the caption
row: frameless window, our own caption (FV_SYSTEM_TITLEBAR restores the
system frame)**; L9.151 quick-analysis pages fresh on every open, defaults
by name; L9.152 pair rows in «Каналы», stale pairs purged, `Properties
Height` per diagram; L9.153 **the circle / differential parameter grid as
the original's property grid** (tick box per characteristic); then
vtable-override sweeps over every own class: L9.154 dialog keys / defaults
(«Каналы» has no OK, the finder's default button, Enter in a formula
form); L9.155 **Alt never reaches the ribbon** (`0x448e80`); L9.156 the
grid's clicks; L9.157 deleting channels sweeps the comments; L9.158 the
open / Save As filters; L9.159 an open file is brought forward, tab
right-click; L9.160 DIALOG 353 order; L9.161 wheel over a pane toolbar;
L9.162 **the block state is the record's `.set`, «Нет» first**; L9.163 W /
var leave the circle diagram on a refresh; L9.164 drag auto-scroll; then
COMPLETION_PLAN (W1 batches, W4 runs of the original on the VM —
`tools/vm/`, auto sign-in, ru-RU locale and Format): L9.165 the start-up
pane layout; L9.166 / L9.167 «Каналы» and the quick-analysis pages read to
the end; L9.168 the parameters bar's ✕; L9.169 the `.omp` file is a
settings document; L9.170 channel colours by phase / name / place; L9.171
channels start on no diagram, `Codepage` 1 = ANSI; L9.172 **the `.cfg` /
`.dat` / `.set` byte-identical to the original's on three records**;
L9.173 the save path.
Lessons: tests that inject `Key::C` passed while the
real app never saw a key — check what egui-winit emits; never chain the
gates and a commit in one command (a commit with clippy warnings slipped
through once, fixed at once); a message box raised by a key must not
take that same key (L9.148).
**Night 5 (L9.107–L9.127), worked continuously with parallel
background research agents (MFC docking / tabs, merge, print, ribbon,
finder, style data, the 401–543 dialogs, engine residuals):** L9.107 the
four diagram panes as docking units of their own and MFC's tab show /
hide rules; L9.108 document tabs reorder, no MDI tab groups (as read);
L9.109 edge blocks in the docking manager's order; L9.110 printing in the
printer's hardware margins (CUPS PPD, preview); L9.111 merge method 2 as
read, N = own rate ÷ 50 / R div trunc(f), flags baked in; L9.112 print
details (device-px offsets, header name −4 chars, preview at the
printer's scale); L9.113 print dialog fields from the spooler (VM-run);
L9.114 ribbon sizes and the original's labels («Дифферен циальная»,
«Осцилло грамма»); L9.115 keytips (recent docs, disabled inert, «A»
Help, Alt-held); L9.116 the finder's shell folder tree / «Выбрать» / My
Documents / date pickers; L9.117 / L9.119 Office 2007 palettes and ribbon
body / panel bands from MFC's style data (Black = light ribbon in a dark
frame); L9.118 the print look keyed to s ≠ 1; L9.120 the merge's zero
padding and sub-sample shift; L9.121 header T from the printer's stock
font (VM: 16 px); L9.122 «Свойства...» on Windows (not exercised);
L9.123 DIALOG 401–543 all mapped + formula tree icons; L9.124 / L9.125
`.osc` stamps, the trigger sample, the dotted trigger line, labels /
readouts / «Абсолютное время» from the trigger; L9.126 merge sync under
the frequency correction; L9.127 the round 45 × 45 application button.
User-visible: diagram tabs dock anywhere; document tabs reorder; ribbon
labels / sizes / colours / application button; finder tree and
calendars; `.osc` dates and trigger line; Linux print margins; merge
results.
**Earlier sessions:** L9.76–L9.92 (unit text, pending lanes, frequency
candidates, page 539, the circle / differential planes as read — the
swapped differential pairs —, the Ohm split, keytips, ribbon reduction,
docking) and L9.68–L9.75; «P» records SECONDARY by default (L9.57); the
measured-frequency correction on by default with a Гц / Hz channel
(L9.65). The carved MFC DLLs live in `extracted/mfc/`. No user questions
pending.
**How to look at the GUI without running it:** `FV_WINDOW_DUMP=<png>
[FV_WINDOW_RECORD=<cfg>] [FV_WINDOW_STATE=analysis (the sheet, two formula
channels)|comments|group|circle|circletable|circlelegend|diff|difftable|keytips|keytipsmenu|trigger|floating|autohide|dockleft|dockbottom|docktab|docksplit|dockmarkers|omp|ompparams|preview|print|pagesetup|
settings|channels|coef|fileparams|explorer|parambar|vizirs|alert|windows|
paste|vecvalues|skin<k>] [FV_WINDOW_XSCALE=f] [FV_WINDOW_SIZE=w,h] [FV_WINDOW_CLICK=x,y]
[FV_WINDOW_HOVER=x,y] cargo test window_dump`, then Read the PNG (crop with
ImageMagick `convert`); `FV_FRAME_BENCH=<cfg> cargo test --release
frame_bench` times a frame. The user's sample records live outside the
repo (`~/Downloads/*.cfg`; never commit them). Dialog control ids: parse
the PE's RT_DIALOG template locally (python), `notes/ui/dialogs.txt` has
texts only. Local disassembly (git-ignored, never commit): `dumps/fv.asm`
(`tools/regen-dumps.sh`) and `dumps/fv-plain.asm` (`objdump -d -M intel
--no-show-raw-insn`, the form the grep recipes assume: `grep -n "^  <addr>:"`);
a class's virtuals: RTTI `.?AV<Name>@@` → COL → vtable; MFC
ordinals: the export table of `extracted/mfc/mfc120u.dll`, disassemble at
the address (an old listing may still sit in a /tmp scratchpad); MFC
strings / dialogs: `tools/pestr.py` on `extracted/mfc/mfc120rus.dll`.
**Open next** (verify a residual is not already done before starting):
33035's message 239 on channels of different rates
(needs per-channel rates, L9.131 — unreachable in practice: only pending
channels carry another rate and they are excluded); the docking marker
bitmaps (ours drawn); L9.64 MDI arrangements under tabs (WM_MDICASCADE /
TILE — visible effect ND); (the caption row: L9.150); backlog §9 #36 / #37 / #45 leftovers (logo, splash:
proprietary assets). Good recurring pass: render the user's real records
(`~/Downloads/DOC_1999.cfg`, `BINARY.cfg`, `WIN_1999.cfg`) after big
changes.
Not yet run on the Windows VM this night (it was unreachable): the
GUI test exe — run it when the VM is back (windows-vm memory).
(Done: night 6 L9.128–L9.150, night 5 L9.107–L9.127.)
---

## L2 — Formula evaluator `Calculator` (tokenizer + recursive descent)  ·  ✅ core language MATCH

Evidence (Ghidra pseudocode + objdump of FastView.exe; spot-checked by hand
where marked ✔):
- Tokenizer `FUN_00418c40` (only reader of the token table @`0x4ecc20`, 12-byte
  entries `{CString name, int id, char* label}`; names are ASCII literals, e.g.
  `0x4c4f1c` = `ib:` ✔ — there is **no `ia:`** token). Lowercases the formula
  (MFC MakeLower) but restores text inside `[...]`; skips **only U+0020** (tab is
  an illegal char); tries the 70 table names **in array order as plain prefixes**
  (so `pi`/`null`, which have no colon, split `pid` → `pi`+`d`); numbers = digits
  and `.` via `_wtof` (no exponent, no leading `.`, >1 `.` throws); identifiers
  start `iswalpha`/`_`/`"` (quoted names run to the next `"`); `[...]` →
  DIMENSION (skipped after an atom); `,` before `,`/`)` gets a NULL inserted;
  unknown identifiers throw at tokenize time (even in ignored trailing input).
- Grammar (evaluating recursive descent, no AST/RPN): list `0x419af0`, add/sub
  `0x419c50`, mul/div `0x419d80` ✔, pow `0x419f20` ✔, unary `0x419fc0` ✔,
  primary `0x41a110`, group `0x41f390`, atom `0x41f960`, binop jump table
  `0x41f944` ✔ (`+`/`-`/`*` `0x41f5f8/0x41f55f/0x41f657`, `/` `0x41f6e7`, `^` `0x41f7cb`).
  `pow := unary ['^' unary]` — **one `^`, no chaining**; the top level
  (`0x419950`) never checks for END, so `2^3^2 = 8` and trailing tokens are
  ignored. Unary sign is applied before `^`: `-2^2 = 4`; `--2` throws. The `)`
  after a group is consumed without being checked. A function takes a group
  `(a, b, …)` **or one bare atom** (`sin:30`); argument count checked exactly
  (`FUN_0041fd00`).
- Value = 0x28-byte element: scalar `+0x18`, complex `re +8 / im +0x10`. Numbers
  and variables set `re = v, im = 0`; `+ - * /` update both slots ✔ (complex
  divide `FUN_00420ca0`); `sin cos tg ctg sqrt exp ^ re im` write **only the
  scalar**; `mod:` = `|re + j·im|` (`FUN_00420610`) — so `mod:(sin:(30)) = 30`.
- Math cases (`0x41a23d`..`0x41a431`) ✔: trig = `f(v * pi / 180)` (multiply by
  pi `0x4d16d8`, then divide by 180.0 `0x4d1738`); `ctg = 1.0 / tan`; no domain
  checks (IEEE inf/NaN). `^` ✔: exponent must satisfy `(double)(int)e == e`
  (else error string 0x157); `n=0 → 1.0`; `n>0` repeated multiply; **`n<0`
  returns the base unchanged** (original bug). `arg:` = `atan2(im,re)·180/pi`,
  `≥180 → −360`, and 0 while the sample index is below the warm-up count
  (vtable[2]). `cosui:(a,b)` = `cos(∠b − ∠a)` (radians); `absig:(a,b)` = the same
  difference in degrees wrapped to (−180, 180].

Our impl (`expr.rs`, rewritten to the above): token pass with the original's
table/order, the exact grammar levels, the scalar/complex value model,
`pi null sin cos tg ctg sqrt exp re im mod arg cosui absig`, the `^` integer
loop, `v*pi/180` conversion order. Pinned by `tests/calculator.rs` (13 tests;
perturbations `to_radians()` and chained `^` are both caught).

Verdict: ✅ **MATCH for the numeric language** (everything the `.fvb` library and
a channel-free formula can reach). Shipped `FastView.fvb` scan: `mod:` only wraps
pure arithmetic of variables, `^` only `x^2`, no exponent literals / leading dots
/ tabs / bare-atom calls / `pi`-prefixed names → identical results.

Not verified / residual (honest):
- CRT `sin/cos/tan/exp/_wtof` last-ulp differences between MSVC 2013 and Rust's
  libm are not modelled.
- Cyrillic case folding: the original uses MakeLower; the program sets
  `setlocale(LC_ALL, "Russian_Russia.1251")` then LC_NUMERIC "C"
  (`0x4484e5`, read 2026-10-23, L9.95), under which the CRT's lower-casing
  folds Cyrillic (CRT behaviour INFERRED) — as ours, which the shipped
  library needs (`ДЗМф2_Rср` vs `ДЗМФ2_Rср`).
- `arg:` warm-up count in the `.fvb` context (vtable[2] of that Calculator
  subclass) not determined; we assume no warm-up.
- ❌ Channel mode (computed channels): identifiers resolve via vtable[0] to
  CHANNEL tokens (flags 7; scalar = sample `FUN_00426020`, complex = phasor
  `FUN_00428ac0`; an unresolved name emits *nothing*). Our GUI feeds channel
  samples as plain variables, so e.g. `mod:(Ia)` gives |sample| where the
  original gives |phasor|. The channel accessors `rms: fourie: a0: fault:
  frequency: ib: … uniik:` are recognised with their exact arg counts but return
  `NeedsChannel`. → handled with the DFT/signal engine (task table).

## L3 — COMTRADE / oscillogram loading  ·  `ComtradeConverter` (RTTI @ `0x4ebe1c`)

Decompiled class methods (MSVC-mangled names recovered):
- `ComtradeConverter(int,bool)` ctor / `~ComtradeConverter`
- `GetInfoFromStoHeader(CFile&, ushort&, ulong&)` — static `.sto` header reader
- `ReadFromXml(CString)` — `.sto` XML metadata
- `ConvertSto2Comtrade(CString, CString, void*, int, int, …)` — `.sto` → COMTRADE
- `CheckDstCompatibility(...)`, progress callbacks
- sibling class `FVComtradeConv` (RTTI @ `0x4ebe3c`)

The file-open filter (`0x905218`) proves the original opens **four input formats**:
`*.cfg` (COMTRADE), `*.cff` (COMTRADE'13 single-file), `*.osc` (OSC2),
`*.sto` (Configurator-MT native).

Full behavioural spec: **`notes/FORMATS_SPEC.md`** (open/save dispatch,
`.cff` reader `0x44fe30` / writer `0x44d0f0`, OSC2 reader `0x4524b0`).

Our impl (`comtrade.rs`): `.cfg`/`.dat` with the original's strict data-file
types (exactly `ASCII`/`BINARY`/`binary32`; FLOAT32 etc. → error 330
"Неправильный формат файла", as the original); `.cff` reader with the
original's rules — text decoded up to the first NUL, CRLF-based section
split, DAT byte count = first digit run on the marker line, payload starting
one byte after the line terminator (the original's LF offset bug, reproduced:
harmless for ASCII, corrupts binary .cff) — and `.cff` writer (rev ≥ 2013 else
error 187; CFG, HDR/INF/SET, `DAT BINARY: n---`; CP1251). GUI opens `.cff`,
keeps its HDR/INF/SET and offers `.cff` in Save-As; load errors read
"<file> - <message>". Pinned by `tests/formats.rs` (5). A test fixture that
only loaded because unknown file types used to fall back to ASCII was fixed.

OSC2 (`.osc`) reader implemented (`parse_osc`): header/calibration/text/info/
data blocks, big-endian, calibration a/b, channel table (incl. "Резерв."),
info ratio → primary rating, per-block rate codes with the first block snapped
to the standard rates, integer-µs time, the version harmonic-1 gain (applied
by `fourie:` n = 1 via `OscChannels`). Pinned by `tests/formats.rs::osc2_reader`
(synthetic file built to the verified layout — no real `.osc` sample ships, so
it is not byte-verified against a device file).

Verdict: 🟡 **PARTIAL** (all readers present).
- ✅ `.cfg`/`.dat` file-type rule, ✅ `.cff` read/write, ✅ `.osc` read.
- ✅ `.osc` Save-As (`0x44ca80` writes the record's own bytes under the new
  name; ours copies the file — triaged 2026-10-23); (OSC stamps and the
  trigger sample: L9.124 / L9.125); unit-prefix
  table beyond micro/milli NOT DETERMINED.
- ⛔ `.sto` — needs the native ConvertSto2Comtrade and a sample file.
- Not modelled: the temp-folder mechanics, `<base>.set` files, the
  save-onto-own-path rule, the `.dat` → `.cfg` open redirect is native already.



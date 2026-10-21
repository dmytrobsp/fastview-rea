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
- `cargo test --workspace`   (currently 266 pass)
- `cargo clippy --workspace` (0)
- `cargo build --workspace --target x86_64-pc-windows-gnu` (0 errors)

**State at handoff.** Working tree clean; 266 tests pass, 0 build warnings, clippy
clean, Windows cross-compiles. The latest commits are this ledger + the engine
work — run `git log --oneline -12` to see them (most recent ledger commit added
this section). Current direction = the **reverse-ALL-behavioural-engines** program
the user approved (not just adding features). Work commits straight to the current
branch (no SDD/feature-branch flow); keep the tree green and the ledger updated at
every commit.

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

Older residuals (lower impact): the `.osc` writer
(`0x44ca80`) and OSC time stamps; the frequency-channel DFT variant
(`FUN_004267e0`); R/X channel pairs on the circle diagram (DIALOG 554); the
parallel-line (mutual coupling) entry in the ОМП UI; dialog-behaviour mapping
of the channel-role dialogs (DIALOG 401–543); a GUI screenshot pass (Wayland
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
- ❌ The **Редактор формул / computed-channel** vocabulary is far larger. Missing:
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
| DFT / phasors / RMS | `FUN_00426fc0` & channel methods | `signal.rs` | ✅ L6 (freq-channel variant ❌) |
| Symmetric components / impedances | Calculator `i1: i2: 3i0: zl: zf:` | `signal.rs`/`analysis.rs` | 🟡 L6 (view code not located) |
| ОМП single/double/k0 | `0x4a00b0` engine | `omp.rs` | ✅ L7 (parallel-line UI ❌) |
| COMTRADE reader/writer | `0x450a30`, `.cff` `0x44fe30`/`0x44d0f0`, `.osc` `0x4524b0` | `comtrade.rs` | 🟡 L3 (`.cfg` ✅ `.cff` ✅ `.osc` read ✅, `.osc` write ❌) |
| `.sto` native reader | `GetInfoFromStoHeader` | — | ⛔ no sample ships |
| Channel-role dialogs | DIALOG 401–543 | `dialogs.rs` map | 🟡 global map done |
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
- Our extra "Сбросить уставки" button and per-characteristic visibility list
  are conveniences (the original's visible flag exists; its UI not mapped).
- GUI change not screenshot-verified this session (Wayland capture missed).

---

## L6 — Signal engine (channel methods)  ·  ✅ (freq-channel DFT, 0x46 retag ❌)

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

Residual: frequency-channel DFT variant (`FUN_004267e0`, records with a
selected "Гц" channel) not implemented; `rec+0x278` harmonic-1 gain assumed 1
for COMTRADE (NOT DETERMINED); which method the frequency-diagram *view* calls
NOT located (we use `fourie:`); primary/secondary/ADC/DC-removal flags of
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

## L7 — ОМП (fault location)  ·  ✅ engine + GUI (parallel-line UI ❌)

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
which signal feeds a parallel line's current NOT DETERMINED.
GUI (done): method 1/2/4, Inom per substation, editable sections with taps,
"Рассчитать" runs the engine on the role-mapped channels (isolated uses line
voltages derived as Ua−Ub etc. — the original reads Uab/Ubc/Uca channels by
name), writes the window to the cursors, adds "Lкз" (m) and "КЗ фазы A/B/C"
channels, shows the original's text; moving the cursors recomputes the result
(double-ended re-applies the direction check). The old single-instant API and
its 4 tests were removed. GUI tests: message 349, window/cursors/channels.
Not done: parallel-line (mutual coupling) entry in the UI, the .omp settings
file and .txt result save, the tree-editor layout of DIALOG 336.

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
Residual: egui has no Tahoma/bold (bold simulated by double drawing); group
lanes list member names + values only (the original's per-member stat rows
inside a group not modelled).

### L9.6 — Comments: moments, level lines, areas  ·  🟡 (no .set persistence)

Read (spec §5: `0x43eb40` placement, `0x43ef50` release, `0x43efa0` drag,
`0x43f630` paint, `0x43df90` info box, `0x45efa0`/`0x45ce40` MENU 563):
three kinds with text / locked / text-hidden; ±10 px hit tests (moment by x,
level by its segment with parts left end / right end / body, area on its
border band only with side edges draggable); one-shot tools 155/157/156
(moment in one click; level on an analog lane and area on any lane dragged
out from the press, normalised on release); LMB on an element whose text is
shown grabs it, locked ones do not move; hover = selected (3 px (192,0,0),
info box with «Время:» / «Уровень: %.2f» / «Длительность: %d.%03d с»);
moment marks feed the difference band; MENU 563 labels per kind
(визир / линию уровня / область), «Изменить уровень» only for levels as an
in-place `%.2f` edit; double-click opens «Комментарии к диаграмме».
Ours: `comments.rs` (`Comment`, `CommentKind`, `hit`, `drag`, `normalise`,
`paint`, the item-mode dialog; 2 unit tests), `lanes.rs` placement / drag /
hover / level edit, three ribbon tools. Headless test: one-shot tools,
level/area drag-out, moment drag, lock, menu by kind.
Residual: comments are not yet saved to / loaded from the oscillogram
settings file (`.set`: `Comments/Moments|Levels|Areas`); the moment info box
shows the time from the record start (no absolute date); bubble marks are
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
(the original subtracts `k·dc` from an unscaled code) not modelled; undo for
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
(tested), dropped files, viewport title. Residual: auto-colour order after
the first NOT DETERMINED (MFC OneNote palette assumed; the bundled vcredist's
mfc120u.dll sits in an attached burn container we did not unpack); the 2 s
splash not implemented (its bitmap 0x1CE is a proprietary asset); single
instance done in L9.18 (`instance.rs`: loopback port + token file instead of
the stored HWND / WM_COPYDATA, the newest plain start wins, a file argument
is handed over and the start exits once accepted); tabs cannot be
dragged into new tab groups; «Окна...» not in the ▾ list.

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
Residual: no shell folder tree (a folder list instead); `.sto` entries not
read (no sample `.sto`; `0x46c960` reads its inner cfg); delete is permanent
after our own confirmation (the original uses the shell prompt and the recycle
bin with Shift); date pickers are day/month/year spinners.

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
Residual: report start/trigger times empty (loader keeps no timestamps), no
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
contents, frequency margins NOT DETERMINED; per-document «* Chart»
persistence, the themed splitter, bit 14 pending merge not modelled;
`egui_plot` dependency now unused.

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
Residual / NOT DETERMINED: the `doc+0x1e4` correction candidates (combo
disabled); digital groups `doc+0x878/+0x884`; bitmap 464 icons (ours);
whether 463 has OK; `ch+0x89` default (secondary); `.set` persistence and the
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
carry text labels; keytips not modelled.

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
Residual: system-clipboard format done in L9.20; method 2's
details beyond the spec summary not re-read; «Объединить» undo fixed in L9.19; values ÷ `ch+0xf0` on merge not
modelled; pasted discrete channels show as 0/1 lanes until merged.

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
396 shows once (the original shows it twice).

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
`Oscillogram/DiscretisationFrequensy`, `Codepage` (0 ANSI / 1 OEM, wins over
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
vs «Сохранить как» now differ. 11 tests. Residual: the prompt wording is
assumed (MFC resource); `Selected Block` subtree still in our program store;
`NetFreq`, `.cff` SET, `.zip`, `.sto` not modelled; comments store file name
vs alias NOT DETERMINED; whether paste / an ОМП run mark modified NOT
DETERMINED (ours: no).

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
test `pasted_channels_go_before_the_first_selected_channel`). Residual: no
EMF picture; an analog paste with no selected analog channel goes to our
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
delay start not modelled.

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
визиры and the comment hover not drawn in print; header/footer line pitch
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
Residual: arrow auto-repeat while held not modelled; whether the V bar is
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

Next in L9 (HANDOFF 2026-10-21, master clean at 4a83adc): §9 items 36 and
48 are done (L9.21, L9.22). **FIRST — four GUI bugs the user found running
the release build (ours, not original behaviour; reproduce headless with the
`lanes::menu_tests` Harness pattern or `window_tests` (whole window via
`App::frame_ui`) before fixing, add a regression test each):** (1) ✅ L9.23
— the program **crashes when the window is made too small**
(suspects: `lanes_ui` geometry / `g::visible_count` / slicing with a
negative or zero plot size, `Canvas`/diagram sizes, `rotate_right`
ranges; the minimum window size 866×500 does not stop panels from
shrinking); (2) ✅ L9.25 — the **«Файл ▾» application menu does not render
properly** (`ribbon.rs` app menu: the two-column `horizontal_top` layout
with the Печать ▸ and «Дополнительно» submenus and the MRU list); (3)
**mouse events fall through the colour picker** (33030 `color_dialog`) and
likely the other dialogs/windows (print, page setup, ОМП, formula …) — the
oscillogram view underneath still receives presses/drags/wheel; the view
must ignore input while a modal is open or the pointer is over a window
(`ctx.is_pointer_over_area()` / a modal layer); ✅ L9.24; (4) the **bottom
horizontal scroll bar is drawn wrongly** (`lanes.rs` `scrollbar(..)` for
`hbar` under the plot: geometry relative to `client.bottom()` /
`HSCROLL_H`) — ✅ L9.23. Then: the EMF picture on copy (L9.20), print residuals
(L9.22), pane re-docking (egui limit), and the smaller residuals listed in
L9.1–L9.22.

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
- Cyrillic case folding: the original uses MakeLower (likely ASCII-only in the C
  locale) — we fold Unicode, which the shipped library needs (`ДЗМф2_Rср` vs
  `ДЗМФ2_Rср`); how the original resolves that pair (map comparator?) is NOT
  DETERMINED.
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
- ❌ `.osc` writer (`0x44ca80`, Save-As .osc) not reversed; OSC trigger/start
  time stamps not kept (our record has no time-stamp fields); unit-prefix
  table beyond micro/milli NOT DETERMINED.
- ⛔ `.sto` — needs the native ConvertSto2Comtrade and a sample file.
- Not modelled: the temp-folder mechanics, `<base>.set` files, the
  save-onto-own-path rule, the `.dat` → `.cfg` open redirect is native already.



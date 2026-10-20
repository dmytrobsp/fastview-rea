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
- `cargo test --workspace`   (currently 53 pass)
- `cargo clippy --workspace` (0)
- `cargo build --workspace --target x86_64-pc-windows-gnu` (0 errors)

**State at handoff.** Working tree clean; 53 tests pass, 0 build warnings, clippy
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

**NEXT TASK (start here).** L4 — implement the reversed geometry/region
semantics in `model.rs`/`geom.rs` (sequential per-mode CombineRgn fold, `xor`,
`mode` default off, `closed` default on, open polypoints filled) with tests,
then continue down the **Engines to reverse** table (`.fvb` parser re-verify →
DFT/signal + Calculator channel mode → ОМП → COMTRADE `.cff`/`.osc`). L2 (the
evaluator) is done ✅.

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
COMTRADE). The MFC ribbon/dialog message-map plumbing is boilerplate and not
where correctness lives; dialog *behaviour* (what each form does) is mapped, its
pixel layout is not.

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
| Formula token table | `FUN_00401000` @0x401000 | `expr.rs` | 🟡 L1 (math ✅, channel accessors ❌) |
| Formula evaluator (tokenizer/grammar/values) | `FUN_00418c40`, `0x419950`..`0x41f960` | `expr.rs` | ✅ L2 (numeric) |
| `.fvb` parser dispatch | `FUN_0040c7e0` (prior session) | `parse.rs` | ⏳ re-verify |
| Geometry / region engine | `Figure` vtables `0x4c3d90..0x4c3dcc`, paint `0x43a4a0` | `geom.rs` | ❌ L4 reversed, fix pending |
| DFT / phasors / RMS | (pending) | `signal.rs` | ⏳ pending |
| Symmetric components / impedances | (pending) | `signal.rs`/`analysis.rs` | ⏳ pending |
| ОМП single/double/k0 | (pending) | `omp.rs` | ⏳ pending |
| COMTRADE reader/writer | `ComtradeConverter` | `comtrade.rs` | ⏳ pending |
| `.sto` native reader | `GetInfoFromStoHeader` | — | ⛔ no sample ships |
| Channel-role dialogs | DIALOG 401–543 | `dialogs.rs` map | 🟡 global map done |

Next: L4 implementation (geometry), then `.fvb` parser re-verify, DFT/signal
(incl. the Calculator channel mode + `rms: fourie: a0: …`), ОМП, COMTRADE.

---

## L4 — Geometry / region engine  ·  ❌ reversed, Rust fix PENDING (next task)

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

Rust action: model.rs (`Mode` default Off, `closed` default true, `Combine::Xor`,
explicit "absent" op), geom.rs sequential CombineRgn fold with first-figure-
starts semantics, open polypoint closed via a far box, degenerate figure =
empty-but-starts. Check the shipped library for figures relying on the
uninitialised-op path before choosing its behaviour.

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

Our impl (`comtrade.rs`): `.cfg`/`.dat` only (ASCII + binary), plus a 1999-ASCII
writer. Verdict: 🟡 **PARTIAL**.
- ✅ `.cfg`/`.dat` reading.
- ❌ `.cff` (single-file COMTRADE 2013 — straightforward: header+data concatenated).
- ❌ `.osc` (OSC2 format — needs RE of its layout).
- ⛔ `.sto` (native Configurator-MT — no sample ships; `GetInfoFromStoHeader`/
  `ConvertSto2Comtrade` can be read but not byte-verified without a file).

Action: implement `.cff`; decompile the `.osc` reader; document `.sto` as
infeasible-to-verify with the decompiled header layout as the best available map.

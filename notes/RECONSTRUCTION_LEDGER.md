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
- `cargo test --workspace`   (currently 40 pass)
- `cargo clippy --workspace` (0)
- `cargo build --workspace --target x86_64-pc-windows-gnu` (0 errors)

**State at handoff.** Working tree clean; 40 tests pass, 0 build warnings, clippy
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

**NEXT TASK (start here).** L2 — decompile the `Calculator` evaluator (RTTI
`.?AVCalculator@@` @ `0x4eb4a4`): find its vtable / evaluate method, verify the
semantics `expr.rs` assumes (trig in **degrees**, `mod:` = abs, `^` right-assoc &
tighter than `* /`, colon-suffix postfix calls) → upgrade L1's shipped-`.fvb`
verdict to MATCH; then implement the missing formula functions (L1 action list)
with per-function decompiled evidence. Then work down the **Engines to reverse**
table top to bottom.

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

Verdict: 🟡 **PARTIAL**.
- ✅ The **shipped `FastView.fvb`** uses only `tg sin cos sqrt mod ctg` + operators
  (verified: none of the richer tokens occur in it), so our characteristic
  rendering is behaviourally correct for the bundled library.
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
| Formula token table | `FUN_00401000` @0x401000 | `expr.rs` | 🟡 L1 above |
| Formula evaluator (per-token cases) | callers of the token table (pending) | `expr.rs` eval | ⏳ pending |
| `.fvb` parser dispatch | `FUN_0040c7e0` (prior session) | `parse.rs` | ⏳ re-verify |
| Geometry / region engine | `CircleDiagramWnd` + GDI `PathToRegion`/`CombineRgn` | `geom.rs` | ⏳ pending |
| DFT / phasors / RMS | (pending) | `signal.rs` | ⏳ pending |
| Symmetric components / impedances | (pending) | `signal.rs`/`analysis.rs` | ⏳ pending |
| ОМП single/double/k0 | (pending) | `omp.rs` | ⏳ pending |
| COMTRADE reader/writer | `ComtradeConverter` | `comtrade.rs` | ⏳ pending |
| `.sto` native reader | `GetInfoFromStoHeader` | — | ⛔ no sample ships |
| Channel-role dialogs | DIALOG 401–543 | `dialogs.rs` map | 🟡 global map done |

Next: decompile the evaluator cases to pin exact semantics (degrees vs radians,
`mod:`=abs, `arg:` range, `fourie:`/`rms:` window), then implement the missing
functions with per-function evidence recorded here.

---

## L2 — Formula evaluator class `Calculator`  ·  RTTI `.?AVCalculator@@` @ `0x4eb4a4`

The evaluator that consumes the L1 token stream is the `Calculator` C++ class
(RTTI confirmed). Next target: locate its vtable/evaluate method and decompile
the per-token arithmetic to **verify** the semantics our `expr.rs` assumes —
trig in degrees, `mod:` = abs, `^` right-assoc tighter than `* /`, colon-suffix
postfix calls — turning L1's "shipped .fvb correct" into a MATCH, then implement
the missing functions (L1 action). Status: ⏳ entry point found, decompile pending.

---

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

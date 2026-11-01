# The original, as read

The record of what Fast View 5 does, item by item: the original's code
(addresses in `FastView.exe` / `mfc120u.dll`), what it does in our words,
how ours implements it, how it was checked, and the verdict. It used to be
one file (`notes/RECONSTRUCTION_LEDGER.md`, split here on 2026-10-30 with
its text unchanged); entries keep their ids (`L4`, `L9.227`), so a
reference like "L9.227" is found in the file for its hundred below.

This is a record of the original's behaviour, not a task list: the work
queue is [GitHub Issues](https://github.com/dmytrobsp/fastview-rea/issues),
the current state [PROGRESS.md](../PROGRESS.md), each milestone's run
[milestones/](../milestones/README.md). Where ours differs on purpose:
[IMPROVEMENTS.md](../IMPROVEMENTS.md).

## Files

| Part | File |
|---|---|
| L1 — formula language token table | [L1-formula-tokens.md](L1-formula-tokens.md) |
| L2 — formula evaluator | [L2-formula-evaluator.md](L2-formula-evaluator.md) |
| L3 — COMTRADE / oscillogram loading | [L3-comtrade-loading.md](L3-comtrade-loading.md) |
| L4 — geometry / region engine | [L4-geometry.md](L4-geometry.md) |
| L5 — `.fvb` loader, characteristics, settings grid | [L5-fvb-loader.md](L5-fvb-loader.md) |
| L6 — signal engine | [L6-signal-engine.md](L6-signal-engine.md) |
| L7 — ОМП (fault location) | [L7-omp.md](L7-omp.md) |
| L8 — analysis views | [L8-analysis-views.md](L8-analysis-views.md) |
| L9 — UI layer: method and scopes | [L9-ui.md](L9-ui.md) |
| L9.1–L9.99 | [L9-ui-000.md](L9-ui-000.md) |
| L9.100–L9.199 | [L9-ui-100.md](L9-ui-100.md) |
| L9.200–L9.299 | [L9-ui-200.md](L9-ui-200.md) |
| L9.300–L9.399 | [L9-ui-300.md](L9-ui-300.md) |
| L9.400 – | [L9-ui-400.md](L9-ui-400.md) (new entries go here, then a new file per hundred) |
| Distilled specs | [specs/](specs/): UI behaviour, views, ОМП, file formats, computed channels, the early UI spec |
| Early findings | [findings.md](findings.md) |

The session notes and handoffs that used to open and close the ledger are
history now: [../history/ledger-session-notes.md](../history/ledger-session-notes.md).

## Writing an entry

- One entry per behaviour read or settled: `### L9.N — <the behaviour, as a
  sentence>  ·  <status>`, then what the original does (addresses as
  references), what ours did and does now, the test that pins it, and how it
  was checked (UNIT, VM — a run in a Windows clone —, NOT).
- Our own words only: no disassembly, no decompiler output, no resource
  text (the repository's rules in [AGENTS.md](../../AGENTS.md)).
- Entries are written by the milestone's controller when a unit merges
  (the unit's pull request carries the draft), so parallel units don't
  conflict over the record.
- Something left open is filed as an issue and the entry links it
  (`Residual: #N`), not left as a line here.

## Scope and method

Goal: behavioural parity **proven by decompilation**, not approximated from
screenshots. For each behaviour we record the original function (address +
Ghidra evidence), what it does, how our Rust implements it, and the verdict
(MATCH / PARTIAL / GAP / INFEASIBLE). Reversing is driven by REA + Ghidra 12.1.3
over `extracted/stage1/FastView.exe`
(sha256 `6c503790d13d9d2d0b80711f27b4daba57b209462cfb626cc5a6f719c150d046`,
image base `0x400000`, 9129 procedures). Raw decompiler dumps stay local
(`.gitignore`); this record keeps the distilled mapping (addresses, token tables,
behaviour, verdicts).

Scope note: we reverse the **behaviourally-significant engines** (formula
evaluator, `.fvb` parser, geometry/region engine, signal/analysis math, ОМП,
COMTRADE) **and — since L9 — the UI layer** (message-map handlers, view input
and painting, ribbon construction, panes, dialogs). The earlier stance that the
MFC message-map plumbing was boilerplate was wrong: the handlers behind it are
where the UI's behaviour lives. Pixel-exact layout remains out of scope;
behaviour and presentation semantics are in scope.

Status legend: ✅ MATCH · 🟡 PARTIAL · ❌ GAP · ⛔ INFEASIBLE (needs absent input).

### Engines to reverse (entry points identified)

| Engine | Original | Our module | Status |
|---|---|---|---|
| Formula token table | `FUN_00401000` @0x401000 | `expr.rs` | ✅ L1/L6 |
| Formula evaluator (tokenizer/grammar/values) | `FUN_00418c40`, `0x419950`..`0x41f960` | `expr.rs` | ✅ L2 (numeric) |
| `.fvb` loader / evaluation / settings | `FUN_0040c7e0`, `FUN_00409260`, `FUN_00407750` | `parse.rs`/`model.rs`/`geom.rs` | ✅ L5 |
| Geometry / region engine | `Figure` vtables `0x4c3d90..0x4c3dcc`, paint `0x43a4a0` | `geom.rs` | ✅ L4 |
| DFT / phasors / RMS | `FUN_00426fc0` & channel methods | `signal.rs`, `freqcorr.rs` | ✅ L6 (freq-channel variant ✅ L9.65) |
| Symmetric components / impedances | Calculator `i1: i2: 3i0: zl: zf:` | `signal.rs`/`analysis.rs` | ✅ L6 + L8 (no dedicated view: the quick-analysis sheet makes formula channels, L9.123) |
| ОМП single/double/k0 | `0x4a00b0` engine | `omp.rs` | ✅ L7 (parallel-line UI ✅ L9.11/L9.12) |
| COMTRADE reader/writer | `0x450a30`, `.cff` `0x44fe30`/`0x44d0f0`, `.osc` `0x4524b0` | `comtrade.rs` | ✅ L3 (`.cfg` ✅ `.cff` ✅ `.osc` read ✅, `.osc` Save-As ✅ copies the record's bytes; `.sto` ⛔ below) |
| `.sto` native reader | `GetInfoFromStoHeader` | — | ⛔ no sample ships |
| Formula forms, sheet pages, misc dialogs | DIALOG 401–543 | `addformula`, `formula_dlg`, `chandlg`, `quick_ui`, `recdlg` | ✅ L9.123 (every used template mapped) |
| Analysis views | CQAnPropSheet `0x48b970`, vector `0x490360`, freq `0x46f5a0`, readout `0x461440` | `quick.rs`, `views.rs`, GUI | ✅ L8 |
| UI layer (message maps → handlers) | 34 maps, CFastViewView `0x45a970`, CMainFrame `0x4720a0`/`0x4728d0` | `fastview-gui` | ✅ L9 (entries in L9-ui-000 … 400; open items are issues) |

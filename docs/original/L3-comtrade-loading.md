# L3 — COMTRADE / oscillogram loading  ·  `ComtradeConverter` (RTTI @ `0x4ebe1c`)

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

Full behavioural spec: **`docs/original/specs/formats.md`** (open/save dispatch,
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
"<message> - <file>" (L9.181). Pinned by `tests/formats.rs` (5). A test fixture that
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
  table beyond micro/milli — settled by L9.34 (the normaliser `0x422b10`).
- ⛔ `.sto` — needs the native ConvertSto2Comtrade and a sample file.
- Not modelled: the temp-folder mechanics, `<base>.set` files, the
  save-onto-own-path rule, the `.dat` → `.cfg` open redirect is native already.

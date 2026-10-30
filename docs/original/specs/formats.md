# Oscillogram file formats — behaviour of the original (ledger L3)

Distilled from the decompiled `FastView.exe`; addresses as evidence.

## Open / save dispatch
- Extension = last 4 chars lowercased, compared exactly (3-letter extensions).
  Open (`0x44ee70`): `.dat` → same base `.cfg`; `.sto` → ComtradeConverter
  (`GetInfoFromStoHeader`, `.dst` compatibility, `ReadFromXml`,
  `ConvertSto2Comtrade` into the temp folder) → load the produced `.cfg`.
  Loader (`0x44f430`): `.cfg` `0x450a30`, `.cff` `0x44fe30`, `.osc` `0x4524b0`.
  Save (`0x44c940`): `.cfg` `0x44d720`, `.osc` `0x44ca80`, `.cff` `0x44d0f0`.
- `.cfg` file-type line (`0x45158a`): after a trim the value must be exactly
  `ASCII`, `BINARY` or lowercase `binary32`; anything else (incl. `FLOAT32`,
  `BINARY32`, `ascii`) → error 330 "Неправильный формат файла" shown as
  "<message> - <file>" (the exception's text, `0x42dfe0`).

## `.cff` reader (`0x44fe30`)
- Writes `tmp_cfg.{cfg,dat,inf,hdr,set}` into `%TEMP%\Fast_View_<FILETIME hex>`
  and loads `tmp_cfg.cfg` with the ordinary loader.
- Text: whole file → narrow string → wide via the ANSI code page, which stops at
  the first NUL byte; sections written back as CP1251.
- Text sections CFG/INF/HDR/SET: case-sensitive substring find of
  `--- file type: CFG` (etc.) from offset 0; content starts 2 chars after the
  next `\r\n`, ends at the next `--- file type:` (the CRLF before it included);
  no later marker → empty content (file still written). The rest of the marker
  line is not checked. CRLF required (LF-only files mis-split). INF/HDR/SET
  optional; without SET a sibling `<base>.set` is copied.
- DAT: lines of the raw bytes (leading tab/LF/CR/space skipped, a line ends at
  CR or LF); the first line containing `--- file type: DAT`; byte count = the
  first run of digits on that line (`ASCII`/`BINARY…` word ignored;
  `BINARY32: 100` → 32; no digits → 0). **Data starts at the line terminator
  + 1** — with CRLF that is the LF byte (payload shifted one byte; harmless for
  ASCII, corrupts binary). n bytes copied to `tmp_cfg.dat`.
- No CFG marker or no DAT line → error 330.

## `.cff` writer (`0x44d0f0`)
- `rev_year < 2013` → error 187 "В формате .cff можно сохранять только
  осциллограммы стандарта comtrade'13 и новее".
- Saving onto the document's own path rewrites only `<base>.set`.
- Sections (string 188 `--- file type: %s---\r\n`, CP1251): `CFG ` + the
  regenerated cfg (CRLF lines); then each temp-folder file whose extension is
  not cfg/dat as `<EXT> ` + contents (folder order); with no temp folder only
  `HDR ` + the header text if non-empty; last string 189 `DAT BINARY: %d` with
  n = byte length of the generated DAT → `--- file type: DAT BINARY: <n>---`,
  then the raw DAT bytes.

## `.osc` (OSC2) reader (`0x4524b0` → `0x4526a0`, samples `0x453010`)

All numbers big-endian. 128-byte blocks, in order:
| block | offset | contents |
|---|---|---|
| 0 | 0 | byte0 major, byte1 minor version, byte2 = H (header blocks incl. block 0, calibration and text blocks), byte 0x7D ≠ 0 → K = 2 calibration blocks else 1 |
| 1..K | 128·i | calibration: f32 F1[8] @0, i16 S1[8] @0x20, f32 F2[8] @0x30, i16 S2[8] @0x50; analog channel i → block i/8, slot i%8; a = F1/(F2·3276.8) (1 if F2 = 0), b = −(S1+S2)·a, non-finite → a = 1, b = 0 |
| K+1..H−1 | 128·i | text, one section per block (ANSI, CR/LF tokens): tag = chars 0..3 exactly `nnA`/`nnD`/`nnF`, count `_wtoi` of chars 4..6, then that many lines (empty past the block's end); `nnA` → analog lines of exactly `idx,name,phase,unit` or `idx,name,phase,ccbm,unit`, untrimmed (else no fields); `nnD` → digital `idx,name,normal` (the name ends at the comma searched from two characters past the first, no phase; L9.198); empty name → "Резерв."; `nnF,<N>` samples per data block (mandatory); other tag / no F → error 330 |
| H | 128·H | info: i16 @2 max data blocks; i16 @0x0E pre-trigger blocks; BCD @5..0xB = ms/10, s, min, h, day, month, year−2000 (trigger); f32 @0x40+4i ratio (0.1 < r < 1e5 else 1) → primary rating, not used in values |
| H+1.. | 128·(H+1+j) | data, fixed 128 bytes, until the max count or EOF (a partial last block ignored): N sample records from offset 0, each nA i16 then ⌊nD/16⌋ u16 (bit b of word w → digital 16w+b, LSB first); i16 timing code @0x7C |

- Value = raw × unit multiplier (`0x422b10`: micro "мк"/"mk"/"u"/"µ" 1e-6, milli
  1e-3, full table NOT DETERMINED) × a + b.
- Block rate Fs = round(1e6/(code·0.03125·M)), M = analog channels whose unit is
  V/В/A/А/empty; < 1 → keep the current rate. The first block's rate is snapped
  (`0x453db0`, fnom 50): [14400,22000]→19200, [8000,14400]→9600, [5600,8000]→6400,
  [4000,5600]→4800, [2800,4000]→3200, [2000,2800]→2400, [1400,2000]→1600,
  [1000,1400]→1200, [700,1000]→800, [500,700]→600, [350,500]→400, [250,350]→300,
  [160,250]→200; fnom 60: [17280,26000]→23040, [9600,17280]→11520,
  [6720,9600]→7680, [4550,6720]→5760, [3110,4550]→3340, [2400,3110]→2880,
  [1680,2400]→1920, [1200,1680]→1440, [840,1200]→960, [600,840]→720,
  [420,600]→480, [300,420]→360, [200,300]→240; otherwise round(Fs/f)·round(f).
  That is the document rate (default 2400, fnom 50). Later blocks are not
  snapped; a change appends a rate segment. Per-sample time (µs, integer) grows
  by ⌊1e6/current rate⌋.
- Start = trigger − (pretrig·N − 1)·⌊1e7/round(Fs)⌋ (100-ns units).
- Version 2.3/2.4/2.5/2.8 → gain 1.0128, 2.6/2.7 → 1.00259, 2.9 → 1.014065: the
  record's harmonic-1 gain used by `fourie:` (L6).
- Device name string 3063 "OSC Осциллограф. Версия %i.%i"; file type "ASCII";
  revision year empty (cannot be saved as .cff).

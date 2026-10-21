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
  "<file> - <message>".

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
- 128-byte blocks, big-endian numbers.
- Block 0: byte0 major, byte1 minor version (device name string 3063
  "OSC Осциллограф. Версия %i.%i"); byte2 = header block count H; byte 0x7D ≠ 0
  → K = 2 calibration blocks else 1. Version 2.3/2.4/2.5/2.8 → gain 1.0128,
  2.6/2.7 → 1.00259, 2.9 → 1.014065 (the `fourie:` harmonic-1 gain, L6).
- K calibration blocks (8 analog channels each): f32 F1[8] @0, i16 S1[8] @0x20,
  f32 F2[8] @0x30, i16 S2[8] @0x50; a = F1/(F2·3276.8) (1 if F2 = 0),
  b = −(S1+S2)·a; non-finite → a = 1, b = 0.
- H−K−1 text blocks (ANSI, split on CR/LF): first line chars 0..3 = tag, 4..6 =
  count c. `nnA` → next c lines analog `idx,name,phase,unit` or
  `idx,name,phase,ccbm,unit` (first field ignored); `nnD` → digital
  `idx,name[,ph,ccbm],normal`; empty name → "Резерв."; `nnF,<N>` = samples per
  data block (mandatory). Other tag / missing F → error 330.
- Info block: i16 @2 = max data blocks; i16 @0x0E = pre-trigger blocks
  (pre-trigger samples = value·N); BCD @5..0xB = ms/10, s, min, h, day, month,
  year−2000 (trigger time); start = trigger − (pretrig·N − 1)/Fs; f32 @0x40+4i
  = ratio of analog channel i, outside (0.1, 1e5) → 1.
- Data blocks (to EOF or the block limit): N samples of nA i16 then nD/16 u16
  words (digital bit b of word w = channel 16w+b, LSB first);
  value = raw·ratio·a + b. i16 @0x7C = timing code: Fs = round(1e6/(code·
  0.03125·M)), M = analog channels with unit V/В/A/А/empty; rate changes
  between blocks become rate segments. Defaults 2400 Hz, 50 Hz.
- File type "ASCII", rev_year empty (cannot be saved as .cff); `<base>.set`
  loaded if present.

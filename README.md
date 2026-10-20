# FastView-REA

A clean-room, cross-platform (Linux **and** Windows) reconstruction in Rust of
**Механотроника Fast View 5** — the relay-protection oscillogram & characteristic
analysis tool shipped by НТЦ «Механотроника» (www.mtrele.ru) for their БМРЗ
microprocessor protection terminals.

The original is a 32-bit Windows MFC application with an Office-2007 ribbon UI.
This project reverse-engineers both its analysis engine **and its user
interface** and reimplements them as portable Rust that builds and runs the same
on Linux, Windows and macOS.

![FastView-REA — Office-2007 ribbon, lane-based oscillogram with dual cursors](docs/screenshot.png)

## UI fidelity (1:1 recreation)

The interface tracks the original closely, because the parts that define it are
**structured resources inside the binary**, extracted rather than guessed
(`notes/UI_SPEC.md`, dumps in `notes/ui/`):

- **Office-2007 ribbon** — quick-access toolbar (Файл menu + open/save/print/
  undo/redo), tabs *Главная* / *ОМП*, and the groups *Объединение · Диаграммы ·
  Видимость · Вычисления · Осциллограмма* (and *ОМП*), with original **vector
  icons** (clean-room line-art, not the proprietary bitmaps) and the **Стиль**
  skin switcher (Office-2007 Голубой/Чёрный/Серебристый/Зеленовато-голубой).
- **Exact keybinds** — the original's accelerator table verbatim: Ctrl+O/S/P
  (open/save/print), Ctrl+C/V (copy/paste), Ctrl+Z/Y (undo/redo). Locked in by a
  test against the extracted table.
- **Lane-based oscillogram** — the real primary view: each channel is a
  horizontal strip with a stats panel (name badge · value@cursor · Max/Min/СКЗ/
  A1), alternating lane colours, two vertical визиры with a shaded selection
  between them, a shared pan/zoom time axis (Ctrl+wheel), vertical scroll for
  many channels, and status-bar readouts.
- **On-waveform comments** — add/edit/delete annotations anchored to a sample
  (right-click → *Добавить комментарий*; click a marker to edit), drawn as the
  original's flagged markers.
- **Dialogs** — channel **Свойства** (double-click a lane: colour, invert,
  remove-DC, per-diagram visibility), the comment editor, and an **About** box
  (the Механотроника chevron, drawn clean-room).
- **Real context menus** — per-channel (colour, invert, remove-DC, value mode,
  show-on-diagram) and time-axis (мкс/мс/с/отсчёты, grid), reproduced from the
  binary's menu resources with their command ids.
- **MDI documents** — several oscillograms open at once as closable tabs.

The modern-ribbon look is a faithful egui rendering of the Office-2007 theme (and
its four skins), not a pixel-for-pixel GDI clone — that would require native
Win32/MFC and sacrifice the Linux/macOS target.

## What the tool does

A relay engineer picks a **device block** (e.g. *БМРЗ – Защита линий*), sees its
**protection characteristics** (distance protection ДЗ, differential ДЗТ/ДТО,
loss-of-field, out-of-step, power-direction РНМ, universal shapes…), edits the
**settings (уставки)**, and the **operate (срабатывание)** and **reset (возврат)**
boundaries are plotted live in the impedance (R–jX) or differential (Id–Ibrake)
plane. The full characteristic library (17 blocks, 62 characteristics) ships with
the app and is the real `FastView.fvb` from the original, so it is useful out of
the box. A different/updated `FastView.fvb` can be loaded via **Открыть .fvb…**.

## How it was reverse-engineered

Tooling: **REA** (Reverse Engineer Anything) driving **Ghidra 12.1.3** for native
decompilation, plus `7z`/`iconv` for the installer. Full notes:
[`notes/FINDINGS.md`](notes/FINDINGS.md).

1. **Installer** — the `Setup_FastView.exe` is a custom installer with an MSZip
   CAB embedded in its PE resources (`.rsrc/1049/BIN/137`). Extracted with `7z`
   it yields `FastView.exe` (MFC, MSVC 2013 x86), DLLs (`OscillHelper`,
   `ZipArchive`, …), the help `FastView.chm`, and the characteristic library
   `FastView.fvb` (UTF-16 XML).
2. **Data model** — `.fvb` is XML: `blocks → characteristic → parameters / vars /
   geometry`. Every numeric attribute is an expression in a small formula
   language (`tg sin cos sqrt mod` + `^` + arithmetic; angles in **degrees**;
   Cyrillic, case-insensitive identifiers).
3. **Engine confirmation (Ghidra)** — the `.fvb` parser dispatch was confirmed in
   `FastView.exe`; identifiers resolve through a name-keyed map; the renderer
   combines primitives with GDI region booleans (`PathToRegion`/`CombineRgn`),
   i.e. `type=and|or|not` → intersect / union / difference, with operate vs reset
   kept separate (`mode=on|off`).
4. **Validation** — the engine was validated by rendering the shipped library and
   checking the shapes against known relay characteristics (distance polygons,
   mho/sector, differential restraint curves, directional bowtie). See
   `docs/validation/`.

## Features

**Круговая диаграмма (impedance plane).** Device-block selection, per-characteristic
visibility, a property-grid settings editor (with inferred units / boolean / enum
widgets), operate + reset boundaries filled with holes punched out, a named legend,
auto-scale, zoom and pan. The real 62-characteristic library is bundled.

**Осциллограмма.** Load COMTRADE (`.cfg`/`.dat`, ASCII or binary) records; analog &
computed channels plotted against time with two movable cursors (визиры) and a live
RMS/angle readout at the main cursor. A synthetic demo fault record is bundled.

**Векторная диаграмма.** Phasors of all channels at the main cursor, voltages and
currents separately normalised, on a unit-circle reference.

**Быстрый анализ.** Symmetric components (3U0/U1/U2, 3I0/I1/I2) and loop impedances
at the cursor, plus a **formula editor** that adds per-sample computed channels using
the same formula language as the characteristics.

**ОМП (fault location).** Single-ended reactance method (and a double-ended estimator
in the engine) from the faulted loop at the cursor, with configurable line parameters.

**Impedance-trajectory overlay.** When an oscillogram is loaded, the phase-A impedance
locus between the cursors is drawn over the characteristics — the core relay-engineering
check of whether a fault entered a protection zone (see `docs/validation/impedance_overlay.png`).

Export: channels to CSV.

## Architecture

```
crates/
  fvb-core/     pure, cross-platform engine (no GUI, no platform deps)
    expr.rs     formula tokenizer + parser + evaluator (degrees, ^, Cyrillic)
    model.rs    Library / Block / Characteristic / Primitive + evaluation
    parse.rs    .fvb XML -> model (quick-xml)
    geom.rs     primitives -> regions, boolean combination (i_overlay)
    comtrade.rs COMTRADE (IEEE C37.111) oscillogram reader
    signal.rs   DFT phasors, RMS/DC/emergency, symmetric components, impedances
    analysis.rs detect three-phase channels, derive phasors/sequences/impedance loci
    omp.rs      single- and double-ended fault location
    tests/      engine + whole-library + oscillogram tests (21 tests)
                (+ 10 GUI tests in fastview-gui: keybinds, commands, helpers)
    examples/     headless PNG renderers + demo COMTRADE generator (validation)
  fastview-gui/ egui/eframe desktop app (Linux + Windows + macOS)
    app.rs        shared state, documents, commands, keybinds, CSV export
    theme.rs      Office-2007 skins (Голубой/Чёрный/Серебристый/Зеленовато-голубой)
    icons.rs      clean-room vector ribbon icons (no proprietary bitmaps)
    ribbon.rs     the ribbon: QAT, tabs, command groups, Стиль switcher
    lanes.rs      lane-based oscillogram: stats, cursors, comments, context menus
    dialogs.rs    About, channel properties, comment editor
    views.rs      круговая/дифференциальная/векторная/частотная/анализ/ОМП
assets/FastView.fvb        the real characteristic library (UTF-8)
assets/demo/demo_fault.*   bundled synthetic COMTRADE A-ground fault record
```

Core dependencies are pure Rust: `quick-xml`, `i_overlay` (robust polygon
booleans, mirroring the original's GDI region combination), `num-complex` (phasor
math), `eframe`/`egui_plot` for the UI, `rfd` for file dialogs.

## Build & run

Requires a stable Rust toolchain (1.75+).

```bash
# Linux / macOS / Windows (native)
cargo run -p fastview-gui --release

# run the engine tests
cargo test -p fvb-core

# render a characteristic block to a PNG headlessly (no GUI needed)
cargo run -p fvb-core --example render -- 0 line_protection.png

# validate the oscillogram -> impedance-overlay chain headlessly
cargo run -p fvb-core --example render_overlay -- overlay.png

# generate a fresh synthetic COMTRADE fault record
cargo run -p fvb-core --example gen_demo -- my_fault
```

In the app, use **Файл → Демо-осциллограмма** to load the bundled fault record,
then the tabs show the waveforms, phasors, symmetric components, fault location,
and (on the круговая диаграмма) the impedance trajectory over the zones.

### Windows

The code is cross-platform; build natively on Windows with the same command, or
cross-compile from Linux:

```bash
rustup target add x86_64-pc-windows-gnu
cargo build -p fastview-gui --release --target x86_64-pc-windows-gnu
```

(The GUI hides the console on Windows release builds.)

## Scope & fidelity

**Implemented:** the Office-2007 **ribbon shell** (QAT, tabs, command groups,
four skins), the original **keybinds** and **context menus**, **MDI documents**,
and the **lane-based oscillogram** (per-channel stats, dual cursors, selection,
time-axis units); the characteristic viewer (`.fvb` parsing, formula language,
geometry/region engine, impedance & differential plane, settings editing);
COMTRADE loading; phasor/vector diagram; harmonic (frequency) diagram; symmetric
components and loop impedances; the formula editor for computed channels; single-
& double-ended fault location; the impedance-trajectory overlay; and CSV export.
All backed by **31 tests** (incl. a keybind-table test) and headless render
validation in `docs/validation/`.

**Residual items** (documented in `notes/FINDINGS.md`), which need the original's
own files/environment to close faithfully rather than being guessed, or are
planned polish:

- **Native `.sto` binary reader** — `.sto` is a CRC-protected SCADA binary + XML
  metadata; no sample `.sto` ships with the installer, so its exact layout can't be
  verified. The tool loads **COMTRADE**, which is precisely what the original's
  `OscillHelper::ConvertSto2Comtrade` emits. Import the COMTRADE export.
- **Pixel-exact native chrome & print-preview** — the ribbon is a faithful egui
  rendering of the Office-2007 theme with the same structure, skins, keybinds,
  menus and icons, not a byte-identical GDI clone (which would require native
  Win32/MFC and drop Linux/macOS). Export (CSV/image) replaces the print spooler.
  The ribbon icons are **clean-room line-art** that evoke the originals; the
  proprietary toolbar bitmaps are deliberately not embedded (kept local).

## Legal

This is an independent, interoperable reimplementation created from behavioural
analysis for engineers who need to view these characteristics on Linux. It
contains no original FastView code. `FastView.fvb` is Mekhanotronika's publicly
distributed characteristic data file (downloadable from www.mtrele.ru), used here
as the data the tool reads, exactly as the original does.

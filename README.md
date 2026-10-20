# FastView-REA

A clean-room, cross-platform (Linux **and** Windows) reconstruction in Rust of
**Механотроника FastView** — the relay-protection *characteristic viewer* shipped
by НТЦ «Механотроника» (www.mtrele.ru) for their БМРЗ microprocessor protection
terminals.

The original is a 32-bit Windows MFC application. This project reverse-engineers
its characteristic engine and reimplements it as portable Rust that builds and
runs the same on Linux, Windows and macOS.

![FastView-REA plotting distance-protection characteristics](docs/screenshot.png)

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

## Architecture

```
crates/
  fvb-core/     pure, cross-platform engine (no GUI, no platform deps)
    expr.rs     formula tokenizer + parser + evaluator (degrees)
    model.rs    Library / Block / Characteristic / Primitive + evaluation
    parse.rs    .fvb XML -> model (quick-xml)
    geom.rs     primitives -> regions, boolean combination (i_overlay)
    tests/      engine + whole-library tests
    examples/render.rs   headless PNG renderer (validation)
  fastview-gui/ egui/eframe desktop app (Linux + Windows + macOS)
assets/FastView.fvb       the real characteristic library (UTF-8)
```

Core dependencies are pure Rust: `quick-xml`, `i_overlay` (robust polygon
booleans, mirroring the original's GDI region combination), `eframe`/`egui_plot`
for the UI, `rfd` for the file dialog.

## Build & run

Requires a stable Rust toolchain (1.75+).

```bash
# Linux / macOS / Windows (native)
cargo run -p fastview-gui --release

# run the engine tests
cargo test -p fvb-core

# render a block to a PNG headlessly (no GUI needed)
cargo run -p fvb-core --example render -- 0 line_protection.png
```

### Windows

The code is cross-platform; build natively on Windows with the same command, or
cross-compile from Linux:

```bash
rustup target add x86_64-pc-windows-gnu
cargo build -p fastview-gui --release --target x86_64-pc-windows-gnu
```

(The GUI hides the console on Windows release builds.)

## Scope

**Implemented faithfully:** the characteristic viewer — `.fvb` parsing, the
expression language, the geometry/region engine (polygon / polypoint / circle /
ellipse with and/or/not and operate/reset), the impedance & differential plane
renderer with auto-scale, grid, zoom (Ctrl+wheel) and pan, per-characteristic
visibility, and live settings editing. The real 62-characteristic library is
bundled.

**Not implemented (the original is a large oscillogram suite):** COMTRADE/`.sto`
oscillogram loading and fault-impedance overlay, fault location (ОМП), the vector/
frequency diagrams, the ribbon UI chrome and print/preview. These are documented
as out of scope in `notes/FINDINGS.md`; the overlay feature is the natural next
step (the original converts its `.sto` format to COMTRADE via `OscillHelper`).

## Legal

This is an independent, interoperable reimplementation created from behavioural
analysis for engineers who need to view these characteristics on Linux. It
contains no original FastView code. `FastView.fvb` is Mekhanotronika's publicly
distributed characteristic data file (downloadable from www.mtrele.ru), used here
as the data the tool reads, exactly as the original does.

# FastView 5.2.1 — Reverse-Engineering Findings

Target: `Setup_FastView.exe` (downloaded as `Sеtuр_FаstView ver. 5.2.1.exe`, Cyrillic-mangled name).
SHA-256 (installer): `20d2909ba5c9a917c4924a869fa6c1b726583accef083ad82bc0c1aad4a44eaf`

## What it is

A Windows desktop tool by **НТЦ «Механотроника»** (NTC Mekhanotronika, www.mtrele.ru), a Russian
manufacturer of microprocessor relay-protection terminals (БМРЗ series). FastView is an **oscillogram
(fault-record) analysis suite for relay engineers**. The product name/version from the PE version
resource: `Программа установки приложения Механотроника FastView`, FileVersion `5.2.1.60491`.

### Installer
- Custom installer (resource-embedded MSZip **CAB** at `.rsrc/1049/BIN/137`, 11.7 MB).
- Payload: `FastView.exe` (5.4 MB), `Uninstall.exe`, `vcredist_vs13_x86.exe` (→ MSVC 2013, x86),
  `FastView.fvb` (characteristic library, UTF-16 XML), `FastView.chm` (help), and DLLs
  `OscillHelper.dll`, `ZipArchive.dll`, `CommonInclude.dll`, `Utils.dll`, `FastView_ResEng.dll`.
- Extract on Linux: `7z x setup.exe` then `7z x 137` (the CAB).

### Tech stack of FastView.exe
- **MFC 12.0 Unicode** (`mfc120u.dll`) **Feature Pack** app: ribbon UI (`CMFCRibbonBar`),
  `CMFCVisualManager`, `CMFCPropertyGridCtrl`, docking toolbars. x86, MSVC 2013.
- Expression-engine string tokens (`tg: sin: cos: mod: ctg:`) live in `FastView.exe`.

## Full feature set (from FastView.chm table of contents)
Oscillogram viewing; channel management; merging oscillograms; **vector/phasor diagram**;
**круговая диаграмма (impedance R–X plane)**; **distance-protection characteristics** (the `.fvb`);
frequency diagram; differential diagram; **quick analysis** (symmetric components, zero sequence,
phase/phase-phase impedances, differential currents, **formula editor** → `.frm` files);
**fault location (ОМП)** single- & double-ended, isolated/grounded neutral; print/preview.

## The `.fvb` characteristic library (fully recovered — it ships with the app)
UTF-16 XML. Structure:
```
<blocks>
  <block name="…device family…">
    <characteristic name="…" dimension="OHM|DIF">
      <parameter name="…" default="…"/>          ← engineer-settable relay settings
      <var name="…" expression="…"/>              ← computed intermediates (ordered)
      <polygon [type="and|or|not"] mode="on|off"> <line k=".." b=".." type="1|2"/> … </polygon>
      <circle  [type="and|or"]     mode="on|off" radius=".." xcenter=".." ycenter=".."/>
      <ellipse [type="and"]        mode="on|off" xcenter ycenter width height angle/>
      <polypoint mode="on|off" closed="on|off"> <line x=".." y=".."/> … </polypoint>
    </characteristic>
  </block>
</blocks>
```
- 17 blocks, 62 characteristics in the shipped library (БМРЗ line/generator/motor/transformer/
  busbar/traction protection, universal shapes, power-direction relay РНМ).
- All geometry attribute values are **expressions** in the same formula language.
- `mode="on"` = operate boundary (срабатывание); `mode="off"` = reset/return boundary (возврат,
  typically ×1.05 of operate). Help: both are shown in **secondary ohms** on the круговая диаграмма.

### Expression language (observed over all 62 characteristics)
- Functions (postfix colon, single parenthesised arg): `tg: sin: cos: sqrt: mod:` (code also
  references `ctg:`). **Angles in degrees.** `mod:` = absolute value.
- Binary operators `+ - * /`, parentheses, unary minus. No power/compare operators in data.
- Identifiers may be Cyrillic or Latin; reference `<parameter>` names and earlier `<var>` names.
- Evaluation order: parameters (defaults, user-overridable) → vars top-to-bottom → geometry attrs.

### Geometry semantics (hypotheses — confirm against decompile `notes/fastview_engine_decomp.txt`)
- `<line k b type>` inside `<polygon>`: a boundary line → half-plane. **type="1": y = k·x + b**
  (b is the jX-axis form); **type="2": x = k·y + b** (near-vertical lines, b is the R-axis form).
  Polygon interior = intersection of its lines' half-planes.
- `<polygon>` combination by `type`: none/base, `and` = intersect, `or` = union, `not` = subtract
  (load-encroachment cutout). Accumulated separately per `mode` (on vs off).
- `<circle>`: disk; `type` and/or combine like polygons. A primitive whose size evaluates to 0
  (e.g. `radius`→0 via a `тип` selector) simply drops out — this is how one `<characteristic>`
  parameterises alternative shapes (polygon vs mho-circle) selected by a `_тип` parameter.
- `<ellipse>`: centre (xcenter,ycenter), axes (width,height), rotation `angle` degrees.
- `<polypoint closed>`: explicit vertex list (x,y expressions). `closed="off"` is still a
  filled region, closed through a box around the view (ДЗТ restraint curves → operate region
  above the curve) — superseded by the decompiled behaviour in the record's L4 ([L4-geometry.md](L4-geometry.md)).
- `dimension`: `OHM` → impedance plane (R on x, jX on y); `DIF` → differential plane
  (I_brake on x, I_diff on y). Same renderer, different axis labels/units.

## Reconstruction target (Rust, cross-platform Linux + Windows)
Faithful reimplementation of the **characteristic plotter** core: `.fvb` parser + expression
evaluator + geometry/region engine + 2D impedance/diff-plane renderer, shipping the real library.
Best-effort/later: COMTRADE oscillogram load + impedance-locus overlay. Out of scope (documented):
full ribbon UI, print preview, other diagram types, ОМП fault location.

## Decompilation confirmations (Ghidra via REA, snapshot cached)
- `.fvb` parser `FUN_0040c7e0` tag/attribute dispatch matches the model exactly (block→name;
  characteristic→name,dimension; parameter→name,default; var→name,expression;
  polygon→type,mode + line(k,b,type); polypoint→mode,closed + line(x,y);
  circle→xcenter,ycenter,radius,mode,type; ellipse→xcenter,ycenter,width,height,angle).
- Variables/parameters resolved by **name via a std::map** (`FUN_00408ab0` is the tree lookup),
  evaluated in document order (parameters then vars then geometry attributes).
- App RTTI confirms architecture: `CFastViewApp/Doc/View`, `CircleDiagramWnd`/`CircleDiagPane`
  (impedance-plane render surface), `Calculator`/`CalcChannels` (expression engine),
  `BlockPropertyGrid` (settings editor), `Circle`/`AreaItem` (geometry).
- **Region combination mechanism**: binary imports GDI `Polygon`, `Ellipse`, **`PathToRegion`**
  → `type=and|or|not` is Windows region boolean (CombineRgn RGN_AND/OR/DIFF). Each `<polygon>`'s
  lines therefore become an ordered vertex polygon (consecutive-line intersections) that is filled;
  characteristics combine by intersect/union/difference. Rust port uses `geo::BooleanOps` — faithful.
- **Angle unit = degrees**, conclusive from the literal idiom `tg:(90-F2)` (complement-to-90°).
- Native oscillogram format is **`.sto`**, converted to COMTRADE by `OscillHelper::ComtradeConverter`
  (`ConvertSto2Comtrade`, `ReadFromXml`, `GetInfoFromStoHeader`) — relevant to the later overlay feature.

## Oscillogram formats
- The native oscillogram is **`.sto`** (Mekhanotronika SCADA format): a CRC-protected
  binary (`GetInfoFromStoHeader` seeks to 0 and reads a 26-byte header — a u32 at
  offset 0 and a u16 channel count) plus XML parameter metadata parsed into a
  `ParamsTreeNode` tree (`ReadFromXml`/`ArmXmlHelper`, nodes `Freq`,
  `FreqAnalogInpName`, …). `OscillHelper::ComtradeConverter::ConvertSto2Comtrade`
  converts `.sto` → **COMTRADE** (`.cfg`/`.dat`), the open IEEE C37.111 interchange
  format. No sample `.sto` ships with the installer, so the full binary layout is
  not verifiable; the reconstruction implements the validated COMTRADE path (what
  the converter emits) and treats `.sto` as recognised-but-convert-first.

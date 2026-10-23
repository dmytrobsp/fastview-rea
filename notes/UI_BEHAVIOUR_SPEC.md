# UI layer — behaviour of the original (ledger L9)

Distilled from the decompiled `FastView.exe` (MFC 12 Feature Pack, image base
0x400000) in seven research scopes: A view input, B view painting, C commands
and CMDUI, D frame/ribbon/panes, E diagram windows, F dialogs and bars, G ОМП
UI. Behaviour in our own words; addresses are evidence. **[R]** = read in code,
data or resources; **[I]** = inferred (MFC idiom, argument shape, help page or
screenshot). MFC ordinals were resolved by call shape only, so every MFC API
name is [I] even where its arguments are [R]. String ids refer to
`notes/ui/strings.txt`. Engine math lives elsewhere: views/loci/spectrum in
`VIEWS_SPEC.md`, ОМП in `OMP_SPEC.md`, files in `FORMATS_SPEC.md`.

Notation. `doc` = CFastViewDoc, `view` = CFastViewView (`view+0x74` = doc),
`ch` = a channel record (stride 0x160 in `doc+0x184..0x188`), `N` = record
sample count, `f` = horizontal scale (`view+0xf0`), `first` = first visible
sample (`view+0xfc`), `leftW` = stats-panel width (`view+0x108`), `H`/`W` =
view client height/width, `A` = bottom band height (30, or 75 when
`doc+0x870`). Colours are RGB.

**Channel flag word `ch+0x60`** [R, 0x42f4b0 and the commands]:

| bit | meaning | bit | meaning |
|---|---|---|---|
| 0 | visible | 8 | display mode RMS |
| 1 | on frequency diagram | 9 | display mode 1st harmonic |
| 2 | on vector diagram | 10 | remove DC |
| 3 | on circle diagram | 11 | invert |
| 4 | stats rows Max/Min | 12 | ADC counts |
| 5 | stats row A1 (1st harmonic) | 13 | computed channel [I] (blocks copy, ADC, rename) |
| 6 | stats row СКЗ | 14 | pasted, pending merge (set by paste, cleared by «Объединить») |
| 7 | display mode instantaneous | | |

Other channel fields [R]: `+0x5c` kind (0 analog, 1 digital/discrete); `+0x89`
primary-values byte; `+0xa8/+0xb0` max/min of the displayed series; `+0xd0/+0xd8`
primary/secondary coefficient; `+0x100` **selected**; `+0x108` own rate (0 = the
record rate `doc+0x228`); `+0x110` digital fill brush; `+0x114` vertical window
zoom active with window `+0x118..+0x138` (reset values 0, 1.0, 0, 1.0, 0);
`+0x140/+0x144` custom-colour flag/COLORREF. `doc+0x1d8` = analog (file)
channel count [I], `doc+0x1dc` = digital count.

**Визир block** `doc+0x280`, stride 0x18 [R]: V1 `+0x280` shown / `+0x284`
sample; V2 `+0x298/+0x29c`; V3 `+0x2b0/+0x2b4`; H1 `+0x2c8` shown / `+0x2d0`
value (channel units) / `+0x2d8` lane (−1 = never placed); H2 `+0x2e0/+0x2e8/
+0x2f0`. "Cursor 1" of the diagrams and ОМП = V1, "cursor 2" = V2.

---

## 1. Window architecture

### 1.1 Main frame [R unless marked]
- `CMainFrame` is a CMDIFrameWndEx (vtable 0x4cbd00, ctor 0x4720b0, OnCreate
  0x4728d0). Application look from setting `ApplicationLook`, default **215**
  (Office 2007 Blue).
- OnCreate order: MDI tabbed groups → `SetDockingMode(DT_SMART)` →
  `EnableAutoHidePanes(any edge)` → windows dialog («Управление дочерними
  окнами», menu item «Окна...») → ribbon (0x472f30) → status bar (0x4729b0) →
  caption bars (0x474c80) → docking panes (0x474e10) → 100 ms timer id 100.
- **Tabbed MDI**: OneNote-style 3D tabs, no tab icons, **auto colour per
  document** (mfc120u's 16 colours, picked by position with two process-wide
  statics — the first tab is the yellow (255,220,120); L9.63), document-menu button "▾" (open documents + «Окна...» = MFC's windows dialog, L9.64), **close
  button on the active tab only**. Tabs can be dragged into new horizontal or
  vertical tab groups [I, MFC]. Child frame is the stock CMDIChildWndEx;
  template IDR 130 (CFastViewDoc, CMDIChildWndEx, CFastViewView).
- **View activation** (0x45f7d0): set active view; show/hide the ОМП result
  bar from the document's result flag (0x454310); re-bind all four diagram
  windows to the active document (0x475420); invalidate the parameters bar;
  window title = «Механотроника FastView - <doc title>».
- Minimum track size **866×500** (0x4728a0). Background strips PatBlt 0xF0F0F0
  (0x472750).
- Deferred repaint: each diagram pane's WM_SIZE posts command 32831, which sets
  a dirty flag; the 100 ms timer invalidates the frame once (0x472700). Net
  effect: at most one full repaint per 100 ms after pane resizes.
- WM_CLOSE (0x4753a0): kill timer; hide the ОМП bar and the merge bar (so
  neither is restored next session [I]); clear active view; set closing flag;
  base close.
- WM_ACTIVATE stores the frame HWND in a 4-byte global in a PE section
  `SHARED` (visible to all instances). WM_COPYDATA with `dwData == 0` opens
  the passed path (FileOpen shell command), activates, restores if iconic, and
  brings the window to the front (topmost then not-topmost).
- Message 0x40A (WM_USER+10): runs and deletes a posted task object on the UI
  thread; ОМП uses it to surface errors (§8.2).
- Command 32830 (posted when the ОМП bar is hidden): reset the active
  document's ОМП result, UpdateAllViews, re-bind diagrams.

### 1.2 Docking panes — defaults [R, 0x474e10]
All four are CDockablePane (gripper, regular tabs, dock anywhere) and are
**created visible**; positions/visibility afterwards come from the saved
Feature-Pack workspace [I].

| pane | caption (str) | id | initial | docking |
|---|---|---|---|---|
| Vector `+0x328c` | Векторная диаграмма (3129) | −2 | 500×500, right | DockPane |
| Circle `+0x43c8` | Круговая диаграмма (3262) | −3 | 500×500, right | **tab in the Vector pane group**, Vector stays active |
| Differential `+0x5930` | Дифференциальная диаграмма (160) | −5 | 500×500, right | separate DockPane on the right edge |
| Frequency `+0x2e34` | Частотная диаграмма (3527) | −4 | 200×200, **bottom** | DockPane |

- Pane chrome: caption with pin (auto-hide) and close; float by drag with
  smart-dock markers; close only hides [I, MFC]. A user screenshot (clip0014)
  shows Vector | Circle | Differential as one tab group with tabs at the
  bottom and Frequency below it.
- Pane content: a vertical toolbar on the right edge (resource 550 vector,
  552 circle, 553 diff; Frequency has none) and the diagram window (§6).
- Toggles (0x475040/0x4750b0): 32809 vector, 32808 circle, 140 diff, 32811
  frequency → `ShowPane(!visible)` + RecalcLayout; 32810 does nothing. CMDUI
  (0x4750f0): checked = pane exists and is visible.
- Diagrams are independent of the oscillogram, which never disappears; they
  show the active document's cursor 1.

### 1.3 Bars under the ribbon [R, 0x474c80]
Three CMFCCaptionBar subclasses in message-bar mode, docked top under the
ribbon (vertical order when several are shown: unknown; creation order 221 →
223 → 810).

| bar | id | height | default | content |
|---|---|---|---|---|
| ОМП result (CMsgCaptionBar) | 221 | auto | hidden | "i" bitmap 230 left; text (initially «   КЗ отсутствует.»); right button «Сохранить...» 32879, tooltip str 3472 — §8.3 |
| Merge (CMergeBar) | 223 | 70 | hidden | sync method combo, two channel combos, «Синхронизация», «Объединить», status text — §7.8 |
| Parameters (CParamOscBar) | 810 | 75 | **visible** | station / recorder / start / trigger / duration, display only — §7.9 |

### 1.4 Status bar [R, 0x4729b0]
CMFCRibbonStatusBar with six static panes (id 32838) of fixed width plus a
right-aligned extended element «НТЦ Механотроника» (32839). **No free-text
message pane.** Each pane holds one complete string (caption + value), set by
a cached setter that only repaints on change (0x472e10).

| # | caption when its визир(s) hidden | width template |
|---|---|---|
| 0 | Верт. визир 1: (3130) | 27 × "_" |
| 1 | Верт. визир 2: (3134) | 27 × "_" |
| 2 | Разница. верт. визиров: (3430; initial text «Разница в. визиров:») | 40 × "_" |
| 3 | Гор. визир 1: (3170) | 27 × "_" |
| 4 | Гор. визир 2: (3172) | 27 × "_" |
| 5 | Разница г. визиров: (3428) | 40 × "_" |

Values (0x4947d0) are **relative to the trigger sample** `doc+0x214`, by
RulerState (§3.6):

| RulerState | В1 / В2 | difference |
|---|---|---|
| 0 µs | `… %i мкс` (3132/3136) | `… %i мкс` (3432) |
| 1 ms | `… %i мс` rounded (3133/3137) | 3433 |
| 2 s | `… %.3f сек.` (199/227) | `… %.3f сек.` (228) |
| 3 absolute | `%02d:%02d:%02d.%03d` (240/241) | `%02d.%03d` (202) |
| 4 samples | `%i` = k − trigger + 1 (3131/3135) | `%i` (3431) |

V-difference shown only when both are shown. H panes: `Гор. визир N: %4g %s`
(3171/3173) with the SI-prefixed unit of that визир's channel (empty unit in
ADC mode). H-difference `Разница г. визиров: %g %s%s` (3429) **only when both
H визиры are on the same channel** (0x4960f0).

### 1.5 Start-up and process [R, InitInstance 0x4483f0]
1. Parse the command line. **Single instance**: if the shared HWND is alive and
   the command is FileOpen, send the path by WM_COPYDATA and exit when it is
   accepted. Without a file a second instance starts normally.
2. Locale Russian_Russia.1251 with numeric "C". If `FastView_ResEng.dll` loads
   it becomes the resource handle (English UI) and sets the language flag.
3. Splash bitmap 0x1CE, topmost, auto-closed after 2000 ms (when the command
   line asks for a splash).
4. Registry key «НТЦ Механотроника» (3281); **MRU of 10**; context-menu,
   keyboard and tooltip managers; hi-colour icons. File dialog filters
   .cfg/.cff/.osc/.zip and .sto (0x447e30).
5. Frame created; **DragAcceptFiles** (dropping files opens them).
6. Switches: `-Dst <dir>` (DST description folder), `-Find` (posts 32880
   Проводник at start-up); then the shell command (open the file argument).
7. Mutex «OOO Mekhanotronika SPb software» [I: installer]; show; store HWND.

Saved settings [R, 0x448c00 / 0x448da0]: `Print Parameters` (Left/Right/Top/
Bottom Margin default 2000, Page Orientation default 1, printer DEVMODE);
`Settings Parameters` (`Dst folder` "", `UTC for Comtrade2013` 1, `Display
Encoding` 1); `View` (`XScale`, `RulerState` 1, `YAnalogSize` 93, `Sells` 1);
`FileFinder` (§7.5); `ApplicationLook`; docking workspace [I, CWinAppEx].

App commands [R]: 57664 About (modal); 32880 Проводник (modal dialog, §7.5);
129 Настройки (modal, §7.4); 57605 page setup seeded from / written to the
stored margins; 57600/57601 MFC defaults. Save CMDUI (0x475750) **disables
Save for `.sto` documents**.

---

## 2. Ribbon [R, built in code 0x472f30–0x474c76]

`CMFCRibbonBarEx` (style 0x50002800). Panel icons 16×16 list 0x71. Image pairs
below are (small, large) indices; "L" large button, "S" small with text,
"i" small icon-only (empty label; the tooltip from the string table
"description\ntooltip" names it). Small buttons stack **3 per column**.

### 2.1 Application button and menu
Button image 0x23A, empty label, keytip **F**, tooltip «Файл», 45×45. Left
column in order (separators between every numbered group):

| # | id | label | img | notes |
|---|---|---|---|---|
| 1 | 57601 | Открыть... | 1,1 | |
| 2 | 57603 | Сохранить | 2,2 | |
| 3 | 57604 | Сохранить как... | 3,3 | |
| 4 | 32880 | Проводник | 8,8 | |
| 5 | 57607 | Печать ▸ | 6,6 | keys "p","w"; main part prints; sub-items: header label «Предварительный просмотр и печать документа», 57605 Параметры страницы (11,11), 57609 Предварительный просмотр (8,8), 57607 Печать (7,7), each with description |
| 6 | 32861 | Параметры файла... | 4,4 | |
| 7 | 32805 | Комментарии | 8,5 | |
| 8 | 32860 | Загрузка настроек... | 16,14 | |
| 9 | 129 | Настройки | 17,15 | |
| 10 | 57602 | Закрыть | 9,9 | |

Right column: «Последние документы», width 300, 10 MRU entries. No bottom
buttons (no Выход/Параметры), no mail/archive item.

### 2.2 Quick Access Toolbar
57601 Open, 57603 Save, 57607 Print — **three commands, no Undo/Redo**; the MFC
customise arrow is present.

### 2.3 Category «Главная»
| panel (icon) | elements in order |
|---|---|
| Объединение (0x1C) | 57637 Вставить (0,0) L, keytip v · 57634 Копировать (2) S, keytip c · 57643 Отмена (3) S · 57644 Возврат (4) S |
| Диаграммы (0x1C) | 32811 Частотная (5,2) L · 32809 Векторная (6,3) L · 32808 Круговая (7,4) L · 140 «Дифферен циальная» (39,11) L |
| Видимость (0x22) | 32862 Каналы (9,6) L · 32845 i(31) Показать дискретные · 32846 i(32) Скрыть дискретные · 32847 i(33) Скрыть неизменяющиеся · **sep** · 32802 i(14) Увел. по гор. · 32803 i(15) Уменьш. по гор. · 138 i(37) Приблизить по вертикали · 32800 i(16) Увел. по верт. · 32801 i(17) Уменьш. по верт. · 141 i(40) Вернуть к исходному размеру · 32813 i(18) Верт. визир 1 · 32814 i(19) Верт. визир 2 · 139 i(38) Верт. визир 3 · 32815 i(20) Гор. визир 1 · 32816 i(21) Гор. визир 2 · **sep** · 155 i(43) Момент времени · 157 i(42) Область · 156 i(44) Линия уровня |
| Вычисления (0x20) | 32817 i(25) Мин/макс · 32818 i(26) СКЗ · 32819 i(27) Оценка 1-й гарм. · 32820 i(28) Мгновенные · 32821 i(29) Среднеквадр. · 32822 i(30) 1-я гармоника · **sep** · 32823 Инвертировать (23) S · 32824 «Удал. постоянную» (24) S · 40000 empty filler (no image, keeps the 3-row column) · 32866 Коэффициенты (22) S · 32867 «Первичные знач.» (34) S · 32868 «Вторичные знач.» (35) S · **sep** · 137 Быстрый анализ (11,7) L |
| " " (one space, icon 0x26; help calls it «Информация») | 142 «Осцилло грамма» (41,12) L — toggles the parameters bar |

Resulting columns in Видимость: [32845/46/47] [32802/03/138] [32800/01/141]
[32813/14/139] [32815/16] [155/157/156] (matches clip0051) [I/R-screenshot].
Two-word big labels wrap at the space. Ids 32862, 57643, 57644, 40000 have no
string-table entry (no tooltip).

### 2.4 Category «ОМП»
Panel ОМП (0x21): 32807 Параметры (12,8) L · 32806 Анализ (13,9) L. Tooltips
«Параметры определения места повреждения» / «Произвести определение места
повреждения». **No contextual categories**; the print-preview tab is the stock
MFC one.

### 2.5 Right side of the tab row; Стиль; ribbon menu
1. **«Стиль»** menu button (right-aligned, tooltip str 287, description 288),
   menu 200: 210 Windows 2000 · 211 Office XP · 212 Windows XP · 213 Office
   2003 · 214 VS.NET 2005 · Office 2007 ▸ {215 Голубой, 216 Черный, 217
   Серебристый, 218 Зеленовато-голубой}. Handler 0x475250: 210–214 select the
   matching visual manager (212 also enables XP-themed 3D tabs; 213/214 smart
   docking); 215–218 set the Office-2007 style (LunaBlue / ObsidianBlack /
   Silver / Aqua) on the app's own Office2007 manager; then smart docking,
   full redraw, **persist `ApplicationLook`**, refresh views. CMDUI = radio on
   the current id.
2. **Help** split button (57667, icon 0): click opens the CHM; arrow opens menu
   342 {Помощь 57667, ─, О программе... 57664}.
- Right-click on the ribbon (0x475800) shows **one item only**: minimise the
  ribbon (MFC string 17086, checked when minimised). Double-click on a tab
  also minimises [I, MFC].

---

## 3. Oscillogram view (CFastViewView, map 0x45a970, vtable 0x4c9774)

### 3.1 Geometry and scales [R]
- **No header row.** Lanes start at y = scroll offset `view+0xf8` (≤ 0); the
  **bottom band** (time ruler, + difference band) of height `A` is fixed to the
  client bottom and never scrolls.
- **Horizontal model**: sample k at `x = leftW + round((k − first)·25·f)`.
  `f` is a power of two, at most 1.0 (25 px per sample). Fit value (0x45c430):
  start at 1.0, halve while `W/(N·25) < f`; this is the minimum (whole record
  visible). Visible count = `(W − leftW)/25/f − 1`. Channels with their own
  rate are scaled by `docRate/chRate`; pending pasted channels are drawn
  shifted by the merge offset `doc+0x6d8`. Pixel→sample rounds to nearest
  (tie up): `(x − leftW)/(25f) + first` (0x495980).
- Scrollbars (0x45d4a0): the view's own **window scroll bars** (SB_HORZ /
  SB_VERT). H: mask RANGE|PAGE|POS|DISABLENOSCROLL — always present, disabled
  when everything is visible; range 0..page+10000, page =
  `visible·10000/(N−visible)`, **line and page step both one screen**, thumb
  → `first = pos·(N−visible)/10000`. V: mask RANGE|PAGE|DISABLENOSCROLL,
  max Σ lane heights, page client H − A + 2 (both 0 when the lanes fit);
  line and page step both 100 px. Standard layout: H along the whole client
  bottom (under the stats column too), V down the whole right side. Document width = `N·25·f + leftW`; height = Σ lane heights + A
  (+2).
- **Lane heights** (0x460290): hidden 0; **digital 20 px**; analog
  `max(YAnalogSize, need)` with `need = hB + 10 + (bit4 ? 2hB) + (bit6 ? hB) +
  (bit5 ? hB) + (icons ? 23)`, `hB` = "Ay" height of Tahoma 15 bold
  (`view+0x84`), icon row reserved when any status icon applies (§3.3). A
  channel group (§4, 33035) is one lane of `members × (hB + 33 [+ hS if any
  stats row])`, ≥ YAnalogSize, members listed in the stats column. **No
  stretch to fit**: blank space below the last lane. `YAnalogSize` default
  **93**, global for all analog lanes, ×/÷1.3 within (70, 500), persisted.
- Lane order (0x467ed0): channel order (analog then digital), except that group
  members are pulled together at the first member's position.
- **Value → y** in an analog lane (0x4944a0, inverse 0x495b40):
  `y = top + h − 5 − round((v − min)·(h − 10)/(max − min))`, `y = top + h/2`
  when max = min. Range `[min, max]` = `[ch+0xb0, ch+0xa8]` of the displayed
  series over the whole record — **not symmetric**. With window zoom
  (`ch+0x114`) the range becomes the stored window (`[+0x128, +0x130]`, or
  ±1.5·`+0x130`, or `[0, max]` for RMS/harmonic modes); clipped to
  `(0, top+5)–(W, top+h)`. Groups share one range: min of mins, max of maxes.
- **Stats panel width** is auto-fitted every paint (0x4606d0): label column
  `view+0x10c` = width of «Max:» widened by a value template (`": -0.00000000 A"`
  for SI units, `": -0.0000000E-000 "` otherwise); `leftW` = max over lanes of
  (name width, name cut to 25 chars, + 10) + label column; in absolute-time
  mode at least `w("99/99/9999   ") + w("99:99:99.999")/2 + 5`. Default before
  measuring 125.

### 3.2 Paint order and frame [R, Draw 0x45f950]
Double-buffered (memory DC, BitBlt; no erase). Copy-to-clipboard renders the
same into an EMF; printing renders at 2× and halftone-stretches.
1. measure widths; 2. lane backgrounds, grid, separators, digital base bars,
trigger line (0x462ff0); 3. stats text (0x461440); 4. waveforms (0x45fbe0);
5. time ruler (0x464350); 6. difference band if `doc+0x870` (0x466630);
7. визиры (0x493110); 8. comments (0x43f630); 9. in rectangle-zoom drag
(mode 8): a vertical rubber line with ±5 px end ticks.

Colours and fonts (all Tahoma):

| element | value |
|---|---|
| fonts | 15 px weight 900 (values) · 15 px bold (names) · 13 px normal (stat rows, ruler, визир labels) |
| odd lanes (1st, 3rd…) | white, full width (stats + plot) |
| even lanes | vertical gradient (214,255,255) → (243,255,255) |
| selected lane (or group containing one) | gradient (254,225,140) → (253,252,173) |
| lane separator | 1 px (200,200,200) at `top+h−1`, x 0..W; same between group member rows in the stats column |
| panel/plot divider | 1 px (100,100,100) at x = leftW down to H−30 |
| grid («Сетка», `view+0x110`) | **vertical only**, every 50 px from leftW+50, y 0..H−30, 1 px (230,230,230), not tied to ticks |
| trigger line | if trigger `doc+0x214` ≠ 0: PS_DOT (35,194,87) at its x, 0..H−A |
| zero line | 1 px (200,200,200) leftW..W, only when min ≤ 0 ≤ max |
| waveform | 1 px solid in channel colour, **every sample** from `first` to the right edge, no decimation; 4×4 circles at each sample when f > 0.16 (> 4 px/sample) |
| digital base bar | (150,150,255) fill, leftW → end-of-record x, mid−2..mid+2 |
| digital "1" runs | rect mid−6..mid+6, outline 1 px (200,0,0), fill `ch+0x110` or (253,219,102) when selected; a run ending at the record end +4 px; < 0.5 px/sample → OR per pixel with 1 px marks for mixed pixels. Lane mid = top+10 |
| stats name (analog) | channel colour, bold 15, at (5, top+5), framed (128,128,128) rect (3, top+5)–(nameW+7, top+5+h) |
| stats value at В1 | heavy 15, (69,85,199), x = labelCol+5; `"-%-6g %s"` / `" %-6g %s"` on \|v\|; unit SI-prefixed, cut to 5 chars, "none" → empty; NaN not drawn. Read at В1 even when hidden |
| stat rows | normal 13, (0,0,127), label at x=5, value at labelCol+5, `" %-8g %s"` (`"%-8g %s"` if negative): bit4 → Max:, Min:; bit6 → СКЗ: (RMS at В1); bit5 → A1: (harmonic 1 at В1) — **only when the bit is set** |
| group lane (L9.50) | each visible member in channel order at top+5+k·S, S = rowH+33 (+hS with any stat row, its own flags): the name / value header above, then (with stat rows) one line `Max:v u|Min:v u|СКЗ:v u|A1:v u` (13 regular, (0,0,127), at top+8+k·S+hS), its icons at top−21+(k+1)·S; lane = N × the owner's S, no +10/+23 |
| digital name | heavy 15, channel colour, DrawText in (5, y, leftW, y+20), no frame; invert icon at (leftW−18, y+2) |
| status icons | bitmap 0x136 16×16, magenta key, right-to-left from x = leftW−21, step 21, at y = top+h−21: window zoom active, remove DC, invert, primary/secondary scaling (coefficient ≠ 1, not computed), RMS mode, harmonic mode, ADC counts. Resampled (bit14) channels: icon at (leftW+2, top+h−21) |

SI prefix (0x43d960) [R]: only for units V, A, VA, W, Ohm/OHM/ОМ, var, F, H,
GN, Hz, m; |v| < 1e-6 → ×1e9 н; < 1e-3 → мк; < 1 → м; > 1e15 П; > 1e12 Т;
> 1e9 Г; > 1e6 М; > 1e3 к; Latin prefixes when the language flag selects
English. Other units print raw.

### 3.3 Time ruler [R, 0x464350]
- Band: white rect (−1, H−30, W+2, H+1) with a black 1 px top border; text
  normal 13, (0,0,127).
- Ticks every **25 px**: `x = leftW + 25i`, i = 0..(W−leftW)/25; length 10 if
  i%4 = 0, 7 if i%4 = 2, 5 if odd; pen (100,100,100) [I].
- Labels only at i%4 = 0 (every 100 px), centred at y = H−20: value
  `t(first + i·⌊1/f⌋) − t(trigger)` → labels are not round numbers.
- **Unit only on the first label** (templates 386 «%s мкс», 387 «%s мс», 388
  «%s с»); others plain.

| RulerState `view+0xe8` | label |
|---|---|
| 0 µs | integer, space thousands grouping, leading "−" |
| 1 ms (default) | round(µs/1000), same grouping |
| 2 s | `%.3f`, 0 when \|v\| < 0.0005 |
| 3 absolute | start time `doc+0x250` (h:m:s) + (t − t_trig) as `%02d:%02d:%02d.%03d`, hours mod 24, ms rounded half up; date `doc+0x24c` `%02d/%02d/%04d` day first (parameters-bar month/day rule, years < 100 +2000) at (5, H−20); no unit |
| 4 samples | k − trigger + 1 |

Trigger `doc+0x214` is written only by a native-format loader; COMTRADE may
leave it 0 (labels start-relative) [unknown].

### 3.4 Difference band (`doc+0x870`, cmd 33033) [R, 0x466630]
- **On by default** (doc ctor 0x44b088). Bottom area 75 px = 45 px band above
  the 30 px ruler; vertical визир lines then end at H−55; the grid continues
  down to H−30.
- Each visible V-визир and each moment comment gets its time label
  (0x466170: plain `%i` µs / rounded `%i` ms / `%.3f` s / time of day /
  k+1 — **no unit, no grouping**; the |Δ| label: samples as a count,
  absolute as `h:mm:ss.mmm` of rounded ms, hours not wrapped) centred at its x at y ≈ H−55; moment comments
  add their text below it. Overlapping labels skipped (±3 px collision rects).
- For consecutive markers more than 16 px apart: a dimension line at y = H−65
  with 8×4 px arrowheads and end ticks; the |Δt| label centred in the span when
  it fits (> 20 px).

### 3.5 Визиры [R, 0x493110]
- x of V1/V2/V3: the sample is looked up among the drawn columns (offsets
  1..M at trunc(o·25f) when 25f ≥ 0.5, else the decimated columns 1..P−1)
  → leftW + xrel; not found → trunc((s − first)·25f) **without leftW**
  (quirk: a визир on `first` sits at 0 and is not drawn; one just past the
  right edge is drawn inside the plot). Truncation, not rounding.
- Lines: shown and x > leftW, 1 px solid, y 0 to H−30 (H−55 with the band);
  В1 (255,100,100), В2 (100,100,255), В3 (0,160,0); **В2 takes В3's green
  pen when x2 = x1** (even with В1 hidden). All lines first, then labels.
- Labels «В1/В2/В3», Tahoma 13 normal, opaque white text cell (no padding),
  DT_CENTER in (x − w/2, 0, x + 1 + w/2, h) at the DC's top. Shown визиры
  left of the plot clamp to x = leftW + 2 + 13·n, past W to W − 5 − 13·n
  (n per side, order В1 В2 В3 — the step is **sideways**). Unclamped labels
  closer than ⌊w1/2⌋ + ⌊w2/2⌋ merge with **one space**: «В1 В2 [В3]», else
  «В1 В3 [В2]», else «В2 В3»; the merged label keeps the first member's
  colour and x; clamped labels never merge (ledger L9.42).
- H1/H2: per channel — y from that channel's lane mapping; lines leftW..W
  while y < H − A, ГВ1 (255,100,100), ГВ2 (100,100,255) or В3's green when
  its y equals ГВ1's (clamped) y. **A label only when the line is out of
  view**: clamped to y = 0 (hanging below) or H − A (sitting above), «ГВ1»
  ending at W/2, «ГВ2» starting at W/2 (W = the whole client width), opaque.
- **No shading between В1 and В2.**
- Toggles (0x45c120, 0x492fa0): 32813 V1 / 32814 V2 / 139 V3 — if hidden and
  never placed (sample 0) → placed at **¼ / ½ / ¾ of the plot width** and
  shown; else visibility flips, position kept. 32815 H1 / 32816 H2 — if never
  placed (lane −1) → placed at content y 10 / 20 (top lane) [I: pixel args];
  else flips. CMDUI: enabled, checked = shown.
- Moving V1/V2 also copies them into the ОМП object (`+0x67c/+0x680`) and
  recomputes the ОМП result text when active (§8.2), then refreshes the status
  bar and all diagram panes.

### 3.6 Mouse input [R]
Drag state `view+0xe4`: 0 idle, 1/2/3 drag V1/V2/V3, 4/5 drag H1/H2, 6 merge
time-shift, 7 rectangle zoom armed, 8 rectangle being dragged, 9 comment drag.

**Button down** (L 0x45dc10; R 0x45a8d0 calls the same; SetCapture first).
Regions tested in order:
1. **Bottom band** (y > H − A): deselect all channels; nothing else.
2. **Stats panel** (x < leftW) — selection: without RMB or Ctrl, deselect all
   first; find the lane (or group member row) under y.
   - RMB: unselected → it becomes the only selection; selected → the
     multi-selection is kept (select-first before the menu).
   - LMB+Shift: select the range in lane order from the anchor (`view+0x114`,
     a channel name) to the clicked row.
   - LMB / Ctrl+LMB: toggle; plain click therefore = "only this one". The row
     becomes the anchor only when it gets selected.
   - Refresh diagram panes; drop modes 7/8.
3. **Plot** (x ≥ leftW, above the band):
   a. визир hit-test |Δ| < 3 px in order V1, V2, V3 (x) then H1, H2 (y), shown
      ones only; **LMB starts the drag (1..5)**; RMB never drags a визир.
   b. comment hit (§5) with LMB and the comment not hidden → mode 9.
   c. lane is a pending pasted channel (bit14) → mode 6 (time-shift drag).
   d. comment placement mode active (`doc+0x858`) and LMB → create (§5.3).
   e. otherwise: **Shift+LMB → H1, Shift+RMB → H2** in the analog lane under y
      (value from y, lane stored); else if mode 7 and LMB on an analog lane →
      mode 8, rectangle starts; else if no comment under the pointer → **LMB
      places V1, RMB places V2** at the rounded sample, shows it, updates
      status/ОМП/panes. Placement does not start a drag.

**Mouse move** (0x45ea70): hover tracking (TrackMouseEvent, 10 ms) when the
inline edit is not up; destroy it when the pointer leaves it. Modes 1/2: V1/V2
follow (sample-rounded) with live status and panes; 3: V3; 4/5: H follows y,
re-picking the lane (it may change lane) and recomputing the value; 6: time
shift (clamped, live redraw); 8: rectangle end clamped to the lane; 9:
comment drag (§5.4). Mode 0/7: LMB held with the pointer over the stats panel (no Ctrl / RMB) calls DoDragDrop for the selected analog channels (copy|move) — MFC then starts the drag once the pointer leaves [x−2, x+2) × [y−2, y+2) around that move or after 200 ms held, a release / press / Esc first cancels (L9.61); in the plot, recompute the
comment under the pointer (`doc+0x854`) and repaint on change. **The plot
never pans on drag.**

**Left up** (0x45e6b0): mode 8 → the rectangle's y-span becomes lane fractions
applied as a window zoom to that channel and its group (equal ends → reset that
channel's zoom); mode 9 (not from right-up) with a placement phase → finish the
comment (§5.3); always mode 0 and release capture.

**Right up** (0x45efa0) runs left-up first, then the context menu by region:

| region | menu |
|---|---|
| bottom band | MENU 560 (§4.9) |
| stats panel | MENU 561, acting on the selection (§4.13) |
| plot with a comment under the pointer | MENU 563, relabelled by kind (§5.5) |
| plot, no comment | **no menu** (the press already placed V2) |

Menus 358/385/386/556/557 are never loaded by the view (the five-colour submenu
33000–33004 is dead).

**Double-click** (0x465890): on a comment → comment text dialog only.
Otherwise the normal button-down runs, then if the click is on an analog
lane's **name badge** (x ∈ [4, nameW+7), y ∈ (top+5, top+5+textH)) an **inline
rename edit** opens over it (one CEdit, style 0x50800080, id 0x65); Enter or a
press outside commits, Esc cancels; the commit (`0x465df0`) takes the text
as typed (no trim, empty allowed); a name equal (case-sensitive, `wcscmp`)
to another channel's (file or computed) → message box 397 «Данное имя
канала уже используется.» (MB_OK) and the edit closes; every commit marks
the document modified, even a refused one (L9.62). Double-click never opens
channel properties.

**Wheel** (0x45d990):

| modifier | action |
|---|---|
| Alt held (takes precedence) | 32800 / 32801: YAnalogSize ×1.3 (if < 500) / ÷1.3 (if > 70), rounded; vertical scroll rescaled about H1's y (else H2, else top) |
| exactly Ctrl | 32802 / 32803: f ×2 (if < 1.0) / ÷2 (not below fit); anchored on **V1 if shown, else V2, else V3**: in `first' = c − (c−first)/2`, out `first' = 2·first − c`, clamped to [0, N − visible − 1]; no визир → left edge stays. Not mouse-centred |
| exactly Shift | horizontal scroll by one full screen per notch |
| anything else | if content > client: scroll lanes by the raw delta (120 px per notch), clamped to [−(content − H), 0]; else offset 0 |

All zoom steps persist `View/XScale` and `View/YAnalogSize` and mark the
document modified. (The max-scroll formula counts all digital channels, the
scrollbar only visible ones — original quirk.)

**Keys** (0x45f490; the only keys the view handles):

| key | action |
|---|---|
| ← / → | V1 −1 / +1 sample, clamped to [0, N]; **with Ctrl: V2** |
| ↑ / ↓ | H1 1 px up/down (value recomputed, may change lane); **with Ctrl: H2** |

Then status update and repaint. No Home/End/PgUp/PgDn/Delete. Accelerators
(table 128): Ctrl+O 57601, Ctrl+S 57604, Ctrl+P 57607, Ctrl+C 57634, Ctrl+V
57637, Ctrl+Z 57643, Ctrl+Y 57644.

**Cursor shapes** (0x45f660):

| condition | cursor |
|---|---|
| over a V-визир (±3 px), dragging V1–V3, or over an unlocked comment's part other than a level line's body (code 9) | size W–E |
| over an H-визир, dragging H1/H2, or over an unlocked level line's body (code 10) | size N–S |
| mode 7 or 8 | up-arrow |
| over a pending pasted lane, or mode 6 | hand |
| else | arrow |

**Hover popup** (0x469060): stats panel only. When the row under the pointer
(name, Max, Min, СКЗ, A1) is truncated (name > 25 chars, or the value
overflows its column), the inline edit shows the full text at that row; it
closes when the pointer leaves. These are the only tooltips in the view.

**Drop target**: the view registers an OLE drop target in WM_CREATE (channels
dragged from another view's stats panel) [I: handler not reversed].

---

## 4. Commands and CMDUI rules

Common tail of channel-changing commands ("full recompute") [R]: recompute
every channel (0x428490), re-evaluate computed channels (0x415780), re-run ОМП
if its result is shown (`doc+0x690` → 0x4a00b0(0)), SetModifiedFlag,
UpdateAllViews, refresh the diagram panes; visible H визиры are re-placed at
their previous **pixel** y. CCmdUI calls: Enable / SetCheck / SetRadio (bullet).
**Undo snapshots** (push `0x454530`, exhaustive call scan) are pushed only by
paste (`0x45ad50`, first), Параметры файла OK with a changed sample rate
(`0x44a24a`, then recompute, silent ОМП run always, SetModifiedFlag) and
«Объединить» (`0x47e834`, after the resampling, before the rate is set).
0x42e554 / 0x450c36 / 0x452f15 (earlier listed as callers) call `0x454960`,
the per-sample nominal-frequency table rebuild. History per document: deques
`doc+0x6ec` / `doc+0x700` (counts `+0x6fc` / `+0x710`), no depth limit, a
push clears redo; Undo `0x454640` / Redo `0x4548d0` swap the record
`doc+0x180…0x27f` (channels with samples, flags, selection, bit14; computed
channels; times; rates; header) and only repaint — визиры, comments, ОМП
state, merge offset and the modified flag are outside it. Invert, DC,
visibility, colour, zoom, comments are **not undoable** [R].

### 4.1 Edit / merge (view 0x45acb0, CMDUI 0x45bf60)
| id | effect | CMDUI |
|---|---|---|
| 57634 Копировать (Ctrl+C, ribbon, 561) | serialise the **selected** channels (analog and digital) with times, rates and header to the system clipboard, format «Mechanotronica FastView Clipboard Data» (keys `times`, `net_frequency`, `num_values`, `channels`/`discr_frequency`, `hdrFile`, `startTime`, `blockID`, `stationID`, `title`, `Comments`, `Codepage`) | enabled iff some selected channel is not computed (bit13) |
| 57637 Вставить (Ctrl+V) | undo snapshot; clear the ОМП result; deselect all; append the channels with their own time array and rate, flags 0x40e1 (bit14 pending); name collisions get "(2)", "(3)"… (no space; pasted channels come in selected, inserted before the first selected channel of their kind; the record comment is rewritten, L9.20); UpdateAllViews; fill and show the merge bar | enabled iff the clipboard holds the format **and** no channel has bit14 |
| 57643 Отмена (Ctrl+Z) | restore the last snapshot | enabled iff undo count `doc+0x6fc` ≠ 0 |
| 57644 Возврат (Ctrl+Y) | symmetric | enabled iff redo count `doc+0x710` ≠ 0 |

### 4.2 Zoom 32800–32803 (0x45c4f0, CMDUI 0x45c8c0) — whole view
See §3.6 wheel. CMDUI: 32800 iff YAnalogSize < 500; 32801 iff > 70; 32802 iff
f < 1.0; 32803 iff f > fit. No check state.

### 4.3 138 / 141
- **138 Приблизить по вертикали** (0x465460): leave comment mode; if mode is 0
  arm mode 7 (pressing again while armed leaves it armed). CMDUI: enabled with
  a document; **checked while mode 7 or 8**.
- **141 Вернуть к исходному размеру** (0x4654f0; ribbon and 561): for every
  selected analog channel with a window zoom, reset it and every member of its
  group; repaint if changed. Always enabled; no undo, no modified flag.

### 4.4 Визиры 32813–32816, 139
See §3.5. Always enabled; checked = shown.

### 4.5 Comment tools 155 / 156 / 157 (0x45c250, CMDUI 0x45c320)
Radio with toggle-off: `mode := (cur == k) ? 0 : k` (155→1 moment, 156→2
level, 157→3 area); off clears the drag phase; any of them cancels mode 7.
CMDUI: **enabled only for `.cfg` and `.osc` files** (last 4 chars lowered);
checked = active tool.

### 4.6 Stats rows / display mode / invert / DC (0x45c980 → 0x42f4b0; CMDUI 0x45c9a0 → 0x42fc60)
| id | target | effect | CMDUI |
|---|---|---|---|
| 32817 Мин/макс (bit4), 32818 СКЗ (bit6), 32819 A1 (bit5) | **all analog channels** | toggle-all (not all set → set all, else clear all) | disabled if no analog; checked iff all have it |
| 32820 Мгновенные (bit7), 32821 СКЗ mode (bit8), 32822 1-я гармоника (bit9) | **selected + members of their groups** | set the chosen bit, clear the other two | disabled if no selection; **SetCheck** (not radio) iff every target has it — mixed selection checks none |
| 32823 Инвертировать (bit11), 32824 Удалить постоянную (bit10) | **selected** (no group expansion) | toggle-all over the selection | as above |
Then the full recompute.

### 4.7 Discrete visibility / hide 32845–32848 (0x45ca10 → 0x430030; no CMDUI)
- 32845: show every **digital** channel. 32846: hide and deselect every digital
  channel. 32847: same, only digital channels with min = max (never change).
- 32848 Скрыть (561): hide and deselect each selected channel and every member
  of its group, and dissolve the group.
- Repaint and scrollbars; no modified flag, no undo. Analog visibility is only
  managed in the «Каналы» dialog.

### 4.8 Diagram membership / ADC 32849–32852 (0x45ca50 → 0x430340; CMDUI 0x45ca80 → 0x430650)
- 32849 frequency (bit1), 32850 vector (bit2), 32851 circle (bit3), 32852 ADC
  counts (bit12, computed channels skipped). Target: **selected analog**;
  toggle-all. 32852 adds the full recompute. All: modified, panes refreshed.
- CMDUI: disabled when the record has no analog channel or any selected analog
  channel is ineligible — tests on the **canonical unit** (`0x414c30`: the
  `0x422b10`-normalised unit, "" in ADC mode; «кА» → "A"), CompareNoCase:
  vector A, V, «ном» (str 162) or empty; circle Ohm (str 3060), W, VA or var;
  frequency and circle reject bit14 (ledger L9.34). Checked iff all selected analog have it (≥ 1). 32849–32851 use
  **SetRadio**, 32852 SetCheck.

### 4.9 Ruler / grid / difference band
- 32853 µs → 0, 32854 ms → 1, 32855 s → 2, 33007 абсолютное время → 3, 32856
  samples → 4; 32857 Сетка toggles `view+0x110`. Persist `View/RulerState`
  and `View/Sells`, mark modified, repaint, status refresh. CMDUI: units
  SetRadio on RulerState, grid SetCheck, all enabled.
- 33033 Разница между визирами (doc 0x4557d0, CMDUI 0x4557b0): toggle
  `doc+0x870` (band 30 ↔ 75 px); checked = flag.
- **MENU 560** (time band): мкс · мс · с · абсолютное время · отсчёты · ─ ·
  Сетка · ─ · Разница между визирами.

### 4.10 Coefficients / primary / secondary 32866–32868 (0x45c9c0 → 0x42fdb0; CMDUI 0x42ffb0)
- 32866 Коэффициенты: modal DIALOG 477 (§7.2); IDOK → modified.
- 32867 Первичные: `ch+0x89 := 1` on every file analog channel; 32868
  Вторичные: `:= 0`. Then full recompute and scrollbars.
- CMDUI: 32866 always enabled; 32867 checked = channel 0 primary, 32868
  checked = channel 0 secondary (a two-button radio).

### 4.11 Combine / split 33035 / 33037 (561 only)
- **33035 Совместить каналы** (0x467370): the selected visible analog channels
  plus the members of their groups; ≥ 2 required; all must share the sample
  rate, else error box str 239 «Выбранные каналы имеют разную частоту» and
  abort; dissolve their old groups; create one group keyed by a fresh GUID
  (`doc+0x174` map); reset each member's window zoom; hide H1/H2 if on a member
  lane [I]; then run 32820 (force instantaneous mode). CMDUI (0x468c80):
  ≥ 2 selected visible channels, all analog, identical non-empty unit, none
  bit14.
- **33037 Разъединить каналы** (0x467c40): dissolve every group containing a
  selected visible channel. CMDUI: some selected visible channel is grouped.

### 4.12 Colour 33030 (561; 0x465850 → 0x4310b0)
Standard colour dialog seeded with the first selected channel's colour; OK →
every selected channel gets the custom colour; modified; repaint.

### 4.13 MENU 561 (stats panel) — order
Частотная / Векторная / Круговая diagram (radio bullets, eligibility-gated) ·
АЦП · Скрыть · Мгновенные / СКЗ / Первая гармоника · Инвертировать · Удалить
постоянную · Вернуть к исходному размеру · Копировать / Вставить (for merge)
· Совместить / Разъединить каналы · Цвет канала. All act on the selection.

### 4.14 Document commands
| id | effect | CMDUI |
|---|---|---|
| 32805 Комментарии (app menu) | CCommentDlg on the record's HDR text (§7.10) | **enabled only for .cfg / .cff** (note: tools 155–157 use .cfg/.osc) |
| 137 Быстрый анализ | modal CQAnPropSheet (VIEWS_SPEC «The sheet»; L9.51); «Рассчитать» → modified, UpdateAllViews, panes | enabled iff analog count ≠ 0 |
| 32806 Анализ (ОМП) | toggle: result shown → clear it; else run (§8) | disabled while any bit14 channel; checked = result shown |
| 32807 Параметры (ОМП) | modal ОМП dialog; OK → modified, run unless bit14 channels | always enabled (with a document) |
| 32860 Загрузка настроек... | open dialog; load a record-settings file (computed channels + ОМП params, 0x44f5b0); modified, refresh | — |
| 32861 Параметры файла... | modal DIALOG 392 (§7.3) | — |
| 32862 Каналы | modal DIALOG 463 (§7.1) | — |
| 32879 Сохранить... (ОМП bar) | save the ОМП report (§8.3) | — |
| 1091 Синхронизация / 1092 Объединить | merge bar (§7.8) | — |
| 57607/57608 Печать, 57609 Просмотр | MFC print / print preview with the custom dialog (§7.11) | — |

With no document open, commands with no frame-level handler are greyed by MFC
[I].

---

## 5. Comments (review elements, «Рецензирование осциллограммы»)

### 5.1 Model [R]
Base item: `text`, `locked` (+8), `visible` (+9 — whether the **text label**
is shown; the marker is always drawn). Kinds (type() slot 1):

| kind | data | hit test (±10 px) | drag parts |
|---|---|---|---|
| 1 Moment (визир) | `position` sample | sample range of x ± 10 px contains position | 2 = whole |
| 2 Level line | channel name, `positionY` (value in channel units), `positionFrom`, `positionTo` (samples) | rect from−10..to+10 px, ±10 px around the line | 2 left end, 3 right end, 4 body (vertical move) |
| 3 Area | `channelFrom`, `channelTo` (lane names), `positionFrom`, `positionTo` | **border band only**: inside rect+10, outside rect−10 | 2 left edge, 3 right edge; the top / bottom band has no part (selects only, arrow cursor, no drag) |

Validity: Level needs its channel to exist; Area both channels. Manager in the
doc at `+0x840` (vector), `+0x854` = item under the pointer (= end → none),
`+0x858` tool mode, `+0x85c` drag part.

### 5.2 Painting [R, 0x43f630]
Clip (leftW, 0)–(W, H−A). Shared pens: 1 px (255,100,100), 3 px
(255,100,100), 3 px (192,0,0) (selected), red brush (255,0,0), normal 13 font.
"Selected" = the item under the pointer: `+0x854`, re-picked only by a mouse
move over the plot (x ≥ leftW, no drag) and kept while the pointer is over
the stats panel or outside the view (no mouse-leave handler); the painter
draws all items, then the selected one again on top (a selected area's fill
is laid twice) and its info box; print and copy use the same Draw, so the
selected item prints selected (L9.59).
- Moment: bubble icon at the top (x − iconW/2 + 4); vertical line below it to
  H−30 (H−55 with the band), 1 px (255,100,100), 3 px (192,0,0) when selected;
  its text appears in the difference band (§3.4); text box y offset 30 (75 and
  +20 with the band).
- Level: 3 px horizontal segment from→to at the y of `positionY` in its
  channel's lane ((192,0,0) when selected); filled red circles of radius 5 at
  both ends (1 px pen); bubble icon at (x2+10, y − icon/2); label at
  (x1+5, y+5) in (128,0,0): text + «Уровень: %.2f» (only when visible).
  Digital channels snap y to 0/1 (value > 0.5).
- Area: rect = sample range × lane span; red fill AlphaBlend **64/255**; then a
  3 px border (normal or selected colour), hollow; text lines bottom-left
  (5 px inset) in (128,0,0) incl. «Длительность: %d.%03d с».
- Info box for the selected item (0x43df90): RoundRect radius 10, pen
  (192,175,175), fill (255,230,230), text (0,0,192), padding 5, at the item's
  right edge + 3. Text (slot 9, also the level / area labels): `text` +
  "\n\n" + a kind line — Moment «Время: » absolute date/time (locale
  `Format()`) + `.%03d` ms + «\nОт начала ОСЦ: %d.%03d с» (empty text when
  the start stamp does not parse); Level «Уровень: %.2f»; Area
  «Длительность: %d.%03d с» of |t_to − t_from| — s and ms truncated
  (ledger L9.47).

### 5.3 Placement [R, 0x43eb40]
Only when the tool mode is set and no item is under the pointer:
- Moment: one LMB click creates it at the rounded sample; tool → 0; modified.
- Area: needs a lane under the cursor; created, enters drag part 1 (the "to"
  end and `channelTo` follow the cursor).
- Level: needs an **analog** lane; value from y via the lane scale; drag part 1
  (the "to" end follows).
- Release (0x43ef50): normalise from ≤ to, tool → 0, part → 0, modified.
  **Tools are one-shot** (the ribbon button pops out after one element).

### 5.4 Editing [R, 0x43efa0]
LMB → `0x43fab0`: the first **unlocked** element whose ±10 px test hits and whose part test (x only, the pointer ±10 px as a sample range) gives a part → mode 9 (10 for a level body); the drag `0x43efa0` then moves the **selected** element (`+0x854`) by that part unless it is locked — the text flag (+9) plays no part (L9.60). Moment moves `position`; Area part 2
moves from, 3 (or 1) moves to; Level part 2 from, part 3 to, part 4 moves the
line vertically (value recomputed only while over the same channel's lane).
Locked items do not move. Double-click → text dialog (CCommentDlg, item mode).

### 5.5 MENU 563 (relabelled per kind, 0x45f121) [R]
| id | item | kind labels (str) | behaviour / CMDUI |
|---|---|---|---|
| 33038 | Изменить комментарий | — | CCommentDlg with the text; OK → modified |
| 33039 | Зафиксировать / Разблокировать | визир 236/229 · линию уровня 238/237 · область 209/208 | toggles `locked`; checked = locked; modified |
| 33040 | Удалить | визир 206 · линию уровня 207 · область 205 | removes immediately, no confirmation; modified |
| 33041 | Скрыть комментарий / Отображать комментарий (203/204) | — | toggles `visible`; checked = hidden |
| 33042 | Изменить уровень | Level only (deleted from the menu otherwise) | inline edit at the line pre-filled `%.2f`; Enter commits `positionY`, Esc cancels |

### 5.6 Persistence [R, 0x4402d0 / 0x43fcf0]
Stored in the per-oscillogram settings tree (the `.set` sidecar; legacy
fallback `<path minus 3 chars>ini`) under `Comments` with lists `Moments`,
`Levels`, `Areas`; each item stores text (CRLF-normalised), locked (default 0),
visible (default 1) and its kind's fields. The same tree holds
`Oscillogram/DiscretisationFrequensy`, `Oscillogram/Codepage`, per-channel
nodes, `WrongPlaceFinding` (ОМП) and the diagram sections (§6.7).

---

## 6. Diagram panes [R unless marked]

Shared: every diagram window derives from `DiagramWnd` (map 0x441b70);
double-buffered `Draw(dc, rect, scale, font, doc)` (scale 1 on screen, > 1 when
printing; pixel constants × scale). **No keyboard, no double-click** handling.
Left click only focuses the pane.

### 6.1 Pane layout
- Pane (0x4360b0 / 0x436270): vertical CMFCToolBar (16×16) on the **right
  edge** `x = W − tb.cx`, full height; diagram window (bordered) gets
  `(0, 0, W − tb.cx − 1, H − 1)`.
- Circle and differential windows (0x4376e0 / 0x4431e0) contain a
  **property grid at the top** (BlockPropertyGrid: no header, text
  (40,40,40), left column default Wd/3; populated from the `.fvb` bases;
  clicking a value of a block property toggles it and repaints; double-click
  swallowed) of height `Hp` and the diagram below it.
- `Hp` from `Circle Diagram` / `Diff Diagram` → `Properties Height`, default
  **200**, persisted on every re-layout. **Splitter** = top 5 px of the diagram:
  size N–S cursor; drag changes Hp clamped to [50, paneH − 150]; painted in the
  Office-2007 theme colour (black (83,83,83), silver (208,212,221), aqua
  (196,202,217), blue (191,219,255)).

### 6.2 Circle diagram — zoom, pan, scrollbars
State: zoom z (default 1.0), pan ox, oy in grid cells.
- **No drag-pan, no box zoom, no pointer-anchored zoom.**
- Wheel (0x437ae0): exactly Ctrl → z ×1.5 / ×⅔; exactly Shift → ox ±2; else
  oy ±2 (notch up = +2). Pan accepted only if `|o| ≤ maxPan = round(spanPx/
  cellPx · z · 0.5)` per axis.
- Zoom-in accepted only if `charMaxAbs · pxPerUnit(z) < 400 px`; zoom-out
  unlimited; z ≤ 1 → pans reset to 0; in Draw, if `charMaxAbs·pxPerUnit >
  800·scale` → z forced to 1. Zoom always about the (panned) origin.
- `oy` positive moves the origin down, `ox` positive right.
- Scrollbars (0x437ca0): hidden (range 0) when z ≤ 1; else range 0..10000,
  page `round(10000/z)`, `M = 10000 − page`, `pos = M/2 − o·M/maxPan`;
  line/page ±100; `o = maxPan·(M/2 − pos)·2/M`.
- Toolbar 552 top→bottom: 33014 Каналы (pops the Каналы submenu) | 33015
  Диапазон между визирами ✓ · 33016 Характеристики возврата ✓ | 33017
  Re+jIm ✓ · 33018 |·|∠ ✓ | 33019 Перезагрузить базы | 33020 + (×1.5) ·
  33021 − (×⅔) | 33022 ↑ (oy −2) · 33023 ↓ (oy +2) | 33024 ← · 33025 →.

### 6.3 Circle diagram — scale and painting
- **Equal scale both axes, origin at the window centre (+ pan)**. Extent:
  `maxOhm` = max |coordinate| of the selected block's visible characteristics
  (operate and reset regions, whatever the reset-display flag), characteristics
  of other dimension → `maxOther`; then every locus sample in range (N..end, or
  [min(c1,c2), max(c1,c2)] with «Диапазон между визирами») updates maxOhm
  (unit Ohm) or maxOther (other units, i.e. VA); R/X pairs update maxOhm.
- Cell = `LOGPIXELSX·scale/5` px (≈ 19 px at 96 dpi); `cells =
  round(halfSize/cell)` [I: half the smaller side]; `unitsPerCell =
  nice(max/(z·cells))`, `nice(v)` = 1 significant digit rounded **up**. Two
  independent scales: Ω and "power" (W/var). Plot rect = client − 6 px, top +5
  (splitter), bottom −20 (legend) or − readout height; nothing drawn below
  121×121.
- Grid: white background; lines every cell outward from the centre (230,230,230);
  frame at ±cells·cell; axes through the origin when inside, darker
  ((200,200,200)/(150,150,150) — assignment [I]). Edge labels (50,50,50):
  coordinate at each frame edge in both scales — « R: a Ω, b W» left/right,
  « X: a Ω, b var» top/bottom; scale box = one cell filled (240,240,240) with
  the one-cell value in Ω and W to its right. SI prefixes per §3.2.
- Characteristics (0x43a4a0) drawn **in reverse order** (first on top), only
  when `charMaxAbs·pxPerUnit < 800·scale`: reset region (only with «Отображать
  характеристики возврата») = 1 px frame in its reset colour; operate region =
  fill clipped to the region at **alpha 50/255**, then 1 px frame. Fill always on.
  Palette by `char+0x68 mod 9` (operate / reset / fill):

  | i | operate | reset | fill |
  |---|---|---|---|
  | 0 | (1,37,255) | (88,112,253) | (25,245,253) |
  | 1 | (202,1,252) | (225,119,251) | (202,1,252) |
  | 2 | (34,177,76) | (50,230,29) | (34,177,76) |
  | 3 | (254,0,0) | (254,159,159) | (252,66,101) |
  | 4 | (0,162,232) | (153,217,234) | (0,162,232) |
  | 5 | (163,73,164) | (200,191,231) | (163,73,164) |
  | 6 | (63,72,204) | (112,146,190) | (63,72,204) |
  | 7 | (163,73,164) | (255,201,14) | (163,73,164) |
  | 8 | (136,0,21) | (185,122,87) | (136,0,21) |

- Loci per VIEWS_SPEC ("+" at c1 red, c2 blue).
- Legend (readout off): bottom 20 px, centred row; per circle channel then per
  enabled R/X pair: 10 px colour square, 6 px gap, name, 6 px gap.
  Characteristics are never in the legend.
- Readout table (ViewChannelType ≠ 0) replaces the legend: height
  `(textH+6)·rows + 12`, row height 1.5·textH, ruled (150,150,150); columns
  swatch, name, value at cursor 1 — mode 1 Re ± jIm (`%.3f`, SI unit), mode 2
  `%.3f` magnitude + angle column `%.2fº`; centred if narrower; falls back to
  the legend when the plot would be < 120 px tall.

### 6.4 Circle diagram — menu and settings
- Right-click (0x437440): MENU 387 = Каналы ▸ {Создать (33027), Показать все,
  Скрыть все, <channels>, ─, <R | X pairs>} · ─ · Отображать характеристики
  возврата ✓ · Диапазон между визирами ✓ · Показывать вещественную и мнимую
  части ✓ · Показывать действующее значение и угол ✓ · ─ · Перезагрузить базы
  характеристик.
- Channel items (0x43d220): analog, not bit14, unit **Ohm or VA**; ids
  35000+i, checked = bit3; pairs 35000+nCh+j. Toggle → recompute, modified,
  repaint.
- Readout toggles: 0 none / 1 Re+jIm / 2 |·|∠; clicking the checked one → 0.
- On document attach the circle flag is **cleared on every channel whose unit
  is not Ohm/VA**.
- Per-document section `Circle Chart`: SightsOnly (default off),
  ShowOffCharacteristics (default **on**), ViewChannelType (0), SelectedBlock;
  each toggle marks modified.

### 6.5 Differential diagram (0x442880) — differences
Same layout, wheel/zoom/pan/scrollbar rules. Toolbar 553: Каналы | Диапазон ✓
· Возврат ✓ | 33028 Показывать значения ✓ | Перезагрузить | + − | ↑ ↓ | ← →.
**One scale**: extent = max(characteristic extent, |Iдиф|, |Iторм| of enabled
pairs); edge labels/scale box single value. **Initial view**: once after the
first paint the origin is pushed to the **lower-left** (oy to +max, ox to −max
in 2-cell steps) showing quadrant I. Readout is boolean (33028, `Diff Chart`
`ViewChannelType`). MENU 388 = 387 with «Показывать значения» replacing the two
readout items; Создать edits the Iдиф/Iторм pairs.

### 6.6 Vector and frequency diagrams
- **Vector** (0x48f550): no zoom/pan/scrollbars/wheel. Toolbar 550: 33010
  Каналы ▾ | 33011 Отсчет углов ▾ | 33012 Показывать значения ✓ · 33013 Только
  направление ✓ (enabled with a document). Right-click MENU 355 with
  «Углы в радианах» (32841) **removed at run time**: Каналы ▸ {Показать все,
  Скрыть все, <channels>} · ─ · Отсчет углов ▸ {Абсолютный угол (36000), ─,
  <vector channels> (36001+i)} radio · ─ · Показывать значения ✓ · Только
  направление ✓. Channel list: units A, V, empty, or one more [unknown].
  Persist `Vector Chart`: AngleInRadians (stored value only), OnlyDirection,
  ShowAnglesValue, AngleChannelName. Painting: concentric circles
  `n = ⌊R/50⌋` → 4 if n < 4, 5 if 4..9, 10 if ≥ 10; outer circle solid
  (150,150,150), inner dotted (200,200,200); 12 radials every 30°; header
  «U max»/«I max» values; phasor = line in channel colour (solid for unit V,
  dotted otherwise) with a filled triangular arrowhead.
- **Frequency** (0x46ece0): no zoom/pan. Right-click MENU 351: Каналы ▸
  {Показать все, Скрыть все, <analog non-bit14 channels> (35000+i, check bit1)}
  · ─ · Настройки → DIALOG 353: «Значения величин» (◉ в процентах / ○
  абсолютные), «Нормализация» (◉ по основной гармонике / ○ по СКЗ — **disabled
  in absolute mode**), Число гармоник, Число обрабатываемых периодов. Persist
  `Frequency Chart`: NumPeriods 1, ShowPersents 0 (= **percent**), UseRMS 0,
  NumHarmonics 10. Painting: bottom 10 px reserved; dotted (200,200,200) grid,
  (150,150,150) axes, text (20,20,20); value labels Tahoma 13 **rotated 90°**;
  **one panel per channel side by side** (width (W − 2·margin)/nCh, K+1 bar
  slots), under each a colour square + name (+ ", %" in percent mode); x labels
  `k·f/P` as `%-.1f` (`%-.0f` when integral), only if they fit. Printing tiles
  panels in 1..10 rows for the page aspect.

### 6.7 Persistence
Per document (the settings tree): `Circle Chart`, `Diff Chart`, `Vector
Chart`, `Frequency Chart` as above; every toggle marks the document modified.
Panel height under `Circle Diagram` / `Diff Diagram`.

---

## 7. Dialogs and bars [R unless marked]

### 7.1 Каналы — DIALOG 463 «Отображение каналов» (ChannelsDlg)
- One owner-drawn report list (full-row select, grid lines), images bitmap 464
  (16×16, white mask). Columns: 0 «Каналы» (width = list − 185), 1–5
  image-only, 32 px, centred.
- Rows: file analog channels, digital group A (`doc+0x878`), digital group B
  (`doc+0x884`), then channels with kind 1. Text colour (150,150,150) for
  "not applicable" cells [I].
- Cell click toggles:

  | col | flag | allowed when |
  |---|---|---|
  | 0 | — | not computed (bit13), else message 398 «Переименование расчетных каналов запрещено» → **in-place rename** |
  | 1 | bit0 visible | always; hiding also dissolves the channel's group and deselects |
  | 2 | bit1 frequency | analog and not bit14 |
  | 3 | bit2 vector | canonical unit A / V / «ном» / "" (click: case-sensitive `0x42deb0`; paint: no case) |
  | 4 | bit3 circle | canonical unit Ohm / W / VA / var, no case (ledger L9.34) |
  | 4 / 5 | digital entry visible | digital rows: group B in col 4, group A in col 5 |

  Every click refreshes views and marks modified.
- **Header click** toggles the whole column: all applicable set → clear, else
  set all.
- Rename commit on OK or a click outside: duplicate → 397 and rejected;
  otherwise rename, redraw, refresh panes, modified.

### 7.2 Коэффициенты — DIALOG 477 «Коэффициенты трансформации»
- List: Канал · Первичное значение · Вторичное значение (33 % each). Rows =
  file analog channels **except computed**; values `%g` of `ch+0xd0` / `+0xd8`.
- Double-click a value cell → in-place edit. **Shift+click extends a
  rectangular cell selection** (highlight (100,100,255), white text); commit
  writes the typed text into **every selected cell of that column**; without
  Shift the selection clears.
- OK: nothing dirty → as Cancel; else parse each cell with `_wtof` (no
  validation, bad text → 0), store changed values, recompute those channels;
  modified.

### 7.3 Параметры файла — DIALOG 392 «Параметры осциллограммы»
Only a «Закрыть» button, which is IDOK (applies); Esc cancels.

| field | content |
|---|---|
| Число каналов / analog / digital | counts |
| Число измерений | N |
| Частота сети | double |
| **Частота дискретизации** | **editable drop-down**; list by line frequency — 50 Hz: 160000 80000 40000 32000 20000 19200 16000 10000 9600 8000 6400 5000 4800 4000 3200 2500 2400 2000 1600 1200 1000 800 600 500 400 300 200; 60 Hz: 192000 96000 48000 38400 24000 23040 19200 11520 9600 7680 6000 5760 4800 3840 3340 3000 2880 2400 1920 1440 1200 960 720 600 480 360 240; other → empty |
| Частота для коррекции расчётных значений | drop-list «Не корректировать» + `doc+0x1e4` candidates; disabled when none; selection `doc+0x874` |
| Станция / Номер регистратора / Начало записи / Момент запуска | text |
| Длительность | `%g c.` of (t_last − t_first)·1e-6 of the first non-bit14 channel |
| Кодировка | 866 MS-DOS / Windows-1251, **applies immediately on change**: station, recorder and every channel name re-decoded; persisted `Oscillogram/Codepage` |

On Закрыть: a changed sample rate → clear cache, store, recompute, silent ОМП
re-run, modified, persisted as `Oscillogram/DiscretisationFrequensy`; a changed
correction selection → store and recompute.

### 7.4 Настройки программы — DIALOG 510 «Установка настроек программы»
- .dst description folder (link, disabled when empty) + «...» folder picker.
- Radios «Отображать в UTC» / «Отображать местное время» (COMTRADE 2013
  times; default **UTC**).
- Encoding radios: **UTF-8 0, Windows-1251 1, IBM866 2, KOI8-R 3** (code pages
  65001 / 1251 / 866 / 20866); **no auto option**; default **1251**.
- Persisted in `Settings Parameters` (§1.5); takes effect on the next open
  (the dialog reopens nothing).

### 7.5 Проводник — FileFinder22Dlg, DIALOG 558 (modal)
- Layout: shell folder tree (left) with «Выбрать» and «Включая вложенные
  папки»; right: «Папка:» path, filter row Станция combo / Блок combo / С: date
  / По: date, report list, buttons «Удалить файлы», «Открыть файлы»,
  «Закрыть».
- List: multi-select, grid, full-row; columns Название файла (W/3, path
  relative to the folder), Станция (W/3), Блок (W/9), Дата `%d/%m/%Y` (W/9),
  Время `%H:%M:%S` (W/9), W = list − 30; icon OSC vs COMTRADE/STO (bitmap 483,
  12×15, magenta key).
- **Scan on a worker thread**: a new scan or close cancels and joins the old;
  list emptied and disabled with «Поиск файлов...» shown; recursion only with
  the checkbox; files by last 3 chars lowered: cfg, cff (COMTRADE header:
  line-1 token 1 = station, token 2 = block or «-»; start timestamp — `.cfg`
  first field = month, `.cff` first = day, swapped when month > 12, year < 100
  +2000), osc (station «OSC Осциллограф. Версия a.b», block «-», header date),
  sto (inner cfg header, name = the .sto). Combos refilled «Все» + sorted unique
  values; saved values re-selected by prefix match.
- **Filters live** (no Apply): station/block exact case-sensitive or «Все»,
  independent; after a scan С:/По: = earliest/latest file date; a file passes
  when `С ≤ t < По + 1 day` at 00:00.
- Sorting: header click ascending, again descending (header arrows); Время
  sorts by time of day only; text compares case-sensitive; **every open forces
  Дата ascending**.
- «Выбрать» rescans with the tree's path (a tree click alone does not);
  checkbox → immediate rescan.
- Open: double-click or «Открыть файлы» opens **every selected file** as its
  own document through the normal open path, then closes.
- «Удалить файлы» (`0x46d4b0`): every selected file plus its openable
  companions — cfg → dat, hdr, ini, inf, set; cff → set; osc → set; sto → dst
  (extension matched case-insensitively on the last 4 characters) — in ONE
  `SHFileOperationW` FO_DELETE, no owner window, `FOF_ALLOWUNDO` (recycle
  bin) unless Shift is held (`GetKeyboardState`; then permanent). No program
  prompt: the shell's own confirmation / progress / errors. Return 0 → full
  rescan; otherwise nothing (ledger L9.49).
- Persisted `FileFinder`: Path (default My Documents), StationName, StationID,
  IncludeSubdirs (SortMode written but overridden); saved on every close.

### 7.6 Загрузка — LoadingDlg, DIALOG 492
Parsing always runs on a worker thread; the modal dialog appears only when the
`.dat` (for .cfg) or the `.cff` is > 200 000 bytes, or the `.osc` > 20 000
bytes. Progress 0..100 refreshed every 500 ms (counters and quirks: ledger L9.31);
no close box; «Отмена» sets a flag the parsers poll → the open is silently abandoned. The worker closes the
dialog with IDOK on completion (original race when it finishes first — avoid).

### 7.7 Pair dialog — DIALOG 554 (ChanPairDlg, command 33027)
Diff mode: title «Задать соответствие между дифференциальным током и током
торможения», columns Дифференциальный ток / Ток торможения. R/X mode: title
str 200, columns Активное / Реактивное сопротивление. Two channel combos;
«Добавить» needs both selected, appends a row and selects it; «Удалить»
deletes the selected row and selects the next (or last); list of pairs (two
half-width columns). The caller reads the pairs after OK [I].

### 7.8 Merge bar (CMergeBar, id 223, 70 px)
- Shown automatically after a paste; no command opens it.

  | id | control | rect | content |
  |---|---|---|---|
  | 1088 | drop-list | 110,10–280 | sync method: 0 По времени начала записи (242) · 1 По авар. составляющей (3376) · 2 По доаварийному режиму (3377); default 0 |
  | 1089 | drop-list | 350,10–440 | base analog channels (kind 0, not bit14) |
  | 1090 | drop-list | 510,10–600 | pasted analog channels (bit14) |
  | 1091 | button | right-aligned at W−220 (W = client − 20), y 9 | «Синхронизация» (3455) |
  | 1092 | button | W−110, y 9 | «Объединить» (3319) |

  Painted labels: «Синхронизация» x=15, «Канал 1» x=290, «Канал 2» x=450 at the
  top (client deflated 15,14); status text along the bottom. 1089/1090 enabled
  only when method ≠ 0 (also at create).
- **Hiding the bar while any bit14 channel remains posts Undo** (cancels the
  paste). Closing the frame hides it.
- Manual alignment: drag a pending lane (mode 6) → offset in seconds clamped
  to `[−(pasted_extent − 1/f_net), base_extent − 1/f_net]`, live.
- **1091 Синхронизация** (doc 0x449930 → 0x47bb90) by method; results are the
  bar's status text:
  - 0 by start time (0x47ce10): offset = pasted start − base start; no overlap
    → **error box** 219 «Осциллограммы не пересекаются по времени»; else 3456
    «Синхронизация произведена успешно».
  - 1 by fault component (0x47bc00): cursor V1 must be > 3 periods in (3260)
    and inside the record (3162); peak of the normalised one-period second
    difference `|x[k] − 2x[k−N] + x[k−2N]|/A(k)` in a ±1 period window around
    the cursor per channel; success needs peak ≥ 4× window mean in both, else
    3507; offset = t1[peak1] − t2[peak2] either way.
  - 2 by pre-fault mode (0x47c440): cursor > 1.5 periods in (3259), inside
    (3162); coarse offset t1(c) − t2(c); per-sample fundamental phase
    difference over ±1 period, mean and spread; spread > 1° → 3305
    «Нестационарный участок…», else 3456; offset = coarse − mean°·1e6/(f_net·360)
    µs either way.
- **1092 Объединить** (doc 0x449960 → 0x47d990): target rate R = max channel
  rate rounded to a multiple of f_net; zero-pad leading (by the offset sign)
  and trailing so all channels align; uniform grid of `round(R·dur) − 4`
  samples; analog by local 3-point quadratic fit, frequency track nearest,
  digital previous-sample hold; per channel rate := R, **bit14 cleared**,
  values ÷ `ch+0xf0` [unknown], recompute; offset 0; undo snapshot; doc rate :=
  R; recompute all, ОМП; modified; **hide the bar**, beep. Names keep "(2)"
  suffixes; result is the current document (dirty).

### 7.9 Parameters bar (CParamOscBar, id 810, 75 px, visible at start)
Display only; repaints on view activation; with no document only the labels.
Let `w` = width of «Номер регистратора: », `w2` = width of «Момент запуска:»,
rows at y = 5 + k·(textH + 5); labels right-aligned, values left-aligned.

| label | label x | value | value x |
|---|---|---|---|
| Станция: | 10..w+15 | `doc+0x244` (end ellipsis) | w+30..w+420 |
| Номер регистратора: | 10..w+15 | `doc+0x248` | w+30..w+470 |
| Начало записи: | w+450..w+w2+465 | start time | w+w2+480..w+w2+870 |
| Момент запуска: | w+460..w+w2+465 | trigger time | same |
| Длительность: | w+460..w+w2+465 | `%g с.` | w+w2+480..w+w2+770 |

Times (0x456e90): parse `d/m/y` (swap when the middle field > 12, two-digit
year +2000) and `h:m:s.frac`; format `dd.mm.YYYY, HH:MM:SS.` + fraction +
« (UTC)» or « (Местное время)» per the program setting; local time applies the
COMTRADE-2013 time-code offset [I]; raw strings if parsing fails. Command 142
toggles it (ShowPane(!visible) + RecalcLayout); CMDUI checked = visible.

### 7.10 Comment text dialog — DIALOG 332 «Комментарии к диаграмме» (CCommentDlg)
Multi-line edit (366×276 DLU, Enter = newline) + code-page combo (866 MS-DOS /
Windows-1251) + OK / Отмена. Item mode (§5): combo hidden/disabled [I]; OK sets
the text. Record mode (32805): text from the settings `Comments/UserData` (the
COMTRADE `.hdr` text per help), `\n` → `\r\n` for display and back on OK;
changing the combo **re-encodes the edit text** between 866 and 1251; OK
stores text and code page; modified only when the code page changed.

### 7.11 Print — DIALOG 347 (CFastViewPrintDialog)
- CPrintDialog (no print-to-file) with a custom template: five checkboxes
  Осциллограмма (**on**), Частотная, Векторная, Круговая, Дифференциальная
  (off); always enabled; state kept per view, **also on Cancel**, not persisted.
- «Предварительный просмотр» closes the dialog and opens print preview with the
  chosen pages; «Параметры страницы» opens page setup (orientation + margins in
  0.01 mm) on top.
- Pages: the oscillogram split vertically into `ceil(lane-area height · scale
  / printable height)` pages, then one page per checked diagram in the order
  Частотная, Векторная, Круговая, Дифференциальная (panes always exist, so
  hidden diagrams print). Each page gets margins and header/footer decorations
  [unknown content]; the print header has logo and «Наименование
  осциллограммы / Начало записи / Длительность / Станция / Номер регистратора»
  (strings 148–152). DIALOG 357 is dead.

### 7.12 Formula dialog — DIALOG 359 «Добавить новую формулу» (AddFormulaDlg)
From the formula-editor page. Template tree (left, ~60 leaves in groups) +
«Название канала:» edit + a placeholder hosting a child form from the chosen
template (DIALOG 400 free «Формула:», 401–461, 501–543, 1069: Ia/Ib/Ic,
Uab/Ubc…); selecting a tree item swaps the form. Name validation: **empty or
starting with a digit → message 396 «Неправильное имя канала.»**, the dialog
stays. OK builds the formula text from the form.

---

## 8. ОМП UI

### 8.1 Commands [R]
- 32806 Анализ: **toggle** — result shown (`doc+0x690`) → clear it (0x4a17a0:
  flag 0, hide the bar, RecalcLayout, delete channels «Lкз», «КЗ фазы А» (3225,
  **Cyrillic А**), «КЗ фазы B», «КЗ фазы C»); else run interactively
  (0x4a00b0(0)). No view switch. CMDUI: disabled while any bit14 channel;
  checked = shown. With no document both ОМП buttons are greyed [I].
- 32807 Параметры: modal dialog on a copy; OK → modified and **run
  immediately** unless bit14 channels exist; Cancel changes nothing.
- Automatic: re-run (interactive) after the full-recompute commands while
  shown; silent re-run after a sample-rate change; run after «Объединить»;
  **paste clears the result**.
- Banner follows the active document (§1.1).

### 8.2 Run (0x4a00b0) [R]
1. Inputs parsed with `_wtof` from the stored strings: Station1 Inom; Station2
   Inom **negated** into `obj+0x78`; sections from Station1 only (length, R, X,
   R0, X0, R1, X1, supports; parallel lines Rm, Xm and the phasor per sample
   k ∈ [P, N) of the channel named by its «I0» property; taps R0, X0).
2. No sections or Inom = 0 → error 349 «Данные для определения места
   повреждения не введены.»
3. Channels **by the names in the station properties**: Ia/Ib/Ic always;
   method 1 reads **Uab/Ubc/Uca channels** (not derived from phase voltages);
   method 2 Ua/Ub/Uc; method 4 Station1 and Station2 Ua..Uc + Ia..Ic **from the
   same document**. Missing name → error 354 «Имя канала задано неправильно.»
4. **Double-ended: Station 2's Ia, Ib, Ic samples are negated in place**
   (0x4a2200, each if non-empty) before detection.
5. Engine per OMP_SPEC. Window → **V1 = c1 (start), V2 = c1 + P − 1**, both
   shown; views refreshed.
6. Banner shown with the result text (leading spaces kept).
7. Channels added (replacing same-named): «Lкз» analog, samples = distance ×
   1000, unit **"m"**; «КЗ фазы C», «КЗ фазы B», «КЗ фазы А» as **digital**
   channels from the phase flags.
8. Errors: flag cleared; interactive → posted as 0x40A to the frame and shown
   [I: message box]; silent → dropped. A previous banner is not hidden on error
   [I].
- Moving V1/V2 while shown recomputes the result text live (Lкз, flags and
  window are not recomputed).

### 8.3 Result banner and report [R]
CMsgCaptionBar (§1.3). Hiding it (its close, WM_SHOWWINDOW false) posts 32830
→ clears the result. «Сохранить...» (32879): only when shown; save dialog
filter 3444 «Результаты ОМП (*.txt)», default ext txt, **no overwrite prompt**;
written **UTF-16LE with BOM, CRLF**. Report order: program name; «Осциллограмма:»
name; algorithm line (3114/3113/3112 isolated / grounded / double-ended);
«Регистратор:», «Станция:»; «Начало записи:», «Момент запуска:»; result («КЗ
отсутствует.» or «1-х/2-х/3-х фазное КЗ: фазы …» + « на участке № %i, опоры
%i - %i.» + distance line, or « на смежной линии.»; no leading spaces); setting
blocks (isolated: «Подстанция:», Inom, channel lines Ia..Ic, Uab..Uca; grounded:
Ua..Uc; double-ended: «Подстанция №1/№2» each with Ia..Uc); per section
«Участок №», Длина, Число опор, R/X or R1/X1/R0/X0, Комментарий; per parallel
line Rm, Xm, I0, comment; per tap R0, X0, comment. Values are the raw strings.

### 8.4 Parameters dialog — DIALOG 336 «Параметры ОМП» (WrongPlaceLocatorDlg)
- ~475×275 DLU; toolbar across the top; tree (≈ left third, thin (171,173,179)
  frame, no label editing) and property grid (right two-thirds: no header, flat
  list, no groups/descriptions, modified values marked); OK / Отмена.
- Toolbar 475: method combo (id 1075, 280 px): 0 «ОМП для сетей с
  изолированной нейтралью» → 1 · 1 «…с заземленной нейтралью» → 2 · 2 «ОМП
  двухстороннего замера» → 4 | Участок (32871, icon+text) · Параллельная линия
  (32872, icon+text) · Отпайка (32873, icon+text) · Удалить (32874) | Вниз
  (32876) · Вверх (32875) | Загрузить (32878) · Сохранить (32877). Method change
  rebuilds tree and grid; data of hidden kinds kept.
- Tree (icons station / section / parallel line / tap): methods 1/2 one root
  «Подстанция»; method 4 «Подстанция №1», «Подстанция №2». Under a station
  «Участок №k»; under a section, **methods 2/4 only**, «Параллельная линия №k»
  then «Отпайка №k» (1-based within the section). Expanded state kept per
  element (default expanded).
- Grid rows for the selected element (records filtered by method mask):

  | element | property | default | methods | kind |
  |---|---|---|---|---|
  | Station | Название | «Подстанция» | 1,2,4 | text |
  | | Номинальный ток прямой последовательности, A | "0" | 1,2,4 | text |
  | | Ia, Ib, Ic | "" | 1,2,4 | channel |
  | | Uab, Ubc, Uca | "" | 1 | channel |
  | | Ua, Ub, Uc | "" | 2,4 | channel |
  | Section | Длина участка, Число опор | "1", "1" | 1,2,4 | text |
  | | R, X удельное (Ом/км) | "0" | 1 | text |
  | | R1, X1, R0, X0 удельное (Ом/км) | "0" | 2,4 | text |
  | | Комментарий | "" | 1,2,4 | text |
  | Parallel line | Rm, Xm удельное (Ом/км) | "0" | 2,4 | text |
  | | I0 | "" | 2,4 | channel |
  | | Комментарий | "" | 2,4 | text |
  | Tap | R0, X0 (Ом) | "0" | 2,4 | text |
  | | Комментарий | "" | 2,4 | text |

  Text = free string (no validation in the grid); channel = drop-down of the
  document's channel names (computed included? unknown). Changes go straight
  into the working copy. Default method **1**.
- Button enable: Участок only on Station index 0 (Подстанция №2 never has
  sections); Параллельная линия / Отпайка on a Section with method 2/4; Удалить
  on a Section, line or tap; Вниз / Вверх when not last / first among siblings;
  Загрузить / Сохранить always.
- Handlers: add appends a default element, inserts it (lines before taps) and
  selects it; delete has **no confirmation and no renumbering** until a rebuild;
  up/down swap with the sibling, rebuild, keep the moved item selected.
- OK validates Station1's sections, first failure → message box, dialog stays:
  length 0 (3187); supports 0 (3535); method 1 R = X = 0 (3068); methods 2/4
  R1 = X1 = 0 (3074), R0 = X0 = 0 (3071); each line Rm = Xm = 0 (3076); each tap
  R0 = X0 = 0 (3071) (`_wtof`, so text = 0). Inom and channels are not checked
  here. Success copies the working copy back.

### 8.5 Settings files [R]
`.omp` (filter 3511, default ext omp, no overwrite prompt) is XML:
`<omp ver="2" type="1|2|4">` with `<Station1 obj_name In Ia Ib Ic Uab Ubc Uca
Ua Ub Uc>` containing `<Section{k} lenght sup_number r x r1 x1 r0 x0 comment>`
(k from 1; "lenght" is the original spelling) with `<child{n} type="parLine"
rm xm I0 comment/>` and `<child{n} type="connection" r0 x0 comment/>` (n from 1,
lines first); `<Station2 …/>` likewise; both stations always written; values
raw strings. Reader accepts legacy ver ≠ 2 (`SectionsNum`, `I1n`,
comma-separated section values) and falls back to a non-XML legacy reader
[format unknown]. The same writer/reader is embedded in the `.set` settings
(`WrongPlaceFinding`; 32860 loads it).

### 8.6 Double-ended workflow
There is no second-document picker: the remote record's channels are pasted
into the first (Ctrl+V → bit14, Анализ disabled), aligned with the merge bar
(§7.8), merged with «Объединить», then Station 2's channel properties name the
merged channels of the same document; the run negates Station 2's currents.

---

## 9. Reconciliation backlog (ranked by user-visible impact)

Modules are under `crates/fastview-gui/src/` unless prefixed. Size S < ½ day,
M ≈ 1–2 days, L > 2 days. ✅ = done in L9.1.

✅ Channel selection model (plain / Ctrl / Shift / RMB select-first, axis band
clears; `chansel.rs`, `Doc::selection`/`sel_anchor`).
✅ Context menus by region: 560 / 561 / 563 in resource order with CMDUI
rules; no plot menu; RMB on the plot places V2 (`lanes.rs`).
✅ Per-channel display mode (bits 7–9) on selection + groups; ribbon checked
iff all selected agree (`app.rs`, `ribbon.rs`).
✅ Channel groups 33035 / 33037 with the CMDUI rules (`ChanView::group`).
✅ Comment lock flag (33039); 155 moment placement mode; copy takes the
selection; colour dialog 33030; selected rows on a gold background (flat).
✅ User-reported GUI bugs (L9.23): no panic at any window size (whole-window
headless sweep); the view's H/V window scroll bars with arrows, H along the
whole bottom; no input reaches the main window while a (modal) dialog is
up (L9.24); «Файл» main panel two columns, sub-items in the right column
(L9.25); diagram panes stay above their tab strip (L9.26); ribbon
icon-only small buttons and panel collapse when narrow (L9.27).

1. 🟡 (L9.3: right column with tab group + frequency below, toggles with
   check, grid-above-plot; no floating/re-dock/auto-hide)
   Diagrams as docked panes beside the always-visible oscillogram (vector +
   circle tabbed right, diff right, frequency bottom; visible by default;
   ribbon buttons toggle with check; panes follow the active document) —
   `main.rs`, `views.rs`, `app.rs` (`View` enum → pane visibility) — L
2. ✅ (L9.4) Визир mouse model: grab ±3 px and drag V1–V3/H1/H2 with live status;
   **remove drag-to-pan**; Shift+LMB/RMB place H1/H2 in the lane under y —
   `lanes.rs` — M
3. ✅ (L9.4) Horizontal model: (first sample, power-of-two f, 25 px/sample) + H
   scrollbar; Ctrl+wheel ×2/÷2 anchored on V1>V2>V3; Shift+wheel one screen;
   persist XScale — `lanes.rs`, `app.rs`, `persist.rs` — L
4. ✅ (L9.4) Amplitude mapping min..max with 5 px margins, zero line only when in range,
   group common range, window-zoom range model — `lanes.rs` — M
5. ✅ (L9.4) Lane geometry: YAnalogSize (93, ×/÷1.3 within 70–500, Alt+wheel, 32800/01),
   digital 20 px, rows/icons growth, no stretch, fixed non-scrolling bottom
   band 30/75, plain wheel 120 px/notch — `lanes.rs` — M
6. ✅ (merged L9.11/L9.12) ОМП: Анализ toggle + result banner under the ribbon (live text, Сохранить
   .txt UTF-16LE, close clears) instead of the ОМП tab; buttons greyed with no
   doc / bit14 — `main.rs`, `ribbon.rs`, `views.rs`, `app.rs` — M
7. ✅ (L9.4) Waveform: every sample, 1 px, sample circles when > 4 px/sample (keep
   extrema if a perf fallback is needed) — `lanes.rs` — S
8. ✅ (L9.4) Time ruler: 25 px ticks (10/5/7/5), labels every 100 px trigger-relative,
   unit on first label, space grouping, `%.3f` s, samples k−trig+1, trigger
   dotted line; carry the trigger sample from the loader — `lanes.rs`,
   `crates/fvb-core` loader — M
9. ✅ (L9.4) Визиры presentation and toggles: three V colours + labels, V3 (139),
   independent 32813–32816 with first-show ¼/½/¾ placement, no shading
   between В1/В2, H визиры per channel (`{shown, lane, value}`) — `lanes.rs`,
   `app.rs`, `ribbon.rs` — M
10. ✅ (L9.4) Difference band (33033, default on): 45 px band, marker labels, Δt
    dimension arrows; «Разница между визирами» enabled in 560 — `lanes.rs` — M
11. ✅ (L9.14) Ribbon Видимость / Вычисления contents in original order (discrete-only
    32845–47, zoom 32800–03, 138, 141, визиры, 155/157/156, stats rows
    32817–19, Коэффициенты, separate Вторичные, filler), unnamed panel for 142,
    and CMDUI enable/check on every button — `ribbon.rs`, `app.rs` — M
12. ✅ (merged L9.11/L9.12) ОМП run semantics: channels by name per station; isolated reads
    Uab/Ubc/Uca; double-ended from the same document (Station 2 current
    negation ✅ done in the engine, ledger L9.2) and Station 2 Inom negated; parallel lines with I0 phasor; Lкз
    unit "m"; КЗ фазы as digital channels with Cyrillic «А»; V1 = start,
    V2 = start+P−1 (ours swapped); defaults method 1, section 1/1;
    errors as message boxes — `app.rs` (`run_omp`), `crates/fvb-core/src/omp.rs`
    — M
13. ✅ (L9.5) Stats panel: framed coloured name (25-char cut), value `%-6g` heavy blue,
    rows only per bits 4/5/6 with the 32817–19 toggles, fonts/colours, status
    icons, auto-fitted leftW (replace LEFT_W 158) — `lanes.rs` — M
14. ✅ (L9.6 + L9.17) Comments: level (156) and area (157) kinds with create-drag, drag editing
    by part, lock respected, 33042 inline level edit, double-click edit,
    hover info box, per-kind drawing, one-shot tools enabled for .cfg/.osc,
    `.set` persistence — `app.rs`, `lanes.rs`, `dialogs.rs`, `persist.rs`/
    `crates/fvb-core` — L
15. 🟡 (L9.15 + L9.19 undo + L9.20 clipboard XML and insertion point; L9.30 EMF picture of the view / focused diagram; L9.58 grid focus → XML only) Merge workflow: system-clipboard format with times/rates/header, pending
    bit14 channels with own rate, "(2)" naming, merge bar with three sync
    methods, drag time-shift (hand cursor), «Объединить» resampling, undo on
    bar close, paste disabled while pending; fix the test pinning index
    alignment — `app.rs`, new merge module, `crates/fvb-core` — L
16. ✅ (merged L9.13) Каналы dialog 463 (icon grid, header toggles column, in-place rename with
    397/398) replacing the view switch; rename our role-assignment window —
    `dialogs.rs`, `ribbon.rs` — M
17. ✅ (L9.4 six complete strings; L9.18 free-text pane → toast) Status bar: six fixed-width panes with full original strings and
    RulerState formats, trigger-relative, H-diff only on the same channel;
    drop the free-text status (toast instead) — `main.rs`, `lanes.rs` — S
18. ✅ (merged L9.13) Parameters bar (142, visible by default, 75 px) — `main.rs`, `ribbon.rs` — S
19. ✅ (merged L9.11/L9.12) Circle/diff diagrams: (z, ox, oy) model with wheel/Ctrl/Shift, limits,
    scrollbars, toolbar; origin-centred equal-scale autoscale incl. loci;
    two scales + edge labels/scale box; property grid on top with splitter;
    9-colour palette, alpha-50 fill, reverse order; bottom legend / readout
    table; diff lower-left initial view; disable egui interactions — `views.rs`
    — L
20. ✅ (merged L9.11/L9.12) Diagram context menus 387/388/355/351 and toolbars 550/552/553 —
    `views.rs` — M
21. ✅ (merged L9.11/L9.12) Frequency: one panel per channel, rotated labels, percent default,
    settings dialog 353 — `views.rs`, `dialogs.rs` — M
22. ✅ (merged L9.11/L9.12) Vector: locked plot, 4/5/10 circle rule, arrowheads, no radians toggle,
    options via toolbar/menu — `views.rs` — S
23. ✅ (L9.7) Rectangle vertical zoom 138 (mode 7/8, up-arrow cursor, lane fractions,
    group) and 141 reset; drop the ×1.25/×0.8 submenu — `lanes.rs`, `chansel.rs`
    — M
24. ✅ (L9.4) Keyboard: ←/→ V1 (Ctrl V2), ↑/↓ H1 (Ctrl H2) — `app.rs`, `lanes.rs` — S
25. ✅ (L9.4) Cursor shapes (W–E, N–S, up-arrow, hand) — `lanes.rs` — S
26. ✅ (L9.4) Inline rename on name-badge double-click (397), not properties —
    `lanes.rs` — S
27. ✅ (merged L9.11/L9.12) Diagram eligibility by unit (vector A/V/«ном»/empty, circle Ohm/W/VA/var;
    circle pane Ohm/VA; clear on attach) in 561 and the dialogs — `chansel.rs`,
    `views.rs` — S
28. ✅ (merged L9.13) Coefficients dialog 477 (bulk Shift-select entry) + Первичные/Вторичные as
    a per-channel radio pair with full recompute — `dialogs.rs`, `app.rs` — M
29. ✅ (merged L9.13) File parameters 392: title, editable sample-rate combo with 50/60 Hz lists
    (re-time + recompute), correction combo, start/trigger, `%g c.`, live
    code-page combo, Закрыть applies — `dialogs.rs`, `app.rs` — M
30. ✅ (L9.7 + L9.19) Invert/DC/visibility semantics: toggle-all over the selection, **no undo**,
    full recompute incl. computed channels and ОМП; undo only for paste/merge,
    Отмена/Возврат greyed by counts — `app.rs`, `chansel.rs` — S
31. ✅ (L9.14 + L9.17 .set + L9.22 print/preview) App menu: original order, Сохранить vs Сохранить как, Комментарии,
    Загрузка настроек, Закрыть, Печать submenu, 10-entry MRU; dev extras
    (Демо, CSV, Archive, .fvb) marked as extensions; QAT = Open/Save/Print,
    Save greyed for .sto — `ribbon.rs`, `persist.rs` — M
32. ✅ (L9.8; tab colour order beyond the first NOT DETERMINED) Document tabs: auto colour, close on the active tab only, ▾ list, window
    title «Механотроника FastView - <doc>» — `main.rs` — S
33. ✅ (L9.14 + L9.18: non-2007 looks selectable, palettes inferred) Стиль menu (9 entries, radio, persisted, default 215), help split button,
    ribbon minimise via ribbon right-click — `ribbon.rs`, `theme.rs`,
    `persist.rs` — S
34. ✅ (merged L9.13) Program settings 510: no auto encoding, default 1251, .dst folder, UTC/
    local radios, persistence (+ print margins) — `dialogs.rs`, `persist.rs` — S
35. 🟡 (L9.9: all but the shell tree, .sto and recycle bin) Проводник as the modal finder (worker scan, filters, sort, multi-open,
    delete with companions, persistence) — `main.rs` → new module — L
36. 🟡 (L9.22: all but logo, paper/printer properties, diagram print branches; L9.59 selected comment printed) Print: dialog with five checkboxes, page setup, paged output (oscillogram
    sliced, diagrams one page each) and preview — new module — L
37. 🟡 (L9.10: worker + thresholds + cancel; L9.31: the original's counters and percent) Loading progress dialog with worker thread and cancel (thresholds
    200 kB/20 kB) — `app.rs`, `crates/fvb-core` parsers — M
38. ✅ (merged L9.11/L9.12) ОМП parameters dialog: toolbar + tree + property grid, enable rules,
    validation messages, `.omp` XML load/save, embed in `.set` — `views.rs` →
    new `omp_ui` module, `crates/fvb-core` — L
39. ✅ (merged L9.13) Record comment (.hdr) editor 32805 for .cfg/.cff with 866/1251 re-encode —
    `dialogs.rs`, `crates/fvb-core` — S
40. ✅ (merged L9.11/L9.12) Pair dialog 554 + R/X pair loci on the circle diagram — `dialogs.rs`,
    `views.rs` — M
41. ✅ (L9.4; decimation + ticks L9.46) Digital lanes look (base bar, red-outlined runs, selected fill) and
    full-width alternating gradient backgrounds, selected gradient — `lanes.rs`
    — S
42. ✅ (L9.4 + L9.18) Grid vertical only every 50 px; ruler units incl. «Абсолютное время»
    (33007) with the date; persist RulerState/Sells — `lanes.rs`, `persist.rs`
    — S
43. ✅ (L9.5) SI prefix only for the listed units; "none" → empty — `crates/fvb-core/src/
    views.rs` — S
44. ✅ (L9.5; names only) Hover popup for truncated stats text — `lanes.rs` — S
45. 🟡 (L9.8 + L9.18: file argument, -Dst, -Find, dropped files, 866×500, single instance; no splash) Start-up: file argument, `-Dst`, `-Find`, dropped files, min size 866×500,
    optional single instance and splash — `main.rs` — M
46. ✅ (merged L9.16) AddFormula template tree (at least name validation 396) — `views.rs` /
    `quick_ui.rs` — M (validation S)
47. ✅ (merged L9.17) Document-modified tracking from the commands that call SetModifiedFlag
    (save prompt depends on an unknown) — `app.rs` — S
48. ✅ (L9.21; L9.61 the 2 px / 200 ms start, self-drop inert) Stats-panel drag of the selected channels: drop on a lane = 33035 (or 1086), on the frequency / vector / circle pane = add them (not a copy between documents) — `lanes.rs`, `views.rs` — M

---

## 10. Unknowns (consolidated)

- **MFC ordinals** are resolved by call shape only (no mfc120u export table):
  e.g. 8227 (gate on destroying the hover edit), 14350 (UpdateAllViews?),
  14094 (ShowWindow? hide vs disable the code-page combo), 3888, 2214, caption
  bar / property grid / toolbar ordinals.
- **Merge-bar ids**: scope C read 1091 = «Объединить» / 1092 = «Синхронизация»;
  scopes D, F and G read 1091 «Синхронизация» (3455) → 0x449930 → sync 0x47bb90
  and 1092 «Объединить» (3319) → 0x449960 → 0x47d990. The behaviour mapping
  (combo-driven sync vs resampling merge) agrees; this spec follows D/F/G.
  **Resolved (read, CMergeBar create):** `0x479b6d` loads string 0xd7f (3455
  «Синхронизация») for the button pushed with id 0x443 (1091) at `0x479b7b`;
  `0x479bc3` loads 0xcf7 (3319 «Объединить») for id 0x444 (1092) at `0x479bd1`.
- After «Объединить»: **resolved (L9.19)** — the bar is hidden (`0x47e8fc`
  SW_HIDE) and ОМП re-runs unconditionally (silent `0x4a00b0(1)`).
- **Circle eligibility** differs by site: CMDUI 32851 accepts Ohm/W/VA/var;
  the circle pane's menu and attach accept Ohm/VA; the Каналы dialog column 4
  rejects W/VA/var. Vector: A/V/«ном»/empty (CMDUI), A/V/""/one more string
  (pane), A/V-type (dialog). The 0x42bf90 / 0x42deb0 / 0x4036f0 rules were not
  fully decoded.
- Initial values at document open: f (fit vs stored XScale), stats-row bits
  4/5/6 (screenshots show СКЗ and A1 on), display-mode default (inferred
  instantaneous), diagram membership defaults.
- Whether SetModifiedFlag from view-preference commands leads to a save prompt
  (SaveModified not inspected).
- `doc+0x214` trigger for COMTRADE (only a native loader writes it).
- The +23 px icon row condition (0x4258e0 vs bits 8–11).
- Comment hit sub-part codes beyond those listed; exact per-kind draw of the
  text boxes; merged визир label separator; the pen used when В2 = В1; ruler
  tick pen; Tahoma "Ay" heights (≈ 16/18 px assumed).
- H1/H2 first-placement coordinates (assumed content y 10/20 on the top lane).
- Circle grid pen assignment (frame vs axes), edge-label rects, `cells` basis
  (half of the smaller side assumed); BlockPropertyGrid content/editing; meaning
  of characteristic field +0x68.
- Vector header placement; frequency margins and bar-label placement.
- Docking: exact first-run placement of the diff pane; stacking order of the
  three caption bars; whether the parameters bar visibility persists; text of
  MFC string 17086; registry layout of ApplicationLook / Workspace.
- CParamOscBar font; fractional-second digits and time-zone logic in 0x456e90.
- Channels dialog: meaning of kind +0x5c = 1 vs the two digital groups
  (`doc+0x878`, `doc+0x884`); bitmap 464 image indices.
- File params: what the `doc+0x1e4` correction candidates are and what
  selecting one does.
- CCommentDlg: how the item/record mode flag is set and how `UserData` maps to
  the `.hdr` file; settings sidecar name (`.set` per help; legacy `ini`).
- Merge: per-period amplitude 0x427670, spread metric 0x4776f0, divisor
  `ch+0xf0`, fractional-sample residual, drag pixel→time constant 0x4d1710,
  insertion position of pasted channels, how combos 1089/1090 are filled.
- Finder: sort-arrow direction mapping; COMTRADE header edge cases; `.sto`
  inner-cfg extraction; progress units of the loader.
- Print: page-setup margin defaults/persistence, header/footer content
  (0x405c20/0x406590), output of never-shown diagram panes.
- ОМП: whether the channel drop-down includes computed channels; whether 354
  appends the name; how 0x4756e0 shows errors; identities of the transform
  routines that re-run ОМП; legacy `.omp` reader 0x4a45f0; whether the bar's
  close "×" is drawn; `SelectItem(method−1)` for method 4.

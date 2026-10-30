# FastView 5 — original UI spec (reverse-engineered, for 1:1 recreation)

Source: `extracted/stage1/FastView.exe` (MFC 12.0, Office-2007 ribbon) PE resources +
187 help screenshots in `extracted/chm/clip*.png`. Full dumps: `notes/ui/`.

## App identity
- Title: `Механотроника FastView - <file>.cfg`
- Splash/About: "Fast View 5 — Программа для просмотра и анализа осциллограмм в формате
  COMTRADE", logo = Механотроника green/blue chevron, "© 1990, НТЦ «Механотроника»".

## Keybinds (accelerator table 128 — the COMPLETE set, all standard MFC ids)
| Shortcut | cmd id | Action |
|---|---|---|
| Ctrl+O | 57601 | Открыть (open COMTRADE) |
| Ctrl+S | 57604 | Сохранить |
| Ctrl+P | 57607 | Печать |
| Ctrl+C | 57634 | Копировать (also: copy channels for merge) |
| Ctrl+V | 57637 | Вставить (paste channels for merge) |
| Ctrl+Z | 57643 | Отмена (undo) |
| Ctrl+Y | 57644 | Возврат (redo) |

## Ribbon (from screenshots clip0051 / clip0081)
QAT (quick access): app-icon, Открыть, Сохранить, Печать, Отмена, Возврат.
Tabs: **Главная**, **ОМП**.  Top-right: **Стиль** dropdown + help (?).

Главная groups:
- **Объединение**: Вставить (big), Копировать, Отмена, Возврат
- **Диаграммы**: Частотная, Векторная, Круговая, Дифференциальная (big buttons)
- **Видимость**: Каналы (big) + grid of small toggles (prev/next chan, zoom, etc.)
- **Вычисления**: Инвертировать, Удал. постоянную, Первичные знач., Вторичные знач.,
  Коэффициенты, Быстрый анализ
- **Осциллограмма**: info button

ОМП tab → group **ОМП**: Параметры, Анализ. Info banner with fault-location result + Сохранить…

## Style skins (MENU 200 — the Стиль dropdown)
Windows 2000 / Office XP / Windows XP / Office 2003 / Visual Studio.NET 2005 /
Office 2007 → {Голубой(Blue), Чёрный(Black), Серебристый(Silver), Зеленовато-голубой(Aqua)}.
Screenshots = **Office 2007 Blue (Голубой)** = our default skin.

## Primary view = stacked per-channel LANES (not one combined plot)
Each analog channel = one horizontal strip:
- Left stats panel: colored name badge, current value @cursor, Max, Min, СКЗ (RMS), A1 (1st harmonic)
- Waveform to the right, lane background alternates (cyan / white; selected lane = yellow)
- Digital channels drawn as step/bar lanes at the bottom (e.g. "КЗ фазы A/B/C")
- Vertical **визиры** (cursors): red (visir 1), blue B1/B2; plus horizontal визиры
- Selection region (pink rect) with duration label "Длительность: X с"
- On-waveform comments (bubble + "Уровень: N") with context menu
- Time axis at bottom; unit modes: мкс / мс / с / абс. время / отсчёты / сетка

## Status bar (bottom)
`Верт. визир 1: | Верт. визир 2: | Разница верт. визиров: | Гор. визир 1: |
Гор. визир 2: | Разница г. визиров:` … right-aligned `НТЦ Механотроника`.

## Context menus (right-click) — exact, with cmd ids (notes/ui/menus.txt)
- **Per-channel (MENU 358)**: show on freq/vector/circle diagram (32849/50/51), ADC counts
  (32852), Скрыть (32848); Мгновенные (32820)/Среднеквадратичные (32821)/Первая гармоника
  (32822); Инвертировать (32823); Удалить постоянную (32824); Копировать/Вставить каналы
  (57634/57637); Цвет канала → Жёлтый/Зелёный/Красный/Синий/Чёрный (33000-33004).
- **Time axis (385/386)**: мкс(32853)/мс(32854)/с(32855)/абс.время(33007)/отсчёты(32856)/
  сетка(32857).
- **Vector diagram (355)**: angle mode absolute(36000)/radians(32841)/direction-only(32842),
  show values(32844).
- **Circle diagram (356/387)**: reset chars(32865), range between cursors(32843), reload
  char bases(32864), real+imag(33008)/mag+angle(33009), create computed channel(33027).

## Assets extracted (notes/ui/png/, bmp/)
Toolbar image strips (16×16 & 32×32 glyph rows): bitmap_110/111/113/114/115/549/552/553…
App icon chevron: bitmap_112 (26×26). About/splash art: bitmap_462/479/481/482/568 (630×400).

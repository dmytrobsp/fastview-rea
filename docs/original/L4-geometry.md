# L4 — Geometry / region engine  ·  ✅ MATCH (world-space; pixel quantisation not modelled)

Evidence (subagent decompile of FastView.exe; scratch at the session scratchpad,
not committed). Each primitive is a `Figure` subclass — header `{vtable, +4 mode,
+8 combine op}`; vtable slots 0 dtor, 1 clone, 2 evaluate exprs, 3 build HRGN
`(CDC*, double scale, POINT* origin, RECT* clip|NULL)`:
PolygonF `0x4c3dcc` (eval `0x409860`, rgn `0x409c00`) · PolypointF `0x4c3d90`
(`0x40acb0`/`0x40ae40`) · Circle `0x4c3db8` (`0x40a010`/`0x40a170`) · EllipseF
`0x4c3da4` (`0x40a3a0`/`0x40a5d0`). All regions: BeginPath/(Polygon|Ellipse)/
EndPath/PathToRegion (no CreatePolygonRgn/CreateEllipticRgn imports).

Parser mapping (`FUN_0040c7e0`): `mode` == "on" → 1, **anything else incl.
absent → 0 (off)**; `type` "and"=1 "or"=2 **"xor"=3** "not"=4 = CombineRgn
RGN_AND/OR/XOR/DIFF; absent `type` leaves +8 **uninitialised**; polypoint
`closed` == "off" → false, **anything else incl. absent → true**; line
`type` = `_wtoi`.

Behaviour:
1. Lines: type 1 `y = kx + b`, type 2 `x = ky + b`, k is a plain slope. Other
   types → every vertex touching that line = (0,0).
2. Polygon vertex[i] = line[i−1] ∩ line[i] (vertex 0 = last ∩ first); no
   parallel guard (inf/NaN → `cvttsd2si` 0x80000000), no clamping.
3. Circle: true radius; ellipse `width` (X) / `height` (Y) are FULL axes; angle
   degrees, positive = CCW in world (stored −angle·pi/180, applied to the GDI
   Bézier path in device space).
4. **Combination is SEQUENTIAL in document order, per mode**: two passes (off,
   on); `rgn = first figure of that mode (its type ignored)`, then
   `CombineRgn(rgn, rgn, fig, fig.op)` for each later figure. No grouping.
   ← our `geom.rs` groups `(base ∪ or) ∩ ands ∖ nots` — WRONG in general.
5. Polypoint `closed="off"` is **still a filled region** combined like any
   other figure: paint closes it through a box around the client area (end
   segments extended; region lies left of p0→pn−1) — our `geom.rs` treats it as
   a polyline only. ← WRONG.
6. World→device `px = R(x·s)+ox, py = oy − R(y·s)` (round-half-up); circle/
   ellipse bbox floor/ceil. Degenerate (zero-area) region → replaced by a 1×1
   pixel rect (so a zero-radius figure still *starts* a pass).
7. Paint (`0x43a4a0`): operate region filled with alpha 50 + FrameRgn outline;
   reset region outline only, drawn only if window flag +0x489; characteristics
   drawn last→first; nothing drawn when `scale·extent ≥ 800·printScale`.

Library scan (original UTF-16 `FastView.fvb`): no later figure lacks `type`
(the uninitialised-op path is never hit); every `mode` is explicit; no `xor`;
all 374 polygon lines are type 1/2; three characteristics start a pass with an
`and` (Эллиптическая ×2, РНМ) — harmless under first-starts semantics; **all 58
polypoints are `closed="off"` in DIF characteristics** — the original paints
them as filled operate/reset regions, we drew bare polylines (visible on every
differential characteristic).

Our impl (reconciled): `model.rs` — `Mode` "on" else Off, `Combine::Xor`,
`LineKind::Invalid`; `parse.rs` — `closed` false only for "off". `geom.rs` —
per-mode sequential CombineRgn fold (first figure starts, its type ignored;
degenerate figures start the pass as empty), the original's per-type-pair line
intersection formulas and vertex order (vertex i = line[i−1] ∩ line[i]),
invalid-type vertices at the origin, `close_open_polypoint` = the `0x40ae40`
closing (end-line intersection test, far box = view ± 2 view-sizes, nearest
outward hit, backwards corner walk incl. the same-edge full-loop rule),
`evaluate` (no view = autoscale pass, direct close) vs `evaluate_in_view`
(paint pass). GUI: view-dependent characteristics re-evaluated against the last
plot bounds; autoscale uses the view-independent extent (no feedback loop).
Pinned by `tests/geometry.rs` (12 tests incl. sequential-vs-grouped,
first-type-ignored, xor, defaults, vertex order, same-edge corner walk, every
shipped DIF characteristic fills). Headless render of block 4 (transformer
ДЗТ/ДТО) shows operate regions above the restraint curves.

Residual / not modelled (honest):
- Integer-pixel GDI regions (round-half-up, floor/ceil bboxes, Bézier ellipse)
  vs our world-space polygons: edges may differ by < 1 pixel; the 1×1-pixel
  stand-in for degenerate figures is treated as empty.
- Parallel consecutive polygon lines: original → inf → `0x80000000` → GDI
  failure; we drop the vertex (graceful divergence on invalid input).
- Absent `type` on a non-first figure: undefined in the original (uninitialised
  heap); we union. Not reachable with the shipped library.
- `mode`/`type`/`closed` string compares: Ordinal_2948 case-sensitivity —
  settled in L5 (CString::Compare, wcscmp: case-sensitive; ours since then,
  test `attribute_values_are_case_sensitive`).
- Paint details (presentation, not geometry): characteristics drawn last→first,
  operate fill alpha 50, reset outline only and gated by a window flag, nothing
  drawn when `scale·extent ≥ 800·printScale`, the 9-entry colour table
  `0x4c3030`. The GUI after this change was NOT screenshot-verified (the
  Wayland capture missed the window this session); the headless example render
  was checked instead.

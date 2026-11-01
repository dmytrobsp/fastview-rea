# L9 — UI layer (message maps, view input/paint, ribbon, panes, dialogs)  ·  ✅ (reconciled entry by entry; what is still open is a GitHub issue)

**Why.** The UI had been reconstructed from help screenshots + PE resources
(menus, dialogs, strings, accelerators) while the handlers that give it its
behaviour were never decompiled. A proficient user of the original found
functional differences in behaviour and presentation.

**Inventory method (deterministic, repeatable).** `tools/msgmaps.py <exe>
<out.json>` (pure-stdlib PE parse): RTTI type descriptors → complete object
locators → vtables; `mov eax, imm32; ret` getters whose target is an
`AFX_MSGMAP {pfnGetBaseMap, lpEntries}` → the 24-byte `AFX_MSGMAP_ENTRY` tables;
getter ↔ class via the vtables that contain it. Result over FastView.exe: 165
RTTI classes, 106 vtables, **34 message maps / ~350 handlers**.
`tools/show.py [Class…]` prints a map with ids resolved via the local
`notes/ui/` dumps; `tools/vt.py` lists vtables (for OnDraw/OnUpdate etc. diff a
class's vtable against its MFC base). Key maps: CFastViewView 50 entries
(getter 0x45a970, vtable 0x4c9774), CMainFrame 25 (0x4720a0, ribbon built in
WM_CREATE 0x4728d0), CFastViewDoc 15 (0x44ade0), CircleDiagramWnd 25 /
CircleDiagPane 18, DiffDiagramWnd 23 / DiffDiagPane 16, VectorDiagramWnd 15 /
VectorDiagPane 10, FourieDiagramWnd 8, the CQAn* pages, ChannelsDlg,
FileFinder22Dlg. Classes with no getter found by the pattern (handlers to be
found via vtable/DDX): CCommentDlg, FileInfoDlg, CParamOscBar, CMsgCaptionBar,
the three diagram toolbars, WrongPlaceLocatorDlg (likely the unattributed map
at getter 0x4a7a41).

**First observation (read, 0x45d990 WM_MOUSEWHEEL).** Alt held → command
32800/32801 (vertical zoom in/out); MK_CONTROL → 32802/32803; MK_SHIFT →
horizontal scroll (position in 1/10000 of the record); plain wheel → vertical
scroll of the lane stack. Structural divergence noted before reversing: the
original shows diagrams as **docking panes** (CircleDiagPane, DiffDiagPane,
VectorDiagPane, CFramePane<FourieDiagramWnd>, each with its own toolbar) next
to the oscillogram; ours shows them as full-window tabs.

**Scopes (research pass, reports in the session scratchpad, distilled into
`docs/original/specs/ui-behaviour.md`):** A view input (mouse/keys/scroll/cursor/hit
geometry) · B view painting (lanes, stats panel, axis, визиры, comments) ·
C view/doc COMMAND + CMDUI rules · D main frame: ribbon construction, panes,
status bar, app · E diagram windows/panes/toolbars UI · F dialogs and bars.

Status: research pass DONE (7 reports: A input, B paint, C commands, D frame/
ribbon/panes, E diagrams, F dialogs/bars, G ОМП UI), distilled into
**`docs/original/specs/ui-behaviour.md`** (§1–8 behaviour, **§9 ranked reconciliation
backlog — work it top-down**, §10 unknowns; the scratchpad reports are
temporary). The entries are in L9-ui-000.md … L9-ui-400.md (index in README).

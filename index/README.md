# Factual indexes of FastView.exe

Facts about the original program's binary — addresses, ids and names —
for anyone who wants to follow the notes in a disassembler. Nothing here is
code or text of the original: no disassembly, no decompiled code, no
resource strings. They describe `FastView.exe` with sha256
`6c503790d13d9d2d0b80711f27b4daba57b209462cfb626cc5a6f719c150d046`
(image base `0x400000`); get your own copy as described in
[`../tools/README.md`](../tools/README.md).

| file | what | made by |
|---|---|---|
| `names.tsv` | the functions (and a few data items) our notes describe, named in our own words, with the note that describes each (`address kind name ref`; kind `func` = entry, `code` = a point inside a function, `data`) | by hand, from `../docs/original/` |
| `msgmaps.json` | every MFC message map: class(es), map and getter addresses, entries (message / command / update-UI / notify, id range, handler address) | `tools/msgmaps.py` |
| `vtables.tsv` | RTTI classes, primary vtable address, the slot addresses | `tools/index.py` |
| `imports.tsv` | import address table: slot, DLL, function name or `#ordinal` (MFC 12 imports by ordinal) | `tools/peimports.py` |
| `symbols.tsv` | everything above as one symbol list (`address kind name source`): `names.tsv` first, then message-map handlers (`Class__OnCommand_<id>`, `Class__OnUpdate_<id>`, `Class__OnWM_PAINT`, …), vtables (`Class__vftable`, `Class__vf<slot>` for a function in one class's vtable only) and import slots (`__imp_<dll>_<name>`) | `tools/index.py` |

Regenerate everything but `names.tsv` from your copy:

```bash
python3 -I tools/index.py extracted/stage1/FastView.exe index
```

Load the names into a disassembler:

```bash
python3 -I tools/symbols_export.py ghidra > fastview_symbols.py   # Ghidra Script Manager
python3 -I tools/symbols_export.py idc    > fastview_symbols.idc  # IDA: File ▸ Script file
python3 -I tools/symbols_export.py r2     > fastview_symbols.r2   # rizin / radare2: . fastview_symbols.r2
```

The generated scripts use each tool's standard naming calls; they were
checked for syntax here, not run in every tool — report what breaks.
Command ids in the handler names can be looked up in the notes
(`docs/original/specs/ui-behaviour.md`).

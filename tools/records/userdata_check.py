#!/usr/bin/env python3
"""Look for the values of a source record in a generated record (counts only).

    python3 -I tools/records/userdata_check.py --source SRC.cfg [--source SRC2.cfg ...] OUT.cfg [OUT2.cfg ...]

Every source record (its `.cfg`, and the `.dat` / `.hdr` beside it; a single-file `.cff` with all its text sections;
an OSC2 `.osc` through its channel lines, calibration, ratios, timing codes and trigger moment) is indexed in memory:

- text: every text field of the `.cfg` and `.hdr` (names, station, device, dates and times, comment
  lines) and the source's file name, case-folded, apart from structure (standard units, phase letters,
  the data-type words, revision years; a time code is a record's own value and is checked);
- words: every word of four letters or more of those texts;
- numbers: every number that identifies (not 0, +-1, a whole number up to 100, a revision year, an
  ADC range or the float32 range) in the `.cfg` and `.hdr`, and every non-whole data value with six or
  more significant digits, compared by value to six significant digits;
- lines: every `.cfg` / `.hdr` line holding one of those, compared whole;
- codes: the rate-segment lines and the lines after the multiplier (time code / local code, time quality /
  leap second) as whole lines, however short their fields;
- labels: the phase and ccbm fields by channel position, apart from format phases (A, B, C, N, L1 ...);
- windows: four consecutive data rows of the analog channels (not constant), and columns: six consecutive
  values of one channel (three distinct values; for whole numbers a spread of 16 or more, so that small
  whole-number samples of unrelated records do not meet by chance; any non-whole value counts), compared as
  numbers, so a copied stretch of samples or a copied channel shows even where single samples are small whole
  numbers; and, for a timestamp-only record, four consecutive steps of its time column (differences, so an offset
  does not hide a copy; three distinct steps).

Not compared, because the generator keeps them on purpose: the channel-count line and channel numbers, digital
normal states and the first row's digital states, format phases, standard units, P/S, the frequency line, the rate-segment count and the
approximate ratios, the layout of name tokens, the .hdr's punctuation, the data type. A clean result means no value
was copied, not that the structure is unrecognisable: an output made from a user record stays local.

Each output record is then searched for all of them. The tool prints counts only, never a value of any
record, and exits 1 when anything matched. At most --max-rows data rows of each record are read (a
default of two million).
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import recfmt  # noqa: E402

WINDOW = 4
COLUMN_WINDOW = 6  # a per-channel window: six values of one channel
COLUMN_SPREAD = 16  # whole numbers: a per-channel window counts from this spread (narrower ones meet by chance)
COLUMN_CAP = 2_000_000  # at most this many per-channel windows of a record are indexed


def _whole(v):
    return v == int(v)


def _data_float(v):
    if _whole(v):
        return None
    digits = ("%.10g" % abs(v)).replace(".", "").replace("-", "").lstrip("0")
    if "e" in digits:
        digits = digits.split("e")[0]
    return recfmt.sig6(v) if len(digits.rstrip("0")) >= 6 else None


def _iter_texts(rec):
    """(line, kind) text of the cfg fields and hdr; yields (field text, whole line text)."""
    if rec.is_osc:
        yield from rec.iter_texts()
        return
    for i, line in enumerate(rec.cfg.lines):
        if i == 1:  # the channel-count line is structure
            continue
        for k, f in enumerate(line.split(",")):
            if k == 0 and rec.analog_at <= i < rec.freq_at:
                continue  # a channel's number
            yield f, line
    for part in rec.others:  # the .hdr, a .cff's HDR / INF / SET sections
        for line in part.lines:
            yield line, line
            for f in line.replace(":", " ").split():
                yield f, line


def codes(rec):
    """Whole lines that are a record's own values even when short: rate segments, time/quality codes."""
    if rec.is_osc:
        return rec.code_lines()
    ls = rec.cfg.lines
    pick = list(range(rec.rates_at, rec.rates_at + max(rec.n_rates, 1))) + list(range(rec.tail_at + 1, len(ls)))
    return {"".join(ls[i].split()).casefold() for i in pick if i < len(ls) and any(c.isalnum() for c in ls[i])}


def labels(rec):
    """Phase and ccbm fields by channel position: a record's own labels, unless a format phase."""
    if rec.is_osc:
        return rec.label_items()
    got = set()
    for kind, at, n in (("a", rec.analog_at, rec.n_analog), ("d", rec.digital_at, rec.n_digital)):
        for c in range(n):
            f = rec.cfg.lines[at + c].split(",")
            for k in (2, 3):
                v = f[k].strip().casefold() if len(f) > k else ""
                if v and not (k == 2 and v in recfmt.SAFE_PHASES):
                    got.add((kind, c, k, v))
    return got


def inventory(rec, max_rows):
    """What identifies a record: sets of texts, words, numbers, lines, data windows."""
    texts, nums, lines = set(), set(), set()
    for f, line in _iter_texts(rec):
        t = recfmt.identifying_text(f)
        n = recfmt.identifying_number(f.strip())
        if t:
            texts.add(t)
        if n is not None:
            nums.add(n)
        if t or n is not None:
            lines.add(" ".join(line.split()).casefold())
    stem = recfmt.identifying_text(rec.stem)  # a name that is also a structure word ("binary") is no identity
    if stem:
        texts.add(stem)
    words = recfmt.words_of(rec.cfg.text + "\n" + rec.others_text)
    windows, columns, steps = set(), set(), set()
    ring, col = [], []
    for a, d in rec.rows(max_rows):
        row = tuple(recfmt.sig6(x) for x in a) if a else tuple(d)
        for x in a:
            v = _data_float(x)
            if v is not None:
                nums.add(v)
        ring.append(row)
        if len(ring) == WINDOW:
            w = tuple(x for r in ring for x in r)
            if len(set(w)) > 2:
                windows.add(hash(w))
            ring.pop(0)
        if a:
            col.append(row)
            if len(col) == COLUMN_WINDOW:
                if len(columns) < COLUMN_CAP:
                    for j in range(len(row)):
                        c = tuple(r[j] for r in col)
                        if len(set(c)) >= 3 and (max(c) - min(c) >= COLUMN_SPREAD or any(x != int(x) for x in c)):
                            columns.add(hash(c))
                col.pop(0)
    if getattr(rec, "timestamp_only", False):  # the time column is the record's own data: windows of its steps
        prev, diffs = None, []
        for t in rec.stamps(max_rows):
            if prev is not None:
                diffs.append(recfmt.sig6(t - prev))
                if len(diffs) == WINDOW:
                    if len(set(diffs)) >= 3:
                        steps.add(hash(tuple(diffs)))
                    diffs.pop(0)
            prev = t
    return {"texts": texts, "words": words, "nums": nums, "lines": lines, "windows": windows, "columns": columns, "steps": steps,
            "codes": codes(rec), "labels": labels(rec)}


def check(src, rec, max_rows):
    """Counts of source items found in the record `rec` (all 0 means clean) and how much was checked."""
    out = inventory(rec, max_rows)
    blob = (rec.cfg.text + "\n" + rec.others_text).casefold()
    fields = set()
    for f, _ in _iter_texts(rec):
        fields.add(f.strip().strip("�").strip().casefold())
    text_hits = 0
    for t in src["texts"]:
        if (t in blob) if len(t) >= 4 else (t in fields):
            text_hits += 1
    name_hit = 1 if rec.stem.casefold() in src["texts"] else 0
    return {
        "text": text_hits + name_hit,
        "words": len(src["words"] & out["words"]),
        "numbers": len(src["nums"] & out["nums"]),
        "lines": len(src["lines"] & out["lines"]),
        "windows": len(src["windows"] & out["windows"]),
        "codes": len(src["codes"] & out["codes"]),
        "columns": len(src["columns"] & out["columns"]),
        "steps": len(src["steps"] & out["steps"]),
        "labels": len(src["labels"] & out["labels"]),
    }, {"texts": len(src["texts"]), "words": len(src["words"]), "numbers": len(src["nums"]),
        "lines": len(src["lines"]), "windows": len(src["windows"]), "out_windows": len(out["windows"]), "codes": len(src["codes"]), "columns": len(src["columns"]), "steps": len(src["steps"]), "labels": len(src["labels"])}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--source", action="append", required=True, help="a source record's .cfg (repeatable)")
    ap.add_argument("--max-rows", type=int, default=2_000_000)
    ap.add_argument("outputs", nargs="+", help="the generated records' .cfg")
    a = ap.parse_args(argv)
    sources = []
    for p in a.source:
        try:
            r = recfmt.open_record(p)
        except recfmt.UnsupportedRecord as e:
            raise SystemExit(f"error: {e}")
        sources.append(inventory(r, a.max_rows))
    bad = 0
    for o in a.outputs:
        rec = recfmt.open_record(o)
        for i, src in enumerate(sources, 1):
            hits, seen = check(src, rec, a.max_rows)
            total = sum(hits.values())
            tag = "FAIL" if total else "ok"
            print(f"{tag}: output {os.path.basename(o)} vs source {i}: matches {hits}; "
                  f"checked {seen['texts']} texts, {seen['words']} words, {seen['numbers']} numbers, "
                  f"{seen['lines']} lines, {seen['codes']} code lines, {seen['labels']} labels, {seen['windows']} row windows, {seen['columns']} column windows, {seen['steps']} time-step windows")
            bad += total
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())

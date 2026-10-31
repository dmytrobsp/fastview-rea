#!/usr/bin/env python3
"""Write a synthetic COMTRADE record with the structure and features of a given one.

    python3 -I tools/records/synth_record.py SRC.cfg OUTDIR [--seed N] [--stem NAME] [--rows N]

The source's `.cfg` (and the `.dat` / `.hdr` beside it) is read as INPUT only. The output keeps what
matters to a reader: the COMTRADE revision and its extra lines, the text encoding and BOM, the line
endings, the channel counts and kinds (analog with or without primary / secondary / P-S, digital),
the rate segments (their number and proportions), the data type (ASCII with the source's widths and
number styles, BINARY, binary32), the shape of the analog scaling (the exponent of `a`, the range
fields, samples spread over the float32 range), the presence and shape of the `.hdr`, and names cut
mid-character (the invented name is built so the cut really leaves half a character). Every name, text,
non-format phase / ccbm label, date, time, rate, time code and quality symbol (shape kept), factor and sample
is invented from the seed (deterministic: the same source structure and seed give the same bytes), then the result is
checked against the source by `userdata_check.py`'s rules; a clash makes it draw again with the next
seed (up to 50 times; a clashing draw's files are removed) and the seed used is printed.

Kept on purpose, and not compared by the check: the channel-count line and channel numbers, digital normal
states and the first row's digital states, format phases, standard units, P/S, the frequency line, the number of
rate segments and the approximate ratios of their rates and sample counts, the layout of name tokens, the
.hdr's punctuation, the data type. So an output made from a user record is local data; commit only fixtures
made from hand-written sources. Records without rate segments (timestamp-only) are refused.
"""
import argparse
import math
import os
import random
import re
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import recfmt  # noqa: E402
import userdata_check  # noqa: E402

CONS_CYR = "бвгдзклмнпрстфх"
VOW_CYR = "аеиоуы"
CONS_LAT = "bdfgklmnprstvz"
VOW_LAT = "aeiou"
BANNED_RATES = {1000, 2000, 4000, 5000}
KEEP_UNITS = recfmt.SAFE_TEXT


class Gen:
    def __init__(self, src, seed, rows):
        self.src = src
        self.rng = random.Random(seed)
        self.rows = rows
        self.cyr = bool(recfmt.CYR.search(src.cfg.text)) or bool(src.hdr and recfmt.CYR.search(src.hdr.text))
        self.avoid_words = recfmt.words_of(src.cfg.text + "\n" + (src.hdr.text if src.hdr else ""))
        self.avoid_texts = set()
        self.avoid_nums = set()
        for f, _ in userdata_check._iter_texts(src):
            n = recfmt.identifying_number(f.strip())
            if n is not None:
                self.avoid_nums.add(n)
        self.used = set()
        self.rate_factor = None

    # -- words and names
    def word(self, minlen=3, maxlen=9):
        r = self.rng
        cons, vow = (CONS_CYR, VOW_CYR) if self.cyr else (CONS_LAT, VOW_LAT)
        for _ in range(100):
            w = ""
            while len(w) < minlen:
                w += r.choice(cons) + r.choice(vow)
            if r.randrange(3) == 0 and len(w) < maxlen:
                w += r.choice(cons)
            w = w[:maxlen]
            if len(w) >= minlen and w not in self.avoid_words and w not in self.used:
                self.used.add(w)
                return w.capitalize()
        return "Xq" + str(r.randrange(100, 999))

    def name(self, like):
        """An invented name about as long as `like`, words and digits in the same places."""
        like = like.strip().strip("�")
        if not like:
            return ""
        out = []
        for tok in like.split(" "):
            if tok.isdigit() or (len(tok) <= 2 and any(c.isdigit() for c in tok)):
                out.append(str(self.rng.randrange(1, 9)))
            elif len(tok) <= 2:
                out.append(self.rng.choice("ABCN") if tok.isupper() else self.word(2, 2).lower())
            else:
                out.append(self.word(max(3, len(tok) - 2), len(tok) + 1))
        return " ".join(out)

    def cuttable(self, like, limit):
        """An invented name (of `like`'s kind) whose UTF-8 is longer than `limit` and in which byte `limit`
        is inside a multi-byte character, so that cutting at `limit` leaves half a character."""
        for n in range(400):
            nm = self.name(like)
            for extra in range(n % 5):  # longer names for the later tries
                nm += " " + self.word(3, 8)
            for pre in ("", "A", "Ab", "Abc"):
                b = (pre + nm).encode("utf-8")
                if len(b) > limit + 1 and 0x80 <= b[limit] < 0xC0:
                    return pre + nm
        raise SystemExit("cannot invent a name that a cut at byte %d leaves in half" % limit)

    def scramble(self, tok):
        """`tok` with every digit and letter replaced (same kind, same case), other characters kept."""
        r = self.rng
        cons, vow = (CONS_CYR, VOW_CYR) if self.cyr else (CONS_LAT, VOW_LAT)
        out = []
        for c in tok:
            if c.isdigit():
                out.append(str(r.randrange(10)))
            elif c.isalpha():
                x = r.choice(cons + vow)
                out.append(x.upper() if c.isupper() else x)
            else:
                out.append(c)
        return "".join(out)

    def timecode(self, tok):
        """An invented time code like `tok` (sign, hours, optional `h` and minutes), never equal to it."""
        m = re.fullmatch(r"([+-]?)(\d{1,2})(?:h(\d{1,2}))?", tok.strip())
        if not m:
            return self.symbol(tok)
        sign, hh, mm = m.groups()
        for _ in range(200):
            new = f"{sign}{self.rng.randrange(1, 13)}"
            if mm is not None:
                new += f"h{self.rng.choice([0, 15, 30, 45])}"
            if new != tok.strip() and not new.startswith(sign + hh + "h"):
                return new
        return tok

    def symbol(self, tok, pool=None):
        """A different symbol of the same kind (a digit, a hex letter) for the quality / leap fields."""
        t = tok.strip()
        if len(t) != 1 or not t.isalnum():
            return tok
        if pool is None:
            pool = "0123456789" if t.isdigit() else "ABCDEF" if t.isalpha() and t.isupper() else "abcdef"
        return self.rng.choice([c for c in pool if c != t])

    def unit(self, u):
        return u if u.strip().casefold() in KEEP_UNITS else self.word(2, 3).lower()

    def label(self, s, phase):
        """A phase or ccbm field: a phase from the format whitelist is kept; any other non-empty field is
        invented with its shape (every letter and digit replaced by a different one of its kind)."""
        t = s.strip()
        if not t or (phase and t.casefold() in recfmt.SAFE_PHASES):
            return s
        cons, vow = (CONS_CYR, VOW_CYR) if self.cyr else (CONS_LAT, VOW_LAT)
        out = []
        for c in t:
            if c.isdigit():
                out.append(self.rng.choice([d for d in "0123456789" if d != c]))
            elif c.isalpha():
                pool = recfmt.CYR.search(c) and CONS_CYR + VOW_CYR or CONS_LAT + VOW_LAT
                x = self.rng.choice([d for d in pool if d != c.lower()])
                out.append(x.upper() if c.isupper() else x)
            else:
                out.append(c)
        del cons, vow
        return s.replace(t, "".join(out))

    def short(self, s):
        """A phase / ccbm field: kept when it is a plain letter or digit, else invented."""
        if recfmt.identifying_number(s.strip()) is not None:
            return self.fresh(s)
        return s if recfmt.identifying_text(s) is None else self.word(2, 3)

    # -- numbers
    def fresh(self, tok, inside=False):
        """A different number written like `tok` (same digits, decimals, exponent, magnitude).

        With `inside` a whole number stays inside its own range (the min / max fields)."""
        t = tok.strip()
        if not t:
            return tok
        try:
            v = float(t)
        except ValueError:
            return tok
        if recfmt.is_structural_number(v):
            return tok
        r = self.rng
        for _ in range(200):
            m = re.fullmatch(r"([+-]?)(\d*)(?:\.(\d*))?(?:([eE])([+-]?\d+))?", t)
            if not m:
                return tok
            sign, ip, fp, e, ex = m.groups()
            if e:  # mantissa x 10^exponent, the exponent kept
                mant = r.randrange(1, 10)
                frac = "".join(str(r.randrange(10)) for _ in range(len(fp or "")))
                new = f"{sign}{mant}{'.' + frac if fp is not None else ''}{e}{ex}"
            elif fp is not None:  # decimal, the decade kept
                x = r.uniform(1, 10) * 10 ** math.floor(math.log10(abs(v)))
                new = f"{sign}{x:.{len(fp)}f}"
            elif inside:  # a whole range limit: somewhere within it
                new = str(round(v * r.uniform(0.5, 0.99)))
            else:  # whole number
                d = len(ip)
                new = f"{sign}{r.randrange(10 ** (d - 1), 10 ** d)}" if d > 1 else f"{sign}{r.randrange(1, 10)}"
            try:
                nv = float(new)
            except ValueError:
                continue
            if nv != v and nv != 0 and recfmt.sig6(nv) not in self.avoid_nums and not recfmt.is_structural_number(nv):
                self.avoid_nums.add(recfmt.sig6(nv))
                return new
        return tok

    def rate(self, tok):
        """An invented rate; one factor for the whole record keeps the segments' ratios."""
        v = float(tok)
        dec = len(tok.split(".")[1]) if "." in tok else 0
        if self.rate_factor is None:
            self.rate_factor = self.rng.uniform(0.55, 1.8)
        for _ in range(200):
            x = round(v * self.rate_factor * self.rng.uniform(0.99, 1.01)) + self.rng.randrange(1, 9)
            if x % 50 == 0 or x in BANNED_RATES or x == v or recfmt.sig6(x) in self.avoid_nums:
                continue
            self.avoid_nums.add(recfmt.sig6(x))
            return f"{x:.{dec}f}" if dec else str(x)
        return tok

    def date(self, tok, rev):
        """An invented date written like `tok` (day 13..27 so the order is unambiguous)."""
        if not tok.strip():
            return tok
        d, m, y = self.rng.randrange(13, 28), self.rng.randrange(1, 13), self.rng.randrange(2011, 2031)
        sep = next((c for c in tok if not c.isdigit()), "/")
        parts = tok.split(sep)
        order = [d, m] if rev == "2013" else [m, d]
        cells = [f"{p:02d}" if len(t) > 1 else str(p) for p, t in zip(order, parts)]
        cells.append(str(y) if len(parts[-1]) > 2 else f"{y % 100:02d}")
        return sep.join(cells)

    def clock(self, tok):
        """An invented time of day written like `tok`; returns (text, seconds)."""
        if not tok.strip():
            return tok, 0.0
        hh, mm, ss = self.rng.randrange(0, 22), self.rng.randrange(0, 60), self.rng.randrange(0, 59)
        frac = ""
        if "." in tok:
            frac = "." + "".join(str(self.rng.randrange(10)) for _ in range(len(tok.split(".")[1])))
        return f"{hh:02d}:{mm:02d}:{ss:02d}{frac}", hh * 3600 + mm * 60 + ss + (float("0" + frac) if frac else 0)

    def clock_at(self, tok, secs):
        n = len(tok.split(".")[1]) if "." in tok else 0
        secs = max(0.0, secs)
        whole = int(secs)
        frac = f".{round((secs - whole) * 10 ** n):0{n}d}"[: n + 1] if n else ""
        return f"{whole // 3600 % 24:02d}:{whole // 60 % 60:02d}:{whole % 60:02d}{frac}"


def generate(src, seed, rows):
    g = Gen(src, seed, rows)
    r = g.rng
    L = [list(f) for f in src.fields]
    out = []

    # line 1: station, device, revision
    first = list(L[0])
    for k in range(min(2, len(first))):
        first[k] = g.name(first[k]) if first[k].strip() else first[k]
    out.append(first)
    out.append(list(L[1]))

    cut_len = {}
    for i, raw in enumerate(src.cfg.raw_lines):
        for k, fb in enumerate(raw.split(b",")):
            try:
                fb.decode("utf-8")
            except UnicodeDecodeError:
                if src.cfg.enc == "utf-8":
                    cut_len[(i, k)] = len(fb)

    def text_field(i, k, val):
        if not val.strip():
            return val
        nm = g.name(val)
        if (i, k) in cut_len:
            return ("CUT", g.cuttable(val, cut_len[(i, k)]), cut_len[(i, k)])
        if "�" in val:
            return nm[:-1] + "�" if len(nm) > 1 else nm + "�"
        return nm

    # analog channels
    na, nd = src.n_analog, src.n_digital
    for n in range(na):
        i = src.analog_at + n
        f = list(L[i])
        for k in range(len(f)):
            if k == 0:
                continue
            elif k == 1:
                f[k] = text_field(i, k, f[k])
            elif k in (2, 3):
                f[k] = g.label(f[k], phase=k == 2)
            elif k == 4:
                f[k] = g.unit(f[k])
            elif k in (5, 6, 7, 8, 9, 10, 11):
                f[k] = g.fresh(f[k], inside=k in (8, 9))
        out.append(f)
    for n in range(nd):
        i = src.digital_at + n
        f = list(L[i])
        if len(f) > 1:
            f[1] = text_field(i, 1, f[1])
        for k in (2, 3):
            if len(f) > k:
                f[k] = g.label(f[k], phase=k == 2)
        out.append(f)

    out.append(list(L[src.freq_at]))
    # rate segments: invented rates, proportional invented counts, a new total
    total_src = max(src.total_samples, 1)
    nr = src.n_rates
    rows = max(rows, 8 * nr)
    if rows == total_src:
        rows += 1
    counts, prev = [], 0
    for k in range(nr):
        e = src.end_samples[k]
        share = (e - prev) / total_src
        prev = e
        counts.append(max(8, round(rows * share)))
    ends = [sum(counts[: k + 1]) for k in range(nr)]
    rows = ends[-1] if ends else rows
    rates = [g.rate(t) for t in src.rates]
    out.append([str(nr)] + L[src.nrates_at][1:])
    for k in range(nr):
        f = list(L[src.rates_at + k])
        f[0], f[1] = rates[k], str(ends[k])
        out.append(f)

    rev = src.rev
    sl, el = L[src.start_at], L[src.end_at]
    d1 = g.date(sl[0], rev)
    t1, s1 = g.clock(sl[1] if len(sl) > 1 else "")
    out.append([d1, t1] if len(sl) > 1 else [d1])
    # the end of the record: the start plus the duration of the data
    duration = sum((ends[k] - (ends[k - 1] if k else 0)) / float(rates[k]) for k in range(nr))
    out.append([d1, g.clock_at(el[1], s1 + duration)] if len(el) > 1 else [d1])
    out.append(list(L[src.ftype_at]))
    tail = []
    for n, line in enumerate(L[src.tail_at:]):
        if n == 1:  # time_code, local_code: the shape kept, the offsets invented
            same = len(line) > 1 and line[0] == line[1]
            new = [g.timecode(t) for t in line]
            if same:
                new[1:] = [new[0]] * (len(line) - 1)
            tail.append(new)
        elif n == 2:  # tmq_code, leapsec: other symbols of the same kind
            tail.append([g.symbol(t, "0123" if k and t.strip().isdigit() else None) for k, t in enumerate(line)])
        else:
            tail.append([g.fresh(t) if recfmt.identifying_number(t.strip()) is not None else
                         (g.short(t) if recfmt.identifying_text(t) else t) for t in line])
    out.extend(tail)

    # time multiplier (if it is not 1) as written in the cfg
    mult = 1.0
    if tail:
        try:
            mult = float(tail[0][0]) or 1.0
        except ValueError:
            mult = 1.0

    # -- text out
    cfg_bytes = encode_cfg(src.cfg, out)

    # -- data
    dat = None
    if src.dat_path:
        dat = make_dat(g, src, rates, ends, rows, mult)

    hdr = None
    if src.hdr:
        hdr = make_hdr(g, src.hdr)
    return cfg_bytes, dat, hdr


def encode_cfg(t, out):
    enc = "utf-8" if t.enc == "ascii" else t.enc
    parts = []
    for f in out:
        cells = []
        for c in f:
            if isinstance(c, tuple):
                _, nm, ln = c
                b = nm.encode("utf-8")
                # a name cut by an exporter at a byte limit, leaving half a character
                b = b[:ln] if len(b) >= ln else b
                try:
                    b.decode("utf-8")
                    raise SystemExit("internal error: a cut name came out whole")
                except UnicodeDecodeError:
                    pass
                cells.append(b)
            else:
                cells.append(c.encode(enc, errors="replace"))
        parts.append(b",".join(cells))
    eol = t.eol.encode()
    body = eol.join(parts) + (eol if t.final_eol else b"")
    return (b"\xef\xbb\xbf" if t.bom else b"") + body


def make_hdr(g, t):
    lines = []
    for line in t.lines:
        if not line.strip():
            lines.append(line)
            continue
        toks = line.split(" ")
        new = []
        for tok in toks:
            core = tok.rstrip(":,.;!?»)")
            tail = tok[len(core):]
            lead = core[: len(core) - len(core.lstrip("«(\"'"))]
            core = core[len(lead):]
            if not core:
                new.append(tok)
            elif any(c.isdigit() for c in core):
                new.append(lead + g.scramble(core) + tail)
            else:
                new.append(lead + g.word(max(3, len(core) - 2), len(core) + 1) + tail)
        lines.append(" ".join(new))
    return t.encode(lines)


def make_dat(g, src, rates, ends, rows, mult):
    r = g.rng
    na, nd = src.n_analog, src.n_digital
    ab, words = src.row_layout()
    # what the source's channels look like: the largest sample of each analog channel
    dmax = [0.0] * na
    first_digital = [0] * nd
    for n, (a, d) in enumerate(src.rows(2000)):
        for i, x in enumerate(a):
            dmax[i] = max(dmax[i], abs(x))
        if n == 0:
            first_digital = d
    # sample instants (seconds) from the segments
    t, ts = 0.0, []
    prev = 0
    for k, e in enumerate(ends):
        dt = 1.0 / float(rates[k])
        for _ in range(prev, e):
            ts.append(t)
            t += dt
        prev = e
    amp = [r.uniform(0.3, 0.9) for _ in range(na)]
    phase = [r.uniform(0, 2 * math.pi) for _ in range(na)]
    f0 = r.uniform(49.0, 51.0)
    switch = [(r.randrange(rows // 4, rows), r.randrange(0, 2)) for _ in range(nd)]
    limit = 32767 if ab == 2 else 2147483647
    style = src.ascii_style()
    wide = [m >= 1e9 for m in dmax]

    def sample(i, row):
        if dmax[i] == 0:
            return 0.0
        if wide[i]:
            mag = 10 ** r.uniform(-2, math.log10(dmax[i]))
            return mag * (1 if r.randrange(2) else -1)
        top = min(dmax[i], limit) if ab else dmax[i]
        v = top * amp[i] * math.sin(2 * math.pi * f0 * ts[row] + phase[i]) + r.uniform(-0.01, 0.01) * top
        return v

    def digital(i, row):
        base = first_digital[i] if first_digital else 0
        at, v = switch[i]
        return 1 - base if v and row >= at else base

    buf = bytearray()
    for row in range(rows):
        us = round(ts[row] * 1e6 / mult)
        vals = [sample(i, row) for i in range(na)]
        dig = [digital(i, row) for i in range(nd)]
        if ab == 0:
            tok = style["tokens"]
            cells = [fmt_like(tok[0], row + 1), fmt_like(tok[1], us)]
            for i, v in enumerate(vals):
                cells.append(fmt_like(tok[2 + i] if len(tok) > 2 + i else "0.0", v))
            for i, v in enumerate(dig):
                cells.append(str(v))
            buf += (style["sep"].join(cells) + style["eol"]).encode()
        else:
            wl = [0] * words
            for i, v in enumerate(dig):
                wl[i // 16] |= v << (i % 16)
            fmt = "<ii" + ("h" if ab == 2 else "i") * na + "H" * words
            ints = [max(-limit, min(limit, int(round(v)))) for v in vals]
            buf += struct.pack(fmt, row + 1, us, *ints, *wl)
    if ab == 0 and style and not style["final_eol"]:
        buf = buf[: -len(style["eol"])]
    if ab == 0 and style and style["ctrl_z"]:
        buf += b"\x1a"
    return bytes(buf)


def fmt_like(tok, v):
    """Write the number `v` the way the example token `tok` is written."""
    tok = tok.strip()
    m = re.fullmatch(r"([+-]?)(\d*)(?:\.(\d*))?(?:([eE])([+-]?)(\d+))?", tok)
    if not m:
        return repr(v)
    sign, ip, fp, e, es, ed = m.groups()
    if e:
        s = f"{abs(v):.{len(fp or '')}e}"
        mant, ex = s.split("e")
        exv = int(ex)
        txt = f"{mant}{e}{'-' if exv < 0 else (es if es == '+' else '')}{abs(exv):0{len(ed)}d}"
        return ("-" if v < 0 else ("+" if sign == "+" else "")) + txt
    if fp is not None:
        txt = f"{abs(v):.{len(fp)}f}"
        return ("-" if v < 0 else ("+" if sign == "+" else "")) + txt
    n = int(round(v))
    if ip.startswith("0") and len(ip) > 1:
        return ("-" if n < 0 else "") + f"{abs(n):0{len(ip)}d}"
    return str(n)


def write_record(src, outdir, stem, seed, rows, tries=50, check_rows=200_000):
    sidx = userdata_check.inventory(src, check_rows)
    for k in range(tries):
        cfg, dat, hdr = generate(src, seed + k, rows)
        os.makedirs(outdir, exist_ok=True)
        ext = src.cfg_ext
        paths = {}
        paths[ext] = cfg
        if dat is not None:
            paths[_case(".dat", ext)] = dat
        if hdr is not None:
            paths[_case(".hdr", ext)] = hdr
        for e, data in paths.items():
            with open(os.path.join(outdir, stem + e), "wb") as f:
                f.write(data)
        out = recfmt.Record(os.path.join(outdir, stem + ext))
        hits, _ = userdata_check.check(sidx, out, check_rows)
        if not sum(hits.values()):
            return seed + k
        for e in paths:  # a clashing draw is never left behind
            os.remove(os.path.join(outdir, stem + e))
    raise SystemExit("no seed in %d draws gave a record without a clash" % tries)


def _case(e, like):
    return e.upper() if like.isupper() else e


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("source", help="the source record's .cfg (read only)")
    ap.add_argument("outdir")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--stem", default="synthetic")
    ap.add_argument("--rows", type=int, default=96, help="data rows to write (default 96)")
    a = ap.parse_args(argv)
    try:
        src = recfmt.Record(a.source)
    except recfmt.UnsupportedRecord as e:
        raise SystemExit(f"error: {e}")
    used = write_record(src, a.outdir, a.stem, a.seed, a.rows)
    print(f"wrote {a.stem} (seed {used}) to {a.outdir}")


if __name__ == "__main__":
    main()
"""Shared COMTRADE helpers of the synthetic-record tools (our own code).

`synth_record.py` writes an invented record with the structure of a given one;
`userdata_check.py` proves that nothing of the given record is in the output.
Both read a record through this module: the `.cfg` as lines of fields with the
text encoding found (BOM, UTF-8, 1251, 866, bytes cut mid-character), the sibling
`.dat` / `.hdr`, and a bounded stream of the data rows.

The module never prints a value of a record.
"""
import io
import math
import os
import re
import struct

# Fields that are structure, not identity: standard units, phase letters, the
# data-type words, revision years, ADC ranges. Time codes, time-quality and leap-second
# values are a record's own: not here. They may be the same in the source and
# the output.
SAFE_TEXT = {
    "", "a", "b", "c", "n", "ka", "ma", "v", "kv", "mv", "hz", "pu", "%", "deg", "va", "w", "kw", "mw", "mva", "var",
    "mvar", "s", "p", "ascii", "binary", "binary32", "float32", "utc", "local", "а", "в", "с", "ка", "ма", "кв", "гц",
    "ма", "вт", "ва",
}
SAFE_WORDS = SAFE_TEXT | {"ascii", "binary", "binary32", "float32", "local", "utc"}
SAFE_NUMBERS = (1991, 1999, 2013, 32767, 32768, 65535, 65536, 4095, 4096, 2047, 2048, 1023, 1024, 255, 256, 127, 128)
# Phase fields that are format, not identity; any other phase or ccbm field is a record's own label.
SAFE_PHASES = {"", "a", "b", "c", "n", "r", "s", "t", "l1", "l2", "l3", "ab", "bc", "ca", "ba", "cb", "ac", "an", "bn",
               "cn", "abc", "1", "2", "3", "0", "а", "в", "с"}
FLOAT32_MAX = 3.4028235e38
COUNT = re.compile(r"^\d+\s*[AaDd]$")  # the channel-count line: 4A, 2D

CYR = re.compile("[Ѐ-ӿ]")
WORD = re.compile(r"[^\W\d_]{4,}", re.UNICODE)


def sig6(v):
    return float("%.6g" % v)


def is_structural_number(v):
    if v == 0 or abs(v) == 1:
        return True
    if v == int(v) and abs(v) in SAFE_NUMBERS:
        return True
    return abs(abs(v) - FLOAT32_MAX) < 1e-6 * FLOAT32_MAX


def identifying_number(tok):
    """The number a token holds if it identifies a record, else None."""
    try:
        v = float(tok)
    except ValueError:
        return None
    if not math.isfinite(v) or is_structural_number(v):
        return None
    if v == int(v) and abs(v) <= 100:
        return None
    return sig6(v)


def identifying_text(field):
    """The text of a field when it identifies a record, else None."""
    f = field.strip().strip("�").strip()
    if len(f) < 3:
        return None
    low = f.casefold()
    if low in SAFE_TEXT or COUNT.match(f):
        return None
    try:
        float(f)
        return None  # numbers: by value
    except ValueError:
        pass
    return low


def words_of(text):
    return {w.casefold() for w in WORD.findall(text) if w.casefold() not in SAFE_WORDS}


# ---- text files -----------------------------------------------------------

class UnsupportedRecord(ValueError):
    """A record layout the tools do not handle (reported to the user, not a crash)."""


class Text:
    """A text file: its decoded lines and what is needed to write it the same way."""

    def __init__(self, raw, enc=None):
        """`enc` fixes the encoding (a section of a `.cff`, whose encoding is the whole file's)."""
        self.raw = raw
        self.bom = enc is None and raw.startswith(b"\xef\xbb\xbf")
        body = raw[3:] if self.bom else raw
        self.eol = "\r\n" if b"\r\n" in body else "\n"
        self.final_eol = body.endswith(b"\n")
        self.enc, self.cut = self._detect(body) if enc is None else (enc, False)
        self.raw_lines = re.split(rb"\r?\n", body)
        if self.final_eol:
            self.raw_lines = self.raw_lines[:-1]
        self.lines = [self._decode(l) for l in self.raw_lines]

    def _decode(self, line):
        if self.enc == "utf-8":
            return line.decode("utf-8", errors="replace")
        return line.decode(self.enc, errors="replace")

    @staticmethod
    def _detect(body):
        if all(b < 0x80 for b in body):
            return "ascii", False
        try:
            body.decode("utf-8")
            return "utf-8", False
        except UnicodeDecodeError:
            pass
        # Mostly valid UTF-8 with a few cut sequences: an exporter's cut names.
        text = body.decode("utf-8", errors="replace")
        bad = text.count("�")
        good = len(CYR.findall(text))
        if good >= 4 * max(bad, 1) and good > 0:
            return "utf-8", True

        def score(enc):
            t = body.decode(enc, errors="replace")
            return sum(t.count(c) for c in "оаеинтсрвлкОАЕИНТСРВЛК")

        return ("cp1251" if score("cp1251") >= score("cp866") else "cp866"), False

    @property
    def text(self):
        return "\n".join(self.lines)

    def encode(self, lines):
        """Lines (str) back to bytes in this file's encoding, EOL and BOM."""
        enc = "utf-8" if self.enc == "ascii" else self.enc
        body = self.eol.join(lines) + (self.eol if self.final_eol else "")
        data = body.encode(enc, errors="replace")
        return (b"\xef\xbb\xbf" if self.bom else b"") + data


# ---- the record -------------------------------------------------------------

CFF_MARKER = re.compile(rb"--- file type: ([A-Za-z]+)[^\r\n]*?(\r?\n)")


class Record:
    """A record read from its `.cfg` (or single-file `.cff`) path: the cfg, its siblings, the layout.

    For a `.cff` the sections are taken apart the way the standard writes them (`--- file type: KIND ---`
    lines; the data section last, its byte count after a colon, or to the end of the file when it has none):
    `cfg` is the CFG section, `others` the text sections apart from CFG and DAT in file order (`hdr` is the HDR
    one), `dat_bytes` the data."""

    is_osc = False

    def __init__(self, cfg_path):
        self.cfg_path = cfg_path
        d, name = os.path.split(cfg_path)
        self.stem, self.cfg_ext = os.path.splitext(name)
        self.dir = d or "."
        self.is_cff = self.cfg_ext.lower() == ".cff"
        self.dat_path = self.hdr_path = None
        self.dat_bytes = None
        self.sections = None
        self.file_bom = False
        self.hdr = None
        self.others = []
        with open(cfg_path, "rb") as f:
            raw = f.read()
        if self.is_cff:
            self._load_cff(raw)
        else:
            self.dat_path = self._sibling(".dat")
            self.hdr_path = self._sibling(".hdr")
            self.cfg = Text(raw)
            if self.hdr_path:
                with open(self.hdr_path, "rb") as f:
                    self.hdr = Text(f.read())
                self.others = [self.hdr]
        self.fields = [l.split(",") for l in self.cfg.lines]
        self._layout()

    def _load_cff(self, raw):
        self.file_bom = raw.startswith(b"\xef\xbb\xbf")
        body = raw[3:] if self.file_bom else raw
        marks = list(CFF_MARKER.finditer(body))
        dat = next((m for m in marks if m.group(1) == b"DAT"), None)
        if dat is None or not any(m.group(1) == b"CFG" for m in marks):
            raise UnsupportedRecord("a .cff without a CFG or a DAT section")
        line = dat.group(0)
        counts = re.findall(rb":\s*(\d+)", line)
        start = dat.end()
        payload = body[start:start + int(counts[-1])] if counts else body[start:]
        if counts and body[start + int(counts[-1]):].strip():
            raise UnsupportedRecord("a .cff with something after its data section is not supported")
        if any(m.start() > dat.start() for m in marks):
            raise UnsupportedRecord("a .cff with a section after its data is not supported")
        head = body[:dat.start()]
        if head[:marks[0].start()].strip():
            raise UnsupportedRecord("a .cff with text before its first section is not supported")
        enc = Text(head).enc
        self.sections = []
        heads = [m for m in marks if m is not dat]
        for k, m in enumerate(heads):
            end = heads[k + 1].start() if k + 1 < len(heads) else dat.start()
            kind = m.group(1).decode()
            text = Text(body[m.end():end], enc=enc)
            self.sections.append((kind, m.group(0), text))
            if kind == "CFG":
                self.cfg = text
            else:
                self.others.append(text)
                if kind == "HDR":
                    self.hdr = text
        self.dat_marker = line
        self.dat_bytes = payload

    @property
    def has_dat(self):
        return self.dat_bytes is not None or bool(self.dat_path)

    @property
    def others_text(self):
        """The text of the hdr and of the other text sections (a `.cff`'s INF / SET), for the checks."""
        return "\n".join(t.text for t in self.others)

    def _open_dat(self):
        return io.BytesIO(self.dat_bytes) if self.dat_bytes is not None else open(self.dat_path, "rb")

    def _sibling(self, ext):
        for n in os.listdir(self.dir):
            s, e = os.path.splitext(n)
            if s == self.stem and e.lower() == ext:
                return os.path.join(self.dir, n)
        return None

    def _layout(self):
        L = self.fields
        self.rev = L[0][2].strip() if len(L[0]) > 2 else ""
        m = re.findall(r"(\d+)\s*([AaDd])", ",".join(L[1][1:]))
        self.n_analog = next((int(n) for n, k in m if k in "Aa"), 0)
        self.n_digital = next((int(n) for n, k in m if k in "Dd"), 0)
        i = 2
        self.analog_at = i
        i += self.n_analog
        self.digital_at = i
        i += self.n_digital
        self.freq_at = i
        self.nrates_at = i + 1
        nr = int(float(L[self.nrates_at][0])) if L[self.nrates_at][0].strip() else 0
        # A timestamp-only record has `0` here and one `0,N` line (no rate, N samples): the times are the data's own.
        self.timestamp_only = nr == 0
        self.n_rates = nr
        self.rates_at = i + 2
        lines = max(nr, 1)
        i = self.rates_at + lines
        self.start_at, self.end_at, self.ftype_at = i, i + 1, i + 2
        self.ftype = L[self.ftype_at][0].strip() if len(L) > self.ftype_at else "ASCII"
        self.tail_at = i + 3
        self.end_samples = [int(float(L[self.rates_at + k][1])) for k in range(lines)]
        self.rates = [L[self.rates_at + k][0].strip() for k in range(nr)]
        self.total_samples = self.end_samples[-1]

    # -- data
    def row_layout(self):
        words = (self.n_digital + 15) // 16
        ft = self.ftype
        if ft == "BINARY":
            return 2, words
        if ft in ("binary32", "float32", "BINARY32", "FLOAT32"):
            return 4, words
        return 0, words  # ASCII

    def rows(self, limit):
        """Up to `limit` data rows as (analog values, digital values) lists of numbers."""
        if not self.has_dat:
            return
        ab, words = self.row_layout()
        na, nd = self.n_analog, self.n_digital
        if ab == 0:
            n = 0
            with self._open_dat() as f:
                for line in f:
                    t = line.replace(b"\x1a", b"").strip().split(b",")
                    if len(t) < 2 + na + nd:
                        continue
                    try:
                        a = [float(x) for x in t[2:2 + na]]
                        d = [int(float(x)) for x in t[2 + na:2 + na + nd]]
                    except ValueError:
                        continue
                    yield a, d
                    n += 1
                    if n >= limit:
                        return
            return
        fmt = "<ii" + ("h" if ab == 2 else "i") * na + "H" * words
        size = struct.calcsize(fmt)
        n = 0
        with self._open_dat() as f:
            while n < limit:
                buf = f.read(size * min(4096, limit - n))
                if len(buf) < size:
                    return
                for t in struct.iter_unpack(fmt, buf[: len(buf) // size * size]):
                    a = list(t[2:2 + na])
                    ws = t[2 + na:]
                    d = [(ws[k // 16] >> (k % 16)) & 1 for k in range(nd)]
                    yield a, d
                    n += 1
                    if n >= limit:
                        return

    def stamps(self, limit):
        """Up to `limit` time stamps (the second field of each data row) of a record's rows, as numbers."""
        if not self.has_dat:
            return
        ab, words = self.row_layout()
        na, nd = self.n_analog, self.n_digital
        n = 0
        with self._open_dat() as f:
            if ab == 0:
                for line in f:
                    t = line.replace(b"\x1a", b"").strip().split(b",")
                    if len(t) < 2 + na + nd:
                        continue
                    try:
                        yield float(t[1])
                    except ValueError:
                        continue
                    n += 1
                    if n >= limit:
                        return
                return
            fmt = "<ii" + ("h" if ab == 2 else "i") * na + "H" * words
            size = struct.calcsize(fmt)
            while n < limit:
                buf = f.read(size * min(4096, limit - n))
                if len(buf) < size:
                    return
                for t in struct.iter_unpack(fmt, buf[: len(buf) // size * size]):
                    yield t[1]
                    n += 1
                    if n >= limit:
                        return

    def ascii_style(self):
        """How the first ASCII row is written: widths, separators, number styles."""
        if not self.has_dat or self.row_layout()[0] != 0:
            return None
        with self._open_dat() as f:
            raw = f.read(65536)
            f.seek(0, os.SEEK_END)
            size = f.tell()
            f.seek(max(0, size - 16))
            tail = f.read()
        line = raw.split(b"\n")[0]
        eol = "\r\n" if line.endswith(b"\r") else "\n"
        sep = ", " if b", " in line else ","
        t = [x.strip().decode("latin-1") for x in line.strip().split(b",")]
        return {"eol": eol, "sep": sep, "tokens": t,
                "final_eol": tail.rstrip(b"\x1a").endswith(b"\n"), "ctrl_z": tail.endswith(b"\x1a")}


# ---- OSC2 oscillograms (`.osc`) ----------------------------------------------------

OSC_BLOCK = 128


def _bcd(b):
    return (b >> 4) * 10 + (b & 15)


class _Lines:
    """The lines of an `.osc`'s channel table as the checks see a text file (`lines`, `text`)."""

    def __init__(self, lines):
        self.lines = lines
        self.text = "\n".join(lines)


class OscRecord:
    """An OSC2 oscillogram, read for the generator and the check (layout: docs/original/specs/formats.md).

    Big-endian 128-byte blocks: a header (version, the number H of header blocks, K calibration blocks), the
    calibration (four numbers per analog channel), text blocks (`nnA` analog / `nnD` digital / `nnF` samples
    per block, one tag line then the channel lines), an info block (trigger moment, the count of data blocks,
    the pre-trigger blocks, a ratio per channel) and the data blocks (N sample records and a timing code).
    What the format keeps in bytes the tools do not name is not read and is written as zeros."""

    is_osc = True
    is_cff = False
    timestamp_only = False
    has_dat = True
    others = ()
    others_text = ""
    hdr = None
    dat_path = None

    def __init__(self, path):
        self.cfg_path = path
        d, name = os.path.split(path)
        self.stem, self.cfg_ext = os.path.splitext(name)
        self.dir = d or "."
        with open(path, "rb") as f:
            self.raw = raw = f.read()
        B = OSC_BLOCK
        if len(raw) < 3 * B:
            raise UnsupportedRecord("an .osc shorter than three blocks")
        self.major, self.minor, self.H = raw[0], raw[1], raw[2]
        self.K = 2 if raw[0x7D] else 1
        if self.H < self.K + 2 or (self.H + 1) * B > len(raw):
            raise UnsupportedRecord("an .osc whose header blocks do not fit the file")
        # text blocks: zeros dropped, the encoding found over all of them
        texts = [bytes(b for b in raw[i * B:(i + 1) * B] if b) for i in range(self.K + 1, self.H)]
        self.enc = Text._detect(b"".join(texts))[0]
        self.blocks = []  # dicts: tag, count, tagline (bytes), eol, trailing, lines (str) after the tag line
        self.n_analog = self.n_digital = 0
        self.n_per_block = None
        for t in texts:
            tag, count = t[:3].decode("latin-1"), self._wtoi(t[4:7])
            if tag not in ("nnA", "nnD", "nnF"):
                raise UnsupportedRecord("an .osc text block with an unknown tag")
            eol = "\r\n" if b"\r\n" in t else ("\r" if b"\r" in t else "\n")
            parts = [p for p in re.split(rb"[\r\n]+", t) if p]
            lines = [p.decode(self.enc if self.enc != "ascii" else "utf-8", errors="replace") for p in parts[1:]]
            self.blocks.append({"tag": tag, "count": count, "tagline": parts[0], "eol": eol,
                                "trailing": t.endswith((b"\r", b"\n")), "lines": lines})
            if tag == "nnA":
                self.n_analog += count
            elif tag == "nnD":
                self.n_digital += count
            else:
                self.n_per_block = self._wtoi(t[t.find(b",") + 1:]) if b"," in t else 0
        if not self.n_per_block or self.n_per_block <= 0:
            raise UnsupportedRecord("an .osc without a samples-per-block (nnF) block")
        self.words = self.n_digital // 16
        self.rec_len = (self.n_analog + self.words) * 2
        if self.n_per_block * self.rec_len > 124:
            raise UnsupportedRecord("an .osc whose sample records do not fit a data block")
        for b in self.blocks:
            if b["tag"] == "nnA" and any(len(l.split(",")) not in (4, 5) for l in b["lines"]):
                raise UnsupportedRecord("an .osc analog line that is not index,name,phase,[ccbm,]unit")
            if b["tag"] == "nnD" and any(len(l.split(",")) != 3 for l in b["lines"]):
                raise UnsupportedRecord("an .osc digital line that is not index,name,normal")
        # calibration, info
        self.cal = []
        for i in range(self.n_analog):
            if i // 8 >= self.K:
                self.cal.append(None)
                continue
            blk = raw[(1 + i // 8) * B:(2 + i // 8) * B]
            j = i % 8
            self.cal.append((struct.unpack_from(">f", blk, 4 * j)[0], struct.unpack_from(">h", blk, 0x20 + 2 * j)[0],
                             struct.unpack_from(">f", blk, 0x30 + 4 * j)[0], struct.unpack_from(">h", blk, 0x50 + 2 * j)[0]))
        info = raw[self.H * B:(self.H + 1) * B]
        self.max_blocks = struct.unpack_from(">h", info, 2)[0]
        self.pretrig = struct.unpack_from(">h", info, 0xE)[0]
        self.trigger = tuple(_bcd(info[i]) for i in (5, 6, 7, 8, 9, 0xA, 0xB))  # ms/10, s, min, h, day, month, year-2000
        self.ratios = [struct.unpack_from(">f", info, 0x40 + 4 * i)[0] if 0x40 + 4 * i + 4 <= B else 1.0 for i in range(self.n_analog)]
        self.data_at = (self.H + 1) * B
        avail = (len(raw) - self.data_at) // B
        self.n_blocks = avail if self.max_blocks <= 0 else min(avail, self.max_blocks)
        self.codes_by_block = [struct.unpack_from(">h", raw, self.data_at + b * B + 0x7C)[0] for b in range(self.n_blocks)]
        self.total_samples = self.n_blocks * self.n_per_block
        self.ftype = "OSC"
        self.rev = ""
        self.cfg = _Lines(self.channel_lines() + [self.trigger_text()])

    @staticmethod
    def _wtoi(b):
        m = re.match(rb"\s*([+-]?\d+)", b)
        return int(m.group(1)) if m else 0

    def trigger_text(self):
        ms, s, mi, h, d, mo, y = self.trigger
        return f"{d:02d}.{mo:02d}.{2000 + y} {h:02d}:{mi:02d}:{s:02d}.{ms:02d}0"

    def analog_lines(self):
        return [l for b in self.blocks if b["tag"] == "nnA" for l in b["lines"]]

    def digital_lines(self):
        return [l for b in self.blocks if b["tag"] == "nnD" for l in b["lines"]]

    def channel_lines(self):
        return self.analog_lines() + self.digital_lines()

    def iter_texts(self):
        """(field, line) of everything that identifies a record: names, phases, units, calibration, ratios, the
        timing codes and the trigger moment (the channel numbers and counts are structure)."""
        for l in self.analog_lines() + self.digital_lines():
            for f in l.split(",")[1:]:
                yield f, l
        for i, c in enumerate(self.cal):
            if c is None:
                continue
            line = "cal %d %s" % (i, " ".join("%.6g" % x for x in c))
            for x in c:
                yield "%.6g" % x, line
        for i, r in enumerate(self.ratios):
            yield "%.6g" % r, "ratio %d %.6g" % (i, r)
        for c in sorted(set(self.codes_by_block)):
            yield str(c), "code %d" % c
        yield self.trigger_text(), self.trigger_text()

    def code_lines(self):
        """Lines that are a record's own values even when short: the timing codes, the trigger moment."""
        return {"code%d" % c for c in set(self.codes_by_block)} | {self.trigger_text().casefold().replace(" ", "")}

    def label_items(self):
        got = set()
        c = 0
        for b in self.blocks:
            if b["tag"] != "nnA":
                continue
            for l in b["lines"]:
                f = l.split(",")
                phase, ccbm = f[2].strip().casefold(), (f[3].strip().casefold() if len(f) == 5 else "")
                if phase and phase not in SAFE_PHASES:
                    got.add(("a", c, 2, phase))
                if ccbm:
                    got.add(("a", c, 3, ccbm))
                c += 1
        return got

    def rows(self, limit):
        """Up to `limit` sample records as (raw analog values, digital bits) lists."""
        na, nd, words, n = self.n_analog, self.n_digital, self.words, 0
        fmt = ">" + "h" * na + "H" * words
        for b in range(self.n_blocks):
            base = self.data_at + b * OSC_BLOCK
            for s in range(self.n_per_block):
                t = struct.unpack_from(fmt, self.raw, base + s * self.rec_len)
                ws = t[na:]
                yield list(t[:na]), [(ws[k // 16] >> (k % 16)) & 1 if k < 16 * words else 0 for k in range(nd)]
                n += 1
                if n >= limit:
                    return


def open_record(path):
    """The record at `path`: an OSC2 oscillogram for `.osc`, else a COMTRADE `.cfg` / `.cff`."""
    return OscRecord(path) if os.path.splitext(path)[1].lower() == ".osc" else Record(path)

"""Shared COMTRADE helpers of the synthetic-record tools (our own code).

`synth_record.py` writes an invented record with the structure of a given one;
`userdata_check.py` proves that nothing of the given record is in the output.
Both read a record through this module: the `.cfg` as lines of fields with the
text encoding found (BOM, UTF-8, 1251, 866, bytes cut mid-character), the sibling
`.dat` / `.hdr`, and a bounded stream of the data rows.

The module never prints a value of a record.
"""
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

    def __init__(self, raw):
        self.raw = raw
        self.bom = raw.startswith(b"\xef\xbb\xbf")
        body = raw[3:] if self.bom else raw
        self.eol = "\r\n" if b"\r\n" in body else "\n"
        self.final_eol = body.endswith(b"\n")
        self.enc, self.cut = self._detect(body)
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

class Record:
    """A record read from its `.cfg` path: the cfg, its siblings, the layout."""

    def __init__(self, cfg_path):
        self.cfg_path = cfg_path
        d, name = os.path.split(cfg_path)
        self.stem, self.cfg_ext = os.path.splitext(name)
        self.dir = d or "."
        self.dat_path = self._sibling(".dat")
        self.hdr_path = self._sibling(".hdr")
        with open(cfg_path, "rb") as f:
            self.cfg = Text(f.read())
        self.hdr = None
        if self.hdr_path:
            with open(self.hdr_path, "rb") as f:
                self.hdr = Text(f.read())
        self.fields = [l.split(",") for l in self.cfg.lines]
        self._layout()

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
        if nr == 0:
            raise UnsupportedRecord("a record without rate segments (timestamp-only) is not supported")
        self.n_rates = nr
        self.rates_at = i + 2
        i = self.rates_at + nr
        self.start_at, self.end_at, self.ftype_at = i, i + 1, i + 2
        self.ftype = L[self.ftype_at][0].strip() if len(L) > self.ftype_at else "ASCII"
        self.tail_at = i + 3
        self.end_samples = [int(float(L[self.rates_at + k][1])) for k in range(nr)]
        self.rates = [L[self.rates_at + k][0].strip() for k in range(nr)]
        self.total_samples = self.end_samples[-1] if nr else 0

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
        if not self.dat_path:
            return
        ab, words = self.row_layout()
        na, nd = self.n_analog, self.n_digital
        if ab == 0:
            n = 0
            with open(self.dat_path, "rb") as f:
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
        with open(self.dat_path, "rb") as f:
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

    def ascii_style(self):
        """How the first ASCII row is written: widths, separators, number styles."""
        if not self.dat_path or self.row_layout()[0] != 0:
            return None
        with open(self.dat_path, "rb") as f:
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

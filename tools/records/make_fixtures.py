#!/usr/bin/env python3
"""Regenerate the committed generated fixtures: `crates/fvb-core/tests/data/generated/`.

    python3 -I tools/records/make_fixtures.py [--check]

The sources are written here by hand (invented names, invented numbers, no user record), each with one
combination of the features the generator keeps; `synth_record.py` makes the fixture from each with a
fixed seed. With --check nothing is written: the run fails if a committed fixture differs from what
the generator now writes (the fixtures stay reproducible), and the output is checked against its source.
"""
import argparse
import os
import struct
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import recfmt  # noqa: E402
import synth_record  # noqa: E402
import userdata_check  # noqa: E402

ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
FIXTURES = os.path.join(ROOT, "crates", "fvb-core", "tests", "data", "generated")
CRLF = "\r\n"


def ascii_rows(n, wide=False):
    out = []
    for i in range(n):
        if wide:
            v = [f"{(-1) ** i * (3.1 + i) * 10 ** (30 - i % 9):.6e}", f"{(i % 5 - 2) * 1.7e+37:.6e}"]
        else:
            v = [str(100 * i - 37), str(5 * i + 11), str(-17 * i)]
        out.append(",".join([f"{i + 1:010d}", f"{i * 800:013d}"] + v + [str(i % 2), "1"]))
    return (CRLF.join(out) + CRLF).encode()


def src_a(d):
    """2013, UTF-8 with a BOM, Cyrillic names, one cut by U+FFFD, ASCII, a tiny `a`, float32-range samples, a .hdr."""
    cfg = CRLF.join([
        "Лабораторія Вега,Геде 7,2013",
        "4,2A,2D",
        "1,Лінія Орса,A,,A,3.1e-37,0.0173,0,-3.4028235E38,3.4028235E38,5750,3,S",
        "2,Шина Нерун напр�,,,kV,2.9e-37,0.0091,0,-3.4028235E38,3.4028235E38,28300,115,S",
        "1,Лупети Кала,,,0",
        "2,Тест Мору,,,0",
        "50",
        "1",
        "1250.000000,19",
        "07/02/2024,13:41:27.731000",
        "07/02/2024,13:41:27.746200",
        "ASCII",
        "1.000000",
        "+9h45,+9h45",
        "B,2",
        "",
    ])
    with open(os.path.join(d, "a.cfg"), "wb") as f:
        f.write(b"\xef\xbb\xbf" + cfg.encode())
    with open(os.path.join(d, "a.dat"), "wb") as f:
        f.write(ascii_rows(19, wide=True))
    with open(os.path.join(d, "a.hdr"), "wb") as f:
        f.write(("Коментар: перевірка зв'язку 17\r\nСтанція «Вега» Геде 7\r\n\r\nКінець\r\n").encode())


def src_b(d):
    """1999, cp1251, BINARY int16, two rate segments, 20 digital channels, no .hdr."""
    lines = ["Подстанция Лотос,Устройство Н9,1999", "26,6A,20D"]
    for i, (nm, ph, u, a, ps) in enumerate([("Ток Ориен", "A", "A", "0.0213", "S"), ("Ток Валдо", "B", "A", "0.0213", "S"),
                                            ("Ток Мина", "C", "A", "0.0213", "S"), ("Тазонувуме Ромо", "A", "V", "0.0517", "S"),
                                            ("Тазонувуме Лунд", "B", "V", "0.0517", "S"), ("Ток Нул", "N", "A", "0.0213", "P")], 1):
        lines.append(f"{i},{nm},{ph},,{u},{a},-0.7,0,-32767,32767,431,1,{ps}")
    for i in range(1, 21):
        lines.append(f"{i},Дискрет Ра{i},,,{i % 2}")
    lines += ["50", "2", "1650.0,40", "825.0,70", "21/03/2023,05:12:44.512345", "21/03/2023,05:12:44.612345", "BINARY", "1"]
    with open(os.path.join(d, "b.CFG"), "wb") as f:
        f.write((CRLF.join(lines) + CRLF).encode("cp1251"))
    rows = bytearray()
    for i in range(70):
        ts = i * 606 if i < 40 else 40 * 606 + (i - 40) * 1212
        an = [int(9000 * __import__("math").sin(i / 3 + k)) for k in range(6)]
        rows += struct.pack("<ii6hHH", i + 1, ts, *an, (i * 37) & 0xFFFF, (i % 16))
    with open(os.path.join(d, "b.DAT"), "wb") as f:
        f.write(bytes(rows))


def src_c(d):
    """2013, UTF-8 without a BOM, a name cut mid-character at a byte limit, binary32 (int32), a cp1251 .hdr."""
    cut = "Повітряна лінія Зета".encode("utf-8")[:20]  # ends in half a character
    lines = [b"Mogu Lab Orion,Recorder Q,2013", b"5,3A,2D"]
    lines.append(b"1," + cut + b",A,,A,0.000731,0,0,-2000000000,2000000000,1180,1,P")
    lines.append(b"2,Bus Dalta,B,,kV,0.000412,0,0,-2000000000,2000000000,14700,100,P")
    lines.append(b"3,Bus Elma,C,,kV,0.000412,0,0,-2000000000,2000000000,14700,100,P")
    lines.append("1,Старт Ліра,,,1".encode())
    lines.append("2,Стоп Ліра,,,0".encode())
    lines += [b"60", b"1", b"3300.0,23", b"11/11/2022,22:09:51.072000", b"11/11/2022,22:09:51.079000", b"binary32", b"1.0",
              b"-6h15,-6h15", b"4,1"]
    with open(os.path.join(d, "c.cfg"), "wb") as f:
        f.write(b"\n".join(lines) + b"\n")
    rows = bytearray()
    for i in range(23):
        rows += struct.pack("<ii3iH", i + 1, i * 303, 1000000 * (i % 7) - 3000000, 77777 * i, -123457 * i, i % 4)
    with open(os.path.join(d, "c.dat"), "wb") as f:
        f.write(bytes(rows))
    with open(os.path.join(d, "c.hdr"), "wb") as f:
        f.write("Заметка по записи: тест Ориона\nвторая строка 2022\n".encode("cp1251"))


def src_d(d):
    """1991 shapes, Latin ASCII, LF, no .hdr."""
    lines = ["Plant Zenith,Unit R3", "6,3A,3D"]
    for i, ph in enumerate("ABC", 1):
        lines.append(f"{i},I {ph}7,{ph},,A,0.2345,0,0,-2048,2047")
    for i in range(1, 4):
        lines.append(f"{i},Contact K{i},0")
    lines += ["50", "1", "1900.0,12", "29/09/91,19:08:07.654321", "29/09/91,19:08:07.660000", "ASCII"]
    with open(os.path.join(d, "d.cfg"), "wb") as f:
        f.write(("\n".join(lines) + "\n").encode())
    rows = []
    for i in range(12):
        rows.append(f"{i + 1},{i * 526},{(i * 211) % 2000 - 1000},{(i * 97) % 700 - 350},{(i * 59) % 500 - 250},{i % 2},0,1")
    with open(os.path.join(d, "d.dat"), "wb") as f:
        f.write(("\n".join(rows) + "\n").encode())


def src_e(d):
    """Not a fixture: short and numeric phase / ccbm labels (bay numbers, component tags), for the tests."""
    lines = ["Plant Orbis,Unit K8,1999", "5,3A,2D"]
    for i, (ph, cc) in enumerate([("Q1", "12"), ("R", "T2"), ("47", "B9")], 1):
        lines.append(f"{i},Line {i}x,{ph},{cc},A,0.3141,0,0,-32767,32767,{4000 + 37 * i},1,S")
    lines.append("1,Relay one,K1,7,0")
    lines.append("2,Relay two,,W3,1")
    lines += ["50", "1", "1850.0,20", "03/05/2020,10:11:12.131415", "03/05/2020,10:11:12.141415", "ASCII", "1"]
    with open(os.path.join(d, "e.cfg"), "wb") as f:
        f.write(("\n".join(lines) + "\n").encode())
    rows = [f"{i + 1},{i * 541},{(i * 313) % 3000 - 1500},{(i * 171) % 2000 - 900},{(i * 83) % 900 - 400},{i % 2},1" for i in range(20)]
    with open(os.path.join(d, "e.dat"), "wb") as f:
        f.write(("\n".join(rows) + "\n").encode())


def src_f(d):
    """1999, Latin ASCII, CRLF, timestamp-only (`0` then `0,N`, irregular instants), a time multiplier, no .hdr."""
    lines = ["Station Quill,Recorder P4,1999", "5,2A,3D",
             "1,Volt Rho,A,,V,0.0731,0,0,-32767,32767,1370,3,P",
             "2,Amp Sigma,B,,A,0.0417,0,0,-32767,32767,225,1,P",
             "1,Trip Tau,0", "2,Block Upsi,0", "3,Alarm Phi,1",
             "50", "0", "0,17", "14/06/2022,08:15:42.318000", "14/06/2022,08:15:42.331000", "ASCII", "0.5"]
    with open(os.path.join(d, "f.cfg"), "wb") as f:
        f.write((CRLF.join(lines) + CRLF).encode())
    gaps = [0, 413, 788, 1203, 1598, 2022, 2417, 3902, 4311, 4707, 5121, 5504, 5933, 6347, 8836, 9244, 9650]
    rows = [f"{i + 1},{gaps[i]},{(i * 307) % 1900 - 800},{(i * 113) % 1200 - 500},{i % 2},0,{1 if i > 9 else 0}" for i in range(17)]
    with open(os.path.join(d, "f.dat"), "wb") as f:
        f.write((CRLF.join(rows) + CRLF).encode())


def cff(sections, dat_kind, dat, bom=False):
    """A single-file record: `--- file type: KIND ---` sections, then the data section with its byte count."""
    out = b"\xef\xbb\xbf" if bom else b""
    for kind, body in sections:
        out += f"--- file type: {kind} ---\r\n".encode() + body
    return out + f"--- file type: DAT {dat_kind}: {len(dat)} ---\r\n".encode() + dat


def src_g(d):
    """2013 single file (.cff), UTF-8 with a BOM, Cyrillic, INF and HDR sections, ASCII data with a byte count."""
    cfg = CRLF.join([
        "Підстанція Ліра,Реєстратор Т3,2013",
        "3,2A,1D",
        "1,Фідер Нота,A,,A,0.1273,0,0,-32767,32767,2310,5,P",
        "2,Шина Серпа,B,,kV,0.0891,0,0,-32767,32767,11700,100,P",
        "1,Вимикач Ронда,0",
        "50",
        "1",
        "1450.000000,21",
        "19/08/2023,16:27:03.512000",
        "19/08/2023,16:27:03.526600",
        "ASCII",
        "1.000000",
        "+3h30,+3h30",
        "C,1",
        "",
    ])
    inf = CRLF.join(["Мова: укр", "Версія файлу 4.2 від 2023", ""])
    hdr = CRLF.join(["Примітка: перевірка зв'язку Ліра 12", "Пристрій «Реєстратор» Т3", ""])
    rows = CRLF.join(f"{i + 1},{i * 690},{(i * 421) % 3000 - 1400},{(i * 173) % 2000 - 900},{(i // 7) % 2}" for i in range(21)) + CRLF
    with open(os.path.join(d, "g.cff"), "wb") as f:
        f.write(cff([("CFG", cfg.encode()), ("INF", inf.encode()), ("HDR", hdr.encode())], "ASCII", rows.encode(), bom=True))


SOURCES = [("a", src_a), ("b", src_b), ("c", src_c), ("d", src_d), ("f", src_f), ("g", src_g)]
SEED = {"a": 11, "b": 22, "c": 33, "d": 44, "f": 66, "g": 77}


def main_file(d):
    """The record's own file in a folder: its .cfg or .cff."""
    return next(os.path.join(d, n) for n in sorted(os.listdir(d)) if n.lower().endswith((".cfg", ".cff")))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    bad = 0
    with tempfile.TemporaryDirectory() as tmp:
        for name, mk in SOURCES:
            sd = os.path.join(tmp, "src_" + name)
            os.makedirs(sd)
            mk(sd)
            src = recfmt.Record(main_file(sd))
            od = os.path.join(tmp if a.check else FIXTURES, "out_" + name if a.check else name)
            seed = synth_record.write_record(src, od, name, SEED[name], 30 if name in ("d", "f") else 96)
            print(f"{name}: seed {seed}")
            for n in sorted(os.listdir(od)):
                if a.check:
                    with open(os.path.join(od, n), "rb") as f1, open(os.path.join(FIXTURES, name, n), "rb") as f2:
                        if f1.read() != f2.read():
                            print("DIFFERS:", name, n)
                            bad += 1
            out = recfmt.Record(main_file(od))
            hits, _ = userdata_check.check(userdata_check.inventory(src, 10**6), out, 10**6)
            if sum(hits.values()):
                print("CLASH:", name, hits)
                bad += 1
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())

"""Tests of the synthetic-record tools. Run: python3 -I -m unittest discover -s tools/records -p 'test_*.py'"""
import os
import shutil
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import make_fixtures  # noqa: E402
import recfmt  # noqa: E402
import synth_record  # noqa: E402
import userdata_check  # noqa: E402


def rb(path):
    with open(path, "rb") as f:
        return f.read()


def wb(path, data):
    with open(path, "wb") as f:
        f.write(data)


def cfg_of(d):
    return next(os.path.join(d, n) for n in sorted(os.listdir(d)) if n.lower().endswith(".cfg"))


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def source(self, mk):
        d = os.path.join(self.tmp, "src_" + mk.__name__)
        os.makedirs(d, exist_ok=True)
        mk(d)
        return recfmt.Record(cfg_of(d))

    def synth(self, src, seed=5, rows=40, name="o"):
        d = tempfile.mkdtemp(dir=self.tmp)
        synth_record.write_record(src, d, name, seed, rows)
        return recfmt.Record(cfg_of(d))

    def hits(self, src, out):
        h, _ = userdata_check.check(userdata_check.inventory(src, 10**6), out, 10**6)
        return h


class Generator(Base):
    def test_same_seed_same_bytes_other_seed_other_bytes(self):
        src = self.source(make_fixtures.src_a)
        a, b, c = (self.synth(src, s, name=n) for s, n in ((5, "x"), (5, "y"), (6, "z")))
        self.assertEqual(a.cfg.raw, b.cfg.raw)
        self.assertEqual(rb(a.dat_path), rb(b.dat_path))
        self.assertNotEqual(a.cfg.raw, c.cfg.raw)

    def test_structure_is_kept(self):
        for mk, enc, bom, na, nd, ftype, hdr in [
            (make_fixtures.src_a, "utf-8", True, 2, 2, "ASCII", True),
            (make_fixtures.src_b, "cp1251", False, 6, 20, "BINARY", False),
            (make_fixtures.src_c, "utf-8", False, 3, 2, "binary32", True),
            (make_fixtures.src_d, "ascii", False, 3, 3, "ASCII", False),
        ]:
            src = self.source(mk)
            out = self.synth(src)
            self.assertEqual((out.cfg.enc, out.cfg.bom, out.cfg.eol), (src.cfg.enc, src.cfg.bom, src.cfg.eol))
            self.assertEqual((out.n_analog, out.n_digital, out.ftype, out.rev), (na, nd, ftype, src.rev))
            self.assertEqual(out.n_rates, src.n_rates)
            self.assertEqual(out.hdr is not None, hdr)
            self.assertEqual(out.cfg_ext, src.cfg_ext)
            self.assertEqual(len(list(out.rows(10**6))), out.total_samples)
            self.assertEqual(out.fields[1], src.fields[1], "the channel counts")

    def test_cut_names_stay_cut(self):
        c = self.source(make_fixtures.src_c)
        out = self.synth(c)
        with self.assertRaises(UnicodeDecodeError):
            out.cfg.raw.decode("utf-8")
        a = self.source(make_fixtures.src_a)
        self.assertIn("�", self.synth(a).cfg.text)

    def test_float32_range_and_tiny_a_shape_kept(self):
        out = self.synth(self.source(make_fixtures.src_a))
        f = out.fields[out.analog_at]
        self.assertTrue(f[5].endswith("e-37"))
        self.assertEqual(f[8:10], ["-3.4028235E38", "3.4028235E38"])
        top = max(abs(x) for a, _ in out.rows(10**6) for x in a)
        self.assertGreater(top, 1e20)
        self.assertLess(top, 3.5e38)

    def test_rate_segments_keep_their_ratio_and_are_not_round(self):
        src = self.source(make_fixtures.src_b)
        out = self.synth(src)
        r = [float(x) for x in out.rates]
        s = [float(x) for x in src.rates]
        self.assertAlmostEqual(r[0] / r[1], s[0] / s[1], delta=0.1)
        for x in r:
            self.assertNotEqual(x % 50, 0)
            self.assertNotIn(x, s)

    def test_a_hdr_token_with_digits_loses_its_letters_too(self):
        d = os.path.join(self.tmp, "h")
        os.makedirs(d)
        make_fixtures.src_a(d)
        wb(os.path.join(d, "a.hdr"), "Путь Qxzv3\\Ляпа7 стр\r\n".encode())
        out = self.synth(recfmt.Record(os.path.join(d, "a.cfg")))
        line = out.hdr.lines[0]
        self.assertIn("\\", line)
        for w in ("Qxzv", "Ляпа"):
            self.assertNotIn(w, line)

    def test_fixtures_are_reproducible(self):
        # Regenerate into a temp folder and compare with the committed ones.
        for name, mk in make_fixtures.SOURCES:
            src = self.source(mk)
            rows = 30 if name == "d" else 96
            od = os.path.join(self.tmp, "fx_" + name)
            synth_record.write_record(src, od, name, make_fixtures.SEED[name], rows)
            for n in os.listdir(od):
                self.assertEqual(rb(os.path.join(od, n)), rb(os.path.join(make_fixtures.FIXTURES, name, n)), f"{name}/{n}")


class Check(Base):
    def test_generated_records_are_clean(self):
        for _, mk in make_fixtures.SOURCES:
            src = self.source(mk)
            self.assertEqual(sum(self.hits(src, self.synth(src)).values()), 0)

    def test_a_copy_trips_every_kind_of_match(self):
        src = self.source(make_fixtures.src_a)
        h = self.hits(src, src)
        for k in ("text", "words", "numbers", "lines", "windows"):
            self.assertGreater(h[k], 0, k)

    def test_one_kept_name_one_kept_number_one_copied_stretch_are_found(self):
        src = self.source(make_fixtures.src_b)
        out = self.synth(src, 3)
        # a kept channel name
        mod = os.path.join(self.tmp, "m1")
        shutil.copytree(os.path.dirname(out.cfg_path), mod)
        p = cfg_of(mod)
        wb(p, rb(p).replace(out.fields[out.analog_at][1].encode("cp1251"), src.fields[src.analog_at][1].encode("cp1251")))
        h = self.hits(src, recfmt.Record(p))
        self.assertGreater(h["text"], 0)
        # a kept rate
        mod2 = os.path.join(self.tmp, "m2")
        shutil.copytree(os.path.dirname(out.cfg_path), mod2)
        p2 = cfg_of(mod2)
        wb(p2, rb(p2).replace(out.rates[0].encode(), src.rates[0].encode(), 1))
        self.assertGreater(self.hits(src, recfmt.Record(p2))["numbers"], 0)
        # a copied stretch of samples
        mod3 = os.path.join(self.tmp, "m3")
        shutil.copytree(os.path.dirname(out.cfg_path), mod3)
        size = 8 + 2 * 6 + 4
        dat = os.path.join(mod3, next(n for n in os.listdir(mod3) if n.lower().endswith(".dat")))
        wb(dat, rb(src.dat_path)[: size * 8] + rb(dat)[size * 8:])
        self.assertGreater(self.hits(src, recfmt.Record(cfg_of(mod3)))["windows"], 0)

    def test_a_copied_time_code_or_quality_line_or_segment_line_is_found(self):
        src = self.source(make_fixtures.src_a)
        out = self.synth(src, 3)
        i = out.tail_at + 1
        for line_no, kind in ((i, "time code"), (i + 1, "quality"), (out.rates_at, "segment")):
            mod = tempfile.mkdtemp(dir=self.tmp)
            for n in os.listdir(os.path.dirname(out.cfg_path)):
                shutil.copy(os.path.join(os.path.dirname(out.cfg_path), n), mod)
            p = cfg_of(mod)
            raw = rb(p).split(b"\r\n")
            raw[line_no] = src.cfg.raw_lines[line_no - out.tail_at + src.tail_at] if kind != "segment" else src.cfg.raw_lines[src.rates_at]
            wb(p, b"\r\n".join(raw))
            self.assertGreater(self.hits(src, recfmt.Record(p))["codes"], 0, kind)

    def test_generated_time_codes_differ_in_value_not_shape(self):
        import re
        for mk in (make_fixtures.src_a, make_fixtures.src_c):
            src = self.source(mk)
            out = self.synth(src)
            a, b = src.fields[src.tail_at + 1], out.fields[out.tail_at + 1]
            self.assertNotEqual(a, b)
            shape = lambda t: re.sub(r"\d+", "N", t)
            self.assertEqual([shape(x) for x in a], [shape(x) for x in b])
            self.assertNotEqual(src.fields[src.tail_at + 2], out.fields[out.tail_at + 2])

    def test_phase_and_ccbm_labels_are_invented_and_compared(self):
        src = self.source(make_fixtures.src_e)
        for seed in range(1, 6):
            out = self.synth(src, seed)
            for c in range(3):
                a, b = src.fields[src.analog_at + c], out.fields[out.analog_at + c]
                if c != 1:
                    self.assertNotEqual(a[2], b[2], "a non-format phase is invented")
                    self.assertEqual(a[2].isdigit(), b[2].isdigit(), "the shape is kept")
                self.assertNotEqual(a[3], b[3])
                self.assertEqual(a[3].isdigit(), b[3].isdigit())
            self.assertEqual(out.fields[out.analog_at + 1][2], "R", "a format phase is kept")
            self.assertNotEqual(src.fields[src.digital_at][2:4], out.fields[out.digital_at][2:4])
            self.assertEqual(self.hits(src, out)["labels"], 0)
        # a copied label is a hit
        mod = tempfile.mkdtemp(dir=self.tmp)
        for n in os.listdir(os.path.dirname(out.cfg_path)):
            shutil.copy(os.path.join(os.path.dirname(out.cfg_path), n), mod)
        p = cfg_of(mod)
        lines = rb(p).split(b"\n")
        lines[out.analog_at] = b",".join(lines[out.analog_at].split(b",")[:3] + [b"12"] + lines[out.analog_at].split(b",")[4:])
        wb(p, b"\n".join(lines))
        self.assertGreater(self.hits(src, recfmt.Record(p))["labels"], 0)

    def copy_column(self, mk, name):
        """Generate from `mk`'s source, then overwrite one analog column of the output with the source's."""
        src = self.source(mk)
        out = self.synth(src, 4, rows=60)
        ab, words = src.row_layout()
        if ab:
            size = 8 + ab * src.n_analog + 2 * words
            s, o = bytearray(rb(src.dat_path)), bytearray(rb(out.dat_path))
            for r in range(min(len(s) // size, len(o) // size)):
                o[r * size + 8: r * size + 8 + ab] = s[r * size + 8: r * size + 8 + ab]
            wb(out.dat_path, bytes(o))
        else:
            s, o = rb(src.dat_path).split(b"\n"), rb(out.dat_path).split(b"\n")
            for r in range(min(len(s), len(o)) - 1):
                st, ot = s[r].split(b","), o[r].split(b",")
                ot[2] = st[2]
                o[r] = b",".join(ot)
            wb(out.dat_path, b"\n".join(o))
        return self.hits(src, recfmt.Record(out.cfg_path)), None

    def test_one_copied_sample_column_is_found(self):
        for mk in (make_fixtures.src_b, make_fixtures.src_d, make_fixtures.src_e):
            h, _ = self.copy_column(mk, mk.__name__)
            self.assertGreater(h["columns"], 0, mk.__name__)
            self.assertEqual(h["windows"], 0, "whole rows differ: only the per-channel windows see it")

    def test_a_cut_name_stays_cut_for_many_geometries(self):
        import random
        rng = random.Random(7)
        cyr = "бвгдзклмнпрстфхаеиоуы"
        tested = 0
        for limit in (15, 16, 17, 20, 21, 23, 25, 28, 31):
            d = os.path.join(self.tmp, f"cut{limit}")
            os.makedirs(d)
            name = " ".join("".join(rng.choice(cyr) for _ in range(rng.randrange(3, 8))) for _ in range(6))
            full = name.encode()
            while 0x80 > full[limit] or full[limit] >= 0xC0:  # the source itself is cut mid-character
                name = name[1:]
                full = name.encode()
            lines = [b"Plant Zed,Unit 4,2013", b"2,1A,1D", b"1," + full[:limit] + b",A,,A,0.5123,0,0,-32767,32767,3100,3,S",
                     b"1,Dig one,,,0", b"50", b"1", b"1700.0,30", b"01/02/2021,03:04:05.600000", b"01/02/2021,03:04:05.700000", b"ASCII", b"1",
                     b"+4h0,+4h0", b"2,1"]
            wb(os.path.join(d, "g.cfg"), b"\n".join(lines) + b"\n")
            wb(os.path.join(d, "g.dat"), b"\n".join(f"{i + 1},{i * 588},{i * 31 - 40},{i % 2}".encode() for i in range(30)) + b"\n")
            src = recfmt.Record(os.path.join(d, "g.cfg"))
            for seed in range(1, 31):
                out = self.synth(src, seed, rows=20)
                field = out.cfg.raw_lines[out.analog_at].split(b",")[1]
                self.assertEqual(len(field), limit)
                with self.assertRaises(UnicodeDecodeError):
                    field.decode("utf-8")
                tested += 1
        self.assertEqual(tested, 9 * 30)

    def test_unsupported_layouts_are_refused_with_a_message(self):
        d = os.path.join(self.tmp, "ts")
        os.makedirs(d)
        wb(os.path.join(d, "t.cfg"), b"S,D,1999\n1,1A,0D\n1,I,A,,A,1,0,0,-1,1,1,1,S\n50\n0\n0,5\n01/01/2020,00:00:00.0\n01/01/2020,00:00:00.1\nASCII\n1\n")
        with self.assertRaises(recfmt.UnsupportedRecord):
            recfmt.Record(os.path.join(d, "t.cfg"))
        with self.assertRaises(SystemExit) as cm:
            synth_record.main([os.path.join(d, "t.cfg"), os.path.join(d, "o")])
        self.assertIn("not supported", str(cm.exception))

    def test_a_failed_draw_leaves_no_files(self):
        src = self.source(make_fixtures.src_d)
        out = os.path.join(self.tmp, "fail")
        orig = userdata_check.check
        userdata_check.check = lambda *a, **k: ({"text": 1}, {})
        self.addCleanup(setattr, userdata_check, "check", orig)
        with self.assertRaises(SystemExit):
            synth_record.write_record(src, out, "z", 1, 20, tries=3)
        self.assertEqual(os.listdir(out), [])

    def test_leap_seconds_stay_within_0_to_3_and_differ(self):
        src = self.source(make_fixtures.src_a)
        for seed in range(1, 40):
            out = self.synth(src, seed)
            leap = out.fields[out.tail_at + 2][1]
            self.assertIn(leap, "0123")
            self.assertNotEqual(leap, src.fields[src.tail_at + 2][1])

    def test_the_cli_exits_one_on_a_match_and_prints_no_value(self):
        import contextlib
        import io

        src = self.source(make_fixtures.src_d)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            self.assertEqual(userdata_check.main(["--source", src.cfg_path, src.cfg_path]), 1)
        text = buf.getvalue()
        self.assertTrue(text.startswith("FAIL"))
        self.assertNotIn("Zenith", text)
        out = self.synth(src)
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(userdata_check.main(["--source", src.cfg_path, out.cfg_path]), 0)


if __name__ == "__main__":
    unittest.main()

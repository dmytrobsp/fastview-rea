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

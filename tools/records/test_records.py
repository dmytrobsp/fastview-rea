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
    return next(os.path.join(d, n) for n in sorted(os.listdir(d)) if n.lower().endswith((".cfg", ".cff", ".osc")))


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def source(self, mk):
        d = os.path.join(self.tmp, "src_" + mk.__name__)
        os.makedirs(d, exist_ok=True)
        mk(d)
        return recfmt.open_record(cfg_of(d))

    def synth(self, src, seed=5, rows=40, name="o"):
        d = tempfile.mkdtemp(dir=self.tmp)
        synth_record.write_record(src, d, name, seed, rows)
        return recfmt.open_record(cfg_of(d))

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
            rows = 30 if name in ("d", "f") else 96
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

    def small_integer_record(self, name, seed, rows=3000):
        """A record whose samples are small whole numbers (invented, uniform within +-20)."""
        import random
        rng = random.Random(seed)
        d = os.path.join(self.tmp, name)
        os.makedirs(d)
        lines = ["Plant %s,Unit 2,1999" % name, "3,3A,0D"]
        for i in range(1, 4):
            lines.append(f"{i},Chan {name} {i},,,A,0.5{i}27,0,0,-32767,32767,{3100 + 17 * i},3,S")
        lines += ["50", "1", f"1710.0,{rows}", "01/02/2021,03:04:05.600000", "01/02/2021,03:04:05.700000", "ASCII", "1"]
        wb(os.path.join(d, "r.cfg"), ("\n".join(lines) + "\n").encode())
        wb(os.path.join(d, "r.dat"), "".join(
            f"{i + 1},{i * 585}," + ",".join(str(rng.randint(-20, 20)) for _ in range(3)) + "\n" for i in range(rows)).encode())
        return recfmt.Record(os.path.join(d, "r.cfg"))

    def test_unrelated_small_integer_records_do_not_match_by_chance(self):
        a, b = self.small_integer_record("alfa", 1), self.small_integer_record("beta", 2)
        h = self.hits(a, b)
        self.assertEqual(h["columns"], 0, "unrelated small whole-number columns must not look like a leak")
        self.assertEqual(h["windows"], 0)

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

    def test_a_timestamp_only_record_is_generated_with_its_structure(self):
        src = self.source(make_fixtures.src_f)
        self.assertTrue(src.timestamp_only)
        for seed in range(1, 8):
            out = self.synth(src, seed, rows=30)
            self.assertTrue(out.timestamp_only)
            self.assertEqual(out.n_rates, 0)
            self.assertEqual(out.fields[out.nrates_at], ["0"])
            line = out.fields[out.rates_at]
            self.assertEqual((line[0], int(line[1])), ("0", 30), "the line `0,N` with the new count")
            self.assertEqual((out.n_analog, out.n_digital, out.ftype, out.rev), (2, 3, "ASCII", "1999"))
            rows = [r.split(",") for r in rb(out.dat_path).decode().split("\r\n") if r]
            self.assertEqual(len(rows), 30)
            stamps = [int(r[1]) for r in rows]
            steps = [b - a for a, b in zip(stamps, stamps[1:])]
            self.assertTrue(all(x > 0 for x in steps), "the instants increase")
            self.assertGreater(len(set(steps)), 5, "the steps are irregular")
            self.assertEqual(self.hits(src, out), {k: 0 for k in self.hits(src, out)})

    def binary_cff(self):
        """A single-file record with int16 BINARY data (src_b's cfg and data) and no other section."""
        src = self.source(make_fixtures.src_b)
        d = os.path.join(self.tmp, "bcff")
        os.makedirs(d)
        wb(os.path.join(d, "b.cff"), make_fixtures.cff([("CFG", rb(src.cfg_path))], "BINARY", rb(src.dat_path)))
        return recfmt.Record(os.path.join(d, "b.cff")), src

    def test_a_cff_source_is_generated_with_its_structure(self):
        src = self.source(make_fixtures.src_g)
        self.assertTrue(src.is_cff)
        self.assertEqual([k for k, _, _ in src.sections], ["CFG", "INF", "HDR"])
        self.assertEqual(len(list(src.rows(10**6))), 21)
        for seed in (1, 2, 3):
            out = self.synth(src, seed, rows=40)
            raw = rb(out.cfg_path)
            self.assertEqual(raw[:3], b"\xef\xbb\xbf")
            self.assertEqual([k for k, _, _ in out.sections], ["CFG", "INF", "HDR"], "the sections keep their order")
            self.assertEqual((out.cfg.enc, out.cfg.eol, out.rev, out.n_analog, out.n_digital, out.ftype), ("utf-8", "\r\n", "2013", 2, 1, "ASCII"))
            self.assertEqual(out.total_samples, 40)
            n = len(out.dat_bytes)
            self.assertTrue(raw.endswith(out.dat_bytes))
            self.assertIn(f"--- file type: DAT ASCII: {n} ---".encode(), raw, "the data section's byte count is the new one")
            self.assertEqual(len(list(out.rows(10**6))), 40)
            self.assertEqual(sum(self.hits(src, out).values()), 0)
            for (_, _, a), (_, _, b) in zip(src.sections, out.sections):
                self.assertNotEqual(a.text, b.text)

    def test_a_binary_cff_is_generated_and_its_count_follows_the_data(self):
        src, plain = self.binary_cff()
        out = self.synth(src, 4, rows=50)
        self.assertEqual((out.n_analog, out.n_digital, out.ftype, out.cfg.enc), (6, 20, "BINARY", "cp1251"))
        size = 8 + 2 * 6 + 2 * 2
        self.assertEqual(len(out.dat_bytes), size * out.total_samples)
        self.assertIn(f"DAT BINARY: {len(out.dat_bytes)} ---".encode(), rb(out.cfg_path))
        self.assertEqual(len(list(out.rows(10**6))), out.total_samples)
        self.assertEqual(sum(self.hits(src, out).values()), 0)
        # the same data as the sibling-file source gives, so the two layouts are checked alike
        self.assertEqual(list(src.rows(10**6)), list(plain.rows(10**6)))

    def test_a_copied_cff_section_or_data_stretch_is_found(self):
        src = self.source(make_fixtures.src_g)
        out = self.synth(src, 2, rows=40)
        raw = rb(out.cfg_path)
        # the source's HDR section in place of the output's
        mod = os.path.join(self.tmp, "m_hdr")
        os.makedirs(mod)
        (_, _, hs), (_, _, ho) = src.sections[2], out.sections[2]
        wb(os.path.join(mod, "x.cff"), raw.replace(ho.raw, hs.raw))
        self.assertGreater(self.hits(src, recfmt.Record(os.path.join(mod, "x.cff")))["text"], 0)
        # the source's data rows in place of the output's
        mod2 = os.path.join(self.tmp, "m_dat")
        os.makedirs(mod2)
        wb(os.path.join(mod2, "x.cff"), raw.replace(out.dat_bytes, src.dat_bytes))
        self.assertGreater(self.hits(src, recfmt.Record(os.path.join(mod2, "x.cff")))["windows"], 0)

    def test_cff_layouts_the_tools_do_not_take_are_refused(self):
        d = os.path.join(self.tmp, "badcff")
        os.makedirs(d)
        cfg = rb(self.source(make_fixtures.src_f).cfg_path)
        good = make_fixtures.cff([("CFG", cfg)], "ASCII", b"1,2,3\r\n")
        for name, data in (("nodat", b"--- file type: CFG ---\r\n" + cfg),
                           ("nocfg", b"--- file type: DAT ASCII: 3 ---\r\n1,2"),
                           ("after", good + b"--- file type: HDR ---\r\nx\r\n"),
                           ("before", b"stray\r\n" + good)):
            wb(os.path.join(d, name + ".cff"), data)
            with self.assertRaises(recfmt.UnsupportedRecord, msg=name):
                recfmt.Record(os.path.join(d, name + ".cff"))
        with self.assertRaises(SystemExit) as cm:
            synth_record.main([os.path.join(d, "nodat.cff"), os.path.join(d, "o")])
        self.assertIn("error:", str(cm.exception))

    def test_an_osc_source_is_generated_with_its_structure(self):
        src = self.source(make_fixtures.src_h)
        self.assertTrue(src.is_osc)
        self.assertEqual((src.n_analog, src.n_digital, src.n_per_block, src.n_blocks, src.enc), (3, 16, 14, 6, "cp1251"))
        self.assertEqual(sorted(set(src.codes_by_block)), [4711, 9421])
        for seed in (1, 2, 3, 4):
            out = self.synth(src, seed, rows=60)
            self.assertEqual((out.major, out.minor, out.H, out.K), (2, 4, src.H, 1), "version and block geometry")
            self.assertEqual((out.n_analog, out.n_digital, out.n_per_block, out.enc), (3, 16, 14, "cp1251"))
            self.assertEqual([(b["tag"], b["count"], len(b["lines"]), b["eol"]) for b in out.blocks],
                             [(b["tag"], b["count"], len(b["lines"]), b["eol"]) for b in src.blocks])
            self.assertEqual([b["tagline"] for b in out.blocks if b["tag"] == "nnF"], [b"nnF,14"])
            self.assertEqual((out.n_blocks, out.max_blocks, out.total_samples), (5, 5, 70))
            self.assertEqual(len(list(out.rows(10**6))), 70)
            self.assertEqual(len(set(out.codes_by_block)), 2, "two timing codes, as in the source")
            self.assertTrue(all(0 < c < 32768 for c in out.codes_by_block))
            self.assertEqual(len(out.raw) % 128, 0)
            self.assertEqual(len(out.analog_lines()[1].split(",")), 5, "the line with a ccbm field keeps its shape")
            self.assertTrue(all(c is not None for c in out.cal))
            self.assertNotEqual(out.trigger, src.trigger)
            self.assertEqual(sum(self.hits(src, out).values()), 0)
        a, b = self.synth(src, 5, name="x"), self.synth(src, 5, name="y")
        self.assertEqual(a.raw, b.raw, "deterministic")
        self.assertNotEqual(a.raw, self.synth(src, 6, name="z").raw)

    def test_osc_rates_are_not_round_ones(self):
        src = self.source(make_fixtures.src_h)
        m = 2  # the analog channels whose unit is V / A / empty
        for seed in range(1, 25):
            out = self.synth(src, seed)
            for c in out.codes_by_block:
                fs = 1e6 / (c * 0.03125 * m)
                for n in (1000, 2000, 4000, 5000):
                    self.assertGreater(abs(fs / n - 1), 0.03, (seed, c))

    def test_a_copied_osc_part_is_found(self):
        src = self.source(make_fixtures.src_h)
        out = self.synth(src, 2, rows=60)
        B = 128

        def altered(name, edit):
            raw = bytearray(out.raw)
            edit(raw)
            p = os.path.join(self.tmp, name + ".osc")
            wb(p, bytes(raw))
            return self.hits(src, recfmt.open_record(p))

        s = src.raw
        text_at = (src.K + 1) * B
        h = altered("name", lambda r: r.__setitem__(slice(text_at, text_at + B), s[text_at:text_at + B]))
        self.assertGreater(h["text"], 0, "a copied text block (names)")
        self.assertGreater(h["labels"], 0)
        h = altered("cal", lambda r: r.__setitem__(slice(B, 2 * B), s[B:2 * B]))
        self.assertGreater(h["numbers"], 0, "copied calibration")
        info = src.H * B

        def copy_info(r):
            r[info:info + B] = s[info:info + B]

        h = altered("info", copy_info)
        self.assertGreater(h["numbers"] + h["text"] + h["codes"], 0, "copied trigger moment and ratios")
        d0 = src.data_at

        def copy_code(r):
            struct_code = s[d0 + 0x7C:d0 + 0x7E]
            for b in range(out.n_blocks):
                r[out.data_at + b * B + 0x7C:out.data_at + b * B + 0x7E] = struct_code

        self.assertGreater(altered("code", copy_code)["codes"], 0, "a copied timing code")

        def copy_data(r):
            r[out.data_at:out.data_at + 2 * B] = s[d0:d0 + 2 * B]
            for b in range(2):  # keep the output's own timing codes
                r[out.data_at + b * B + 0x7C:out.data_at + b * B + 0x7E] = out.raw[out.data_at + b * B + 0x7C:out.data_at + b * B + 0x7E]

        h = altered("data", copy_data)
        self.assertGreater(h["windows"], 0, "copied sample rows")
        self.assertGreater(h["columns"], 0)

    def test_osc_layouts_the_tools_do_not_take_are_refused(self):
        src = self.source(make_fixtures.src_h)
        d = os.path.join(self.tmp, "badosc")
        os.makedirs(d)
        raw = bytearray(src.raw)
        cases = {"short": bytes(raw[:200]), "nodata_header": bytes(raw[:3]) + bytes(300)}
        bad_tag = bytearray(raw)
        bad_tag[(src.K + 1) * 128:(src.K + 1) * 128 + 3] = b"xyz"
        cases["tag"] = bytes(bad_tag)
        wide = bytearray(raw)
        wide[(src.H - 1) * 128 + 4:(src.H - 1) * 128 + 6] = b"99"  # nnF,14 -> nnF,99: 99 records do not fit a block
        cases["wide"] = bytes(wide)
        for name, data in cases.items():
            wb(os.path.join(d, name + ".osc"), data)
            with self.assertRaises(recfmt.UnsupportedRecord, msg=name):
                recfmt.open_record(os.path.join(d, name + ".osc"))
        with self.assertRaises(SystemExit) as cm:
            synth_record.main([os.path.join(d, "tag.osc"), os.path.join(d, "o")])
        self.assertIn("error:", str(cm.exception))

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

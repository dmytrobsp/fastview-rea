"""Tests of cloneutil.py (the logic of clone.sh). Run: python3 -I -m unittest discover -s tools/vm -p 'test_*.py'"""
import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import cloneutil as cu  # noqa: E402

G = cu.GUEST


def wr(path, data, mode="wb"):
    with open(path, mode) as f:
        f.write(data)


def wj(path, obj):
    with open(path, "w") as f:
        json.dump(obj, f)


class Sha(unittest.TestCase):
    def test_same_bytes_same_hash_changed_bytes_other(self):
        with tempfile.TemporaryDirectory() as d:
            a, b = os.path.join(d, "a.exe"), os.path.join(d, "b.exe")
            wr(a, b"MZ synthetic 1234", "wb")
            wr(b, b"MZ synthetic 1234", "wb")
            self.assertEqual(cu.sha256_file(a), cu.sha256_file(b))
            wr(b, b"x", "ab")
            self.assertNotEqual(cu.sha256_file(a), cu.sha256_file(b))


class Overlay(unittest.TestCase):
    def test_line_and_warning_thresholds(self):
        gib = cu.GIB
        line, warn = cu.overlay_line(int(0.5 * gib), 4)
        self.assertEqual(line, "overlay: 0.50 GB of 4 GB (12 %)")
        self.assertFalse(warn)
        line, warn = cu.overlay_line(int(2.6 * gib), 4)
        self.assertTrue(warn)
        self.assertIn("WARNING", line)
        line, warn = cu.overlay_line(int(4.1 * gib), 4)
        self.assertIn("OVER THE CAP", line)

    def test_fractional_cap_prints_plainly(self):
        self.assertIn("of 1.5 GB", cu.overlay_line(0, 1.5)[0])

    def test_cap_from_boxes_json_and_defaults(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "boxes.json")
            wj(p, {"instances": {"overlay_cap_gb": {"windows": 4}}})
            self.assertEqual(cu.cap_gb(p), 4.0)
            wj(p, {"instances": {}})
            self.assertEqual(cu.cap_gb(p), cu.DEFAULT_CAP_GB)
            wr(p, "not json", "w")
            self.assertEqual(cu.cap_gb(p), cu.DEFAULT_CAP_GB)
            self.assertEqual(cu.cap_gb(os.path.join(d, "missing.json")), cu.DEFAULT_CAP_GB)

    def test_folder_bytes_counts_regular_files_only(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(cu.folder_bytes(d), 0)
            wr(os.path.join(d, "overlay.qcow2"), os.urandom(50000), "wb")
            os.mkdir(os.path.join(d, "sub"))
            wr(os.path.join(d, "sub", "x"), b"y" * 40000, "wb")
            os.symlink("/nonexistent", os.path.join(d, "link"))
            n = cu.folder_bytes(d)
            self.assertGreaterEqual(n, 50000)
            self.assertLess(n, 50000 + 40000)  # the subfolder is not counted
            self.assertIsNone(cu.folder_bytes(os.path.join(d, "missing")))

    def test_cli_prints_a_line_for_a_folder(self):
        with tempfile.TemporaryDirectory() as root:
            os.mkdir(os.path.join(root, "tb-abc123"))
            wr(os.path.join(root, "tb-abc123", "o"), os.urandom(70000), "wb")
            boxes = os.path.join(root, "boxes.json")
            wj(boxes, {"instances": {"overlay_cap_gb": {"windows": 4}}})
            env = dict(os.environ, TESTBOX_VM_ROOT=root, TESTBOX_BOXES=boxes)
            out = subprocess.run([sys.executable, "-I", os.path.join(HERE, "cloneutil.py"), "overlay", "abc123"],
                                 env=env, capture_output=True, text=True, stdin=subprocess.DEVNULL).stdout
            self.assertTrue(out.startswith("overlay: 0.00 GB of 4 GB"), out)
            out = subprocess.run([sys.executable, "-I", os.path.join(HERE, "cloneutil.py"), "overlay", "nope"],
                                 env=env, capture_output=True, text=True, stdin=subprocess.DEVNULL).stdout
            self.assertIn("unknown", out)


class PullCleanup(unittest.TestCase):
    def test_outputs_are_deleted(self):
        for src in (G + r"\harness\shots\a.png", G + r"\out\x.pdf", G + "\\out\\sub\\y.txt", G + "/out/z.txt"):
            cmd = cu.pull_cleanup(src)
            self.assertIsNotNone(cmd, src)
            self.assertIn("del /f /q", cmd)
        self.assertIn(r'"C:\Users\tbuser\fv-test\out\z.txt"', cu.pull_cleanup(G + "/out/z.txt"))

    def test_installation_and_areas_themselves_are_kept(self):
        for src in (G + r"\FastView5\FastView.exe", G + r"\ours\fastview.exe", G + r"\records\x.cfg",
                    G + r"\harness\drive.ps1", G + r"\harness\shots", G + "\\out", G + "\\out\\",
                    G, r"C:\Windows\System32\x", r"C:\Users\tbuser\Documents\a.pdf", "", G + r"\outside\a"):
            self.assertIsNone(cu.pull_cleanup(src), src)

    def test_traversal_and_metacharacters_refused(self):
        for src in (G + r"\out\..\ours\fastview.exe", G + r"\out\a & del x", G + r"\out\a|b",
                    G + r'\out\a"b', G + r"\out\%TEMP%", G + "\\out\\a\nb"):
            self.assertIsNone(cu.pull_cleanup(src), src)

    def test_case_insensitive_like_windows(self):
        self.assertIsNotNone(cu.pull_cleanup(G.upper() + r"\OUT\a.png"))
        self.assertIsNone(cu.pull_cleanup(G.upper() + r"\OURS\a.exe"))


if __name__ == "__main__":
    unittest.main()

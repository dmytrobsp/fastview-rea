#!/usr/bin/env python3
"""cloneutil.py: the logic clone.sh keeps out of bash so it can be tested without a clone.

  cloneutil.py sha256 FILE            the file's SHA-256
  cloneutil.py overlay ID             one line: the clone folder's size against the cap (+ a warning near it)
  cloneutil.py pull-cleanup SRC       the cmd.exe command that deletes SRC in the clone after a pull (exit 1: keep it)

The overlay is the clone's folder in the host's tmpfs, counted as the testbox tool counts it: the allocated bytes of
the regular files in it (st_blocks * 512), against the cap `instances.overlay_cap_gb.windows` of its boxes.json
(read only; default 2). A clone past its cap is removed by the tool, so the script warns well before.
Run the tests: python3 -I -m unittest discover -s tools/vm -p 'test_*.py'
"""
import hashlib
import json
import os
import stat
import sys

GIB = 1 << 30
DEFAULT_CAP_GB = 2.0
WARN_FRACTION = 0.6  # a warning from 60 % of the cap
GUEST = "C:\\Users\\tbuser\\fv-test"
# Under GUEST the folders the harness writes outputs to: a pull deletes what is inside them, nothing else
# (FastView5, records, ours and the harness scripts are the clone's installation).
PULLED_AREAS = ("harness\\shots", "out")
BAD_CHARS = set('"&|%^<>\r\n\t!')


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def folder_bytes(d):
    """The allocated bytes of the regular files in D (what the testbox tool counts)."""
    total = 0
    try:
        names = os.listdir(d)
    except OSError:
        return None
    for n in names:
        try:
            st = os.lstat(os.path.join(d, n))
        except OSError:
            continue
        if stat.S_ISREG(st.st_mode):
            total += st.st_blocks * 512
    return total


def cap_gb(boxes_json):
    """The windows overlay cap in GiB from the testbox boxes.json (the default when unreadable)."""
    try:
        with open(boxes_json) as f:
            v = json.load(f)["instances"]["overlay_cap_gb"]["windows"]
        v = float(v)
        return v if v > 0 else DEFAULT_CAP_GB
    except (OSError, ValueError, KeyError, TypeError):
        return DEFAULT_CAP_GB


def overlay_line(used, cap_g):
    """('overlay: 0.52 GB of 4 GB (13 %)', warn). WARN text is part of the line near the cap."""
    cap = cap_g * GIB
    frac = used / cap
    capt = ("%g" % cap_g)
    line = "overlay: %.2f GB of %s GB (%d %%)" % (used / GIB, capt, round(frac * 100))
    warn = frac >= WARN_FRACTION
    if frac >= 1:
        line += " - OVER THE CAP: the tool removes the clone"
    elif warn:
        line += " - WARNING near the cap: delete pulled outputs, push no more, finish and rm"
    return line, warn


def pull_cleanup(src, guest=GUEST):
    """The cmd.exe command deleting SRC (a file or folder in the clone) after it was pulled, or None to keep it.

    Only what lies inside the two areas the harness writes outputs to (out\\ and harness\\shots\\ under the guest's
    fv-test folder) qualifies, never an area itself; a path with a shell metacharacter or a `..` never does."""
    if not src or any(c in BAD_CHARS for c in src) or ".." in src.replace("\\", "/").split("/"):
        return None
    s = src.replace("/", "\\").rstrip("\\")
    low, g = s.lower(), guest.lower() + "\\"
    if not low.startswith(g):
        return None
    rel = low[len(g):]
    if not any(rel.startswith(a + "\\") for a in PULLED_AREAS):
        return None
    # Windows trims trailing dots and spaces, so `out\.` or `out\ ` would name the area itself; wildcards and
    # stream / drive colons are never a single pulled output.
    if any(c in "*?:" for c in rel) or any(p.strip(". ") == "" or p[-1] in ". " for p in rel.split("\\")):
        return None
    return 'if exist "%s\\*" (rmdir /s /q "%s") else (del /f /q "%s")' % (s, s, s)


def clone_folder(clone_id, root=None):
    root = root or os.environ.get("TESTBOX_VM_ROOT") or "/dev/shm/testbox-%d" % os.getuid()
    return os.path.join(root, "tb-" + clone_id)


def main(argv):
    if len(argv) >= 3 and argv[1] == "sha256":
        print(sha256_file(argv[2]))
        return 0
    if len(argv) >= 3 and argv[1] == "overlay":
        used = folder_bytes(clone_folder(argv[2]))
        if used is None:
            print("overlay: size unknown (clone folder not found)")
            return 0
        boxes = os.environ.get("TESTBOX_BOXES") or os.path.expanduser("~/projects/testbox/boxes.json")
        line, _ = overlay_line(used, cap_gb(boxes))
        print(line)
        return 0
    if len(argv) >= 3 and argv[1] == "pull-cleanup":
        cmd = pull_cleanup(argv[2])
        if cmd is None:
            return 1
        print(cmd)
        return 0
    sys.stderr.write(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))

#!/usr/bin/env python3
"""Publish guard: keeps the original program, raw decompilation / disassembly
output and non-approved identities out of this repository's history.

  tools/publish_guard.py --staged          what is about to be committed
  tools/publish_guard.py --range A..B      commits about to be pushed
  tools/publish_guard.py --all             the whole history (CI)

Checks
  * author / committer emails and any email in commit messages must be on
    the allowlist (an allowlist, so no other address is ever written here);
  * no binaries of the original, no regenerated dumps, no carved payloads
    (by path), no file over the size limit;
  * no added line that looks like decompiler pseudocode or an objdump
    listing.
Wired as git hooks by `git config core.hooksPath tools/git-hooks`; CI runs
`--all`. Exit status 1 with a list of findings when anything is caught.
"""
import re
import subprocess
import sys

ALLOWED_EMAILS = {
    "dmytrobsp@gmail.com",
    "noreply@anthropic.com",  # Co-Authored-By trailers
    "noreply@github.com",  # merges made on github.com
}
ALLOWED_EMAIL_SUFFIXES = ("@users.noreply.github.com",)

FORBIDDEN_PATHS = [
    r"^extracted/",  # the original installer payload
    r"^dumps/",  # tools/regen-dumps.sh output
    r"^vcr/",  # carved vcredist payload
    r"(^|/)notes/ui/",  # resource dumps
    r"^notes/.*\.(txt|json|log)$",  # raw tool output next to the specs
    r"\.(exe|dll|chm|cab|msi|ocx|sys|pdb|asm)$",
    r"(^|/)decomp[^/]*$",
    r"(^|/)[^/]*pseudo[^/]*\.(c|txt)$",
]
MAX_BYTES = 2_000_000
CONTENT = [
    (r"\bundefined[1248]\b|\bin_ECX\b|\bExceptionList\b|\bpu?Stack_[0-9a-f]+\b|\bOrdinal_\d+\(", "decompiler pseudocode"),
    (r"^\s*[0-9a-f]{6,8}:\t([0-9a-f]{2} )+", "objdump listing"),
]
EMAIL = re.compile(r"[\w.+-]+@[\w-]+(\.[\w-]+)+")


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True, errors="replace").stdout


def email_ok(e):
    e = e.lower()
    return e in ALLOWED_EMAILS or e.endswith(ALLOWED_EMAIL_SUFFIXES)


def check_path(path, size, where, problems):
    for pat in FORBIDDEN_PATHS:
        if re.search(pat, path):
            problems.append(f"{where}: forbidden path {path}")
            return
    if size is not None and size > MAX_BYTES:
        problems.append(f"{where}: {path} is {size} bytes (limit {MAX_BYTES})")


def check_added_lines(diff, where, problems):
    path = None
    for line in diff.splitlines():
        if line.startswith("+++ "):
            path = line[6:] if line.startswith("+++ b/") else None
        elif line.startswith("+") and not line.startswith("+++") and path and path != "tools/publish_guard.py":
            for pat, what in CONTENT:
                if re.search(pat, line[1:]):
                    problems.append(f"{where}: {what} in {path}: {line[1:80].strip()}")
                    break


def check_staged(problems):
    e = git("config", "user.email").strip()
    if not email_ok(e):
        problems.append("git config user.email is not an approved address for this repository")
    for line in git("diff", "--cached", "--name-only", "--diff-filter=AMR", "-z").split("\0"):
        if line:
            size = int(git("cat-file", "-s", f":{line}").strip())
            check_path(line, size, "staged", problems)
    check_added_lines(git("diff", "--cached", "--no-color", "-U0"), "staged", problems)


def check_commits(rev_args, problems):
    for c in git("rev-list", *rev_args).split():
        meta = git("show", "-s", "--format=%ae%n%ce%n%B", c)
        a, cm, body = meta.split("\n", 2)
        short = c[:9]
        for who, e in (("author", a), ("committer", cm)):
            if not email_ok(e):
                problems.append(f"{short}: {who} email is not approved")
        for m in EMAIL.finditer(body):
            if not email_ok(m.group(0)):
                problems.append(f"{short}: commit message contains a non-approved email")
        for row in git("diff-tree", "-r", "--root", "--no-commit-id", "--diff-filter=AMR", "-z", c).split("\0:"):
            parts = row.lstrip(":").split("\0")
            if len(parts) >= 2 and parts[0]:
                fields = parts[0].split()
                blob, path = fields[3], parts[1]
                size = int(git("cat-file", "-s", blob).strip()) if set(blob) != {"0"} else None
                check_path(path, size, short, problems)
        check_added_lines(git("show", "--no-color", "--format=", "-U0", c), short, problems)


def main():
    args = sys.argv[1:]
    problems = []
    if args == ["--staged"]:
        check_staged(problems)
    elif len(args) == 2 and args[0] == "--range":
        check_commits([args[1]], problems)
    elif args == ["--all"]:
        check_commits(["--all"], problems)
    else:
        print(__doc__)
        return 2
    if problems:
        print("publish guard: refusing —", file=sys.stderr)
        for p in problems:
            print("  " + p, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

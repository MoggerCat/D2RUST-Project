"""Shared cache of the recorded 1.14d side of scenario-diff checks.

Spec: specs/tools/scenario-diff.md §4 (rule 3b, the orig cache).

One entry per check and channel under traces/orig-cache/<check>/<channel>/:
`cache.json` (format `orig-cache-1`, the key, the command that made it, the
files with size and sha256) and the recorder's small text output
(`orig.state.jsonl`, `orig.rng.jsonl`, `orig.packets.jsonl`,
`orig.frames.jsonl`). These are our own measurements of 1.14d (CLAUDE.md rule
1). Anything with rendered game art (PNG frames) is never stored here: a
recorder output with a PNG, or a file that is not text, is refused.

The key is what the recorded bytes depend on: the check file's recording
lines (comments and the comparator-only `ignore` lines left out), the save
d2s-tool makes from it (`--time 1`), `Game.exe`, the install manifest, and the
channel's recorder (its script and the sibling modules it imports). A changed
check, recorder or install is a miss, never a stale hit.
"""
import hashlib
import json
import os
import re
import shutil

KEY_FORMAT = "orig-cache-key-2"
ENTRY_FORMAT = "orig-cache-1"
# recorder script -> (channel, output file name in the work dir)
RECORDERS = {"record_state.py": ("state", "orig.state.jsonl"),
             "record_frames.py": ("draws", "orig.frames.jsonl"),
             "record_rng.py": ("rng", "orig.rng.jsonl"),
             "record_packets.py": ("packets", "orig.packets.jsonl")}
MAX_BYTES = 64 << 20          # one cached file; bigger is refused (keep the repo small)
_IMPORT = re.compile(r"^\s*(?:from\s+([A-Za-z_]\w*)\s+import|import\s+([A-Za-z_]\w*))", re.M)


def sha256_bytes(b):
    return hashlib.sha256(b).hexdigest()


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def recorder_version(rec_dir, script):
    """sha256 over the recorder script and, transitively, the modules of the
    same folder it imports (name and bytes, sorted)."""
    seen, todo = {}, [script]
    while todo:
        name = todo.pop()
        if name in seen:
            continue
        p = os.path.join(rec_dir, name)
        if not os.path.exists(p):
            continue
        with open(p, "rb") as f:
            data = f.read()
        seen[name] = data
        for m in _IMPORT.finditer(data.decode("utf-8", "replace")):
            todo.append((m.group(1) or m.group(2)) + ".py")
    h = hashlib.sha256()
    for name in sorted(seen):
        h.update(name.encode() + b"\0" + sha256_bytes(seen[name]).encode() + b"\n")
    return h.hexdigest()


def install_manifest_hash(game_dir):
    """sha256 of the private repo's install/manifest.json (every install file's
    size and sha256), else of the install's Game.exe alone (then marked)."""
    repo = os.environ.get("D2_PRIVATE_REPO", "/home/user/d2rust-private-repo")
    for p in (os.path.join(repo, "install", "manifest.json"),
              os.path.join(game_dir, "manifest.json")):
        if os.path.exists(p):
            return sha256_file(p)
    return "game-exe:" + sha256_file(os.path.join(game_dir, "Game.exe"))


def recording_text(check_text):
    """The part of a check file the 1.14d recording depends on: its lines
    without comments (an `input` line keeps its `#`, as the parser does),
    blank lines and `ignore` lines (comparator only), joined by newlines."""
    out = []
    for ln in check_text.splitlines():
        ln = ln.strip() if ln.lstrip().startswith("input ") else ln.split("#", 1)[0].strip()
        if ln and ln.partition(" ")[0] != "ignore":
            out.append(ln)
    return "\n".join(out) + "\n"


def make_key(check_text, save_bytes, game_dir, rec_dir, script):
    return {"format": KEY_FORMAT,
            "check": sha256_bytes(recording_text(check_text).encode()),
            "save": sha256_bytes(save_bytes),
            "game_exe": sha256_file(os.path.join(game_dir, "Game.exe")),
            "install": install_manifest_hash(game_dir),
            "recorder": recorder_version(rec_dir, script)}


def _text_ok(path):
    """Small text only: no PNG/binary bytes, bounded size."""
    if os.path.getsize(path) > MAX_BYTES:
        return False
    with open(path, "rb") as f:
        head = f.read(1 << 16)
    return b"\x89PNG" not in head and b"\0" not in head


class OrigCache:
    def __init__(self, root, check_name, fill=False, read=True):
        self.root, self.name, self.fill, self.read = root, check_name, fill, read

    def dir(self, channel):
        return os.path.join(self.root, self.name, channel)

    def lookup(self, key, script, dest):
        """Restore the entry's file to `dest` when its key equals `key`; True on
        a hit, else False (miss: absent, other key, or a file that no longer
        matches its recorded sha256)."""
        channel, fname = RECORDERS[script]
        d = self.dir(channel)
        if not self.read:
            return False
        try:
            with open(os.path.join(d, "cache.json"), encoding="utf-8") as f:
                entry = json.load(f)
        except (OSError, ValueError):
            return False
        if entry.get("format") != ENTRY_FORMAT or entry.get("key") != key:
            return False
        meta = (entry.get("files") or {}).get(fname)
        src = os.path.join(d, fname)
        if not meta or not os.path.exists(src) or sha256_file(src) != meta.get("sha256"):
            return False
        os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
        shutil.copyfile(src, dest)
        return True

    def store(self, key, script, src, command):
        """Write the entry (key, command, file sha256) for a fresh recording."""
        channel, fname = RECORDERS[script]
        if not os.path.exists(src) or not _text_ok(src):
            return False
        d = self.dir(channel)
        os.makedirs(d, exist_ok=True)
        shutil.copyfile(src, os.path.join(d, fname))
        entry = {"format": ENTRY_FORMAT, "check": self.name, "channel": channel, "key": key,
                 "command": command,
                 "files": {fname: {"size": os.path.getsize(src), "sha256": sha256_file(src)}}}
        with open(os.path.join(d, "cache.json"), "w", encoding="utf-8") as f:
            json.dump(entry, f, indent=1, sort_keys=True)
            f.write("\n")
        return True


def migrate_v1(root, check_dirs):
    """Rewrite each `orig-cache-key-1` entry whose check hash is still the
    sha256 of its check file's text to `orig-cache-key-2` (the hash of
    `recording_text`); the recorded files are not touched. Returns
    (migrated, left) counts; a left entry is stale and misses as before."""
    paths = {}
    for d in check_dirs:
        for n in sorted(os.listdir(d)):
            if n.endswith(".check"):
                paths.setdefault(n[:-6], os.path.join(d, n))
    done = left = 0
    for name in sorted(os.listdir(root)):
        for ch in sorted(os.listdir(os.path.join(root, name))):
            p = os.path.join(root, name, ch, "cache.json")
            try:
                with open(p, encoding="utf-8") as f:
                    entry = json.load(f)
            except (OSError, ValueError):
                continue
            key = entry.get("key") or {}
            if key.get("format") != "orig-cache-key-1":
                continue
            text = None
            if name in paths:
                with open(paths[name], encoding="utf-8") as f:
                    text = f.read()
            if text is None or key.get("check") != sha256_bytes(text.encode()):
                left += 1
                continue
            entry["key"] = dict(key, format=KEY_FORMAT,
                                check=sha256_bytes(recording_text(text).encode()))
            with open(p, "w", encoding="utf-8") as f:
                json.dump(entry, f, indent=1, sort_keys=True)
                f.write("\n")
            done += 1
    return done, left


def selftest():
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        rec = os.path.join(td, "rec")
        os.makedirs(rec)
        for n, body in (("record_state.py", "import helper\n"), ("helper.py", "X = 1\n")):
            with open(os.path.join(rec, n), "w") as f:
                f.write(body)
        game = os.path.join(td, "game")
        os.makedirs(game)
        with open(os.path.join(game, "Game.exe"), "wb") as f:
            f.write(b"exe")
        os.environ["D2_PRIVATE_REPO"] = os.path.join(td, "none")
        k1 = make_key("check a", b"save", game, rec, "record_state.py")
        assert k1 == make_key("check a", b"save", game, rec, "record_state.py")
        # M08: a changed check, save, recorder (also an imported module) or exe misses
        assert k1 != make_key("check b", b"save", game, rec, "record_state.py")
        assert k1 != make_key("check a", b"save2", game, rec, "record_state.py")
        # comments, blank lines and comparator-only `ignore` lines do not change it
        assert k1 == make_key("# note\ncheck a  # x\n\nignore q seed\n", b"save", game, rec,
                              "record_state.py")
        assert recording_text("input a # b\n") == "input a # b\n"
        with open(os.path.join(rec, "helper.py"), "w") as f:
            f.write("X = 2\n")
        k2 = make_key("check a", b"save", game, rec, "record_state.py")
        assert k2["recorder"] != k1["recorder"]
        with open(os.path.join(game, "Game.exe"), "wb") as f:
            f.write(b"exe2")
        assert make_key("check a", b"save", game, rec, "record_state.py")["game_exe"] != k2["game_exe"]
        # store / lookup / perturbed key / tampered file / art refused
        out = os.path.join(td, "orig.state.jsonl")
        with open(out, "w") as f:
            f.write('{"k":"snap"}\n')
        c = OrigCache(os.path.join(td, "cache"), "demo", fill=True)
        assert not c.lookup(k2, "record_state.py", os.path.join(td, "w", "o"))
        assert c.store(k2, "record_state.py", out, ["record_state.py", "--out", "x"])
        dest = os.path.join(td, "w", "o")
        assert c.lookup(k2, "record_state.py", dest) and open(dest).read() == '{"k":"snap"}\n'
        assert not c.lookup(dict(k2, check="0" * 64), "record_state.py", dest)
        with open(os.path.join(c.dir("state"), "orig.state.jsonl"), "a") as f:
            f.write("x")
        assert not c.lookup(k2, "record_state.py", dest)
        png = os.path.join(td, "orig.frames.jsonl")
        with open(png, "wb") as f:
            f.write(b"\x89PNG\r\n")
        assert not c.store(k2, "record_frames.py", png, [])
    print("orig_cache selftest ok")
    return 0


if __name__ == "__main__":
    import sys
    if sys.argv[1:2] == ["--migrate-v1"]:
        # orig_cache.py --migrate-v1 DIR CHECKS_DIR...
        print("migrated %d, left %d (stale)" % migrate_v1(sys.argv[2], sys.argv[3:]))
        raise SystemExit(0)
    raise SystemExit(selftest())

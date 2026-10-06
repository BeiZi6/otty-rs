"""Demangle Rust legacy (_ZN…E) symbols and build the module/type tree they imply."""
from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

ESC = {
    "$C$": ",", "$SP$": "@", "$BP$": "*", "$RF$": "&", "$LT$": "<", "$GT$": ">",
    "$LP$": "(", "$RP$": ")", "$u20$": " ", "$u27$": "'", "$u5b$": "[", "$u5d$": "]",
    "$u7b$": "{", "$u7d$": "}", "$u3b$": ";", "$u2b$": "+", "$u21$": "!", "$u3d$": "=",
    "$u7e$": "~", "$u2d$": "-", "$u2c$": ",", "$u7c$": "|", "$u3a$": ":", "$u2e$": ".",
    "$u24$": "$", "$u25$": "%", "$u23$": "#", "$u40$": "@", "$u5e$": "^", "$u26$": "&",
    "$u2a$": "*", "$u2f$": "/", "$u3f$": "?", "$u5c$": "\\", "$u60$": "`", "$u22$": '"',
}
ESC_RE = re.compile("|".join(re.escape(k) for k in sorted(ESC, key=len, reverse=True)))

HASH_TAIL = re.compile(r"17h[0-9a-f]{16}E$")


def demangle(sym: str) -> tuple[list[str], str] | None:
    """Return (path_segments, final_component) or None."""
    if "_ZN" not in sym:
        return None
    body = sym.split("_ZN", 1)[1]
    if not body.endswith("E"):
        return None
    body = body[:-1]
    segs: list[str] = []
    i = 0
    while i < len(body):
        m = re.match(r"(\d+)", body[i:])
        if not m:
            break
        n = int(m.group(1))
        i += len(m.group(1))
        segs.append(body[i:i + n])
        i += n
    if not segs:
        return None
    if len(segs) > 1 and re.fullmatch(r"h[0-9a-f]{16}", segs[-1]):
        segs = segs[:-1]
    out = []
    for s in segs:
        s = ESC_RE.sub(lambda m: ESC[m.group()], s)
        s = s.replace("..", "::")
        out.append(s)
    return out[:-1], out[-1]


def main() -> None:
    src = Path(sys.argv[1])
    txt = src.read_text(encoding="utf-8", errors="replace")
    syms = set(re.findall(r"__ZN[0-9A-Za-z_$.]+", txt))

    tree: dict[str, set[str]] = defaultdict(set)      # module path -> item names
    crates: dict[str, int] = defaultdict(int)
    types: set[str] = set()
    fns: set[str] = set()
    traits: set[str] = set()

    for s in syms:
        d = demangle(s)
        if not d:
            continue
        path, last = d
        path = [p for p in path if not p.startswith("$LT$") and p not in ("core", "alloc", "std")]
        if not path:
            continue
        crates[path[0]] += 1
        mod = "::".join(path[:-1]) if len(path) > 1 else path[0]
        tree[mod].add(last)
        if "::$LT$" in last or "$LT$" in "".join(path):
            traits.add(last)
        elif last and last[0].isupper():
            types.add(last)
        else:
            fns.add(last)

    print(f"# {src.name}: {len(syms)} legacy symbols -> {len(tree)} modules\n")
    print("## crates (by symbol count)")
    for c, n in sorted(crates.items(), key=lambda x: -x[1]):
        print(f"  {c:32s} {n:6d}")

    print("\n## module tree")
    for mod in sorted(tree):
        items = sorted(tree[mod])
        print(f"\n{mod}   ({len(items)})")
        for it in items:
            print(f"    {it}")


if __name__ == "__main__":
    main()

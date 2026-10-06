"""Extract and categorise strings from a Mach-O binary to recover the app's surface area."""
from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

CATS: list[tuple[str, re.Pattern]] = [
    ("rust-marker", re.compile(r"rustc|rust_begin_unwind|core::panicking|\.cargo/registry|"
                               r"alloc::(alloc|string|vec)|std::(io|process|sync)|"
                               r"panic_abort|_ZN[0-9]+h[0-9a-f]{16}E")),
    ("swift-marker", re.compile(r"Swift|swift_|__TtC|__TtV|_TtP|\$s[0-9]+[A-Za-z]")),
    ("swift-demangle", re.compile(r"^_\$s[A-Za-z0-9_$]+$")),
    ("objc-class", re.compile(r"^_?OBJC_CLASS_\$_")),
    ("toml-key", re.compile(r"^[a-z][a-z0-9_]*(\.(font|theme|shell|terminal|window|keybind|"
                            r"agent|editor|scroll|cursor|paste|bell|confirm|notify|tab|"
                            r"appearance|advanced)[a-z0-9_]*)+$")),
    ("config-path", re.compile(r"~/\.config/otty|/otty/|config\.toml|ottytheme|ottyrecipe")),
    ("osc", re.compile(r"\bOSC\b|1337|sixel|kitty.*graphic|iterm", re.I)),
    ("vt-name", re.compile(r"^(CSI|OSC|DCS|ESC|DECSET|DECRST|DECSCUSR|SGR|XTWINOPS|DECRQSS|"
                           r"XTPUSHCOLORS|XTPOPCOLORS|FTCS|DECALN|DECSTR|DECSCA|DECKPAM|"
                           r"DECKPNM|HTS|RIS|NEL|IND|RI|DECSC|DECRC)\b")),
    ("uri", re.compile(r"^(https?|otty|ssh|file|x-man-page)://?\S*")),
    ("appkit", re.compile(r"^NS[A-Z][A-Za-z]+$")),
    ("bundle-id", re.compile(r"io\.appmakes\.otty|appmakes")),
    ("cmdline", re.compile(r"^(/usr/bin/|/bin/|/opt/homebrew/|/usr/local/)(bash|zsh|fish|sh|"
                           r"login|env|git|rg|ssh|open|osascript|defaults|pmset|caffeinate)\b")),
    ("font-name", re.compile(r"(JetBrains|OfficeCodePro|NerdFont|Styrene|Tiempos|SF Mono|"
                             r"Menlo|Monaco|Iosevka|Fira|Hack|Cascadia)", re.I)),
    ("agent", re.compile(r"\b(claude|codex|opencode|cursor|copilot|grok|kimi|gemini|antigravity|"
                         r"codebuddy|reasonix|aider|aider-chat|pi)\b", re.I)),
]

MINLEN = 6
PERCAT = 400


def main() -> None:
    src, out = Path(sys.argv[1]), Path(sys.argv[2])
    buf = src.read_bytes()
    pat = re.compile(rb"[\x20-\x7e]{%d,}" % MINLEN)
    found: dict[str, list[str]] = defaultdict(list)
    allstr = set()
    for m in pat.finditer(buf):
        s = m.group().decode("ascii", "replace")
        allstr.add(s)

    print(f"{src.name}: {len(allstr):,} unique strings")

    for cat, rx in CATS:
        hits = [s for s in allstr if rx.search(s)]
        found[cat] = sorted(hits)
        print(f"  {cat:16s} {len(hits):>7,}")

    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8") as fh:
        fh.write(f"# strings of {src.name} ({len(allstr)} unique)\n")
        for cat, _ in CATS:
            items = found[cat]
            fh.write(f"\n\n## {cat}  ({len(items)})\n")
            for s in items[:PERCAT]:
                fh.write(s + "\n")
            if len(items) > PERCAT:
                fh.write(f"... +{len(items) - PERCAT} more\n")
        fh.write("\n\n## ALL (sorted)\n")
        for s in sorted(allstr):
            fh.write(s + "\n")
    print(f"-> {out}")


if __name__ == "__main__":
    main()

"""Recover embedded text resources from the shipped Otty binary.

The Rust core compiles its resources in with `include_str!`/`include_bytes!`, so
a shipped Mach-O still carries the original text verbatim. This walks the file,
finds long runs of readable text, and writes each run out with its byte offset so
anything we reconstruct can be checked against it.
"""
import pathlib
import re
import sys

BIN = pathlib.Path(
    r"E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app\Contents\MacOS\Otty")
OUT = pathlib.Path(r"E:\study_serve\otty-re\analysis\embedded")
OUT.mkdir(parents=True, exist_ok=True)

# A run of printable text: letters/digits/punctuation/whitespace, at least 160
# bytes long, and containing at least one newline (so it is a document, not a
# symbol name or a format string).
RUN = re.compile(rb"[\t\n\x20-\x7e\xc2-\xf4][\t\n\x20-\x7e\x80-\xbf]{159,}")


def looks_like_text(b: bytes) -> bool:
    if b.count(b"\n") < 2:
        return False
    printable = sum(1 for c in b if 32 <= c < 127 or c in (9, 10, 13))
    return printable / max(len(b), 1) > 0.9


def classify(b: bytes) -> str:
    t = b.lstrip()
    if t.startswith(b"{") or t.startswith(b"["):
        return "json"
    if b"[terminal]" in b or b"[meta]" in b or b"[colors" in b:
        return "theme-toml"
    if t.startswith(b"#!") or b"#!/" in t[:16]:
        return "script"
    if b"=" in t[:200] and (b"\n[" in t[:4000] or b"palette" in t[:4000]):
        return "config"
    if t.startswith(b"#") or b"# " in t[:80]:
        return "doc"
    return "text"


def main() -> None:
    buf = BIN.read_bytes()
    print(f"{BIN.name}: {len(buf):,} bytes")
    hits = []
    for m in RUN.finditer(buf):
        run = bytes(m.group())
        if not looks_like_text(run):
            continue
        hits.append((m.start(), run))

    print(f"{len(hits)} text runs >= 160 bytes")

    index = []
    total = 0
    for i, (off, run) in enumerate(hits):
        kind = classify(run)
        name = f"{off:09d}-{kind}.txt"
        (OUT / name).write_bytes(run)
        index.append((off, len(run), kind, name))
        total += len(run)

    with (OUT / "_index.tsv").open("w", encoding="utf-8") as fh:
        fh.write("offset\tlength\tkind\tfile\n")
        for row in index:
            fh.write("\t".join(str(x) for x in row) + "\n")

    print(f"{total:,} bytes of embedded text -> {OUT}")
    from collections import Counter
    print(Counter(k for _, _, k, _ in index))
    print()
    for off, ln, kind, name in index:
        if ln > 2000:
            head = (OUT / name).read_bytes()[:110].decode("utf-8", "replace")
            head = head.replace("\n", " / ")
            print(f"  {off:>10}  {ln:>8}  {kind:<10} {head}")


if __name__ == "__main__":
    main()

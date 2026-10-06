"""Strip the VitePress navigation from the crawled docs, keeping every content
line — including the bullet lists, which an earlier pass destroyed."""
import pathlib
import re

SRC = pathlib.Path(r"E:\study_serve\tmp\otty\pages")
DST = pathlib.Path(r"E:\study_serve\tmp\otty\clean2")
DST.mkdir(parents=True, exist_ok=True)

FOOTER = re.compile(r"\nPagerPrevious.*\Z", re.S)


def trim(text: str) -> str:
    lines = text.split("\n")
    start = 0
    for i, l in enumerate(lines):
        if l.strip() == "On this page":
            start = i + 1
            break
    # Advance to the page's first `# ` heading: everything before it is nav.
    for i in range(start, len(lines)):
        if lines[i].startswith("# "):
            start = i
            break
    body = "\n".join(lines[start:])
    body = FOOTER.sub("", body)
    body = re.sub(r"\n{4,}", "\n\n\n", body)
    return body.strip() + "\n"


kept = 0
for f in sorted(SRC.glob("*.txt")):
    t = f.read_text(encoding="utf-8", errors="replace")
    body = trim(t)
    (DST / f.name).write_text(body, encoding="utf-8")
    kept += 1

print(f"{kept} pages -> {DST}")
bullets = sum(
    sum(1 for l in (DST / f.name).read_text(encoding="utf-8").splitlines() if l.startswith("- "))
    for f in DST.glob("*.txt")
)
print(f"{bullets} content bullet lines preserved")

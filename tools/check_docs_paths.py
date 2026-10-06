"""Check that every repository path the docs name actually exists.

The documentation is a deliverable, and the one failure mode that makes it worse
than useless is naming a file that is not there. This walks every backticked
path in README.md and docs/*.md and reports the ones that do not resolve.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(r"E:\study_serve\otty-re\otty")

DOCS = [ROOT / "README.md"] + sorted((ROOT / "docs").glob("*.md"))
PATH_RE = re.compile(
    r"`([A-Za-z0-9_./-]+\.(?:rs|toml|md|json|sh|ps1|ottytheme|db|sdef|plist|yml|toml))`"
)


def main() -> int:
    refs: dict[str, list[str]] = {}
    for doc in DOCS:
        text = doc.read_text(encoding="utf-8", errors="replace")
        for m in PATH_RE.finditer(text):
            refs.setdefault(m.group(1), []).append(doc.name)

    missing = []
    for rel, where in sorted(refs.items()):
        if rel.startswith(("~", "http", "./")):
            rel_clean = rel[2:] if rel.startswith("./") else rel
            if rel.startswith(("~", "http")):
                continue
        else:
            rel_clean = rel
        candidates = [
            ROOT / rel_clean,
            ROOT / "packages" / rel_clean,
            ROOT / "resources" / rel_clean,
            ROOT / "docs" / rel_clean,
            ROOT / "scripts" / rel_clean,
        ]
        if not any(c.exists() for c in candidates):
            missing.append((rel, where))

    print(f"docs checked: {len(DOCS)}")
    print(f"repository paths referenced: {len(refs)}")
    print(f"unresolvable: {len(missing)}")
    for rel, where in missing:
        print(f"  {rel}   (named in {', '.join(sorted(set(where)))})")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())

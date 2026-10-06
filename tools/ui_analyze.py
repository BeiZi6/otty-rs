"""Extract the design system (fonts, colors, tokens) embedded in Otty's settings UI bundle."""
import json
import re
import sys
from pathlib import Path

src = Path(sys.argv[1])
s = src.read_text(encoding="utf-8", errors="replace")
print(f"# {src.name}: {len(s):,} chars")

print("\n## @font-face blocks")
for m in re.findall(r"@font-face\s*\{[^}]{0,600}?\}", s):
    print(m[:400].replace("\n", " "))
    print("---")

print("\n## font-family declarations")
for x in sorted(set(re.findall(r"font-family\s*:\s*([^;}\"']{0,140})", s))):
    print(repr(x))

print("\n## font shorthand")
for x in sorted(set(re.findall(r"font\s*:\s*([^;}\"']{0,120})", s)))[:40]:
    print(repr(x))

print("\n## font file references")
print(sorted(set(re.findall(r"[\w\-.]+\.(?:woff2?|ttf|otf)", s))))

print("\n## embedded data: URIs by mime")
from collections import Counter
print(Counter(re.findall(r"data:([a-z0-9/+.\-]+)[;,]", s)))

print("\n## CSS custom properties (design tokens)")
props = sorted(set(re.findall(r"(--[a-zA-Z0-9\-_]+)\s*:", s)))
print(f"{len(props)} tokens")
for p in props:
    print(p)

print("\n## script/link tags")
for m in re.findall(r"<(?:script|link)[^>]{0,200}>", s)[:30]:
    print(m)

print("\n## title / meta")
for m in re.findall(r"<title[^>]*>(.*?)</title>", s):
    print(m)

print("\n## top-level JS globals / mount points")
for m in re.findall(r"getElementById\(\s*[\"']([^\"']+)", s)[:20]:
    print(m)

print("\n## look for i18n / settings schema keys")
for pat in [r"settingsSchemas?", r"SETTINGS_SPEC", r"configKeys", r"fontFamily"]:
    hits = re.findall(pat, s)
    print(f"{pat}: {len(hits)}")

# Try to find an embedded JSON blob describing settings (the schema)
for m in re.finditer(r"\bid:\s*\\?[\"']([a-z0-9_]{3,60})\\?[\"']", s):
    pass
ids = re.findall(r'\\?"id\\?":\\?"([a-z0-9_.\-]{3,60})\\?"', s)
print(f"\n## json ids found: {len(set(ids))}")
print(json.dumps(sorted(set(ids))[:200], indent=0)[:4000])

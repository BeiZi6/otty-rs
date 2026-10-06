"""One-shot docs.otty.sh recrawl for GAP-K01. Not part of the product."""

from __future__ import annotations

import html as html_lib
import json
import re
import sys
import time
import urllib.error
import urllib.request
from html.parser import HTMLParser
from pathlib import Path
from xml.etree import ElementTree as ET

UA = "Otty-corpus-restore/1.0 (GAP-K01; +https://docs.otty.sh)"
SITEMAP = "https://docs.otty.sh/sitemap.xml"
ROOT = Path(__file__).resolve().parent
OUT = ROOT / "corpus"


class VPDoc(HTMLParser):
    """Pull VitePress main doc and turn it into markdown-ish text."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.in_doc = False
        self.depth = 0
        self.skip = 0
        self.parts: list[str] = []
        self._tag: list[str] = []
        self._li_level = 0
        self._in_pre = False
        self._in_table = False
        self._cell_buf: list[str] = []
        self._row: list[str] = []
        self._table_rows: list[list[str]] = []
        self._href: str | None = None
        self._th = False

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        ad = dict(attrs)
        cls = ad.get("class") or ""
        if tag == "script" or tag == "style" or tag == "nav" or tag == "aside":
            self.skip += 1
            return
        if not self.in_doc:
            if "vp-doc" in cls.split() or ad.get("id") == "VPContent":
                self.in_doc = True
                self.depth = 1
            return
        self.depth += 1
        if self.skip:
            return
        if tag in {"h1", "h2", "h3", "h4", "h5", "h6"}:
            self.parts.append("\n" + "#" * int(tag[1]) + " ")
        elif tag == "p":
            self.parts.append("\n")
        elif tag == "br":
            self.parts.append("\n")
        elif tag == "li":
            self.parts.append("\n" + "  " * max(self._li_level - 1, 0) + "- ")
        elif tag == "ul" or tag == "ol":
            self._li_level += 1
            self.parts.append("\n")
        elif tag == "pre":
            self._in_pre = True
            self.parts.append("\n```\n")
        elif tag == "code" and not self._in_pre:
            self.parts.append("`")
        elif tag == "strong" or tag == "b":
            self.parts.append("**")
        elif tag == "em" or tag == "i":
            self.parts.append("*")
        elif tag == "a":
            self._href = ad.get("href")
            self.parts.append("[")
        elif tag == "table":
            self._in_table = True
            self._table_rows = []
        elif tag == "tr" and self._in_table:
            self._row = []
        elif tag in {"td", "th"} and self._in_table:
            self._th = tag == "th"
            self._cell_buf = []
        elif tag == "blockquote":
            self.parts.append("\n> ")
        elif tag == "hr":
            self.parts.append("\n---\n")
        self._tag.append(tag)

    def handle_endtag(self, tag: str) -> None:
        if tag in {"script", "style", "nav", "aside"}:
            if self.skip:
                self.skip -= 1
            return
        if not self.in_doc:
            return
        self.depth -= 1
        if self.depth <= 0:
            self.in_doc = False
        if self.skip:
            return
        if self._tag and self._tag[-1] == tag:
            self._tag.pop()
        if tag in {"h1", "h2", "h3", "h4", "h5", "h6", "p"}:
            self.parts.append("\n")
        elif tag == "ul" or tag == "ol":
            self._li_level = max(self._li_level - 1, 0)
            self.parts.append("\n")
        elif tag == "pre":
            self._in_pre = False
            self.parts.append("\n```\n")
        elif tag == "code" and not self._in_pre:
            self.parts.append("`")
        elif tag == "strong" or tag == "b":
            self.parts.append("**")
        elif tag == "em" or tag == "i":
            self.parts.append("*")
        elif tag == "a":
            if self._href:
                self.parts.append(f"]({self._href})")
            else:
                self.parts.append("]")
            self._href = None
        elif tag in {"td", "th"} and self._in_table:
            cell = "".join(self._cell_buf).replace("|", "\\|").replace("\n", " ").strip()
            self._row.append(cell)
            self._cell_buf = []
        elif tag == "tr" and self._in_table:
            if self._row:
                self._table_rows.append(self._row)
            self._row = []
        elif tag == "table" and self._in_table:
            self._in_table = False
            self.parts.append(self._fmt_table(self._table_rows))
            self._table_rows = []

    def handle_data(self, data: str) -> None:
        if not self.in_doc or self.skip:
            return
        if self._in_table and self._cell_buf is not None and (
            self._tag and self._tag[-1] in {"td", "th", "code", "a", "strong", "em", "span", "p"}
            or True
        ):
            if self._tag and self._tag[-1] in {"td", "th"} or (
                self._in_table and any(t in {"td", "th"} for t in self._tag)
            ):
                if any(t in {"td", "th"} for t in self._tag):
                    self._cell_buf.append(data)
                    return
        if self._in_pre:
            self.parts.append(data)
            return
        text = data
        if not text:
            return
        self.parts.append(text)

    def _fmt_table(self, rows: list[list[str]]) -> str:
        if not rows:
            return "\n"
        width = max(len(r) for r in rows)
        norm = [r + [""] * (width - len(r)) for r in rows]
        lines = ["", "| " + " | ".join(norm[0]) + " |", "| " + " | ".join("---" for _ in range(width)) + " |"]
        for r in norm[1:]:
            lines.append("| " + " | ".join(r) + " |")
        lines.append("")
        return "\n".join(lines)

    def text(self) -> str:
        raw = "".join(self.parts)
        raw = html_lib.unescape(raw)
        raw = re.sub(r"\n{3,}", "\n\n", raw)
        return raw.strip() + "\n"


def fetch(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "text/html"})
    last: Exception | None = None
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                return resp.read().decode("utf-8", errors="replace")
        except (urllib.error.URLError, TimeoutError, ConnectionError, OSError) as e:
            last = e
            time.sleep(0.8 * (attempt + 1))
    raise last  # type: ignore[misc]


def to_md(page_html: str) -> str:
    p = VPDoc()
    p.feed(page_html)
    t = p.text()
    if len(t) < 80:
        # fallback: strip tags from body
        body = re.sub(r"(?is)<script.*?>.*?</script>", " ", page_html)
        body = re.sub(r"(?is)<style.*?>.*?</style>", " ", body)
        body = re.sub(r"(?is)<[^>]+>", " ", body)
        t = html_lib.unescape(re.sub(r"[ \t]+", " ", body))
        t = re.sub(r"\n{3,}", "\n\n", t).strip() + "\n"
    return t


def sitemap_urls() -> list[str]:
    xml = fetch(SITEMAP)
    root = ET.fromstring(xml)
    ns = {"sm": "http://www.sitemaps.org/schemas/sitemap/0.9"}
    locs = [el.text.strip() for el in root.findall(".//sm:loc", ns) if el.text]
    if not locs:
        locs = [el.text.strip() for el in root.findall(".//{http://www.sitemaps.org/schemas/sitemap/0.9}loc") if el.text]
    return locs


def dest_for(url: str) -> Path:
    path = url.split("docs.otty.sh/", 1)[-1].strip("/") or "index"
    if "/vt/" in url:
        return OUT / "vt" / (path.replace("vt/", "").replace("/", "__") + ".md")
    if path.endswith("skills"):
        return OUT / "skills.md"
    return OUT / "import-export.md"


def tidy(md: str) -> str:
    md = re.sub(r" \[​\]\(#[^)]+\)", "", md)
    md = re.sub(r"[*`[\]]{4,}", "", md)
    md = re.sub(r"\n{3,}", "\n\n", md)
    return md.strip() + "\n"


def main() -> int:
    urls = sitemap_urls()
    wanted = [
        u
        for u in urls
        if "/vt/" in u
        or u.rstrip("/").endswith("/agents/skills")
        or u.rstrip("/").endswith("/customization/import-export")
    ]
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "vt").mkdir(exist_ok=True)
    manifest: list[dict] = []
    failed: list[str] = []
    for i, url in enumerate(wanted):
        dest = dest_for(url)
        dest.parent.mkdir(parents=True, exist_ok=True)
        if dest.exists() and dest.stat().st_size > 200:
            print(f"[{i+1}/{len(wanted)}] skip  {url}")
            manifest.append(
                {
                    "url": url,
                    "file": str(dest.relative_to(ROOT)).replace("\\", "/"),
                    "bytes": dest.stat().st_size,
                }
            )
            continue
        try:
            page = fetch(url)
        except Exception as e:
            failed.append(f"{url}: {e}")
            print(f"[{i+1}/{len(wanted)}] FAIL  {url}: {e}")
            time.sleep(0.6)
            continue
        md = tidy(to_md(page))
        dest.write_text(f"<!-- source: {url} -->\n\n{md}", encoding="utf-8")
        manifest.append(
            {
                "url": url,
                "file": str(dest.relative_to(ROOT)).replace("\\", "/"),
                "bytes": dest.stat().st_size,
                "chars": len(md),
            }
        )
        print(f"[{i+1}/{len(wanted)}] {len(md):5d}c  {url}")
        time.sleep(0.2)
    (OUT / "manifest.json").write_text(
        json.dumps({"sitemap_total": len(urls), "fetched": manifest, "failed": failed}, indent=2),
        encoding="utf-8",
    )
    print(f"done fetched={len(manifest)} failed={len(failed)} sitemap={len(urls)}")
    if failed:
        print("FAILED:", *failed, sep="\n  ")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

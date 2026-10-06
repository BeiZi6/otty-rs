"""Bring the keymap and the key tests onto the shipped key map's canonical spelling."""
import pathlib

REPL = [
    # keybind defaults
    ('"cmd+shift+["', '"cmd+shift+left_bracket"'),
    ('"cmd+shift+]"', '"cmd+shift+right_bracket"'),
    ('"cmd+,"', '"cmd+comma"'),
    ('"cmd+alt+d"', '"cmd+opt+d"'),
    ('"cmd+alt+shift+d"', '"cmd+opt+shift+d"'),
    ('"cmd+ctrl+="', '"cmd+ctrl+equal"'),
    ('"cmd+ctrl+arrow-up"', '"cmd+ctrl+up"'),
    ('"cmd+ctrl+arrow-down"', '"cmd+ctrl+down"'),
    ('"cmd+ctrl+arrow-left"', '"cmd+ctrl+left"'),
    ('"cmd+ctrl+arrow-right"', '"cmd+ctrl+right"'),
    ('"cmd+ctrl+shift+arrow-up"', '"cmd+ctrl+shift+up"'),
    ('"cmd+ctrl+shift+arrow-down"', '"cmd+ctrl+shift+down"'),
    ('"cmd+ctrl+shift+arrow-left"', '"cmd+ctrl+shift+left"'),
    ('"cmd+ctrl+shift+arrow-right"', '"cmd+ctrl+shift+right"'),
    ('"cmd+page-up"', '"cmd+page_up"'),
    ('"cmd+page-down"', '"cmd+page_down"'),
    ('"cmd+alt+arrow-up"', '"cmd+opt+up"'),
    ('"cmd+alt+arrow-down"', '"cmd+opt+down"'),
    ('"cmd+alt+page-up"', '"cmd+opt+page_up"'),
    ('"cmd+alt+page-down"', '"cmd+opt+page_down"'),
    ('"cmd+arrow-left"', '"cmd+left"'),
    ('"cmd+arrow-right"', '"cmd+right"'),
    ('"alt+arrow-left"', '"opt+left"'),
    ('"alt+arrow-right"', '"opt+right"'),
    ('"alt+backspace"', '"opt+backspace"'),
    ('"alt+delete"', '"opt+forward_delete"'),
    ('"cmd+delete"', '"cmd+forward_delete"'),
    ('"cmd+="', '"cmd+equal"'),
    ('"cmd+-"', '"cmd+minus"'),
]

ROOT = pathlib.Path(r"E:\study_serve\otty-re\otty\packages")
files = [
    ROOT / "config/src/keybind.rs",
    ROOT / "core/src/key.rs",
]

for f in files:
    text = f.read_text(encoding="utf-8")
    before = text
    for a, b in REPL:
        text = text.replace(a, b)
    # key.rs parity expectations
    text = text.replace('("option+escape", "opt+escape")', '("option+e", "opt+e")')
    text = text.replace('("opt+e", "opt+e")', '("option+e", "opt+e")')
    text = text.replace('("pgup", "page-up")', '("pgup", "page_up")')
    text = text.replace('("opt+escape", "alt+escape")', '("option+escape", "opt+escape")')
    text = text.replace('("super+t", "cmd+t")', '("super+t", "cmd+t")')
    if text != before:
        f.write_text(text, encoding="utf-8")
        print(f"patched {f.relative_to(ROOT.parent)}")
    else:
        print(f"unchanged {f.relative_to(ROOT.parent)}")

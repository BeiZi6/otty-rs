"""Extract the authoritative Otty config key list from the generated vim syntax file."""
import re
from pathlib import Path

VIM = Path(r"E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app\Contents\Resources\editors\vim\syntax\otty.vim")
OUT = Path(r"E:\study_serve\otty-re\analysis\config_keys.txt")

text = VIM.read_text(encoding="utf-8")
line = next(l for l in text.splitlines() if "ottyKnownKey" in l and "syn match" in l)
body = line.split(r"\%(")[1].rsplit(r"\)\ze", 1)[0]
keys = sorted(set(body.split(r"\|")))
assert len(keys) > 150, len(keys)

OUT.write_text("\n".join(keys) + "\n", encoding="utf-8")
print(f"{len(keys)} config keys -> {OUT}\n")
for i in range(0, len(keys), 4):
    print("".join(f"{k:<46}" for k in keys[i:i + 4]).rstrip())

# enum values
enum_line = next(l for l in text.splitlines() if "ottyEnum" in l and "syn match" in l)
ebody = enum_line.split(r"\%(")[1].rsplit(r"\)\>", 1)[0]
enums = sorted(set(ebody.split(r"\|")))
Path(r"E:\study_serve\otty-re\analysis\enum_values.txt").write_text(
    "\n".join(enums) + "\n", encoding="utf-8")
print(f"\n{len(enums)} enum literals")
print(" ".join(enums))

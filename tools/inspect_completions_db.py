"""Inspect the shipped Fig-compatible completions database so the reader can be
modelled on the real schema rather than a guess."""
import sqlite3
import sys

DB = r"E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app\Contents\Resources\completions-client.db"

con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
tables = [r[0] for r in con.execute("SELECT name FROM sqlite_master WHERE type='table'")]
print("tables:", tables)
for name in tables:
    sql = con.execute(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?", (name,)
    ).fetchone()[0]
    count = con.execute(f'SELECT COUNT(*) FROM "{name}"').fetchone()[0]
    print(f"\n--- {name}  ({count} rows)")
    print(sql)
    cols = [r[1] for r in con.execute(f'PRAGMA table_info("{name}")')]
    for row in con.execute(f'SELECT * FROM "{name}" LIMIT 2'):
        print("   ", dict(zip(cols, [str(v)[:120] for v in row])))

idx = [r[0] for r in con.execute("SELECT name FROM sqlite_master WHERE type='index'")]
print("\nindexes:", idx)
sys.exit(0)

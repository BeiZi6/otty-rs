"""Clean up the dead indirection in raster.rs before it compiles into the build."""
import pathlib
import re
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\raster.rs")
t = F.read_text(encoding="utf-8")
before = t

# 1. `height_to_rows` returned u16::MAX, which is not a row count. The frame's
#    own row count is authoritative.
t = t.replace("        let rows = frame.rows.min(self.height_to_rows());", "        let rows = frame.rows;")
t = re.sub(
    r"    fn height_to_rows\(&self\) -> u16 \{.*?\n    \}\n\n",
    "",
    t,
    flags=re.S,
)

# 2. The "rows the frame no longer covers" block was unreachable — a row count
#    change forces `full`, and the resize truncates the table anyway.
t = re.sub(
    r"        // Rows the frame no longer covers are cleared once, then left alone\.\n.*?\n        \}\n\n",
    "",
    t,
    flags=re.S,
)

# 3. `clear_row` is now unused.
t = re.sub(r"    fn clear_row\(&mut self, row: u32, background: Rgba\) \{.*?\n    \}\n\n", "", t, flags=re.S)

# 4. The `RunColors` trait was indirection around public fields.
t = re.sub(
    r"/// A run's two resolved colours, as the frame carries them\.\ntrait RunColors \{.*?\n\}\n\nimpl RunColors for Run \{.*?\n\}\n\n",
    "",
    t,
    flags=re.S,
)
t = t.replace("let bg = effective_bg(&run.style_colors(), frame.palette);", "let bg = run.style.bg;")
t = t.replace("let fg = effective_fg(&run.style_colors(), frame.palette);", "let fg = run.style.fg;")
t = re.sub(
    r"/// The foreground for a run, with `INVERSE` already applied by the frame\.\nfn effective_fg\(colors: &\(Rgba, Rgba\), _palette: &Palette\) -> Rgba \{\n    colors\.0\n\}\n\nfn effective_bg\(colors: &\(Rgba, Rgba\), _palette: &Palette\) -> Rgba \{\n    colors\.1\n\}\n\n",
    "",
    t,
)

# 5. `hash_line` had two leftover no-op statements.
t = t.replace(
    """    let _ = row;
    if let Some(line) = frame.lines.first() {
        let _ = line;
    }
    eat(row as u8);""",
    "    eat(row as u8);",
)

# 6. `paint_cursor` captured an unused previous hash.
t = t.replace(
    """            let was = self.row_hashes.get(frame.cursor.row as usize).copied();
            self.paint_cursor(frame);
            if !dirty.contains(&(frame.cursor.row as u32)) {
                dirty.push(frame.cursor.row as u32);
            }
            let _ = was;""",
    """            self.paint_cursor(frame);
            if !dirty.contains(&(frame.cursor.row as u32)) {
                dirty.push(frame.cursor.row as u32);
            }""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("cleaned raster.rs")

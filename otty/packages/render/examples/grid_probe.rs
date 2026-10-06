//! The grid pipeline, printed as text, with no window and no PTY.
//!
//! `otty-ui`'s tests assert that *something* reached the pixels; this says *what*
//! the pixels are, which is the question that comes up when the text looks wrong.
//! It walks the same path the shell does — theme → font stack → face → cell
//! metrics → glyph bitmap → composited row — and prints each step, so a
//! disagreement between two of them can be seen instead of inferred.
//!
//! Usage:
//!   cargo run -p otty-render --example grid_probe -- [width height] [text...]
//!
//! The default size is the pane a 1000×640 window leaves after its 220 px sidebar
//! and 38 px titlebar, which is what `OTTY_DUMP_FRAME` writes on this machine.

use otty_render::grid::{encode_line, Cursor, GridFrame, Palette};
use otty_render::raster::{GlyphBitmap, GlyphSource, GridRasterizer};
use otty_render::swash_source::{bundled_database, Face, SwashGlyphs};
use otty_term::{Parser, TermConfig, Terminal};

const DEFAULT_TEXT: &str = "Microsoft Windows [Version 10.0.22631.6060]\r\n\
                            (c) Microsoft Corporation. All rights reserved.\r\n\
                            \r\n\
                            C:\\Users\\hitxy>";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (width, height, text) = match args.len() {
        0 => (780u32, 602u32, DEFAULT_TEXT.to_owned()),
        _ if args.len() >= 2 => (
            args[0].parse().unwrap_or(780),
            args[1].parse().unwrap_or(602),
            if args.len() > 2 { args[2..].join(" ") } else { DEFAULT_TEXT.to_owned() },
        ),
        _ => (780, 602, args[0].clone()),
    };

    let config = otty_config::Config::load();
    let theme = config.active_theme(false);
    let size_cfg = config.get_float("font-size").unwrap_or(13.0) as f32;
    let scale: f32 = std::env::var("OTTY_PROBE_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let font_px = size_cfg * scale;

    println!("config font-size {size_cfg}  scale {scale}  →  glyph size {font_px}");
    println!("theme {}", theme.slug);

    let stack = otty_fonts::TypeStack::from_theme(&theme);
    for role in otty_fonts::FontRole::ALL {
        println!("  stack {role:?}: {:?}", stack.family_names(role));
    }

    let catalog = otty_fonts::FontCatalog::discover_default();
    let resolved = catalog.resolve_role(&stack, otty_fonts::FontRole::TerminalGrid);
    println!("terminal-grid chain {:?}", resolved.report.requested);
    println!(
        "  answered by {:?}  substituted {}  missing {:?}",
        resolved.report.answered_by, resolved.report.substituted, resolved.report.missing
    );
    for line in &resolved.report.lines {
        println!("  report: {line}");
    }

    let db = bundled_database(false);
    println!("bundled faces:");
    for info in db.faces() {
        let family = info.families.first().map(|(n, _)| n.clone()).unwrap_or_default();
        let face = Face::load(&db, info.id, font_px);
        match face {
            Some(f) => {
                let cell = f.cell_size(font_px);
                println!(
                    "  {family:<28} mono {:<5} advance {:.3} cell {:.3}x{:.3} baseline {:.3}",
                    info.monospaced, f.advance_of('M', font_px), cell.width, cell.height, cell.baseline
                );
            }
            None => println!("  {family:<28} (swash could not load it)"),
        }
    }

    let (face, origin) = match resolved.face() {
        Some(id) => match Face::load(&catalog.database(), id, font_px) {
            Some(f) => {
                let info = catalog.database().face(id);
                let family = info
                    .and_then(|i| i.families.first().map(|(n, _)| n.clone()))
                    .unwrap_or_default();
                let post = info.map(|i| i.post_script_name.clone()).unwrap_or_default();
                // Which of the three sources answered matters as much as which
                // family: "JetBrains Mono" is both a name the machine may have
                // installed and a face this build ships.
                let source = catalog
                    .entry(id)
                    .map(|e| e.source.as_str())
                    .unwrap_or("unknown");
                println!("face from the catalog: {family} ({post}) [{source}], id {id:?}");
                (f, format!("{family} ({source})"))
            }
            None => (fallback(&db, font_px), "bundled".to_owned()),
        },
        None => (fallback(&db, font_px), "bundled".to_owned()),
    };

    let cell = face.cell_size(font_px);
    let cols = (width as f32 / cell.width).floor().max(1.0) as u16;
    let rows = (height as f32 / cell.height).floor().max(1.0) as u16;
    println!(
        "using {origin}: cell {:.3}x{:.3} baseline {:.3} → {cols}x{rows} in {width}x{height} \
         (grid {:.0}x{:.0} px, remainder {:.0}x{:.0} px of pane background)",
        cell.width,
        cell.height,
        cell.baseline,
        cols as f32 * cell.width,
        rows as f32 * cell.height,
        width as f32 - cols as f32 * cell.width,
        height as f32 - rows as f32 * cell.height,
    );

    let glyphs = SwashGlyphs::new(face, font_px);
    for ch in ['M', 'C', 'i', 'y', 'g', '|'] {
        let bitmap = glyphs.glyph(ch, font_px);
        println!(
            "glyph {ch:?}: {}x{} left {} top {}",
            bitmap.width, bitmap.height, bitmap.left, bitmap.top
        );
        for line in ascii(&bitmap) {
            println!("    {line}");
        }
    }

    let palette = Palette::from_theme(&theme);
    let mut term = Terminal::new(cols, rows, TermConfig::default());
    let mut parser = Parser::new();
    parser.advance(&mut term, text.as_bytes());
    let frame = GridFrame {
        cols: term.cols(),
        rows: term.rows(),
        cell,
        cursor: Cursor { col: term.cursor.col, row: term.cursor.row, visible: false, ..Cursor::default() },
        lines: term.grid().rows().map(|r| encode_line(r, &palette)).collect(),
        palette,
        revision: 0,
    };

    let mut raster = GridRasterizer::new(width, height);
    let out = raster.rasterize(&frame, &glyphs);
    println!("composited {width}x{height}, dirty rows {:?}", &out.dirty_rows[..out.dirty_rows.len().min(8)]);

    // `OTTY_PROBE_PNG=<path>` writes the composited pixels out. The ASCII below
    // says where the ink is; this says whether it reads as text.
    if let Ok(path) = std::env::var("OTTY_PROBE_PNG") {
        match image::save_buffer(&path, raster.pixels(), width, height, image::ColorType::Rgba8) {
            Ok(()) => println!("wrote {path} ({width}x{height})"),
            Err(e) => println!("could not write {path}: {e}"),
        }
    }

    // The same window the dump showed: the first four rows, one pixel per glyph.
    let rows_shown = 5.min(rows as u32);
    for row in 0..rows_shown {
        let y0 = (row * height / rows as u32) as u32;
        println!("--- row {row} (y {y0}..) ---");
        for y in y0..(y0 + height / rows as u32).min(height) {
            let line: String = (0..300.min(width))
                .map(|x| if raster.pixel(x, y) != frame.palette.background { '#' } else { '.' })
                .collect();
            println!("  {y:4} {line}");
        }
    }
}

fn fallback(db: &fontdb::Database, size_px: f32) -> Face {
    let id = db.faces().next().map(|f| f.id).expect("the bundle ships faces");
    Face::load(db, id, size_px).expect("a bundled face must load into swash")
}

fn ascii(bitmap: &GlyphBitmap) -> Vec<String> {
    (0..bitmap.height)
        .map(|y| {
            (0..bitmap.width)
                .map(|x| match bitmap.at(x, y) {
                    0 => '.',
                    c if c < 128 => ':',
                    _ => '#',
                })
                .collect()
        })
        .collect()
}

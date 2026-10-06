# Typography

The typeface decision, as set explicitly by whoever owns the product: **the application's UI chrome
uses Anthropic's brand stack — Styrene B / Styrene A**; the **editorial serif is Tiempos Text**; the
**terminal grid keeps JetBrains Mono**, which is what Otty 1.5.4 already defaults to. This document
records what each of those three statements is anchored to, where the stack is declared in code, and
what the repository ships versus what it only names.

## The decision

| Surface | Family | Where it is declared | Status |
|---|---|---|---|
| UI chrome — titlebar, tab strip, sidebar, Details panel, menu bar, dialogs, Settings | `Styrene B` → `Styrene A` | `[token].font-ui` in **all 25** files under `resources/themes/`, and the same stack in `packages/config/src/theme.rs`'s `Token::font_ui` | **Declared.** No resolver exists yet — no package loads or renders a font (`otty-fonts` and `otty-render` are 1-line stubs). |
| Editorial / long-form — prose in the file pane and the settings prose blocks | `Tiempos Text` | Not yet declared anywhere. This is the one part of the decision with **no repository location**; see *Tiempos*, below. | **Stated intent only.** |
| Terminal grid | `JetBrains Mono` | The product default (`font-family`, documented default `JetBrains Mono`), and `[token].font-mono` where a theme overrides it | **Recovered** — this is Otty 1.5.4's own default, not a change. |
| Terminal fallback glyphs | `Symbols Nerd Font Mono`, plus the platform's system mono | The shipped `resources/fonts/` set — see the table under *Fallback chain* | **Recovered, and shipped here.** The seven font files are present under `resources/fonts/` and are byte-identical to the bundle (SHA-256). |

## Why Styrene, and why it is a substitution rather than a recovery

The shipped themes did declare `[token].font-ui`. Reading those declarations straight back out of the
shipped binary (`docs/RESTORATION.md` §7 gives the offsets and the method) shows **11 distinct
declarations per bundle**, and not one of them names a brand typeface:

| Declared by the shipped build | Note |
|---|---|
| `["Segoe UI Variable Text", "Segoe UI", "sans-serif"]` | One theme (the Windows-acrylic one). |
| `["-apple-system", "BlinkMacSystemFont", "Segoe UI", "sans-serif"]` | Three themes. |
| `["-apple-system", "BlinkMacSystemFont", "SF Pro", "sans-serif"]` | Four themes. |
| `["Inter", "-apple-system", "sans-serif"]` | Three themes, including the light default. |

And the shipped Settings window, analysed as a 1 498 025-byte Vite bundle
(`docs/RESTORATION.md` §10), contains **zero** occurrences of `Styrene`, `Tiempos` or `Anthropic`, and
**zero** `@font-face` blocks. Its own chrome is set in the host stack — the dominant declarations are
`600 11px -apple-system, system-ui, sans-serif` and `600 11px -apple-system,BlinkMacSystemFont,system-ui,sans-serif`,
with `font-family: -apple-system,BlinkMacSystemFont,Inter,Segoe UI,Roboto,sans-serif` as the primary
family and Inter reachable only as one entry inside it.

So the position is exact, and worth stating plainly rather than glossing:

> The shipped product's own UI chrome is set in the operating system's UI stack, not in a brand
> typeface. **Replacing `[token].font-ui` is therefore a deliberate deviation from the recovered
> bytes, not a recovery of them** — and it is the only substantive deviation in the entire theme set.

Each theme file carries a provenance banner naming which of the three edits was made, because the
shipped files differ in how they spell the key:

| Banner | Meaning | Files |
|---|---|---|
| `(replaced here)` | The shipped theme had a `[token].font-ui` with the platform stack; the value was substituted. | `acrylic-dark`, `april`, `april-dark`, `floating-card`, `glass-dark`, `glass-light`, `newsprint`, `night`, `nord`, `paper` |
| `(inserted here)` | The shipped theme's `[token]` section had no `font-ui` at all; the line was added. | `ayu-dark`, `ayu-light`, `pink` |
| `(appended here)` | The key was added at the end of an existing `[token]` block. | `catppuccin-mocha`, `dracula`, `gruvbox-dark`, `monokai-classic`, `one-dark`, `one-light`, `owl`, `rose-pine`, `seafoam-pastel`, `solarized-dark`, `solarized-light`, `tokyo-night` |

The banners themselves are **not** recovered text: the strings `Sole deliberate deviation`, `replaced
here`, `inserted here` and `appended here` return zero hits in either shipped binary, which confirms
they are added by this repository. Everything else in those 25 files — comments, palette, materials,
radii, per-surface colours — matches the binary byte for byte.

## Where the stack is declared in code

Two places, and they must stay in step:

**1. Every theme file.** Each of the 25 files under `resources/themes/` carries the identical line at
the top of its `[token]` section:

```toml
font-ui    = ["Styrene B", "Styrene A"]
```

**2. `packages/config/src/theme.rs`.** The `Token` struct holds it as an ordered fallback stack, not
a single name:

```rust
/// The `[token]` section: typography and shape.
pub struct Token {
    pub accent: Option<Color>,
    /// Terminal grid font, as a family name or an ordered fallback stack.
    pub font_mono: Option<Vec<String>>,
    /// Window-chrome font.
    pub font_ui: Option<Vec<String>>,
    pub font_size: Option<f32>,
    /// Ghostty-compatible line-height delta: `Npx`, `N`, `N%`, or `0`.
    pub adjust_cell_height: Option<String>,
}
```

It is read by `get_font_stack(table, "font-ui")` while the typed `Theme` is built, so a theme with no
`font-ui` yields `None` rather than an error — the key is optional everywhere, and the rebuild's
`Token` models only the five keys above (`docs/RESTORATION.md`, *Limits*, records that the shipped
`[token]` section is a superset).

`package otty-config` asserts the invariant, so a theme that drifts out of line fails the build:

```rust
// packages/config/src/theme.rs, in
// theme::tests::every_builtin_theme_parses_and_has_16_colours
// The UI typeface is the Anthropic stack, in every shipped theme.
assert_eq!(
    t.token.font_ui.as_deref(),
    Some(&["Styrene B".to_owned(), "Styrene A".to_owned()][..]),
    "{slug} does not carry the Styrene UI stack"
);
```

The 25 built-ins are compiled in with `include_str!` (`theme.rs:454`–`478`) *and* read from the same
paths by the round-trip test (`theme.rs:1646`), so the file on disk is the single source of truth for
both the shipped bytes and the assertion above.

## Which family is which, and the fallback chain

Both `font-ui` and `font-mono` accept either a single family name or an ordered array that the
resolver tries left to right. The documented shape is
`font-mono = ["JetBrains Mono", "Menlo"]   # terminal grid font — name or fallback stack` and
`font-ui = ["-apple-system", "SF Pro"]   # window-chrome font` (`customization/themes`).

**UI chrome — the chain this repository declares:**

```text
Styrene B  →  Styrene A  →  (resolver, not yet written)
```

`Styrene B` first and `Styrene A` second is the order the Anthropic references describe: B is the
text/UI cut, A the display cut, so chrome leans on B and falls back to A before it ever reaches a
system font.

**The chain it replaces (the shipped build's, for comparison):**

```text
-apple-system  →  BlinkMacSystemFont  →  Segoe UI / SF Pro / Inter  →  sans-serif
```

**Terminal grid — JetBrains Mono, as shipped.** Seven themes override it, and the overrides are
recovered text, not edits:

| Theme | `[token].font-mono` |
|---|---|
| `nord` | `["JetBrains Mono", "SF Mono", "Menlo", "monospace"]` |
| `april`, `april-dark`, `floating-card`, `glass-dark`, `night`, `paper` | `["JetBrains Mono", "Menlo", "monospace"]` |
| `acrylic-dark` | `["Cascadia Mono", "Cascadia Code", "Consolas", "monospace"]` — the Windows console families |
| `ayu-light` | `"Menlo"` (a single name, not a stack) + `adjust-cell-height = "20%"` |
| `newsprint` | `"Office Code Pro"` — the bundled serif-mono, the editorial theme's choice |

Every other theme takes `font-family`'s documented default, `JetBrains Mono`.

**The fallback files, and where they are.** These are the files present in
`Contents/Resources/fonts/` of both installers, with the licences the bundle's own `CREDITS.md`
records. All seven are recovered into this repository at `resources/fonts/` and are byte-identical
to the shipped bundle (SHA-256, verified for all 160 recovered resource files), because every one of
these licences permits redistribution:

| File | Bytes | Licence (per the bundle's `CREDITS.md`) | Redistributable |
|---|---:|---|---|
| `JetBrainsMono.ttf` | 303 128 | SIL Open Font License 1.1 | yes |
| `JetBrainsMono-Italic.ttf` | 308 872 | SIL Open Font License 1.1 | yes |
| `OfficeCodePro-Regular.ttf` | 80 476 | SIL Open Font License 1.1 | yes |
| `OfficeCodePro-Bold.ttf` | 77 268 | SIL Open Font License 1.1 | yes |
| `OfficeCodePro-Italic.ttf` | 81 872 | SIL Open Font License 1.1 | yes |
| `OfficeCodePro-BoldItalic.ttf` | 78 704 | SIL Open Font License 1.1 | yes |
| `SymbolsNerdFontMono-Regular.ttf` | 2 333 488 | MIT License | yes |
| — | — | **Styrene A / Styrene B** | **no — absent from the repository** |
| — | — | **Tiempos Text** | **no — absent from the repository** |

That contrast is the point of this section: the OFL/MIT families are shipped because they may be,
and the two commercially licensed families are *not* shipped because they may not be.
`Get-ChildItem resources -Recurse -File | Where-Object Name -match 'Styrene|Tiempos'` returns
nothing.

Drop-in fonts are discovered from `~/.config/otty/fonts/` (`.ttf`, `.otf`, `.ttc`, `.otc`), which is
the documented user path, and `Symbols Nerd Font Mono` is the documented fallback for glyphs the
primary family lacks.

**Resolution order for a family name: installed → dropped in → shipped.** The bundle is scanned *last*
of the three, which is what lets it be a default rather than an override — `JetBrains Mono` resolves on
a machine that never installed it, because the shipped copy answers the name, while a copy the user
installed themselves stays theirs. Which of the three answered is recorded rather than inferred
(`FontSource::{System, User, Bundled}` on every entry, `ResolutionReport` on every role), and every
resolution that is not exact writes its sentence to the log at info level, so "the grid is rendering in
a family nobody named" is a thing the log says out loud. Before `Bundled` existed it was silent *and*
wrong: the grid rendered in Courier New — the platform's `monospace` — with `JetBrainsMono.ttf` sitting
unopened beside the executable (`docs/RESTORATION.md` §12.1).

## Tiempos Text, and what is actually in the repository

Tiempos Text is the editorial serif in the decision. Its repository footprint is the smallest of the
three, and it must be stated as such rather than presented as wired up:

- **No theme declares it.** Zero occurrences of `Tiempos` in any `resources/themes/*.ottytheme`.
- **No code declares it.** Zero occurrences of `Tiempos` in `packages/`.
- **The shipped product does not use it either.** Zero occurrences of `Tiempos` in
  `settings-ui.html` and zero in the app binary's recovered theme text.
- **The surface it is destined for does not exist yet.** The editorial prose in question lives in the
  file pane (Markdown/reST live preview, the agent-transcript renderer) and in the settings prose,
  and neither is written: `otty-ui` and `otty-render` have code (1 895 and 737 lines) but no verified
  rendering, and the file pane and Settings window are not among the modules either crate contains.

So: Tiempos Text is a **declared intent with no declaration site in the tree yet**. The place it will
land is a serif companion to `Token::font_ui` — the `Config` key registry's `font-family` family for
prose surfaces, not the terminal `font-family` — and `docs/ARCHITECTURE.md`'s layering already puts
that decision in `otty-config` (which owns the type model) and `otty-render` (which will resolve it).

## Licensing, and what this repository ships

**Styrene (A and B) and Tiempos Text are commercially licensed retail typefaces.** Styrene is from
[Commercial Type](https://vault.commercialtype.com/catalog/styrene/styrene_b/regular); the Tiempos
collection is from [Klim Type Foundry](https://klim.co.nz/collections/tiempos/). Neither is open
source, neither may be redistributed without a licence, and **this repository contains none of their
font code** — no `.ttf`, `.otf`, `.ttc`, `.otc` or `.woff2` for either family appears anywhere under
`resources/` or `packages/`.

What the repository ships instead, precisely:

1. **The declaration** — the family names, in the theme files and in the `Token` type, so a machine
   that has the fonts licensed and installed picks them up, and so the intent is auditable.
2. **The metric-compatible fallbacks** — the chain the declaration degrades through
   (`Styrene A`, then the platform's UI family) and, for the grid, the OFL/MIT families listed above
   that the shipped bundle already relied on.
3. **Nothing else.** No bundled brand font, and no downloader that would fetch one.

## Grounding the claim about which families Anthropic uses

The claim is not this repository's invention, and it is not an assertion to take on trust. It is in
Anthropic's own published material:

- Anthropic ships a **brand-guidelines skill** in its own skills repository, `anthropics/skills`,
  whose typography section names **Styrene A and Styrene B** for display/headline use and **Tiempos
  Text** for body and long-form copy. ([anthropics/skills — brand-guidelines](https://github.com/anthropics/skills/tree/main/skills/brand-guidelines))
- The same pairing is independently catalogued in public design-system write-ups derived from
  Anthropic's sites and app: e.g. [`design-ai/design-md/anthropic/DESIGN.md`](https://github.com/Khalidabdi1/design-ai/blob/main/design-md/anthropic/DESIGN.md),
  [`VoltAgent/awesome-design-md` — claude](https://github.com/VoltAgent/awesome-design-md/blob/main/design-md/claude/DESIGN.md),
  and the type-foundry side — [Klim Type Foundry · Tiempos Collection](https://klim.co.nz/collections/tiempos/),
  [Commercial Type · Styrene B](https://vault.commercialtype.com/catalog/styrene/styrene_b/regular).

One caveat on method, recorded because it affects how much weight the citations carry: on this host
every direct fetch of those pages was refused by a host-policy check (`URL hostname … resolves to a
non-public IP address`), so the stack was confirmed from **search results and the documents' own
titles and excerpts**, not from a full retrieval of each page. The verified-by-hand part of the
picture is the **negative** result, and it is the part that constrains this repository's claims: the
shipped Otty bundle names no brand typeface anywhere, so nothing here claims to have recovered
Styrene from it. See `docs/RESTORATION.md` §7 and §10 for the byte-level evidence, including the 22
`font-ui` occurrences in the app binary and the `settings-ui.html` probe that returns zero for
`Styrene`, `Tiempos` and `Anthropic`.

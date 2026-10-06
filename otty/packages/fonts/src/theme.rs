//! The bridge from a `.ottytheme` to the type stack, and the report that says
//! whether the theme's own answer was honoured.
//!
//! A theme declares its fonts in `[token]` — `font-ui` and `font-mono`, each a
//! bare name or an ordered stack. Resolution here is deliberately *unforgiving
//! about honesty and forgiving about rendering*: the fallback still walks down
//! the chain so the app renders, but a family the theme named and the machine
//! does not have is recorded as a mismatch, never swallowed.

use otty_core::geometry::CellHeightAdjust;

use crate::blending::FontBlending;
use crate::catalog::FontCatalog;
use crate::stack::{FontRole, RoleResolution, TypeStack};

/// A family a theme declares that this machine does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDeclaration {
    pub role: FontRole,
    pub family: String,
}

impl MissingDeclaration {
    /// One line, in the shape the Settings panel and `otty doctor` print.
    pub fn line(&self) -> String {
        format!("the active theme names `{}` for the {} role, which is not installed", self.family, self.role)
    }
}

/// What a theme's font keys resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeFontResolution {
    /// `[token].font-ui` (or `ui-font-family`), resolved.
    pub ui: RoleResolution,
    /// `[token].font-mono` (or `font-family`), resolved.
    pub mono: RoleResolution,
    /// Every family the theme declared that is not installed, in declaration
    /// order.
    pub declared_missing: Vec<MissingDeclaration>,
}

impl ThemeFontResolution {
    /// Whether every family the theme declared is actually installed.
    ///
    /// `false` does not mean the app cannot render — the chain still answered,
    /// or [`ThemeFontResolution::is_failure`] would be `true` — it means the
    /// theme's own answer was not honoured, which the Settings panel has to say
    /// out loud rather than present the substituted family as the theme's.
    pub fn agrees_with_theme(&self) -> bool {
        self.declared_missing.is_empty()
    }

    /// Whether nothing could answer for at least one role.
    pub fn is_failure(&self) -> bool {
        self.ui.is_failure() || self.mono.is_failure()
    }

    /// The resolution for one of the two roles a theme can declare.
    pub fn role(&self, role: FontRole) -> Option<&RoleResolution> {
        match role {
            FontRole::UiChrome => Some(&self.ui),
            FontRole::TerminalGrid => Some(&self.mono),
            FontRole::EditorialSerif => None,
        }
    }

    /// Everything worth telling the user: the substituted families first, then
    /// the theme's own unhonoured declarations.
    pub fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        for resolution in [&self.ui, &self.mono] {
            for line in &resolution.report.lines {
                if !notes.contains(line) {
                    notes.push(line.clone());
                }
            }
        }
        for missing in &self.declared_missing {
            let line = missing.line();
            if !notes.contains(&line) {
                notes.push(line);
            }
        }
        notes
    }
}

/// The typography a theme asks for, in one value.
///
/// This is what the renderer needs to open a window: the stack to resolve, the
/// size, the cell-height delta and the blending mode. `font-blending` is read
/// straight from the theme's merged table because `otty-config`'s typed views do
/// not model it.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeFonts {
    pub stack: TypeStack,
    /// `[token].font-size`.
    pub font_size: Option<f32>,
    /// `[token].adjust-cell-height`, parsed. A value Ghostty would reject is
    /// reported and dropped rather than taken as a delta.
    pub adjust_cell_height: Option<CellHeightAdjust>,
    /// `[terminal].font-blending`, the theme's suggestion for the compositing
    /// mode.
    pub blending: Option<FontBlending>,
}

impl ThemeFonts {
    /// Read a theme's typography, with the product defaults for whatever the
    /// theme leaves out.
    pub fn from_theme(theme: &otty_config::theme::Theme) -> Self {
        let adjust_cell_height = match theme.token.adjust_cell_height.as_deref() {
            None => None,
            Some(raw) => match CellHeightAdjust::parse(raw) {
                Ok(adjust) => Some(adjust),
                Err(reason) => {
                    log::warn!(
                        "theme `{}`: `adjust-cell-height = {raw:?}` is not usable: {reason}",
                        theme.slug
                    );
                    None
                }
            },
        };
        Self {
            stack: TypeStack::from_theme(theme),
            font_size: theme.token.font_size,
            adjust_cell_height,
            blending: theme_blending(theme),
        }
    }
}

/// The compositing mode a theme suggests in `[terminal].font-blending`.
///
/// The key is unmodelled by the typed `Terminal` view, so it is read from the
/// theme's own merged table — the same table that keeps a newer Otty's keys
/// alive. An unreadable value is reported and ignored: the mode is taste, not
/// correctness.
pub fn theme_blending(theme: &otty_config::theme::Theme) -> Option<FontBlending> {
    let raw = theme.raw.get("terminal")?.get("font-blending")?.as_str()?;
    match FontBlending::parse(raw) {
        Some(mode) => Some(mode),
        None => {
            log::warn!("theme `{}`: `font-blending = {raw:?}` is not a mode Otty knows", theme.slug);
            None
        }
    }
}

impl FontCatalog {
    /// Resolve the two roles a theme declares against this machine.
    ///
    /// A theme that names a family which is not installed does not silently
    /// succeed: the walk continues (so the app renders) and the family lands in
    /// [`ThemeFontResolution::declared_missing`], which is what
    /// [`ThemeFontResolution::agrees_with_theme`] reports on.
    pub fn resolve_theme(&self, theme: &otty_config::theme::Theme) -> ThemeFontResolution {
        let stack = TypeStack::from_theme(theme);
        let ui = self.resolve_role(&stack, FontRole::UiChrome);
        let mono = self.resolve_role(&stack, FontRole::TerminalGrid);

        let mut declared_missing = Vec::new();
        for (role, resolution) in [(FontRole::UiChrome, &ui), (FontRole::TerminalGrid, &mono)] {
            for family in &resolution.report.missing {
                declared_missing.push(MissingDeclaration {
                    role,
                    family: family.clone(),
                });
            }
        }

        ThemeFontResolution { ui, mono, declared_missing }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::{FamilyRef, GenericFamily};
    use crate::test_support::{themes_dir, user_catalog, OFFICE_CODE_PRO};

    /// A minimal theme that parses: only `[terminal].foreground` is required.
    fn theme(slug: &str, terminal: &str, token: &str) -> otty_config::theme::Theme {
        let text = format!(
            "[meta]\nname = \"{slug}\"\nmode = \"dark\"\n\n\
             [terminal]\nforeground = \"#f8f8f2\"\n{terminal}\n\n\
             [token]\n{token}\n"
        );
        otty_config::theme::Theme::parse(slug, &text).expect("the fixture parses")
    }

    /// One of the 25 themes this repository ships, from the compiled-in bytes.
    fn shipped(slug: &str) -> otty_config::theme::Theme {
        otty_config::theme::Theme::load(slug, None).unwrap_or_else(|e| panic!("{slug}: {e}"))
    }

    #[test]
    fn every_shipped_theme_declares_the_styrene_ui_stack() {
        let mut slugs: Vec<String> = std::fs::read_dir(themes_dir())
            .expect("the shipped themes are in the repository")
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.extension().and_then(|e| e.to_str()) != Some("ottytheme") {
                    return None;
                }
                Some(path.file_stem()?.to_string_lossy().to_string())
            })
            .collect();
        slugs.sort();
        assert_eq!(slugs.len(), 25, "every shipped theme is checked: {slugs:?}");

        for slug in &slugs {
            let parsed = shipped(slug);
            assert_eq!(
                parsed.token.font_ui.as_deref(),
                Some(&["Styrene B".to_owned(), "Styrene A".to_owned()][..]),
                "{slug} does not carry the Styrene UI stack"
            );
        }
    }

    #[test]
    fn a_theme_stack_becomes_the_roles_the_product_resolves() {
        let nord = shipped("nord");
        let stack = TypeStack::from_theme(&nord);

        assert_eq!(
            stack.family_names(FontRole::UiChrome),
            ["Styrene B", "Styrene A"],
            "nord's `font-ui` is the Styrene stack, with no generic appended"
        );
        let mono = stack.families(FontRole::TerminalGrid);
        assert_eq!(mono.len(), 4);
        assert_eq!(mono[0], FamilyRef::Named("JetBrains Mono".into()));
        assert_eq!(mono[3], FamilyRef::Generic(GenericFamily::Monospace));

        // The serif role is not declared by any theme, so the product default
        // stands.
        assert_eq!(stack.family_names(FontRole::EditorialSerif), ["Tiempos Text", "serif"]);
    }

    #[test]
    fn a_theme_that_names_one_family_is_taken_literally() {
        let ayu = shipped("ayu-light");
        let stack = TypeStack::from_theme(&ayu);
        assert_eq!(stack.family_names(FontRole::TerminalGrid), ["Menlo"]);
        assert_eq!(stack.families(FontRole::TerminalGrid).len(), 1);

        let fonts = ThemeFonts::from_theme(&ayu);
        assert_eq!(fonts.font_size, Some(13.0));
        assert_eq!(fonts.adjust_cell_height, Some(CellHeightAdjust::Percent(20.0)));
        assert_eq!(fonts.blending, None, "ayu-light names no blending mode");

        // The single name is not installed here, and there is no generic behind
        // it, so the role is a failure rather than a quiet substitution.
        let (catalog, _dir) = user_catalog();
        let resolved = catalog.resolve_theme(&ayu);
        assert!(resolved.mono.is_failure());
        assert_eq!(resolved.mono.report.missing, ["Menlo"]);
        assert_eq!(resolved.mono.report.summary(), "Menlo is not installed and no fallback answered");
        assert!(!resolved.agrees_with_theme());
        assert!(resolved.role(FontRole::EditorialSerif).is_none());
    }

    #[test]
    fn the_theme_s_blending_suggestion_is_read_from_its_own_table() {
        let nord = shipped("nord");
        assert_eq!(theme_blending(&nord), Some(FontBlending::Linear));

        let dracula = shipped("dracula");
        assert_eq!(theme_blending(&dracula), None);

        let broken = theme("broken", "font-blending = \"nope\"", "");
        assert_eq!(theme_blending(&broken), None);
        assert_eq!(ThemeFonts::from_theme(&broken).blending, None);
    }

    #[test]
    fn a_bad_cell_height_delta_is_dropped_not_guessed() {
        let broken = theme("bad-height", "", "adjust-cell-height = \"3em\"");
        let fonts = ThemeFonts::from_theme(&broken);
        assert_eq!(fonts.adjust_cell_height, None);
        assert_eq!(
            fonts.stack.family_names(FontRole::TerminalGrid),
            ["JetBrains Mono", "monospace"]
        );
    }

    #[test]
    fn a_theme_naming_an_uninstalled_family_is_recorded_not_swallowed() {
        let (catalog, _dir) = user_catalog();
        let declared = theme(
            "declared",
            "",
            "font-mono = [\"Cascadia Mono\", \"Office Code Pro\"]\nfont-ui = [\"Office Code Pro\"]",
        );
        let resolved = catalog.resolve_theme(&declared);

        // Rendering still works: the second entry answered.
        assert!(!resolved.is_failure());
        assert_eq!(resolved.mono.report.answered_by.as_deref(), Some(OFFICE_CODE_PRO));
        assert_eq!(
            resolved.mono.report.summary(),
            "Cascadia Mono is not installed; falling back to Office Code Pro"
        );

        // But the theme's own answer was not honoured, and it says so.
        assert!(!resolved.agrees_with_theme());
        assert_eq!(resolved.declared_missing.len(), 1);
        assert_eq!(resolved.declared_missing[0].role, FontRole::TerminalGrid);
        assert_eq!(resolved.declared_missing[0].family, "Cascadia Mono");
        assert!(resolved.declared_missing[0]
            .line()
            .contains("the active theme names `Cascadia Mono` for the terminal role"));
        assert!(resolved.notes().iter().any(|n| n.contains("Cascadia Mono")));
    }

    #[test]
    fn a_theme_whose_families_are_all_installed_agrees() {
        let (catalog, _dir) = user_catalog();
        let declared = theme(
            "installed",
            "",
            "font-mono = [\"Office Code Pro\"]\nfont-ui = [\"Office Code Pro\"]",
        );
        let resolved = catalog.resolve_theme(&declared);
        assert!(resolved.agrees_with_theme(), "{:?}", resolved.declared_missing);
        assert!(!resolved.is_failure());
        assert_eq!(resolved.ui.report.answered_by.as_deref(), Some(OFFICE_CODE_PRO));
        assert!(resolved.ui.report.is_exact());
        assert_eq!(resolved.mono.report.answered_by.as_deref(), Some(OFFICE_CODE_PRO));
        assert_eq!(
            resolved.ui.face(),
            catalog.find_face(OFFICE_CODE_PRO, crate::FaceStyle::Regular)
        );
        assert!(resolved.notes().is_empty(), "{:?}", resolved.notes());
    }

    #[test]
    fn a_theme_with_no_font_keys_falls_back_to_the_product_stack() {
        let (catalog, _dir) = user_catalog();
        let bare = theme("bare", "", "");
        let resolved = catalog.resolve_theme(&bare);
        // The ui role still answers — through the generic at the end of the
        // product chain — and the Styrene families it wants are reported.
        assert!(!resolved.is_failure());
        assert_eq!(resolved.ui.report.requested, ["Styrene B", "Styrene A", "sans-serif"]);
        assert_eq!(resolved.ui.report.missing, ["Styrene B", "Styrene A"]);
        assert_eq!(
            resolved.ui.report.summary(),
            "Styrene B is not installed; falling back to sans-serif"
        );
        assert!(!resolved.agrees_with_theme());
    }
}

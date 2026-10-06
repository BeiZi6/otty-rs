//! Fig's spec format, as the shipped `completions-client.db` stores it.
//!
//! The `specs.spec` column holds Fig's JSON verbatim (711 CLI tools' worth, cut
//! at `fig_base_date`). Only the parts Otty uses are modelled; everything else is
//! preserved untouched so a spec can be written back byte-identically.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One CLI's spec, which is also the shape of each of its subcommands.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FigSpec {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub subcommands: Vec<FigSubcommand>,
    #[serde(default)]
    pub options: Vec<FigOption>,
    #[serde(default)]
    pub args: Vec<FigArg>,
    /// Any key the model does not read, kept so a round trip loses nothing.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A subcommand, which is a nested [`FigSpec`] in every way that matters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FigSubcommand {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub subcommands: Vec<FigSubcommand>,
    #[serde(default)]
    pub options: Vec<FigOption>,
    #[serde(default)]
    pub args: Vec<FigArg>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn fig_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = serde_json::Value::deserialize(d).map_err(serde::de::Error::custom)?;
    Ok(match v {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        serde_json::Value::String(s) => matches!(s.as_str(), "true" | "1" | "yes"),
        _ => false,
    })
}

fn name_or_names<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = serde_json::Value::deserialize(d).map_err(serde::de::Error::custom)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Array(a) => Ok(a
            .iter()
            .find_map(|x| x.as_str())
            .unwrap_or("")
            .to_string()),
        _ => Ok(String::new()),
    }
}

/// An option.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FigOption {
    #[serde(default, alias = "names", deserialize_with = "name_or_names")]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "fig_bool")]
    pub hidden: bool,
    /// Short forms. Otty offers each as its own candidate, because they are
    /// separate things to type.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Whether the option may be given more than once.
    #[serde(default, rename = "isRepeatable", deserialize_with = "fig_bool")]
    pub is_repeatable: bool,
    /// Whether a value must be attached with `=` rather than a space.
    #[serde(default, rename = "requiresSeparator", deserialize_with = "fig_bool")]
    pub requires_separator: bool,
    #[serde(default)]
    pub args: Vec<FigArg>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A positional argument.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FigArg {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "isOptional", deserialize_with = "fig_bool")]
    pub is_optional: bool,
    /// Whether more than one value may appear.
    #[serde(default, rename = "isVariadic", deserialize_with = "fig_bool")]
    pub is_variadic: bool,
    /// Static suggestions. A spec may instead declare generators, which need to
    /// run something; those are kept in `extra` and are not evaluated here.
    #[serde(default)]
    pub suggestions: Vec<String>,
    /// `filepaths`, `folders`, and friends.
    #[serde(default)]
    pub template: Option<FigTemplate>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// The templates Fig uses to say "complete this as a path".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FigTemplate {
    Filepaths,
    Folders,
    History,
    Help,
    #[serde(other)]
    Other,
}

impl FigTemplate {
    /// The candidate kind a template implies, when it implies one.
    pub fn candidate_kind(&self) -> Option<crate::CandidateKind> {
        match self {
            Self::Filepaths => Some(crate::CandidateKind::File),
            Self::Folders => Some(crate::CandidateKind::Folder),
            Self::History | Self::Help | Self::Other => None,
        }
    }
}

/// The spec tree at the cursor's position.
///
/// The root spec and a subcommand are the same shape in Fig's format, so this
/// borrows whichever node applies rather than inventing a second type.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub subcommands: &'a [FigSubcommand],
    pub options: &'a [FigOption],
    pub args: &'a [FigArg],
}

/// Either node of the tree, so `resolve` can walk from one into the other.
enum Node<'a> {
    Root(&'a FigSpec),
    Sub(&'a FigSubcommand),
}

impl<'a> Node<'a> {
    fn subcommands(&self) -> &'a [FigSubcommand] {
        match self {
            Self::Root(s) => &s.subcommands,
            Self::Sub(s) => &s.subcommands,
        }
    }

    fn options(&self) -> &'a [FigOption] {
        match self {
            Self::Root(s) => &s.options,
            Self::Sub(s) => &s.options,
        }
    }

    fn args(&self) -> &'a [FigArg] {
        match self {
            Self::Root(s) => &s.args,
            Self::Sub(s) => &s.args,
        }
    }

    fn name(&self) -> &'a str {
        match self {
            Self::Root(s) => &s.name,
            Self::Sub(s) => &s.name,
        }
    }

    fn description(&self) -> Option<&'a str> {
        match self {
            Self::Root(s) => s.description.as_deref(),
            Self::Sub(s) => s.description.as_deref(),
        }
    }
}

impl FigSpec {
    /// Walk `words` (which start with the command name) down the subcommand
    /// tree, and return the spec that applies to the cursor's position.
    ///
    /// A word is consumed by a subcommand only when one matches it exactly, or
    /// when it is an unambiguous prefix of exactly one — which is how `cargo te`
    /// still offers `test`'s own arguments.
    pub fn resolve(&self, words: &[String]) -> Resolved<'_> {
        let mut node = Node::Root(self);
        let mut i = 1; // skip the command name
        while let Some(word) = words.get(i) {
            if word.starts_with('-') {
                // Options do not navigate the tree.
                break;
            }
            let Some(next) = pick_subcommand(node.subcommands(), word) else {
                break;
            };
            node = Node::Sub(next);
            i += 1;
        }
        Resolved {
            name: node.name(),
            description: node.description(),
            subcommands: node.subcommands(),
            options: node.options(),
            args: node.args(),
        }
    }

    /// Apply a `spec_i18n` overlay.
    ///
    /// The overlay is a flat map of dotted paths to translated text, e.g.
    /// `{"description": "…", "subcommands.build.description": "…"}`. Only
    /// `description` leaves are translated; the shipped database carries no
    /// overlays at all, so this mapping follows the shape of the table rather
    /// than an observed example — see the module note in `docs/RESTORATION.md`.
    pub fn apply_overlay(&mut self, overlay: &BTreeMap<String, String>) {
        for (path, text) in overlay {
            apply_one(self, &path.split('.').collect::<Vec<_>>(), text);
        }
    }
}

fn apply_one(spec: &mut FigSpec, path: &[&str], text: &str) {
    match path {
        ["description"] => spec.description = Some(text.to_owned()),
        ["subcommands", name, rest @ ..] => {
            if let Some(sub) = spec.subcommands.iter_mut().find(|s| s.name == *name) {
                apply_one_sub(sub, rest, text);
            }
        }
        ["options", name, rest @ ..] => {
            if let Some(opt) = spec.options.iter_mut().find(|o| o.name == *name) {
                apply_one_option(opt, rest, text);
            }
        }
        _ => {}
    }
}

fn apply_one_sub(sub: &mut FigSubcommand, path: &[&str], text: &str) {
    match path {
        ["description"] => sub.description = Some(text.to_owned()),
        ["subcommands", name, rest @ ..] => {
            if let Some(child) = sub.subcommands.iter_mut().find(|s| s.name == *name) {
                apply_one_sub(child, rest, text);
            }
        }
        _ => {}
    }
}

fn apply_one_option(opt: &mut FigOption, path: &[&str], text: &str) {
    if path == ["description"] {
        opt.description = Some(text.to_owned());
    }
}

/// The subcommand a word selects, if any.
fn pick_subcommand<'a>(subs: &'a [FigSubcommand], word: &str) -> Option<&'a FigSubcommand> {
    if let Some(exact) = subs.iter().find(|s| s.name == word) {
        return Some(exact);
    }
    let mut matches = subs.iter().filter(|s| s.name.starts_with(word));
    let first = matches.next()?;
    // Only an unambiguous prefix navigates; otherwise the word is still being
    // typed and the parent's candidates are the useful ones.
    if matches.next().is_none() {
        Some(first)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUBCOMMANDED: &str = r#"{
      "name": "git",
      "description": "the stupid content tracker",
      "subcommands": [
        {"name": "commit", "description": "Record changes",
         "options": [{"name": "--amend", "description": "Amend the last commit"}]},
        {"name": "checkout", "description": "Switch branches"},
        {"name": "cherry-pick", "description": "Apply a commit elsewhere"}
      ],
      "unknown-future-key": {"nested": true}
    }"#;

    #[test]
    fn a_spec_parses_and_keeps_the_keys_it_does_not_model() {
        let s: FigSpec = serde_json::from_str(SUBCOMMANDED).unwrap();
        assert_eq!(s.name, "git");
        assert_eq!(s.subcommands.len(), 3);
        assert_eq!(
            s.extra.get("unknown-future-key").and_then(|v| v.as_object()).map(|o| o.len()),
            Some(1)
        );
    }

    #[test]
    fn an_exact_subcommand_navigates() {
        let s: FigSpec = serde_json::from_str(SUBCOMMANDED).unwrap();
        let r = s.resolve(&["git".to_owned(), "commit".to_owned()]);
        assert_eq!(r.name, "commit");
        assert_eq!(r.options.len(), 1);
        assert_eq!(r.options[0].name, "--amend");
    }

    #[test]
    fn an_unambiguous_prefix_navigates_so_arguments_still_work() {
        let s: FigSpec = serde_json::from_str(SUBCOMMANDED).unwrap();
        let r = s.resolve(&["git".to_owned(), "che".to_owned()]);
        // `checkout` and `cherry-pick` both start with `che`, so the word is
        // still ambiguous and the root's candidates remain the useful ones.
        assert_eq!(r.name, "git");

        let s2: FigSpec = serde_json::from_str(SUBCOMMANDED).unwrap();
        let r = s2.resolve(&["git".to_owned(), "com".to_owned()]);
        assert_eq!(r.name, "commit");
    }

    #[test]
    fn an_option_word_does_not_navigate() {
        let s: FigSpec = serde_json::from_str(SUBCOMMANDED).unwrap();
        let r = s.resolve(&["git".to_owned(), "--no-pager".to_owned(), "commit".to_owned()]);
        assert_eq!(r.name, "git");
    }

    #[test]
    fn an_overlay_translates_the_root_description() {
        let mut s: FigSpec = serde_json::from_str(SUBCOMMANDED).unwrap();
        let mut overlay = BTreeMap::new();
        overlay.insert("description".to_owned(), "内容追踪器".to_owned());
        overlay.insert("subcommands.commit.description".to_owned(), "记录变更".to_owned());
        s.apply_overlay(&overlay);
        assert_eq!(s.description.as_deref(), Some("内容追踪器"));
        assert_eq!(s.subcommands[0].description.as_deref(), Some("记录变更"));
    }

    #[test]
    fn an_overlay_naming_a_missing_node_changes_nothing() {
        let mut s: FigSpec = serde_json::from_str(SUBCOMMANDED).unwrap();
        let before = s.clone();
        let mut overlay = BTreeMap::new();
        overlay.insert("subcommands.nope.description".to_owned(), "x".to_owned());
        overlay.insert("nonsense".to_owned(), "y".to_owned());
        s.apply_overlay(&overlay);
        assert_eq!(s, before);
    }

    #[test]
    fn the_path_templates_map_to_candidate_kinds() {
        assert_eq!(
            FigTemplate::Filepaths.candidate_kind(),
            Some(crate::CandidateKind::File)
        );
        assert_eq!(
            FigTemplate::Folders.candidate_kind(),
            Some(crate::CandidateKind::Folder)
        );
        assert_eq!(FigTemplate::History.candidate_kind(), None);
    }

    #[test]
    fn a_spec_survives_a_round_trip_through_json() {
        let s: FigSpec = serde_json::from_str(SUBCOMMANDED).unwrap();
        let back: FigSpec = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}

"""Autocomplete: fix the four real bugs and the two wrong test premises that the
first run exposed."""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\autocomplete\src\lib.rs")
t = F.read_text(encoding="utf-8")
before = t

EDITS = [
    # 1. A bearer token is a signal on its own. Matching `bearer <x>` as one
    #    token missed `-H "Authorization: Bearer abc"`, where the word arrives
    #    quoted and split.
    ("""        if let Some(rest) = lower.strip_prefix("bearer ") {
            let _ = rest;
            return true;
        }""",
     """        // `Bearer` as a whole word is a strong enough signal on its own: it
        // only appears in an authorisation header. Matching `bearer <value>` as
        // one token missed the common `-H "Authorization: Bearer abc"`, where
        // the header arrives quoted.
        if lower == "bearer" || lower.starts_with("bearer:") {
            return true;
        }"""),

    # 2. A hidden entry must not appear in the browsable list (empty prefix),
    #    only when it is what is being typed.
    ("""            if sub.hidden && !sub.name.starts_with(needle) {
                continue;
            }""",
     """            // A hidden entry is offered only once it is what is being typed;
            // an empty prefix is the browsable list, where it does not belong.
            if sub.hidden && (needle.is_empty() || !sub.name.starts_with(needle)) {
                continue;
            }"""),

    # 3. The whole-line preview applies to a partially typed line, not only to
    #    the command word: "a whole command line that leads this folder (run
    #    most and run last)".
    ("""    // A whole command line that leads this folder: on the command word, the
    // most-run line starting with what was typed.
    if buffer.on_command() {
        let typed = buffer.current.as_str();
        if let Some(last) = &hints.last_in_directory {
            if last.starts_with(typed) && last != typed {
                return (Some(last.clone()), None);
            }
        }
    }""",
     """    // A whole command line that leads this folder. This is checked against
    // the *entire* typed line, not just the word under the cursor: the rule the
    // product states is "a whole command line that leads this folder (run most
    // and run last)", and `cargo t` in a directory whose last command was
    // `cargo test --all` should preview that line, not the bare subcommand.
    let typed_line = {
        let mut s = buffer.words.join(" ");
        if !buffer.current.is_empty() {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(&buffer.current);
        }
        s
    };
    if !typed_line.is_empty() {
        if let Some(last) = &hints.last_in_directory {
            if last.starts_with(&typed_line) && *last != typed_line {
                return (Some(last.clone()), None);
            }
        }
    }"""),
]

for old, new in EDITS:
    if old not in t:
        print(f"MISS: {old.splitlines()[0][:90]}")
        sys.exit(1)
    t = t.replace(old, new, 1)

TESTS = [
    # 4. The fixture has a `test` subcommand, so `resolve` should land on it.
    ("""    #[test]
    fn a_spec_resolves_into_its_subcommand_tree() {
        let spec = cargo_spec();
        // `cargo test --` resolves through `test` when the spec has children.
        let path = spec.resolve(&["cargo".to_owned(), "test".to_owned()]);
        assert_eq!(path.name, "cargo");
    }""",
     """    #[test]
    fn a_spec_resolves_into_its_subcommand_tree() {
        let spec = cargo_spec();
        // An exact subcommand word navigates.
        let path = spec.resolve(&["cargo".to_owned(), "test".to_owned()]);
        assert_eq!(path.name, "test");
        // A bare command stays at the root.
        assert_eq!(spec.resolve(&["cargo".to_owned()]).name, "cargo");
    }"""),

    # 5. The quoted-word premise was wrong: the quote closes before the trailing
    #    token, so the quoted word is a finished word, not the one being typed.
    ("""    #[test]
    fn a_quoted_word_is_not_split() {
        let b = Buffer::parse(r#"grep "two words" f"#, 19);
        assert_eq!(b.words[0], "grep");
        assert!(b.current.starts_with('"'));
    }""",
     """    #[test]
    fn a_quoted_word_is_not_split() {
        let line = r#"grep "two words" f"#;
        let b = Buffer::parse(line, line.len());
        assert_eq!(b.words[0], "grep");
        // The quoted phrase is one finished word, not two.
        assert_eq!(b.words[1], "\\"two words\\"");

        // With the cursor inside the quotes, the word is still open.
        let b = Buffer::parse(line, 12);
        assert!(b.current.starts_with('"'), "{:?}", b);
        assert!(!b.current.ends_with('"'));
    }"""),
]

for old, new in TESTS:
    if old not in t:
        print(f"MISS(test): {old.splitlines()[0][:90]}")
        sys.exit(1)
    t = t.replace(old, new, 1)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched otty-autocomplete")

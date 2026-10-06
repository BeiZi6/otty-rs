//! Keys, modifiers and key-chord spelling.
//!
//! Otty has to speak three dialects at once:
//!
//! * the **config dialect** — `keybind = cmd+shift+t=action`, sequences joined
//!   with `>`, and the `tab+right-arrow` / `ctrl+space` spellings the
//!   autocomplete keys use;
//! * the **Kitty keyboard protocol** — keys are reported with their base layout
//!   character, alternates, and an explicit modifier bitmask (`kitty-keyboard`);
//! * the **legacy encoding** — the bytes a VT program expects from a keypress,
//!   including DECCKM application-cursor mode and DECKPAM keypad mode.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

bitflags_lite::bitflags! {
    /// Modifier keys held during an event.
    ///
    /// The bit values follow the convention shared by xterm's
    /// `modifyOtherKeys` and the Kitty keyboard protocol — shift 1, alt 2,
    /// ctrl 4, super 8 — so the protocol encoders are a plain `+ 1`.
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct Modifiers: u16 {
        const SHIFT = 1 << 0;
        const ALT   = 1 << 1;   // Option on macOS; Alt elsewhere
        const CTRL  = 1 << 2;
        const SUPER = 1 << 3;   // ⌘ / Win
        const HYPER = 1 << 4;
        const META  = 1 << 5;
        const CAPS_LOCK = 1 << 6;
        const NUM_LOCK  = 1 << 7;
    }
}

impl Modifiers {
    /// Modifier bits that participate in chord matching. Lock keys never do.
    pub const MATCH_MASK: Self = Self(
        Self::SHIFT.0 | Self::CTRL.0 | Self::ALT.0 | Self::SUPER.0 | Self::HYPER.0 | Self::META.0,
    );

    /// The bits a chord must compare against.
    pub const fn chord_bits(self) -> Self {
        Self(self.0 & Self::MATCH_MASK.0)
    }

    /// The Kitty keyboard protocol modifier value (1 = shift, 2 = alt, 4 = ctrl,
    /// 8 = super, 16 = hyper, 32 = meta, 64 = caps lock, 128 = num lock).
    pub const fn kitty_bits(self) -> u32 {
        self.0 as u32 + 1 // Kitty sends modifiers + 1
    }

    /// A canonical, sorted spelling for round-tripping and conflict detection.
    ///
    /// The order and the words match how the shipped key map is written
    /// (`cmd+shift+t`, `cmd+opt+d`, `ctrl+shift+space`), so a binding read from
    /// `config.toml` is written back byte-identically.
    pub fn canonical(self) -> String {
        let mut parts = Vec::new();
        for (bit, name) in [
            (Self::SUPER, "cmd"),
            (Self::CTRL, "ctrl"),
            (Self::ALT, "opt"),
            (Self::SHIFT, "shift"),
            (Self::HYPER, "hyper"),
            (Self::META, "meta"),
        ] {
            if self.contains(bit) {
                parts.push(name);
            }
        }
        parts.join("+")
    }
}

/// A key that has no printable character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NamedKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    Space,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    /// F1 – F24.
    F(u8),
    /// The numeric keypad, distinguished from the main block because
    /// application-keypad mode (DECKPAM) changes what they send.
    Keypad(KeypadKey),
    /// Print Screen, Scroll Lock, Pause, Menu, and friends.
    PrintScreen,
    ScrollLock,
    Pause,
    Menu,
    /// Punctuation keys, named rather than spelled as a character, because a
    /// key map written on a US layout has to load on a German one. These are the
    /// spellings the shipped key map uses.
    Comma,
    Period,
    Slash,
    Backslash,
    Semicolon,
    Quote,
    Grave,
    Minus,
    Equal,
    LeftBracket,
    RightBracket,
    /// Media / browser keys reach the terminal as their own codes.
    Media(MediaKey),
    /// A dead key or an input-method result Otty cannot name.
    Unidentified,
}

/// Keys on the numeric keypad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum KeypadKey {
    Digit(u8),
    Decimal,
    Add,
    Subtract,
    Multiply,
    Divide,
    Enter,
    Equal,
    Separator,
    Begin,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

/// Media and browser keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MediaKey {
    VolumeUp,
    VolumeDown,
    VolumeMute,
    PlayPause,
    Stop,
    NextTrack,
    PrevTrack,
    BrowserBack,
    BrowserForward,
    BrowserRefresh,
    BrowserHome,
}

/// Either a printable character or a named key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Key {
    /// A character produced by this physical key on the *current* layout.
    Char(char),
    Named(NamedKey),
}

impl Key {
    /// The `shift`-less form of a character, used when matching chords: `cmd+;`
    /// and `cmd+:` are the same physical chord on different layouts.
    pub fn unshifted(self) -> Self {
        match self {
            Self::Char(c) if c.is_ascii_uppercase() => Self::Char(c.to_ascii_lowercase()),
            Self::Char(c) => Self::Char(SHIFTED_PAIRS
                .iter()
                .find(|(_, shifted)| *shifted == c)
                .map(|(base, _)| *base)
                .unwrap_or(c)),
            other => other,
        }
    }

    /// Whether this key is a printable character.
    pub const fn is_char(self) -> bool {
        matches!(self, Self::Char(_))
    }
}

/// US-layout shifted/unshifted pairs, used for chord normalisation so a binding
/// written as `cmd+;` still fires when the user's layout emits `:`.
const SHIFTED_PAIRS: &[(char, char)] = &[
    ('`', '~'), ('1', '!'), ('2', '@'), ('3', '#'), ('4', '$'), ('5', '%'), ('6', '^'), ('7', '&'),
    ('8', '*'), ('9', '('), ('0', ')'), ('-', '_'), ('=', '+'), ('[', '{'), (']', '}'), ('\\', '|'),
    (';', ':'), ('\'', '"'), (',', '<'), ('.', '>'), ('/', '?'),
];

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(' ') => f.write_str("space"),
            Self::Char(c) => write!(f, "{c}"),
            Self::Named(NamedKey::Space) => f.write_str("space"),
            Self::Named(n) => f.write_str(n.name()),
        }
    }
}

impl NamedKey {
    /// The canonical spelling used in `config.toml` and in `otty keybind list`.
    ///
    /// These are the spellings the shipped key map uses — `left`, `page_up`,
    /// `forward_delete`, `right_bracket` — not the longer `arrow-left` style.
    /// The parser accepts both.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Tab => "tab",
            Self::Backspace => "backspace",
            Self::Escape => "escape",
            Self::Space => "space",
            Self::Delete => "forward_delete",
            Self::Insert => "insert",
            Self::Home => "home",
            Self::End => "end",
            Self::PageUp => "page_up",
            Self::PageDown => "page_down",
            Self::ArrowUp => "up",
            Self::ArrowDown => "down",
            Self::ArrowLeft => "left",
            Self::ArrowRight => "right",
            Self::F(1) => "f1",
            Self::F(2) => "f2",
            Self::F(3) => "f3",
            Self::F(4) => "f4",
            Self::F(5) => "f5",
            Self::F(6) => "f6",
            Self::F(7) => "f7",
            Self::F(8) => "f8",
            Self::F(9) => "f9",
            Self::F(10) => "f10",
            Self::F(11) => "f11",
            Self::F(12) => "f12",
            Self::F(_) => "f",
            Self::Keypad(_) => "keypad",
            Self::PrintScreen => "print_screen",
            Self::ScrollLock => "scroll_lock",
            Self::Pause => "pause",
            Self::Menu => "menu",
            Self::Comma => "comma",
            Self::Period => "period",
            Self::Slash => "slash",
            Self::Backslash => "backslash",
            Self::Semicolon => "semicolon",
            Self::Quote => "quote",
            Self::Grave => "grave",
            Self::Minus => "minus",
            Self::Equal => "equal",
            Self::LeftBracket => "left_bracket",
            Self::RightBracket => "right_bracket",
            Self::Media(_) => "media",
            Self::Unidentified => "unidentified",
        }
    }

    /// Parse the canonical spelling back.
    pub fn parse(s: &str) -> Option<Self> {
        let n = s.trim().to_ascii_lowercase();
        Some(match n.as_str() {
            "enter" | "return" | "cr" => Self::Enter,
            "tab" => Self::Tab,
            "backspace" | "bs" => Self::Backspace,
            "escape" | "esc" => Self::Escape,
            "space" | "spacebar" => Self::Space,
            "delete" | "del" | "forward-delete" | "forward_delete" => Self::Delete,
            "insert" | "ins" => Self::Insert,
            "home" => Self::Home,
            "end" => Self::End,
            "page-up" | "pageup" | "pgup" | "page_up" => Self::PageUp,
            "page-down" | "pagedown" | "pgdn" | "page_down" => Self::PageDown,
            "up" | "arrow-up" => Self::ArrowUp,
            "down" | "arrow-down" => Self::ArrowDown,
            "left" | "arrow-left" => Self::ArrowLeft,
            "right" | "arrow-right" => Self::ArrowRight,
            // Punctuation keys, by the names the shipped key map uses.
            "right-bracket" | "right_bracket" | "]" => Self::RightBracket,
            "left-bracket" | "left_bracket" | "[" => Self::LeftBracket,
            "comma" | "," => Self::Comma,
            "period" | "." => Self::Period,
            "equal" | "=" => Self::Equal,
            "minus" | "-" => Self::Minus,
            "semicolon" | ";" => Self::Semicolon,
            "quote" | "'" => Self::Quote,
            "grave" | "`" => Self::Grave,
            "slash" | "/" => Self::Slash,
            "backslash" | "\\" => Self::Backslash,
            "print_screen" | "print-screen" | "printscreen" | "prtsc" => Self::PrintScreen,
            "scroll_lock" | "scroll-lock" | "scrolllock" => Self::ScrollLock,
            "pause" => Self::Pause,
            "menu" => Self::Menu,
            other => {
                if let Some(rest) = other.strip_prefix('f') {
                    if let Ok(n) = rest.parse::<u8>() {
                        if (1..=24).contains(&n) {
                            return Some(Self::F(n));
                        }
                    }
                }
                return None;
            }
        })
    }
}

/// A key press, release, or auto-repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: Modifiers,
    pub kind: KeyEventKind,
    /// The character the key produces with no modifiers, when the platform can
    /// supply it. The Kitty protocol's `report alternate keys` flags need it.
    pub base_layout: Option<char>,
}

impl KeyEvent {
    pub const fn press(key: Key, mods: Modifiers) -> Self {
        Self { key, mods, kind: KeyEventKind::Press, base_layout: None }
    }

    pub const fn release(key: Key, mods: Modifiers) -> Self {
        Self { key, mods, kind: KeyEventKind::Release, base_layout: None }
    }

    pub const fn with_base_layout(mut self, c: char) -> Self {
        self.base_layout = Some(c);
        self
    }
}

/// Whether the key went down or came up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

/// A chord: a key plus the modifiers held with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Chord {
    pub key: Key,
    pub mods: Modifiers,
}

impl Chord {
    pub const fn new(key: Key, mods: Modifiers) -> Self {
        Self { key, mods }
    }

    /// Normalise so equivalent chords hash the same regardless of how they were
    /// spelled.
    ///
    /// Two invariants, and deliberately only two:
    ///
    /// * letters are case-folded *according to the shift bit*, so `cmd+shift+t`
    ///   matches `cmd+shift+T` but never `cmd+t`. Shift stays part of the chord
    ///   because a terminal must be able to tell the two apart.
    /// * modifier bits are masked to [`Modifiers::MATCH_MASK`], so a stray
    ///   Caps Lock or Num Lock does not break a binding.
    ///
    /// Layout equivalence (`cmd+;` on a US layout versus `cmd+ö` on a German
    /// one) is *not* claimed: the key a chord names is the key the user's
    /// layout produces, and Otty binds that. Guessing otherwise creates
    /// collisions that are worse than the inconvenience.
    pub fn normalized(self) -> Self {
        let shift = self.mods.contains(Modifiers::SHIFT);
        let key = match self.key {
            Key::Char(c) if c.is_ascii_alphabetic() => Key::Char(if shift {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }),
            Key::Char(' ') => Key::Named(NamedKey::Space),
            other => other,
        };
        Self { key, mods: self.mods.chord_bits() }
    }

    /// Render back into config spelling, e.g. `cmd+shift+arrow-up`.
    pub fn to_trigger_string(self) -> String {
        let m = self.mods.canonical();
        if m.is_empty() {
            self.key.to_string()
        } else {
            format!("{m}+{}", self.key)
        }
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_trigger_string())
    }
}

impl FromStr for Chord {
    type Err = ChordParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw = s.trim();
        if raw.is_empty() {
            return Err(ChordParseError::Empty);
        }
        let mut mods = Modifiers::default();
        let mut key: Option<Key> = None;
        for part in raw.split('+') {
            let p = part.trim();
            if p.is_empty() {
                // A literal `+` written as `cmd++`.
                key = Some(Key::Char('+'));
                continue;
            }
            let lower = p.to_ascii_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => mods |= Modifiers::CTRL,
                "alt" | "option" | "opt" => mods |= Modifiers::ALT,
                "shift" => mods |= Modifiers::SHIFT,
                "cmd" | "command" | "super" | "meta-cmd" | "cmdorctrl" => mods |= Modifiers::SUPER,
                "hyper" => mods |= Modifiers::HYPER,
                "meta" => mods |= Modifiers::META,
                _ => {
                    if key.is_some() {
                        return Err(ChordParseError::TrailingKey(p.to_owned()));
                    }
                    key = Some(parse_key_token(p)?);
                }
            }
        }
        Ok(Self::new(key.ok_or(ChordParseError::NoKey)?, mods))
    }
}

fn parse_key_token(t: &str) -> Result<Key, ChordParseError> {
    if let Some(n) = NamedKey::parse(t) {
        return Ok(Key::Named(n));
    }
    let mut chars = t.chars();
    match (chars.next(), chars.next()) {
        // Space is always the named key, so `Key` has exactly one spelling for
        // it and chords round-trip through their own text form.
        (Some(' '), None) => Ok(Key::Named(NamedKey::Space)),
        (Some(c), None) => Ok(Key::Char(c)),
        _ => Err(ChordParseError::UnknownKey(t.to_owned())),
    }
}

/// Failure to parse a chord spelling.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChordParseError {
    #[error("empty key chord")]
    Empty,
    #[error("chord has no key, only modifiers")]
    NoKey,
    #[error("more than one key in chord: {0}")]
    TrailingKey(String),
    #[error("unknown key name: {0}")]
    UnknownKey(String),
}

/// A binding trigger: either one chord, or a `>`-joined sequence of chords.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Trigger(pub Vec<Chord>);

impl Trigger {
    pub fn single(c: Chord) -> Self {
        Self(vec![c])
    }

    pub const fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The first (and only, for simple bindings) chord.
    pub fn first(&self) -> Option<Chord> {
        self.0.first().copied()
    }

    /// True when the binding is a multi-chord sequence.
    pub fn is_sequence(&self) -> bool {
        self.0.len() > 1
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.0.iter().map(|c| c.to_trigger_string()).collect();
        f.write_str(&parts.join(">"))
    }
}

impl FromStr for Trigger {
    type Err = ChordParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chords: Result<Vec<Chord>, _> = s.split('>').map(str::parse).collect();
        let chords = chords?;
        if chords.is_empty() {
            return Err(ChordParseError::Empty);
        }
        Ok(Self(chords))
    }
}

/// Minimal `bitflags` replacement so `otty-core` stays dependency-free.
mod bitflags_lite {
    macro_rules! bitflags {
        (
            $(#[$meta:meta])*
            $vis:vis struct $name:ident: $ty:ty {
                $(const $flag:ident = $value:expr;)*
            }
        ) => {
            $(#[$meta])*
            $vis struct $name(pub $ty);

            impl $name {
                $(pub const $flag: Self = Self($value);)*

                pub const fn empty() -> Self { Self(0) }
                pub const fn all() -> Self { Self(0 $(| $value)*) }
                pub const fn bits(self) -> $ty { self.0 }
                pub const fn from_bits_truncate(bits: $ty) -> Self { Self(bits & Self::all().0) }
                pub const fn contains(self, other: Self) -> bool {
                    (self.0 & other.0) == other.0
                }
                pub const fn intersects(self, other: Self) -> bool {
                    (self.0 & other.0) != 0
                }
                pub const fn is_empty(self) -> bool { self.0 == 0 }
                pub fn insert(&mut self, other: Self) { self.0 |= other.0; }
                pub fn remove(&mut self, other: Self) { self.0 &= !other.0; }
                pub fn toggle(&mut self, other: Self) { self.0 ^= other.0; }
                pub fn set(&mut self, other: Self, on: bool) {
                    if on { self.insert(other) } else { self.remove(other) }
                }
            }

            impl std::ops::BitOr for $name {
                type Output = Self;
                fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
            }
            impl std::ops::BitOrAssign for $name {
                fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
            }
            impl std::ops::BitAnd for $name {
                type Output = Self;
                fn bitand(self, rhs: Self) -> Self { Self(self.0 & rhs.0) }
            }
            impl std::ops::BitAndAssign for $name {
                fn bitand_assign(&mut self, rhs: Self) { self.0 &= rhs.0; }
            }
            impl std::ops::Not for $name {
                type Output = Self;
                fn not(self) -> Self { Self(!self.0 & Self::all().0) }
            }
            impl core::fmt::Debug for $name {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    let mut first = true;
                    write!(f, "{}(", stringify!($name))?;
                    $(
                        if self.contains(Self::$flag) {
                            if !first { write!(f, " | ")?; }
                            write!(f, "{}", stringify!($flag))?;
                            first = false;
                        }
                    )*
                    if first { write!(f, "0x{:x}", self.0)?; }
                    write!(f, ")")
                }
            }
        };
    }
    pub(crate) use bitflags;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chords_in_both_directions() {
        let c: Chord = "cmd+shift+t".parse().unwrap();
        assert_eq!(c.mods, Modifiers::SUPER | Modifiers::SHIFT);
        assert_eq!(c.key, Key::Char('t'));
        assert_eq!(c.to_trigger_string(), "cmd+shift+t");

        let c: Chord = "ctrl+space".parse().unwrap();
        assert_eq!(c.key, Key::Named(NamedKey::Space));
        assert_eq!(c.to_trigger_string(), "ctrl+space");

        let c: Chord = "shift+arrow-up".parse().unwrap();
        assert_eq!(c.key, Key::Named(NamedKey::ArrowUp));

        // Canonical spellings survive a round trip byte for byte.
        for s in [
            "cmd+shift+o",
            "cmd+ctrl+f",
            "ctrl+space",
            "tab",
            "shift+left",
            "cmd+k>cmd+t",
            "opt+escape",
            "f12",
        ] {
            let t: Trigger = s.parse().unwrap();
            assert_eq!(t.to_string(), s, "round trip of {s}");
        }

        // Aliases are accepted on the way in and rewritten to the canonical
        // spelling on the way out, so two spellings can never become two
        // different bindings.
        for (alias, canonical) in [
            ("option+escape", "opt+escape"),
            ("opt+e", "opt+e"),
            ("super+t", "cmd+t"),
            ("control+a", "ctrl+a"),
            ("return", "enter"),
            ("esc", "escape"),
            ("pgup", "page_up"),
        ] {
            assert_eq!(alias.parse::<Chord>().unwrap().to_string(), canonical, "alias {alias}");
        }
    }

    #[test]
    fn chord_normalisation_folds_case_but_keeps_shift_meaningful() {
        let a: Chord = "cmd+shift+t".parse().unwrap();
        let b: Chord = "cmd+shift+T".parse().unwrap();
        assert_eq!(a.normalized(), b.normalized());

        let plain: Chord = "cmd+t".parse().unwrap();
        assert_ne!(plain.normalized(), a.normalized());

        // Caps/Num Lock never take part in matching.
        let mut noisy = a;
        noisy.mods |= Modifiers::CAPS_LOCK | Modifiers::NUM_LOCK;
        assert_eq!(noisy.normalized(), a.normalized());

        // Modifier order in the source text is irrelevant.
        assert_eq!(
            "ctrl+alt+delete".parse::<Chord>().unwrap().normalized(),
            "alt+ctrl+delete".parse::<Chord>().unwrap().normalized()
        );
    }

    #[test]
    fn sequences_split_on_the_arrow_marker() {
        let t: Trigger = "cmd+k>cmd+t".parse().unwrap();
        assert_eq!(t.len(), 2);
        assert!(t.is_sequence());
        assert_eq!(t.to_string(), "cmd+k>cmd+t");
    }

    #[test]
    fn rejects_modifier_only_and_unknown_keys() {
        assert_eq!("ctrl+alt".parse::<Chord>(), Err(ChordParseError::NoKey));
        assert!(matches!(
            "ctrl+notakey".parse::<Chord>(),
            Err(ChordParseError::UnknownKey(_))
        ));
    }

    #[test]
    fn kitty_modifier_offset_is_applied() {
        assert_eq!(Modifiers::default().kitty_bits(), 1);
        assert_eq!((Modifiers::SHIFT | Modifiers::CTRL).kitty_bits(), 1 + 1 + 4);
    }

    #[test]
    fn keypad_and_fkeys_have_stable_names() {
        assert_eq!(NamedKey::F(12).name(), "f12");
        assert_eq!(NamedKey::parse("F12"), Some(NamedKey::F(12)));
        assert_eq!(NamedKey::parse("f25"), None);
        assert_eq!(NamedKey::parse("pgup"), Some(NamedKey::PageUp));
    }
}

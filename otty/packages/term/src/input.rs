//! Turning key events into the bytes a program expects.
//!
//! Two encodings coexist:
//!
//! * the **legacy** encoding — what a VT program has understood for forty years,
//!   with `DECCKM` application-cursor mode, `DECKPAM` keypad mode and the
//!   `modifyOtherKeys`-style `CSI 1 ; <mod> <final>` forms xterm added;
//! * the **Kitty keyboard protocol** — an explicit, unambiguous encoding that
//!   reports press/repeat/release, the base-layout key, and the modifier mask
//!   (`kitty-keyboard`).
//!
//! A terminal has to pick one per keypress: Kitty when the running program
//! enabled it, legacy otherwise.

use otty_core::key::KeypadKey;
use otty_core::{Chord, Key, KeyEvent, KeyEventKind, NamedKey, Modifiers};

use crate::screen::{Charset, MouseTracking, Terminal};

/// Which encoding to use for a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyEncodeOptions {
    /// Application cursor keys (`DECCKM`).
    pub application_cursor: bool,
    /// Application keypad (`DECKPAM`).
    pub application_keypad: bool,
    /// Kitty keyboard flags, when the program enabled the protocol. Zero means
    /// "not enabled".
    pub kitty_flags: u16,
    /// `macos-option-as-alt` — whether Option produces an escape prefix.
    pub option_as_alt: bool,
    /// The Numpad is in numeric mode, so digits send their literal characters.
    pub new_line_mode: bool,
    /// The active charset, for the bytes a key produces in DEC graphics mode.
    pub charset: Charset,
}

impl KeyEncodeOptions {
    /// Read the options the terminal currently implies.
    pub fn for_terminal(term: &Terminal) -> Self {
        Self {
            application_cursor: term.modes.get(crate::screen::Modes::DECCKM),
            application_keypad: term.keypad_application,
            kitty_flags: term.kitty_flags(),
            option_as_alt: false,
            new_line_mode: false,
            charset: Charset::Ascii,
        }
    }
}

/// Kitty's `report event types` flag.
pub const KITTY_REPORT_EVENT_TYPES: u16 = 1 << 1;
/// Kitty's `report alternate keys` flag.
pub const KITTY_REPORT_ALTERNATE_KEYS: u16 = 1 << 2;
/// Kitty's `report all keys as escape codes` flag.
pub const KITTY_REPORT_ALL_KEYS: u16 = 1 << 3;
/// Kitty's `report associated text` flag.
pub const KITTY_REPORT_ASSOCIATED_TEXT: u16 = 1 << 4;

/// Encode one key event, or return `None` when the key produces nothing.
///
/// A keypress in the Kitty protocol is a `CSI <unicode> ; <mods> [ ; <text> ] u`
/// sequence; `key.release` and `key.repeat` append `:2` / `:3` to the modifier
/// field when the program asked for event types.
pub fn encode_key(event: KeyEvent, opts: &KeyEncodeOptions) -> Option<Vec<u8>> {
    if opts.kitty_flags != 0 {
        if let Some(bytes) = encode_kitty(event, opts) {
            return Some(bytes);
        }
    }
    encode_legacy(event, opts)
}

fn encode_kitty(event: KeyEvent, opts: &KeyEncodeOptions) -> Option<Vec<u8>> {
    let mods = event.mods.kitty_bits();
    let mut params = Vec::new();

    // Keys the legacy encoding already gives an unambiguous code are reported
    // with their functional number; printable keys use their codepoint. A key
    // the map names but that is really a character — `comma`, Space — counts as
    // printable too.
    let printable = match event.key {
        Key::Char(c) => Some(c as u32),
        Key::Named(n) => named_char(n).map(|c| c as u32),
    };
    // `report all keys as escape codes` is what makes a plain `a` an escape
    // code; without it, an unmodified printable is sent as text.
    if printable.is_some()
        && opts.kitty_flags & KITTY_REPORT_ALL_KEYS == 0
        && event.mods.chord_bits().is_empty()
    {
        return None;
    }
    let code: u32 = match printable {
        Some(code) => code,
        None => match event.key {
            Key::Named(n) => kitty_functional_code(n)?,
            Key::Char(c) => c as u32,
        },
    };

    let event_suffix = match event.kind {
        KeyEventKind::Press => "",
        KeyEventKind::Repeat if opts.kitty_flags & KITTY_REPORT_EVENT_TYPES != 0 => ":2",
        KeyEventKind::Release if opts.kitty_flags & KITTY_REPORT_EVENT_TYPES != 0 => ":3",
        _ => return None,
    };
    // The protocol's modifier field already carries Kitty's +1, so the mask
    // `kitty_bits` produced is what goes on the wire: 1 means "no modifiers".
    params.push(code.to_string());
    params.push(format!("{mods}{event_suffix}"));
    if opts.kitty_flags & KITTY_REPORT_ALTERNATE_KEYS != 0 {
        if let Some(base) = event.base_layout {
            params.push(format!("{}", base as u32));
        }
    }
    let body = params.join(";");
    Some(format!("\x1b[{body}u").into_bytes())
}

/// The Kitty functional key numbers for the named keys.
fn kitty_functional_code(key: NamedKey) -> Option<u32> {
    // A key that is really a character — the punctuation keys, Space — travels
    // as its codepoint.
    if let Some(c) = named_char(key) {
        return Some(c as u32);
    }
    Some(match key {
        NamedKey::Escape => 27,
        NamedKey::Enter => 13,
        NamedKey::Tab => 9,
        NamedKey::Backspace => 127,
        NamedKey::Delete => 57_441,
        NamedKey::Insert => 57_345,
        NamedKey::Home => 57_344,
        NamedKey::End => 57_346,
        NamedKey::PageUp => 57_347,
        NamedKey::PageDown => 57_348,
        NamedKey::ArrowUp => 57_352,
        NamedKey::ArrowDown => 57_353,
        NamedKey::ArrowLeft => 57_350,
        NamedKey::ArrowRight => 57_351,
        NamedKey::F(n) => 57_376 + (n.saturating_sub(1) as u32) * 2,
        NamedKey::Keypad(k) => 57_398 + keypad_offset(k),
        NamedKey::PrintScreen => 57_349,
        NamedKey::ScrollLock => 57_354,
        NamedKey::Pause => 57_355,
        NamedKey::Menu => 57_356,
        NamedKey::Media(_) | NamedKey::Unidentified => return None,
        // Everything else is a character key, handled above.
        _ => return None,
    })
}

/// A key the map names for layout independence but that is really a character.
///
/// These have to travel as their codepoint: `comma` is `,` on the wire, both as
/// text and under the Kitty protocol.
fn named_char(key: NamedKey) -> Option<char> {
    Some(match key {
        NamedKey::Space => ' ',
        NamedKey::Comma => ',',
        NamedKey::Period => '.',
        NamedKey::Slash => '/',
        NamedKey::Backslash => '\\',
        NamedKey::Semicolon => ';',
        NamedKey::Quote => '\'',
        NamedKey::Grave => '`',
        NamedKey::Minus => '-',
        NamedKey::Equal => '=',
        NamedKey::LeftBracket => '[',
        NamedKey::RightBracket => ']',
        _ => return None,
    })
}

/// The Kitty functional number offsets for the numeric keypad, which occupy the
/// block after the function keys.
fn keypad_offset(k: KeypadKey) -> u32 {
    use KeypadKey::*;
    match k {
        Digit(0) => 57_399,
        Digit(1) => 57_400,
        Digit(2) => 57_401,
        Digit(3) => 57_402,
        Digit(4) => 57_403,
        Digit(5) => 57_404,
        Digit(6) => 57_405,
        Digit(7) => 57_406,
        Digit(8) => 57_407,
        Digit(9) => 57_408,
        Decimal => 57_409,
        Divide => 57_410,
        Multiply => 57_411,
        Subtract => 57_412,
        Add => 57_413,
        Enter => 57_414,
        Equal => 57_415,
        Separator => 57_416,
        ArrowLeft => 57_417,
        ArrowRight => 57_418,
        ArrowUp => 57_419,
        ArrowDown => 57_420,
        PageUp => 57_421,
        PageDown => 57_422,
        Home => 57_423,
        End => 57_424,
        Insert => 57_425,
        Delete => 57_426,
        Begin => 57_427,
        _ => 0,
    }
}

fn encode_legacy(event: KeyEvent, opts: &KeyEncodeOptions) -> Option<Vec<u8>> {
    // Kitty's modifier encoding for the xterm-style forms: 1 + the bitmask.
    let m = event.mods.kitty_bits();
    let mods_empty = event.mods.chord_bits().is_empty();
    // A key the map names but that is really a character — Space and the
    // punctuation keys — takes the character path below.
    let key = match event.key {
        Key::Named(n) => named_char(n).map_or(event.key, Key::Char),
        other => other,
    };

    match key {
        Key::Char(c) => {
            // `macos-option-as-alt` prefixes an escape here, and only here: a
            // key that carries its modifiers inside its own sequence (an arrow,
            // a function key) must not collect a second, stray escape.
            let mut out = if opts.option_as_alt && event.mods.contains(Modifiers::ALT) {
                b"\x1b".to_vec()
            } else {
                Vec::new()
            };
            // A Ctrl chord with a letter becomes the matching control byte.
            if event.mods.contains(Modifiers::CTRL) && !event.mods.contains(Modifiers::SUPER) {
                if let Some(b) = control_byte(c) {
                    out.push(b);
                    return Some(out);
                }
            }
            if event.mods.contains(Modifiers::SUPER) {
                // The window manager owns Cmd; nothing reaches the pty.
                return None;
            }
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            Some(out)
        }
        Key::Named(named) => {
            if mods_empty {
                match named {
                    NamedKey::Enter => return Some(vec![0x0d]),
                    NamedKey::Tab => return Some(vec![0x09]),
                    NamedKey::Backspace => return Some(vec![0x7f]),
                    NamedKey::Escape => return Some(vec![0x1b]),
                    NamedKey::ArrowUp => {
                        return Some(cursor_key(b'A', opts.application_cursor));
                    }
                    NamedKey::ArrowDown => {
                        return Some(cursor_key(b'B', opts.application_cursor));
                    }
                    NamedKey::ArrowRight => {
                        return Some(cursor_key(b'C', opts.application_cursor));
                    }
                    NamedKey::ArrowLeft => {
                        return Some(cursor_key(b'D', opts.application_cursor));
                    }
                    NamedKey::Home => return Some(cursor_key(b'H', opts.application_cursor)),
                    NamedKey::End => return Some(cursor_key(b'F', opts.application_cursor)),
                    NamedKey::Insert => return Some(b"\x1b[2~".to_vec()),
                    NamedKey::Delete => return Some(b"\x1b[3~".to_vec()),
                    NamedKey::PageUp => return Some(b"\x1b[5~".to_vec()),
                    NamedKey::PageDown => return Some(b"\x1b[6~".to_vec()),
                    NamedKey::F(n) => return legacy_function_key(n, m),
                    NamedKey::Keypad(k) => return keypad_key(k, opts),
                    _ => {}
                }
            }
            let final_byte = match named {
                NamedKey::ArrowUp => b'A',
                NamedKey::ArrowDown => b'B',
                NamedKey::ArrowRight => b'C',
                NamedKey::ArrowLeft => b'D',
                NamedKey::Home => b'H',
                NamedKey::End => b'F',
                NamedKey::Insert => b'2',
                NamedKey::Delete => b'3',
                NamedKey::PageUp => b'5',
                NamedKey::PageDown => b'6',
                NamedKey::F(n) => return legacy_function_key(n, m),
                _ => return None,
            };
            let tilde = matches!(final_byte, b'2' | b'3' | b'5' | b'6');
            let mut out = Vec::new();
            if tilde {
                out.extend_from_slice(format!("\x1b[{};{m}~", final_byte as char).as_bytes());
            } else {
                out.extend_from_slice(format!("\x1b[1;{m}{}", final_byte as char).as_bytes());
            }
            Some(out)
        }
    }
}

fn cursor_key(final_byte: u8, application: bool) -> Vec<u8> {
    if application {
        vec![0x1b, b'O', final_byte]
    } else {
        vec![0x1b, b'[', final_byte]
    }
}

/// F1–F4 have their own codes; F5 onwards use the `~` family.
///
/// `mods` is the Kitty modifier mask (1 + the modifier bits), which is the form
/// both xterm's `modifyOtherKeys` and the `CSI 1 ; <mod> <final>` family use.
fn legacy_function_key(n: u8, mods: u32) -> Option<Vec<u8>> {
    let base: &[u8] = match n {
        1 => b"\x1bOP",
        2 => b"\x1bOQ",
        3 => b"\x1bOR",
        4 => b"\x1bOS",
        5 => b"\x1b[15~",
        6 => b"\x1b[17~",
        7 => b"\x1b[18~",
        8 => b"\x1b[19~",
        9 => b"\x1b[20~",
        10 => b"\x1b[21~",
        11 => b"\x1b[23~",
        12 => b"\x1b[24~",
        13 => b"\x1b[25~",
        14 => b"\x1b[26~",
        15 => b"\x1b[28~",
        16 => b"\x1b[29~",
        17 => b"\x1b[31~",
        18 => b"\x1b[32~",
        19 => b"\x1b[33~",
        20 => b"\x1b[34~",
        _ => return None,
    };
    if mods <= 1 {
        return Some(base.to_vec());
    }
    // The `; <mod>` form drops the leading `ESC` and rewrites the tail.
    let text = String::from_utf8_lossy(base);
    let body = text.trim_start_matches('\x1b');
    if let Some(rest) = body.strip_prefix('[') {
        if let Some(num) = rest.strip_suffix('~') {
            return Some(format!("\x1b[{num};{mods}~").into_bytes());
        }
    }
    if let Some(letter) = body.strip_prefix('O') {
        return Some(format!("\x1b[1;{mods}{letter}").into_bytes());
    }
    Some(base.to_vec())
}

/// The bytes a numeric-keypad key sends.
///
/// `DECKPAM` puts the keypad in application mode, where every key has its own
/// SS3 code — `ESC O p` … `ESC O y` for the digits, per the DECKPAM page's
/// "now keypad '5' sends \eOu instead of '5'". In the default numeric mode the
/// keys send the characters printed on them, which is what a shell expects.
fn keypad_key(k: KeypadKey, opts: &KeyEncodeOptions) -> Option<Vec<u8>> {
    use KeypadKey::*;
    // The navigation half of the keypad behaves like the main block's keys.
    let nav = match k {
        Home => Some(NamedKey::Home),
        End => Some(NamedKey::End),
        PageUp => Some(NamedKey::PageUp),
        PageDown => Some(NamedKey::PageDown),
        Insert => Some(NamedKey::Insert),
        Delete => Some(NamedKey::Delete),
        ArrowUp => Some(NamedKey::ArrowUp),
        ArrowDown => Some(NamedKey::ArrowDown),
        ArrowLeft => Some(NamedKey::ArrowLeft),
        ArrowRight => Some(NamedKey::ArrowRight),
        _ => None,
    };
    if let Some(nav) = nav {
        return encode_legacy(KeyEvent::press(Key::Named(nav), Modifiers::default()), opts);
    }
    if opts.application_keypad {
        let final_byte = match k {
            Digit(n) if n < 10 => b'p' + n,
            Decimal => b'n',
            Multiply => b'j',
            Add => b'k',
            Separator => b'l',
            Subtract => b'm',
            Divide => b'o',
            Enter => b'M',
            Equal => b'X',
            Begin => b'E',
            _ => return None,
        };
        return Some(vec![0x1b, b'O', final_byte]);
    }
    Some(vec![match k {
        Digit(n) if n < 10 => b'0' + n,
        Decimal => b'.',
        Add => b'+',
        Subtract => b'-',
        Multiply => b'*',
        Divide => b'/',
        Equal => b'=',
        Separator => b',',
        Enter | Begin => 0x0d,
        _ => return None,
    }])
}

/// `Ctrl+<letter>` → the C0 byte.
fn control_byte(c: char) -> Option<u8> {
    let lower = c.to_ascii_lowercase();
    if lower.is_ascii_lowercase() {
        return Some(lower as u8 - b'a' + 1);
    }
    Some(match c {
        ' ' | '@' => 0x00,
        '[' => 0x1b,
        '\\' => 0x1c,
        ']' => 0x1d,
        '^' => 0x1e,
        '_' => 0x1f,
        '?' => 0x7f,
        _ => return None,
    })
}

/// Wrap pasted text for `DECSET 2004`.
///
/// With bracketed paste on, the payload is fenced with `CSI 200~` / `CSI 201~`
/// so a shell can tell typed input from pasted input — which is what stops a
/// pasted newline from running a command by itself.
pub fn encode_paste(data: &[u8], bracketed: bool) -> Vec<u8> {
    if !bracketed {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len() + 12);
    out.extend_from_slice(b"\x1b[200~");
    // A payload containing the terminator would end the paste early; the
    // documented defence is to strip the introducer from the payload.
    let mut i = 0;
    while i < data.len() {
        if data[i..].starts_with(b"\x1b[201~") {
            i += 6;
            continue;
        }
        out.push(data[i]);
        i += 1;
    }
    out.extend_from_slice(b"\x1b[201~");
    out
}

/// Encode a mouse event in the active reporting flavour.
///
/// Returns `None` when reporting is off, so the caller falls back to local
/// selection.
pub fn encode_mouse(
    tracking: MouseTracking,
    sgr: bool,
    button: u8,
    col: u16,
    row: u16,
    pressed: bool,
    motion: bool,
    mods: Modifiers,
) -> Option<Vec<u8>> {
    if tracking == MouseTracking::Off {
        return None;
    }
    let mut code = button;
    if motion {
        code += 32;
    }
    if mods.contains(Modifiers::SHIFT) {
        code += 4;
    }
    if mods.contains(Modifiers::ALT) {
        code += 8;
    }
    if mods.contains(Modifiers::CTRL) {
        code += 16;
    }
    if sgr {
        let final_byte = if pressed { 'M' } else { 'm' };
        return Some(format!("\x1b[<{code};{};{}{final_byte}", col + 1, row + 1).into_bytes());
    }
    // The legacy form can only reach column 223.
    if col > 222 || row > 222 {
        return None;
    }
    Some(vec![
        0x1b,
        b'[',
        b'M',
        32 + code,
        33 + col as u8,
        33 + row as u8,
    ])
}

/// The chord a key event represents, for keybinding lookup.
pub fn chord_of(event: KeyEvent) -> Chord {
    Chord::new(event.key, event.mods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use otty_core::{Chord, Modifiers};

    fn opts() -> KeyEncodeOptions {
        KeyEncodeOptions::default()
    }

    fn press(chord: &str) -> KeyEvent {
        let c: Chord = chord.parse().unwrap();
        KeyEvent::press(c.key, c.mods)
    }

    #[test]
    fn plain_text_and_control_bytes() {
        assert_eq!(encode_key(press("a"), &opts()).unwrap(), b"a");
        assert_eq!(encode_key(press("ctrl+c"), &opts()).unwrap(), vec![0x03]);
        assert_eq!(encode_key(press("ctrl+d"), &opts()).unwrap(), vec![0x04]);
        assert_eq!(encode_key(press("ctrl+space"), &opts()).unwrap(), vec![0x00]);
        assert_eq!(encode_key(press("enter"), &opts()).unwrap(), vec![0x0d]);
        assert_eq!(encode_key(press("tab"), &opts()).unwrap(), vec![0x09]);
        assert_eq!(encode_key(press("backspace"), &opts()).unwrap(), vec![0x7f]);
        assert_eq!(encode_key(press("escape"), &opts()).unwrap(), vec![0x1b]);
    }

    #[test]
    fn cmd_never_reaches_the_pty() {
        assert_eq!(encode_key(press("cmd+c"), &opts()), None);
    }

    #[test]
    fn arrows_follow_application_cursor_mode() {
        assert_eq!(encode_key(press("up"), &opts()).unwrap(), b"\x1b[A");
        let app = KeyEncodeOptions { application_cursor: true, ..opts() };
        assert_eq!(encode_key(press("up"), &app).unwrap(), b"\x1bOA");
    }

    #[test]
    fn modified_arrows_use_the_xterm_form() {
        assert_eq!(encode_key(press("shift+up"), &opts()).unwrap(), b"\x1b[1;2A");
        assert_eq!(encode_key(press("ctrl+left"), &opts()).unwrap(), b"\x1b[1;5D");
        assert_eq!(encode_key(press("shift+delete"), &opts()).unwrap(), b"\x1b[3;2~");
    }

    #[test]
    fn function_keys_cover_both_families() {
        assert_eq!(encode_key(press("f1"), &opts()).unwrap(), b"\x1bOP");
        assert_eq!(encode_key(press("f5"), &opts()).unwrap(), b"\x1b[15~");
        assert_eq!(encode_key(press("shift+f5"), &opts()).unwrap(), b"\x1b[15;2~");
        assert_eq!(encode_key(press("ctrl+f1"), &opts()).unwrap(), b"\x1b[1;5P");
    }

    #[test]
    fn option_as_alt_prefixes_an_escape() {
        let o = KeyEncodeOptions { option_as_alt: true, ..opts() };
        assert_eq!(encode_key(press("opt+b"), &o).unwrap(), b"\x1bb");
        // With the setting off, Alt still prefixes — the byte is what a shell
        // reads as Meta regardless.
        let o = KeyEncodeOptions { option_as_alt: true, ..opts() };
        assert_eq!(encode_key(press("opt+left"), &o).unwrap(), b"\x1b[1;3D");
    }

    #[test]
    fn kitty_protocol_reports_modifiers_and_event_types() {
        let o = KeyEncodeOptions { kitty_flags: KITTY_REPORT_EVENT_TYPES, ..opts() };
        // A plain letter is still text while `report all keys` is off.
        assert_eq!(encode_key(press("a"), &o).unwrap(), b"a");
        // A named key is always an escape code under Kitty.
        assert_eq!(encode_key(press("up"), &o).unwrap(), b"\x1b[57352;1u");
        assert_eq!(encode_key(press("ctrl+up"), &o).unwrap(), b"\x1b[57352;5u");
        let release = KeyEvent {
            kind: KeyEventKind::Release,
            ..press("up")
        };
        assert_eq!(encode_key(release, &o).unwrap(), b"\x1b[57352;1:3u");

        let all = KeyEncodeOptions { kitty_flags: KITTY_REPORT_ALL_KEYS, ..opts() };
        assert_eq!(encode_key(press("a"), &all).unwrap(), b"\x1b[97;1u");
    }

    #[test]
    fn a_kitty_program_still_gets_text_for_unmodified_printables() {
        let o = KeyEncodeOptions { kitty_flags: KITTY_REPORT_EVENT_TYPES, ..opts() };
        assert_eq!(encode_key(press("7"), &o).unwrap(), b"7");
        assert_eq!(encode_key(press("comma"), &o).unwrap(), b",");
    }

    #[test]
    fn paste_is_fenced_only_when_bracketed_paste_is_on() {
        assert_eq!(encode_paste(b"ls", false), b"ls");
        assert_eq!(encode_paste(b"ls", true), b"\x1b[200~ls\x1b[201~");
        // A payload carrying the terminator cannot end the paste early.
        assert_eq!(
            encode_paste(b"a\x1b[201~b", true),
            b"\x1b[200~ab\x1b[201~"
        );
    }

    #[test]
    fn mouse_events_use_sgr_when_asked() {
        assert_eq!(encode_mouse(MouseTracking::Off, true, 0, 0, 0, true, false, Modifiers::default()), None);
        assert_eq!(
            encode_mouse(MouseTracking::Click, true, 0, 3, 4, true, false, Modifiers::default()).unwrap(),
            b"\x1b[<0;4;5M"
        );
        assert_eq!(
            encode_mouse(MouseTracking::Click, true, 0, 3, 4, false, false, Modifiers::default()).unwrap(),
            b"\x1b[<0;4;5m"
        );
        // The legacy form is offset by 32 and 33.
        assert_eq!(
            encode_mouse(MouseTracking::Click, false, 0, 0, 0, true, false, Modifiers::default()).unwrap(),
            vec![0x1b, b'[', b'M', 32, 33, 33]
        );
        // …and gives up beyond column 222 rather than lying.
        assert_eq!(
            encode_mouse(MouseTracking::Click, false, 0, 300, 0, true, false, Modifiers::default()),
            None
        );
    }

    #[test]
    fn mouse_modifiers_are_encoded() {
        let m = Modifiers::SHIFT | Modifiers::CTRL;
        assert_eq!(
            encode_mouse(MouseTracking::Click, true, 0, 0, 0, true, false, m).unwrap(),
            b"\x1b[<20;1;1M"
        );
    }
}

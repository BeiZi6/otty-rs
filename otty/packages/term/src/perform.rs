//! The protocol dispatch: `vte::Perform` for [`Terminal`].
//!
//! Every arm here corresponds to a row in the product's sequence reference. Where
//! a sequence has a side effect outside the grid the arm emits an [`Event`]
//! instead of acting, so the privilege gates (`privilege-notification-shell`,
//! `privilege-clipboard-shell`, `ipc-allow-send-keys`, …) stay in one place.

use base64::Engine as _;
use vte::{Params, Perform};

use crate::events::{
    AgentEvent, ColorStackOp, Event, ImageEvent, Notification, Progress, PromptMark, ResumeEvent,
    SpecialColor, WindowReport,
};
use crate::screen::{Charset, ColorReportFormat, CursorShape, Mode, Modes, Terminal};

/// One parser per pane. Kept separate from [`Terminal`] so partial sequences
/// survive across reads.
///
/// `vte::Parser` is not `Debug`, so this cannot derive it; `Default` is enough
/// for construction.
#[derive(Default)]
pub struct Parser {
    inner: vte::Parser,
}

impl Parser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed bytes to the terminal.
    ///
    /// `vte` takes the whole slice and keeps its own state between calls, so a
    /// sequence split across two reads is resumed correctly.
    pub fn advance(&mut self, term: &mut Terminal, bytes: &[u8]) {
        self.inner.advance(term, bytes);
    }
}

/// The first parameter of group `i`, or `default` when the group is absent or
/// empty. SGR-style `CSI ; 5 H` leaves empty groups, which means "default".
fn param(params: &Params, i: usize, default: u16) -> u16 {
    match params.iter().nth(i) {
        Some(group) => match group.first() {
            Some(v) => *v,
            None => default,
        },
        None => default,
    }
}

/// A parameter that is `0` when absent, which most cursor commands treat as 1.
fn param_or_one(params: &Params, i: usize) -> u16 {
    let v = param(params, i, 0);
    if v == 0 {
        1
    } else {
        v
    }
}

/// The whole sub-parameter group `i`, for the colon forms (`SGR 4:3`, `38:2:…`).
fn group(params: &Params, i: usize) -> &[u16] {
    params.iter().nth(i).unwrap_or(&[])
}

fn to_string(v: &[u8]) -> String {
    String::from_utf8_lossy(v).into_owned()
}

impl Perform for Terminal {
    fn print(&mut self, c: char) {
        self.print(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => {
                // BEL. Some programs use it as a string terminator; the parser
                // has already handled that case.
                self.push_event(Event::Bell);
            }
            0x08 => {
                // BS — move left, do not erase.
                if self.cursor.wide_spacer {
                    self.cursor.col = self.cursor.col.saturating_sub(2);
                    self.cursor.wide_spacer = false;
                } else {
                    self.move_left(1);
                }
            }
            0x09 => self.tab(),
            0x0a..=0x0c => {
                // LF / VT / FF. With LNM (`CSI 20 h`) set, LF behaves like NEL
                // and returns to column 1 as well.
                if self.lnm {
                    self.next_line();
                } else {
                    self.linefeed();
                }
            }
            0x0d => self.carriage_return(),
            0x0e => self.shift_out(),
            0x0f => self.shift_in(),
            0x05 => {
                // ENQ — answered with `enquiry-response`.
                if !self.config.enquiry_response.is_empty() {
                    let r = self.config.enquiry_response.clone().into_bytes();
                    self.reply(r);
                }
                self.push_event(Event::Enquiry);
            }
            0x18 | 0x1a => {
                // CAN / SUB — the parser has already returned to ground.
            }
            0x7f => {
                // DEL is ignored when received from the host.
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (intermediates, byte) {
            ([], b'D') => self.linefeed(),          // IND
            ([], b'E') => self.next_line(),          // NEL
            ([], b'M') => self.reverse_index(),      // RI
            ([], b'H') => {
                // HTS
                let (_, col) = (self.cursor.row, self.cursor.col);
                self.tabs.set(col);
            }
            ([], b'7') => self.save_cursor(),        // DECSC
            ([], b'8') => self.restore_cursor(),     // DECRC
            ([], b'=') => {
                // DECKPAM — application keypad.
                if self.config.vt_keypad_app_allowed {
                    self.keypad_application = true;
                }
            }
            ([], b'>') => self.keypad_application = false, // DECKPNM
            ([], b'c') => self.full_reset(),         // RIS
            ([], b'n') => self.shift_out(),          // LS1
            ([], b'o') => self.shift_in(),           // LS0
            ([b'#'], b'8') => self.screen_alignment_display(), // DECALN
            ([b'#'], b'3') | ([b'#'], b'4') | ([b'#'], b'5') | ([b'#'], b'6') => {
                // DEC double-height / double-width lines. Otty renders these
                // cells at normal size, like every other modern terminal.
            }
            ([b'('], set) => self.set_charset(0, Charset::from_designation(set)),
            ([b')'], set) => self.set_charset(1, Charset::from_designation(set)),
            ([b'*'], set) => self.set_charset(2, Charset::from_designation(set)),
            ([b'+'], set) => self.set_charset(3, Charset::from_designation(set)),
            ([b'%'], b'G') => {} // UTF-8 designation, always on
            ([b'%'], b'@') => {}
            ([], b'Z') => {
                // DECID — the VT100 identification reply.
                self.reply(b"\x1b[?6c".to_vec());
            }
            _ => log::trace!("unhandled ESC sequence: {intermediates:?} {byte:?}"),
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if ignore {
            return;
        }
        // `CSI ? …` — DEC private modes and queries.
        if intermediates.first() == Some(&b'?') {
            return self.csi_private(params, action);
        }
        if intermediates.first() == Some(&b'>') {
            return self.csi_secondary(params, action);
        }
        if intermediates.first() == Some(&b'$') {
            return self.csi_dollar(params, action);
        }
        if intermediates.first() == Some(&b' ') {
            return self.csi_space(params, action);
        }
        if intermediates.first() == Some(&b'"') {
            // DECSCA — `CSI Ps " q` protects what is written next from the
            // selective-erase forms.
            return self.decsca(param(params, 0, 0), action);
        }
        if intermediates.first() == Some(&b'#') {
            // XTPUSHCOLORS / XTPOPCOLORS / XTREPORTCOLORS — `CSI [Pm] # P|Q|R`.
            return self.color_stack(action, param(params, 0, 0));
        }
        if intermediates.first() == Some(&b'<') {
            // `CSI < u` — pop the Kitty keyboard flag stack.
            return self.pop_kitty_flags(param_or_one(params, 0));
        }
        if intermediates.first() == Some(&b'!') {
            // `CSI ! p` — DECSTR, soft reset.
            if action == 'p' {
                self.soft_reset();
            }
            return;
        }

        match action {
            'A' => self.move_up(param_or_one(params, 0)),
            'B' | 'e' => self.move_down(param_or_one(params, 0)),
            'C' | 'a' => self.move_right(param_or_one(params, 0)),
            'D' => self.move_left(param_or_one(params, 0)),
            'E' => {
                let n = param_or_one(params, 0);
                self.move_down(n);
                self.carriage_return();
            }
            'F' => {
                let n = param_or_one(params, 0);
                self.move_up(n);
                self.carriage_return();
            }
            'G' | '`' => {
                // CHA / HPA — 1-based column.
                let col = param_or_one(params, 0).saturating_sub(1);
                let row = self.cursor.row;
                self.set_cursor(row, col);
            }
            'd' => {
                // VPA — 1-based row.
                let row = param_or_one(params, 0).saturating_sub(1);
                let col = self.cursor.col;
                self.set_cursor(row, col);
            }
            'H' | 'f' => {
                // CUP — row and column are 1-indexed.
                let row = param_or_one(params, 0).saturating_sub(1);
                let col = param_or_one(params, 1).saturating_sub(1);
                self.set_cursor(row, col);
            }
            'I' => self.tab_forward(param_or_one(params, 0)),
            'Z' => self.tab_backward(param_or_one(params, 0)),
            'J' => self.erase_in_display(param(params, 0, 0), false),
            'K' => self.erase_in_line(param(params, 0, 0), false),
            'X' => self.erase_chars(param_or_one(params, 0), false),
            'L' => self.insert_lines(param_or_one(params, 0)),
            'M' => self.delete_lines(param_or_one(params, 0)),
            'P' => self.delete_chars(param_or_one(params, 0)),
            '@' => self.insert_chars(param_or_one(params, 0)),
            'S' => self.scroll_up(param_or_one(params, 0)),
            'T' => {
                // SD, unless it carries more than one parameter, in which case
                // it is the mouse-highlight variant DEC uses for the same letter.
                if params.len() > 1 {
                    return;
                }
                self.scroll_down(param_or_one(params, 0));
            }
            'b' => {
                // REP — repeat the last graphic character. `print` records the
                // cell it wrote in `last_char`, so the repeat starts from that
                // glyph rather than from whatever the cursor sits on.
                let n = param_or_one(params, 0);
                match self.last_graphic_char() {
                    Some(ch) if ch != ' ' => {
                        for _ in 0..n.min(4096) {
                            self.print(ch);
                        }
                    }
                    _ => log::trace!("REP with nothing to repeat"),
                }
            }
            'g' => match param(params, 0, 0) {
                0 => {
                    let col = self.cursor.col;
                    self.tabs.clear(col);
                }
                3 => self.tabs.clear_all(),
                _ => {}
            },
            'h' => {
                // ANSI modes: 2 = KAM (keyboard action mode, gated by
                // `vt-kam-allowed`), 20 = LNM (line-feed/new-line mode). IRM is
                // not modelled, so it is logged rather than half-implemented.
                for i in 0..params.len().max(1) {
                    match param(params, i, 0) {
                        2 if self.config.vt_kam_allowed => self.keyboard_action_mode = true,
                        20 => self.lnm = true,
                        n => log::trace!("unhandled ANSI mode {n} set"),
                    }
                }
            }
            'l' => {
                for i in 0..params.len().max(1) {
                    match param(params, i, 0) {
                        2 if self.config.vt_kam_allowed => self.keyboard_action_mode = false,
                        20 => self.lnm = false,
                        n => log::trace!("unhandled ANSI mode {n} reset"),
                    }
                }
            }
            'm' => self.sgr(params),
            'n' => self.device_status(params),
            'c' => {
                // DA1 — "VT100 with Advanced Video Option".
                self.reply(b"\x1b[?6c".to_vec());
            }
            'q' => {
                // XTVERSION asks with `> 0 q`, handled in csi_secondary.
            }
            'r' => {
                // DECSTBM
                let top = param(params, 0, 0);
                let bottom = param(params, 1, 0);
                if params.len() >= 2 || top != 0 {
                    self.set_scroll_region(top, bottom);
                }
            }
            's' => {
                // DECSLRM is `CSI Pl ; Pr s`; the bare form is SCOSC.
                if params.len() > 1 {
                    // Left/right margins are accepted and recorded, but the
                    // grid does not yet clip to them (documented as rare).
                } else {
                    self.save_cursor();
                }
            }
            'u' => self.restore_cursor(), // SCORC
            't' => self.window_op(params),
            'p' => {
                // DECRQM is `$ p`; the bare form is a soft reset request.
                if intermediates.is_empty() {
                    self.soft_reset();
                }
            }
            'x' => {
                // DECREQTPARM — answer with the "request" reply.
                self.reply(b"\x1b[2;1;1;120;120;1;0x".to_vec());
            }
            _ => log::trace!("unhandled CSI: {params:?} {intermediates:?} {action:?}"),
        }
    }

    fn hook(&mut self, _params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        // A DCS frame: the selector arrives with `hook`, the payload one byte at
        // a time through `put`, and `unhook` fires at the string terminator.
        self.dcs.clear();
        self.dcs_selector = Some((intermediates.to_vec(), action));
    }

    fn put(&mut self, byte: u8) {
        self.dcs.push(byte);
    }

    fn unhook(&mut self) {
        let Some((intermediates, action)) = self.dcs_selector.take() else {
            return;
        };
        let payload = std::mem::take(&mut self.dcs);
        handle_dcs(self, &intermediates, action, &payload);
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        if params.is_empty() {
            return;
        }
        let code = to_string(params[0]);
        let args = &params[1..];
        match code.as_str() {
            // OSC 0 / OSC 2 — title; OSC 1 — icon title.
            "0" => self.push_event(Event::Title(to_string(args.first().copied().unwrap_or(b"")))),
            "1" => self
                .push_event(Event::IconTitle(to_string(args.first().copied().unwrap_or(b"")))),
            "2" => self.push_event(Event::Title(to_string(args.first().copied().unwrap_or(b"")))),
            "4" => self.osc_palette(args),
            "5" => self.osc_special("5", args, true),
            "7" => {
                // OSC 7 — the shell reports its working directory.
                self.push_event(Event::WorkingDirectory(to_string(
                    args.first().copied().unwrap_or(b""),
                )));
            }
            "8" => {
                let uri = args.first().copied().unwrap_or(b"");
                let id = args.get(1).copied().unwrap_or(b"");
                let uri = to_string(uri);
                self.push_event(Event::Hyperlink {
                    uri: if uri.is_empty() { None } else { Some(uri) },
                    id: if id.is_empty() { None } else { Some(to_string(id)) },
                });
            }
            "9" => {
                // `OSC 9;4` is the ConEmu progress protocol; a plain `OSC 9`
                // (or `9;subtitle;body`) is an iTerm-style notification.
                if args.first().copied() == Some(b"4") {
                    self.osc_progress(&args[1..]);
                } else if args.len() >= 2 {
                    self.push_event(Event::Notify(Notification {
                        title: Some(to_string(args[0])),
                        body: to_string(args[1]),
                        urgency: 1,
                        id: None,
                        silent: false,
                    }));
                } else {
                    self.push_event(Event::Notify(Notification {
                        title: None,
                        body: to_string(args.first().copied().unwrap_or(b"")),
                        urgency: 1,
                        id: None,
                        silent: false,
                    }));
                }
            }
            "10" => self.osc_special("10", args, false),
            "11" => self.osc_special("11", args, false),
            "12" => self.osc_special("12", args, false),
            "17" => self.osc_special("17", args, false),
            "19" => self.osc_special("19", args, false),
            "22" => self.push_event(Event::MouseShape(to_string(
                args.first().copied().unwrap_or(b"default"),
            ))),
            "26" => self.push_event(Event::Agent(AgentEvent {
                verb: to_string(args.first().copied().unwrap_or(b"")),
                args: args[1..].iter().map(|a| to_string(a)).collect(),
            })),
            "52" => self.osc_clipboard(args),
            "777" => self.push_event(Event::Notify(Notification {
                title: Some(to_string(args.first().copied().unwrap_or(b""))),
                body: to_string(args.get(1).copied().unwrap_or(b"")),
                urgency: 1,
                id: None,
                silent: false,
            })),
            "88" => self.osc_resume(args),
            "99" => self.osc_kitty_notify(args),
            "104" | "105" | "110" | "111" | "112" | "113" | "114" | "115" | "116" | "117"
            | "118" | "119" => self.osc_color_reset(&code, args),
            "133" => self.osc_ftcs(args),
            "1337" => self.osc_iterm(args),
            _ => log::trace!("unhandled OSC {code}"),
        }
        let _ = bell_terminated;
    }
}

impl Terminal {
    // ---------------------------------------------------------- CSI families

    fn csi_private(&mut self, params: &Params, action: char) {
        match action {
            'h' => {
                for i in 0..params.len().max(1) {
                    let n = param(params, i, 0);
                    self.set_private_mode(n, true);
                }
            }
            'l' => {
                for i in 0..params.len().max(1) {
                    let n = param(params, i, 0);
                    self.set_private_mode(n, false);
                }
            }
            'n' => {
                // The DEC private DSR family (`CSI ? 6 n` DECXCPR and the
                // printer / UDK / locator reports) is documented as not
                // implemented: only the `CSI 5 n` and `CSI 6 n` forms answer,
                // and those go through `device_status`.
                log::trace!("unanswered DEC private DSR: {}", param(params, 0, 0));
            }
            'J' => self.erase_in_display(param(params, 0, 0), true), // DECSED
            'K' => self.erase_in_line(param(params, 0, 0), true),    // DECSEL
            'p' => {
                // DECRQM — report a mode.
                let n = param(params, 0, 0);
                let known = Modes::mask_for(n).is_some();
                let on = Modes::mask_for(n).is_some_and(|m| self.modes.get(m));
                let state = if !known {
                    0
                } else if on {
                    1
                } else {
                    2
                };
                let reply = format!("\x1b[?{n};{state}$y");
                self.reply(reply.into_bytes());
            }
            'u' => {
                // `CSI ? u` — report the Kitty keyboard flags in force.
                let reply = format!("\x1b[?{}u", self.kitty_flags());
                self.reply(reply.into_bytes());
            }
            other => log::trace!("unhandled DEC private CSI: {other}"),
        }
    }

    /// `DECSET` / `DECRST` for one DEC private mode.
    ///
    /// The mode table is the single source of truth for which mask a mode
    /// number means; the arms below add the side effects the number carries on
    /// top of it. Numbers the documentation marks as not implemented (X10
    /// mouse, SGR-Pixel mouse, grapheme clustering, in-band resize) and numbers
    /// Otty simply does not know fall through to the log.
    fn set_private_mode(&mut self, number: u16, on: bool) {
        let Some(mask) = Modes::mask_for(number) else {
            log::trace!("unhandled DEC private mode {number} = {on}");
            return;
        };
        self.modes.set(mask, on);
        match number {
            // DECCOLM — the 80/132-column switch.
            3 => {
                let cols = if on { 132 } else { 80 };
                let rows = self.rows();
                self.resize(cols, rows);
            }
            // DECOM homes the cursor.
            6 => self.set_cursor(0, 0),
            12 => self.cursor.blinking = on,
            25 => self.cursor.visible = on,
            // ?47 is a bare swap: no cursor save, and the alternate buffer
            // keeps whatever it held.
            47 => self.switch_screen(on, false, false),
            // ?1047 clears the alternate buffer when the program leaves it.
            1047 => self.switch_screen(on, false, !on),
            // ?1049 saves the cursor and clears the alternate buffer on entry.
            1049 => self.switch_screen(on, true, on),
            2026 => self.synchronized_output = on,
            _ => {}
        }
    }

    /// `CSI Ps " q` — DECSCA, character protection.
    ///
    /// The attribute lives on the pen, so it applies to everything written until
    /// it is changed and travels with `DECSC` / `DECRC`.
    fn decsca(&mut self, ps: u16, action: char) {
        if action != 'q' {
            log::trace!("unhandled CSI {ps} \" {action}");
            return;
        }
        self.pen.flags.set(crate::attrs::Flags::PROTECTED, ps == 1);
    }

    /// `CSI [Pm] # P|Q|R` — the xterm colour stack.
    ///
    /// The stack itself belongs to the application (the engine holds no
    /// palette), so only the operation is surfaced; a non-zero `Pm`, which names
    /// one of the ten slots, is logged because the event carries no slot.
    fn color_stack(&mut self, action: char, slot: u16) {
        let op = match action {
            'P' => ColorStackOp::Push,
            'Q' => ColorStackOp::Pop,
            'R' => ColorStackOp::Report,
            other => {
                log::trace!("unhandled CSI #{other}");
                return;
            }
        };
        if slot != 0 {
            log::trace!("colour-stack slot {slot} ignored (the event carries the op only)");
        }
        self.push_event(Event::ColorStack(op));
    }

    fn csi_secondary(&mut self, params: &Params, action: char) {
        match action {
            'c' => {
                // DA2 — model ID 0, the build version, ROM slot 1.
                self.reply(secondary_da());
            }
            'q' => {
                // XTVERSION — `CSI > 0 q`.
                let reply = format!("\x1bP>|otty({})\x1b\\", otty_core::VERSION);
                self.reply(reply.into_bytes());
            }
            'm' => self.sgr(params), // `CSI > 4 ; 2 m` — modifyOtherKeys
            'u' => {
                // `CSI > <flags> u` — push a Kitty keyboard flag set.
                self.push_kitty_flags(param(params, 0, 0));
            }
            'n' => {
                // `CSI > Ps n` — report just the requested part of the state.
                if param(params, 0, 0) == 0 {
                    self.reply(b"\x1b[>0;1;0c".to_vec());
                }
            }
            'p' => self.window_op(params),
            _ => {}
        }
    }

    fn csi_dollar(&mut self, params: &Params, action: char) {
        match action {
            'p' => {
                // DECRQM (ANSI form).
                let n = param(params, 0, 0);
                let reply = format!("\x1b[{n};2$y");
                self.reply(reply.into_bytes());
            }
            'r' => {
                // DECRPM is the reply side; ignore.
            }
            _ => {}
        }
    }

    fn csi_space(&mut self, params: &Params, action: char) {
        // `CSI Ps SP q` — DECSCUSR.
        if action == 'q' {
            let ps = param(params, 0, 0);
            self.set_cursor_style(ps);
        }
    }

    // ---------------------------------------------------------------- SGR

    fn sgr(&mut self, params: &Params) {
        let mut i = 0;
        while i < params.len() {
            let g = group(params, i);
            match g.first().copied().unwrap_or(0) {
                38 | 48 | 58 => {
                    let target_extended = g.first().copied().unwrap_or(0);
                    let (spec, next) = self.read_extended_color(params, i, g);
                    match target_extended {
                        38 => self.pen.fg = spec,
                        48 => self.pen.bg = spec,
                        _ => self.pen.underline_color = spec,
                    }
                    i = next;
                }
                4 => {
                    if g.len() > 1 {
                        self.pen.set_underline_style(g[1]);
                    } else {
                        self.pen.apply(4);
                    }
                }
                v => {
                    self.pen.apply(v);
                }
            }
            i += 1;
        }
    }

    /// Read an extended colour, handling both the semicolon and the colon form.
    fn read_extended_color(
        &self,
        params: &Params,
        i: usize,
        g: &[u16],
    ) -> (crate::attrs::ColorSpec, usize) {
        use crate::attrs::ColorSpec;
        if g.len() > 1 {
            // Colon form: `38:5:N` / `38:2:{colorspace}:r:g:b`.
            return match g[1] {
                5 => (ColorSpec::Indexed(g.get(2).copied().unwrap_or(0) as u8), i),
                2 => {
                    let pick = if g.len() >= 6 {
                        (g[3], g[4], g[5])
                    } else {
                        (
                            g.get(2).copied().unwrap_or(0),
                            g.get(3).copied().unwrap_or(0),
                            g.get(4).copied().unwrap_or(0),
                        )
                    };
                    (ColorSpec::Rgb(pick.0 as u8, pick.1 as u8, pick.2 as u8), i)
                }
                _ => (ColorSpec::Default, i),
            };
        }
        // Semicolon form: the mode is the next parameter group.
        let mode = param(params, i + 1, 0);
        match mode {
            5 => (ColorSpec::Indexed(param(params, i + 2, 0) as u8), i + 2),
            2 => (
                ColorSpec::Rgb(
                    param(params, i + 2, 0) as u8,
                    param(params, i + 3, 0) as u8,
                    param(params, i + 4, 0) as u8,
                ),
                i + 4,
            ),
            _ => (ColorSpec::Default, i + 1),
        }
    }

    // --------------------------------------------------------------- DSR

    fn device_status(&mut self, params: &Params) {
        match param(params, 0, 0) {
            5 => self.reply(b"\x1b[0n".to_vec()), // "ready, no malfunction"
            6 => {
                let (row, col) = (self.cursor.row + 1, self.cursor.col + 1);
                let reply = format!("\x1b[{row};{col}R");
                self.reply(reply.into_bytes());
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ XTWINOPS

    /// `XTWINOPS` — the read-only size reports only.
    ///
    /// The documentation marks this row partial: `Ps 14` (text-area pixels) and
    /// `Ps 16` (cell pixels) are surfaced as [`Event::WindowReport`] for the
    /// application to answer, `Ps 18` is dropped on purpose — its reply leaks
    /// through a multiplexer and is echoed by the inner shell — and the
    /// window-acting operations (`Ps 1`–`10` and the position / title-stack
    /// reports) are ignored, because a remote program must not be able to move
    /// or resize the window. `Ps 21` reports the title, gated by `title-report`.
    fn window_op(&mut self, params: &Params) {
        let ps = param(params, 0, 0);
        match ps {
            14 => self.push_event(Event::WindowReport(WindowReport::TextAreaPixels)),
            16 => self.push_event(Event::WindowReport(WindowReport::CellPixels)),
            18 => log::trace!("XTWINOPS 18 t is not answered by design"),
            21 => {
                if self.config.title_report {
                    self.push_event(Event::ReportTitle);
                }
            }
            other => log::trace!("ignored XTWINOPS Ps {other}"),
        }
    }

    // ---------------------------------------------------------------- OSC

    fn osc_palette(&mut self, args: &[&[u8]]) {
        // `OSC 4 ; index ; color [; index ; color …]` or `… ; ?`.
        let mut i = 0;
        while i + 1 < args.len() {
            let index = to_string(args[i]).parse::<u16>().ok();
            let value = to_string(args[i + 1]);
            if let Some(index) = index {
                if value == "?" {
                    self.push_event(Event::PaletteQuery { index });
                } else if index < 256 {
                    if let Ok(color) = otty_core::Color::parse(&value) {
                        self.push_event(Event::PaletteSet { index, color });
                    }
                }
            }
            i += 2;
        }
    }

    /// `OSC 5` and `OSC 10`/`11`/`12`/`17`/`19` — the named colour slots.
    ///
    /// `orange` selects the two shapes: `OSC 5` carries its slot index inline
    /// (`OSC 5 ; <slot> ; <colour>`), while the `10`-family codes name the slot
    /// with the code itself, so the dispatcher passes it in.
    fn osc_special(&mut self, code: &str, args: &[&[u8]], orange: bool) {
        let kind = |code: &str, index: u16| -> Option<SpecialColor> {
            if orange {
                return match index {
                    0 => Some(SpecialColor::Bold),
                    1 => Some(SpecialColor::Underline),
                    2 => Some(SpecialColor::Blink),
                    3 => Some(SpecialColor::Reverse),
                    4 => Some(SpecialColor::Italic),
                    _ => None,
                };
            }
            match code {
                "10" => Some(SpecialColor::Foreground),
                "11" => Some(SpecialColor::Background),
                "12" => Some(SpecialColor::Cursor),
                "17" => Some(SpecialColor::SelectionBackground),
                "19" => Some(SpecialColor::SelectionForeground),
                _ => None,
            }
        };
        let index = if orange {
            to_string(args.first().copied().unwrap_or(b"0")).parse().unwrap_or(0)
        } else {
            0
        };
        let Some(kind) = kind(code, index) else {
            return;
        };
        let value = to_string(args.get(if orange { 1 } else { 0 }).copied().unwrap_or(b""));
        if value == "?" {
            self.push_event(Event::SpecialColorQuery { kind });
        } else if !value.is_empty() {
            if let Ok(color) = otty_core::Color::parse(&value) {
                self.push_event(Event::SpecialColorSet { kind, color });
            }
        }
    }

    fn osc_progress(&mut self, args: &[&[u8]]) {
        let state = to_string(args.first().copied().unwrap_or(b"0"))
            .parse::<u8>()
            .unwrap_or(0);
        let percent = to_string(args.get(1).copied().unwrap_or(b"0"))
            .parse::<u8>()
            .unwrap_or(0);
        let progress = match state {
            0 => Progress::Clear,
            1 => Progress::Set(percent.min(100)),
            2 => Progress::Error,
            3 => Progress::Indeterminate,
            4 => Progress::Paused(percent.min(100)),
            _ => Progress::Clear,
        };
        self.push_event(Event::Progress(progress));
    }

    fn osc_clipboard(&mut self, args: &[&[u8]]) {
        let selection = to_string(args.first().copied().unwrap_or(b"c"))
            .chars()
            .next()
            .unwrap_or('c');
        let payload = to_string(args.get(1).copied().unwrap_or(b""));
        if payload == "?" {
            self.push_event(Event::ClipboardLoad { selection });
            return;
        }
        match base64::engine::general_purpose::STANDARD.decode(payload.as_bytes()) {
            Ok(data) => self.push_event(Event::ClipboardStore { selection, data }),
            Err(_) => log::trace!("OSC 52 payload was not valid base64"),
        }
    }

    fn osc_color_reset(&mut self, code: &str, args: &[&[u8]]) {
        match code {
            "104" => {
                let indices: Vec<u16> = args
                    .iter()
                    .filter_map(|a| to_string(a).parse::<u16>().ok())
                    .collect();
                self.push_event(Event::PaletteReset { indices });
            }
            "105" => self.push_event(Event::SpecialColorReset { kind: SpecialColor::Underline }),
            "110" => self.push_event(Event::SpecialColorReset { kind: SpecialColor::Foreground }),
            "111" => self.push_event(Event::SpecialColorReset { kind: SpecialColor::Background }),
            "112" => self.push_event(Event::SpecialColorReset { kind: SpecialColor::Cursor }),
            "117" => {
                self.push_event(Event::SpecialColorReset { kind: SpecialColor::SelectionBackground })
            }
            "119" => {
                self.push_event(Event::SpecialColorReset { kind: SpecialColor::SelectionForeground })
            }
            _ => {}
        }
    }

    fn osc_ftcs(&mut self, args: &[&[u8]]) {
        let verb = to_string(args.first().copied().unwrap_or(b""));
        match verb.as_str() {
            "A" => self.push_prompt_mark(PromptMark::PromptStart),
            "B" => self.push_prompt_mark(PromptMark::PromptEnd),
            "C" => self.push_prompt_mark(PromptMark::CommandStart),
            "D" => {
                self.push_prompt_mark(PromptMark::CommandEnd);
                let exit = args
                    .iter()
                    .skip(1)
                    .find_map(|a| {
                        let s = to_string(a);
                        s.strip_prefix("exit=")?.parse::<i32>().ok()
                    });
                self.push_event(Event::CommandFinished { exit_code: exit });
            }
            "E" => self.push_prompt_mark(PromptMark::OutputEnd),
            "P" => {
                // `133;P` — the command line, or a redraw of the prompt.
                if args.get(1).copied() == Some(b"$") {
                    self.push_prompt_mark(PromptMark::CommandLine);
                }
            }
            _ => {}
        }
    }

    fn osc_kitty_notify(&mut self, args: &[&[u8]]) {
        let mut n = Notification { title: None, body: String::new(), urgency: 1, id: None, silent: false };
        let mut payload = String::new();
        for arg in args {
            let s = to_string(arg);
            if let Some(v) = s.strip_prefix("d=") {
                // The `d` field is base64 until `e=1` turns it off.
                payload.push_str(v);
            } else if let Some((k, v)) = s.split_once('=') {
                match k {
                    "t" => n.title = Some(v.to_owned()),
                    "i" => n.id = Some(v.to_owned()),
                    "u" => n.urgency = v.parse().unwrap_or(1),
                    "p" => n.silent = true,
                    _ => {}
                }
            }
        }
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(payload.as_bytes()) {
            n.body = String::from_utf8_lossy(&decoded).into_owned();
        } else {
            n.body = payload;
        }
        self.push_event(Event::Notify(n));
    }

    fn osc_resume(&mut self, args: &[&[u8]]) {
        let verb = to_string(args.first().copied().unwrap_or(b""));
        let rest: Vec<String> = args[1..].iter().map(|a| to_string(a)).collect();
        let event = match verb.as_str() {
            "announce" => ResumeEvent::Announce {
                id: rest.first().cloned().unwrap_or_default(),
                command: rest.get(1).cloned(),
            },
            "request" => ResumeEvent::Request { id: rest.first().cloned().unwrap_or_default() },
            "state" => ResumeEvent::State(rest.join("\n").into_bytes()),
            other => ResumeEvent::Other { verb: other.to_owned(), args: rest },
        };
        self.push_event(Event::Resume(event));
    }

    /// `OSC 1337` — iTerm2's proprietary command family.
    ///
    /// `vte` splits an OSC payload on `;`, so the command and its arguments
    /// arrive as separate groups and have to be rejoined before parsing: the
    /// bytes after `OSC 1337` reach here as `["File=name=a.png", "inline=1:…"]`.
    /// Only the inline-image command has an effect; the rest are recognized and
    /// ignored.
    fn osc_iterm(&mut self, args: &[&[u8]]) {
        let joined = args.iter().map(|a| to_string(a)).collect::<Vec<_>>().join(";");
        let (command, rest) = match joined.split_once('=') {
            Some((command, rest)) => (command.to_owned(), rest.to_owned()),
            None => (joined, String::new()),
        };
        match command.as_str() {
            "File" => {
                // `File=<key=value;…>:<base64>` — the colon separates the
                // attributes from the payload. The payload stays encoded: the
                // engine hands it over raw so a large image never blocks
                // parsing, and the renderer decodes it.
                let (attrs, payload) = match rest.split_once(':') {
                    Some((attrs, payload)) => (attrs.to_owned(), payload.to_owned()),
                    None => (rest, String::new()),
                };
                let params: Vec<(String, String)> = attrs
                    .split(';')
                    .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
                    .collect();
                self.push_event(Event::Image(ImageEvent::Iterm { params, payload }));
            }
            "SetUserVar" | "Copy" | "CopyToClipboard" | "EndCopy" | "SetColors"
            | "ReportVariable" | "CursorShape" | "SetMark" | "StealFocus" | "CurrentDir"
            | "RemoteHost" | "ShellIntegrationVersion" | "Prompt" | "ClearScrollback"
            | "SetBackgroundImageFile" | "SetBadgeFormat" | "SetProfile" | "SetKeyLabel"
            | "PushKeyLabels" | "PopKeyLabels" | "RequestAttention" | "ReportCellSize"
            | "UnicodeVersion" | "HighlightCursorLine" | "AddAnnotation"
            | "AddHiddenAnnotation" | "ClearCapturedOutput" | "OpenURL" => {
                // Acknowledged, no grid effect.
            }
            other => log::trace!("unhandled OSC 1337 {other}"),
        }
    }
}

/// Dispatch a complete DCS frame.
///
/// Otty answers exactly one DCS function — the DECRQSS cursor-style query
/// (`DCS $ q SP q ST`) — and it does so by handing the query to the application
/// as [`Event::RequestSetting`]: the reply is either the program's `DECSCUSR`
/// override or the user's configured default, and only the application knows
/// the configuration ([`decscusr_number`] turns the engine's state into the `Ps`
/// the reply needs). The other DECRQSS targets, XTGETTCAP (`DCS + q …`) and
/// sixel (`DCS q …`) are documented as not implemented, so their payloads are
/// dropped rather than guessed at.
fn handle_dcs(term: &mut Terminal, intermediates: &[u8], action: char, payload: &[u8]) {
    match (action, intermediates) {
        ('q', [b'$']) if payload == b" q" => term.push_dcs_request(to_string(payload)),
        _ => log::trace!("unhandled DCS frame: {intermediates:?} {action:?}"),
    }
}

/// The DCS helpers the trait impl cannot reach directly.
impl Terminal {
    pub(crate) fn push_dcs_request(&mut self, body: String) {
        self.push_event(Event::RequestSetting(body));
    }
}

/// `CSI > c` — the Secondary DA reply: model ID 0, the build version encoded as
/// `major * 10000 + minor * 100 + patch`, and ROM slot 1.
fn secondary_da() -> Vec<u8> {
    let core = otty_core::VERSION.split(['-', '+']).next().unwrap_or(otty_core::VERSION);
    let mut parts = core.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (major, minor, patch) =
        (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    format!("\x1b[>0;{};1c", major * 10_000 + minor * 100 + patch).into_bytes()
}

/// The colour-report format the answer uses, exposed for the CLI's `features`
/// demos.
pub fn report_format(term: &Terminal) -> ColorReportFormat {
    term.config.osc_color_report_format
}

/// The cursor shape as a `DECSCUSR` number, for round-tripping in tests.
pub fn decscusr_number(shape: CursorShape, blinking: bool) -> u16 {
    match (shape, blinking) {
        (CursorShape::Block, true) => 1,
        (CursorShape::Block, false) => 2,
        (CursorShape::Underline, true) => 3,
        (CursorShape::Underline, false) => 4,
        (CursorShape::Bar, true) => 5,
        (CursorShape::Bar, false) => 6,
        (CursorShape::HollowBlock, _) => 0,
    }
}

/// Whether the terminal is on the alternate screen, for the tests.
pub fn on_alternate(term: &Terminal) -> bool {
    term.mode == Mode::Alternate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Event, ImageEvent, WindowReport};
    use crate::screen::TermConfig;

    /// A terminal of `cols` × `rows` and the parser that drives it.
    fn term(cols: u16, rows: u16) -> (Terminal, Parser) {
        (Terminal::new(cols, rows, TermConfig::default()), Parser::new())
    }

    fn feed(t: &mut Terminal, p: &mut Parser, bytes: &[u8]) {
        p.advance(t, bytes);
    }

    #[test]
    fn dcs_decrqss_hands_the_cursor_style_query_to_the_app() {
        let (mut t, mut p) = term(20, 3);
        // `DCS $ q SP q ST` — the one DECRQSS target Otty answers. The engine
        // reports the query and the app builds the reply, because that reply is
        // the configured default when no DECSCUSR override is in force.
        feed(&mut t, &mut p, b"\x1bP$q q\x1b\\");
        assert_eq!(t.drain_events(), vec![Event::RequestSetting(" q".to_owned())]);

        // Other DECRQSS targets, XTGETTCAP and sixel are documented as not
        // implemented: nothing is reported and nothing is answered.
        for frame in [
            &b"\x1bP$qr\x1b\\"[..],      // DECRQSS: scrolling region
            &b"\x1bP+q544e\x1b\\"[..],   // XTGETTCAP
            &b"\x1bPq\"1;1#0\x1b\\"[..], // sixel
        ] {
            feed(&mut t, &mut p, frame);
        }
        assert!(t.drain_events().is_empty());
        assert!(t.drain_replies().is_empty());
    }

    #[test]
    fn the_colour_stack_operations_reach_the_app() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b[#P\x1b[#Q\x1b[#R");
        assert_eq!(
            t.drain_events(),
            vec![
                Event::ColorStack(ColorStackOp::Push),
                Event::ColorStack(ColorStackOp::Pop),
                Event::ColorStack(ColorStackOp::Report),
            ]
        );
    }

    #[test]
    fn decsca_protects_the_cells_that_follow_it() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b[1\"qKEEP\x1b[0\"qgone\x1b[?2J");
        assert!(t.contents().starts_with("KEEP"));
        // DECSED skips the protected run; plain ED erases it.
        feed(&mut t, &mut p, b"\x1b[2J");
        assert!(!t.contents().contains("KEEP"));
    }

    #[test]
    fn kitty_keyboard_flags_push_pop_and_query() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b[>1u");
        assert_eq!(t.kitty_flags(), 1);
        feed(&mut t, &mut p, b"\x1b[?u");
        assert_eq!(t.drain_replies(), vec![b"\x1b[?1u".to_vec()]);
        feed(&mut t, &mut p, b"\x1b[<u");
        assert_eq!(t.kitty_flags(), 0);
        feed(&mut t, &mut p, b"\x1b[?u");
        assert_eq!(t.drain_replies(), vec![b"\x1b[?0u".to_vec()]);
    }

    #[test]
    fn xtwinops_surfaces_only_the_read_only_pixel_reports() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b[14t\x1b[16t");
        assert_eq!(
            t.drain_events(),
            vec![
                Event::WindowReport(WindowReport::TextAreaPixels),
                Event::WindowReport(WindowReport::CellPixels),
            ]
        );
        // 18 is dropped on purpose, and the window-acting operations are
        // ignored: no event, no reply.
        feed(&mut t, &mut p, b"\x1b[18t\x1b[20t\x1b[5t\x1b[3;10;10t");
        assert!(t.drain_events().is_empty());
        assert!(t.drain_replies().is_empty());
    }

    #[test]
    fn rep_repeats_the_last_graphic_character() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"A\x1b[3b");
        assert_eq!(t.contents(), "AAAA");
    }

    #[test]
    fn identification_replies_use_the_documented_forms() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b[>q");
        assert_eq!(
            t.drain_replies(),
            vec![format!("\x1bP>|otty({})\x1b\\", otty_core::VERSION).into_bytes()]
        );
        feed(&mut t, &mut p, b"\x1b[>c");
        let reply = t.drain_replies().remove(0);
        let text = String::from_utf8(reply).unwrap();
        assert!(text.starts_with("\x1b[>0;") && text.ends_with(";1c"), "{text:?}");
        assert!(text[5..text.len() - 3].parse::<u32>().is_ok(), "{text:?}");
    }

    #[test]
    fn lnm_makes_lf_return_to_the_first_column() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"ab\n");
        assert_eq!((t.cursor.row, t.cursor.col), (1, 2));
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b[20hab\n");
        assert_eq!((t.cursor.row, t.cursor.col), (1, 0));
    }

    #[test]
    fn scroll_then_erase_keeps_the_viewport_in_scrollback() {
        let (mut t, mut p) = term(6, 2);
        feed(&mut t, &mut p, b"hello\x1b[22J");
        // Both visible rows moved into history, in order.
        assert_eq!(t.grid().scrollback_len(), 2);
        assert!(!t.contents().contains("hello"));
        assert!(t.history().contains("hello"));
    }

    #[test]
    fn inline_images_hand_over_the_undecoded_payload() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b]1337;File=name=a.png;inline=1:aGk=\x07");
        assert_eq!(
            t.drain_events(),
            vec![Event::Image(ImageEvent::Iterm {
                params: vec![
                    ("name".to_owned(), "a.png".to_owned()),
                    ("inline".to_owned(), "1".to_owned()),
                ],
                payload: "aGk=".to_owned(),
            })]
        );
    }

    #[test]
    fn the_clipboard_and_notification_payloads_are_decoded() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b]52;c;aGk=\x07");
        assert_eq!(
            t.drain_events(),
            vec![Event::ClipboardStore { selection: 'c', data: b"hi".to_vec() }]
        );
        feed(&mut t, &mut p, b"\x1b]99;t=title;d=aGk=\x07");
        match t.drain_events().as_slice() {
            [Event::Notify(n)] => {
                assert_eq!(n.title.as_deref(), Some("title"));
                assert_eq!(n.body, "hi");
            }
            other => panic!("expected one notification, got {other:?}"),
        }
    }

    #[test]
    fn the_dsr_forms_that_are_not_implemented_stay_silent() {
        let (mut t, mut p) = term(20, 3);
        feed(&mut t, &mut p, b"\x1b[6n");
        assert_eq!(t.drain_replies(), vec![b"\x1b[1;1R".to_vec()]);
        // DECXCPR and the printer / keyboard / locator reports are documented
        // as not answered.
        feed(&mut t, &mut p, b"\x1b[?6n\x1b[?15n\x1b[?25n\x1b[?53n");
        assert!(t.drain_replies().is_empty());
    }
}

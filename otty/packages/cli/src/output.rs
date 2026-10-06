//! Output shaping: the `--format text|json`, `--json`, `--no-headers` and
//! `-q/--quiet` contract, and the exit codes the CLI reference documents.

use std::io::Write;

use serde::Serialize;

/// The exit codes. Each one is a distinct answer, because a script has to be
/// able to tell "no such pane" from "no app running".
pub mod exit {
    /// Success.
    pub const OK: i32 = 0;
    /// The request was malformed, or an unknown key was named.
    pub const USAGE: i32 = 2;
    /// Otty is not running.
    pub const NO_APP: i32 = 3;
    /// A selector matched nothing, or a wait never saw a usable state.
    pub const NO_MATCH: i32 = 4;
    /// `otty watch:<agent>` never saw a usable state. The reference documents
    /// the same code as a selector miss, because from a script's point of view
    /// both mean "what you asked for is not there".
    pub const NO_STATE: i32 = 4;
    /// The agent has no integration installed, so it can never report.
    pub const NO_INTEGRATION: i32 = 6;
    /// The wait timed out.
    pub const TIMEOUT: i32 = 9;
    /// A privilege gate refused the action.
    pub const REFUSED: i32 = 13;
}

/// How the output should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    Text,
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// The global switches that shape every response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputOptions {
    pub format: Format,
    /// `--no-headers` — drop the header row from a text table.
    pub no_headers: bool,
    /// `-q/--quiet` — suppress the success line of a command that did work.
    pub quiet: bool,
}

/// One column of a text table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    pub header: &'static str,
    /// Whether the column is right-aligned (numbers read better that way).
    pub numeric: bool,
}

impl Column {
    pub const fn text(header: &'static str) -> Self {
        Self { header, numeric: false }
    }

    pub const fn number(header: &'static str) -> Self {
        Self { header, numeric: true }
    }
}

/// Render `rows` as an aligned text table, or as JSON when asked.
///
/// The text form pads each column to its widest cell; the JSON form is an array
/// of objects keyed by the column headers, lower-cased with spaces turned into
/// underscores, so `otty pane list --json | jq '.[].pane_id'` works without the
/// caller having to know the display spelling.
pub fn render_table<W: Write>(
    out: &mut W,
    columns: &[Column],
    rows: &[Vec<String>],
    opts: OutputOptions,
) -> std::io::Result<()> {
    if opts.format == Format::Json {
        let objects: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut map = serde_json::Map::new();
                for (c, cell) in columns.iter().zip(row.iter()) {
                    map.insert(json_key(c.header), serde_json::Value::String(cell.clone()));
                }
                serde_json::Value::Object(map)
            })
            .collect();
        return write_json(out, &serde_json::Value::Array(objects));
    }

    if rows.is_empty() {
        // An empty table prints nothing rather than a header with no rows, so a
        // script piping the text form does not have to filter the header out.
        return Ok(());
    }

    let mut widths: Vec<usize> = columns.iter().map(|c| c.header.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
    }

    if !opts.no_headers {
        write_row(out, columns, &widths, &columns.iter().map(|c| c.header.to_owned()).collect::<Vec<_>>())?;
    }
    for row in rows {
        write_row(out, columns, &widths, row)?;
    }
    Ok(())
}

fn write_row<W: Write>(
    out: &mut W,
    columns: &[Column],
    widths: &[usize],
    cells: &[String],
) -> std::io::Result<()> {
    let mut line = String::new();
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            line.push_str("  ");
        }
        let width = widths.get(i).copied().unwrap_or(0);
        let pad = width.saturating_sub(cell.chars().count());
        if columns.get(i).is_some_and(|c| c.numeric) {
            line.push_str(&" ".repeat(pad));
            line.push_str(cell);
        } else {
            line.push_str(cell);
            if i + 1 < cells.len() {
                line.push_str(&" ".repeat(pad));
            }
        }
    }
    writeln!(out, "{}", line.trim_end())
}

/// `Pane Id` → `pane_id`.
pub fn json_key(header: &str) -> String {
    let mut s = String::with_capacity(header.len());
    for c in header.chars() {
        if c == ' ' || c == '-' {
            s.push('_');
        } else {
            s.extend(c.to_lowercase());
        }
    }
    s
}

/// Write one JSON value followed by a newline.
pub fn write_json<W: Write, T: Serialize>(out: &mut W, value: &T) -> std::io::Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    writeln!(out)
}

/// Write a single `status` object, which is what `--json` prints for a command
/// that has no table to show.
pub fn write_status<W: Write>(
    out: &mut W,
    opts: OutputOptions,
    message: &str,
) -> std::io::Result<()> {
    if opts.quiet {
        return Ok(());
    }
    match opts.format {
        Format::Json => write_json(out, &serde_json::json!({ "status": "ok", "message": message })),
        Format::Text => writeln!(out, "{message}"),
    }
}

/// A command's outcome.
#[derive(Debug)]
pub enum CliError {
    /// A message plus the exit code a script should see.
    Message { code: i32, message: String },
}

impl CliError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self::Message { code: exit::USAGE, message: message.into() }
    }

    pub fn code(&self) -> i32 {
        match self {
            Self::Message { code, .. } => *code,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Message { message, .. } => message,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl From<otty_config::ConfigError> for CliError {
    fn from(e: otty_config::ConfigError) -> Self {
        Self::Message { code: exit::USAGE, message: e.to_string() }
    }
}

impl From<otty_ipc::ClientError> for CliError {
    fn from(e: otty_ipc::ClientError) -> Self {
        Self::Message { code: e.exit_code(), message: e.to_string() }
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        Self::Message { code: exit::USAGE, message: e.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(format: Format) -> OutputOptions {
        OutputOptions { format, ..Default::default() }
    }

    #[test]
    fn formats_parse_their_documented_values() {
        assert_eq!(Format::parse("text"), Some(Format::Text));
        assert_eq!(Format::parse("JSON"), Some(Format::Json));
        assert_eq!(Format::parse("yaml"), None);
    }

    #[test]
    fn a_text_table_aligns_columns() {
        let mut buf = Vec::new();
        render_table(
            &mut buf,
            &[Column::text("Pane"), Column::number("Rows")],
            &[
                vec!["pane-1".into(), "24".into()],
                vec!["pane-10".into(), "120".into()],
            ],
            opts(Format::Text),
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "Pane     Rows");
        assert_eq!(lines[1], "pane-1     24");
        assert_eq!(lines[2], "pane-10   120");
    }

    #[test]
    fn no_headers_drops_only_the_header_row() {
        let mut buf = Vec::new();
        render_table(
            &mut buf,
            &[Column::text("A")],
            &[vec!["x".into()]],
            OutputOptions { no_headers: true, ..opts(Format::Text) },
        )
        .unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "x\n");
    }

    #[test]
    fn an_empty_table_prints_nothing_in_text_mode() {
        let mut buf = Vec::new();
        render_table(&mut buf, &[Column::text("A")], &[], opts(Format::Text)).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn json_output_keys_are_snake_case() {
        let mut buf = Vec::new();
        render_table(
            &mut buf,
            &[Column::text("Pane Id"), Column::text("Working Directory")],
            &[vec!["pane-1".into(), "/work".into()]],
            opts(Format::Json),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v[0]["pane_id"], "pane-1");
        assert_eq!(v[0]["working_directory"], "/work");
    }

    #[test]
    fn json_key_mapping_is_mechanical() {
        assert_eq!(json_key("Pane Id"), "pane_id");
        assert_eq!(json_key("Rows"), "rows");
        assert_eq!(json_key("Working Directory"), "working_directory");
    }

    #[test]
    fn quiet_suppresses_the_success_line_but_not_json() {
        let mut buf = Vec::new();
        write_status(&mut buf, OutputOptions { quiet: true, ..opts(Format::Text) }, "done").unwrap();
        assert!(buf.is_empty());

        let mut buf = Vec::new();
        write_status(&mut buf, opts(Format::Json), "done").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["status"], "ok");
    }

    #[test]
    fn exit_codes_are_the_documented_ones() {
        assert_eq!(exit::OK, 0);
        assert_eq!(exit::USAGE, 2);
        assert_eq!(exit::NO_APP, 3);
        assert_eq!(exit::NO_MATCH, 4);
        assert_eq!(exit::NO_INTEGRATION, 6);
        assert_eq!(exit::TIMEOUT, 9);
        assert_eq!(exit::REFUSED, 13);
    }

    #[test]
    fn an_ipc_failure_carries_its_own_exit_code() {
        let e: CliError = otty_ipc::ClientError::NoApp.into();
        assert_eq!(e.code(), exit::NO_APP);
        let e: CliError = otty_ipc::ClientError::Timeout.into();
        assert_eq!(e.code(), exit::TIMEOUT);
    }
}

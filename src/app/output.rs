use crate::app::errors::{error_type, format_error};
use crate::app::render::{self, Doc, Term};
use anyhow::Result;
use serde_json::{Value, json};
use std::io::{self, Write};

type Renderer = Box<dyn FnOnce(&Value, &mut Doc)>;

pub enum Output {
    Json(Value),
    View(Value, Renderer),
    Check {
        passed: bool,
        value: Value,
        renderer: Renderer,
    },
    Text(String, Renderer),
    None,
}

impl Output {
    #[must_use]
    pub fn passed(&self) -> bool {
        !matches!(self, Self::Check { passed: false, .. })
    }

    pub fn check(
        passed: bool,
        value: Value,
        renderer: impl FnOnce(&Value, &mut Doc) + 'static,
    ) -> Self {
        Self::Check {
            passed,
            value,
            renderer: Box::new(renderer),
        }
    }

    pub fn text(value: String, renderer: impl FnOnce(&Value, &mut Doc) + 'static) -> Self {
        Self::Text(value, Box::new(renderer))
    }

    pub fn view(value: Value, renderer: impl FnOnce(&Value, &mut Doc) + 'static) -> Self {
        Self::View(value, Box::new(renderer))
    }
}

/// # Errors
/// Returns an error if serialization or writing to standard output fails.
pub fn print(output: Output, json_mode: bool) -> Result<()> {
    print_to(
        output,
        json_mode,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
        Term::stdout(),
        Term::stderr(),
    )
}

fn print_to(
    output: Output,
    json_mode: bool,
    writer: &mut impl Write,
    stderr: &mut impl Write,
    stdout_term: Term,
    stderr_term: Term,
) -> Result<()> {
    match output {
        Output::None => {}
        Output::Text(text, _) if json_mode => writeln!(writer, "{text}")?,
        Output::Text(text, renderer) => {
            let mut doc = Doc::default();
            renderer(&Value::String(text), &mut doc);
            doc.write(writer, stdout_term, stderr, stderr_term)?;
        }
        Output::Json(value) | Output::View(value, _) | Output::Check { value, .. } if json_mode => {
            serde_json::to_writer_pretty(&mut *writer, &value).map_err(|error| {
                match error.io_error_kind() {
                    Some(kind) => anyhow::Error::from(io::Error::new(kind, error)),
                    None => error.into(),
                }
            })?;
            writeln!(writer)?;
        }
        Output::Json(value) => {
            let mut doc = Doc::default();
            render::generic(&value, &mut doc);
            doc.write(writer, stdout_term, stderr, stderr_term)?;
        }
        Output::View(value, renderer)
        | Output::Check {
            value, renderer, ..
        } => {
            let mut doc = Doc::default();
            renderer(&value, &mut doc);
            doc.write(writer, stdout_term, stderr, stderr_term)?;
        }
    }
    Ok(())
}

pub fn print_error(err: &anyhow::Error, json_mode: bool) {
    if json_mode {
        let payload = json!({
            "ok": false,
            "error": {
                "type": error_type(err),
                "message": format_error(err),
            }
        });
        let _ = print(Output::Json(payload), true);
    } else {
        let mut doc = Doc::default();
        render::error(err, &mut doc);
        let _ = doc.write(
            &mut io::stdout().lock(),
            Term::stdout(),
            &mut io::stderr().lock(),
            Term::stderr(),
        );
    }
}

#[must_use]
pub fn redact(value: Option<&str>, reveal: bool) -> Value {
    match value {
        Some(value) if reveal => Value::String(value.to_string()),
        Some(value) => Value::String(redact_str(value)),
        None => Value::Null,
    }
}

#[must_use]
pub fn redact_str(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return "********".to_string();
    }
    let first: String = chars[..4].iter().collect();
    let last: String = chars[chars.len() - 4..].iter().collect();
    format!("{first}…{last}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_preserves_pretty_json_strings_text_and_newlines() {
        for (output, json_mode, expected) in [
            (Output::None, true, ""),
            (Output::text("text".into(), render::generic), true, "text\n"),
            (Output::Json(json!("hello")), false, "hello\n"),
            (Output::Json(json!("hello")), true, "\"hello\"\n"),
            (
                Output::Json(json!({"a":[1,2]})),
                true,
                "{\n  \"a\": [\n    1,\n    2\n  ]\n}\n",
            ),
        ] {
            let mut bytes = Vec::new();
            print_to(
                output,
                json_mode,
                &mut bytes,
                &mut Vec::new(),
                Term::default(),
                Term::default(),
            )
            .unwrap();
            assert_eq!(bytes, expected.as_bytes());
        }
    }

    #[test]
    fn json_views_skip_human_rendering_for_every_response_fixture() {
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mock_responses");
        for entry in std::fs::read_dir(fixtures).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            let value = value.get("data").unwrap_or(&value).clone();
            let expected = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            print_to(
                Output::view(value, |_, _| panic!("JSON must not call a human renderer")),
                true,
                &mut stdout,
                &mut stderr,
                Term::default(),
                Term::default(),
            )
            .unwrap();
            assert_eq!(stdout, expected.as_bytes());
            assert_eq!(stderr, b"");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn output_returns_write_errors_for_text_and_json() {
        for output in [
            Output::text("text".into(), render::generic),
            Output::Json(json!({"value":1})),
        ] {
            let mut writer = std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .unwrap();
            assert!(
                print_to(
                    output,
                    true,
                    &mut writer,
                    &mut Vec::new(),
                    Term::default(),
                    Term::default()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn credential_redaction_counts_unicode_characters() {
        assert_eq!(redact_str(""), "********");
        assert_eq!(redact_str("abcdefgh"), "********");
        assert_eq!(redact_str("abcdefghi"), "abcd…fghi");
        assert_eq!(redact_str("αβγδεζηθ"), "********");
        assert_eq!(redact_str("αβγδεζηθι"), "αβγδ…ζηθι");
    }
}

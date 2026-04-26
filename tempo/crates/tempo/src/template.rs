use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("unknown template variable {0:?}")]
    UnknownVariable(String),
    #[error("unmatched '{{' in template")]
    UnmatchedOpenBrace,
    #[error("unmatched '}}' in template")]
    UnmatchedCloseBrace,
    #[error("empty variable name in template")]
    EmptyVariable,
}

pub struct TemplateVars<'a> {
    pub run_id: &'a str,
    pub cell_id: &'a str,
}

pub fn render(template: &str, vars: &TemplateVars) -> Result<String, TemplateError> {
    process(template, Some(vars))
}

pub fn validate(template: &str) -> Result<(), TemplateError> {
    process(template, None).map(|_| ())
}

pub fn cell_id(scenario: &str, model: &str, prompt: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;
    let mut h: u64 = FNV_OFFSET;
    for part in [
        "tempo-cell-id-v1",
        "\x1f",
        scenario,
        "\x1f",
        model,
        "\x1f",
        prompt,
    ] {
        for &b in part.as_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
    }
    format!("{:08x}", h as u32)
}

pub fn new_run_id() -> String {
    format!("{:08x}", rand::random::<u32>())
}

fn process(template: &str, vars: Option<&TemplateVars>) -> Result<String, TemplateError> {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '{' => {
                if matches!(chars.peek(), Some((_, '{'))) {
                    chars.next();
                    out.push('{');
                    continue;
                }
                let rest = &template[i + 1..];
                let Some(close_off) = rest.find('}') else {
                    return Err(TemplateError::UnmatchedOpenBrace);
                };
                let name = &rest[..close_off];
                if name.is_empty() {
                    return Err(TemplateError::EmptyVariable);
                }
                let value = match name {
                    "run_id" => vars.map(|v| v.run_id),
                    "cell_id" => vars.map(|v| v.cell_id),
                    other => return Err(TemplateError::UnknownVariable(other.to_string())),
                };
                if let Some(v) = value {
                    out.push_str(v);
                }
                let target = i + 1 + close_off + 1;
                while let Some(&(j, _)) = chars.peek() {
                    if j >= target {
                        break;
                    }
                    chars.next();
                }
            }
            '}' => {
                if matches!(chars.peek(), Some((_, '}'))) {
                    chars.next();
                    out.push('}');
                    continue;
                }
                return Err(TemplateError::UnmatchedCloseBrace);
            }
            _ => out.push(c),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> TemplateVars<'static> {
        TemplateVars {
            run_id: "abcd1234",
            cell_id: "deadbeef",
        }
    }

    #[test]
    fn renders_known_variables() {
        let out = render("run={run_id} cell={cell_id}", &vars()).unwrap();
        assert_eq!(out, "run=abcd1234 cell=deadbeef");
    }

    #[test]
    fn brace_doubling_escapes_to_literals() {
        assert_eq!(render("{{a}}", &vars()).unwrap(), "{a}");
        assert_eq!(render("{{", &vars()).unwrap(), "{");
        assert_eq!(render("}}", &vars()).unwrap(), "}");
    }

    #[test]
    fn unknown_variable_is_error() {
        let err = render("hi {foobar}", &vars()).unwrap_err();
        assert_eq!(err, TemplateError::UnknownVariable("foobar".into()));
    }

    #[test]
    fn unmatched_open_brace_is_error() {
        let err = render("hello {run_id", &vars()).unwrap_err();
        assert_eq!(err, TemplateError::UnmatchedOpenBrace);
    }

    #[test]
    fn unmatched_close_brace_is_error() {
        let err = render("hello }", &vars()).unwrap_err();
        assert_eq!(err, TemplateError::UnmatchedCloseBrace);
    }

    #[test]
    fn empty_variable_is_error() {
        let err = render("hi {}", &vars()).unwrap_err();
        assert_eq!(err, TemplateError::EmptyVariable);
        assert_eq!(validate("hi {}").unwrap_err(), TemplateError::EmptyVariable);
    }

    #[test]
    fn empty_string_passes_through() {
        assert_eq!(render("", &vars()).unwrap(), "");
    }

    #[test]
    fn mixed_escape_and_var() {
        let out = render("See {{example}} with id={run_id}", &vars()).unwrap();
        assert_eq!(out, "See {example} with id=abcd1234");
    }

    #[test]
    fn validate_catches_unknown_var_without_values() {
        assert_eq!(
            validate("oops {bogus}").unwrap_err(),
            TemplateError::UnknownVariable("bogus".into()),
        );
    }

    #[test]
    fn validate_accepts_known_vars() {
        assert!(validate("run={run_id} cell={cell_id}").is_ok());
        assert!(validate("plain text with no vars").is_ok());
        assert!(validate("escaped {{braces}}").is_ok());
    }

    #[test]
    fn cell_id_is_stable_and_distinct() {
        let a = cell_id("decode", "m1", "short");
        let b = cell_id("decode", "m1", "short");
        let c = cell_id("decode", "m2", "short");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 8);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn cell_id_separator_prevents_collisions() {
        let a = cell_id("ab", "c", "d");
        let b = cell_id("a", "bc", "d");
        assert_ne!(a, b);
    }
}

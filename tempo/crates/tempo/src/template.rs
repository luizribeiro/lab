use std::collections::BTreeMap;

use thiserror::Error;

use crate::dimensions::Dimensions;
use crate::var::VarValue;

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

pub trait Resolver {
    fn resolve(&self, name: &str) -> Option<&str>;
    fn resolve_typed(&self, _name: &str) -> Option<VarValue> {
        None
    }
}

#[derive(Debug, Default, Clone)]
pub struct SimpleResolver {
    values: BTreeMap<String, VarValue>,
    strings: BTreeMap<String, String>,
}

impl SimpleResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<VarValue>) -> &mut Self {
        let name = name.into();
        let value = value.into();
        self.strings.insert(name.clone(), value_to_string(&value));
        self.values.insert(name, value);
        self
    }
}

impl Resolver for SimpleResolver {
    fn resolve(&self, name: &str) -> Option<&str> {
        self.strings.get(name).map(String::as_str)
    }

    fn resolve_typed(&self, name: &str) -> Option<VarValue> {
        self.values.get(name).cloned()
    }
}

fn value_to_string(v: &VarValue) -> String {
    match v {
        VarValue::String(s) => s.clone(),
        VarValue::Integer(i) => i.to_string(),
        VarValue::Float(f) => f.to_string(),
        VarValue::Bool(b) => b.to_string(),
    }
}

pub fn render(template: &str, resolver: &dyn Resolver) -> Result<String, TemplateError> {
    process(template, Mode::Render(resolver))
}

pub fn validate(template: &str, allowed: &[&str]) -> Result<(), TemplateError> {
    process(template, Mode::Validate(allowed)).map(|_| ())
}

pub fn render_typed(template: &str, resolver: &dyn Resolver) -> Option<VarValue> {
    let inner = template.strip_prefix('{')?.strip_suffix('}')?;
    if inner.is_empty() || inner.contains('{') || inner.contains('}') {
        return None;
    }
    resolver.resolve_typed(inner)
}

pub fn cell_id_for(d: &Dimensions) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;
    const SEP: u8 = 0x1f;

    fn feed(h: &mut u64, bytes: &[u8]) {
        for &b in bytes {
            *h ^= b as u64;
            *h = h.wrapping_mul(FNV_PRIME);
        }
    }

    let mut h: u64 = FNV_OFFSET;
    feed(&mut h, b"tempo-cell-id-v2");
    feed(&mut h, &[SEP]);
    feed(&mut h, d.scenario.as_bytes());
    feed(&mut h, &[SEP]);
    feed(&mut h, d.provider.as_bytes());
    feed(&mut h, &[SEP]);

    let mut keys: Vec<&str> = d.vars.keys().map(String::as_str).collect();
    keys.sort();
    for key in keys {
        let value = d.vars.get(key).expect("key came from d.vars");
        feed(&mut h, key.as_bytes());
        feed(&mut h, &[SEP]);
        match value {
            VarValue::String(s) => {
                feed(&mut h, &[b's', SEP]);
                feed(&mut h, s.as_bytes());
            }
            VarValue::Integer(i) => {
                feed(&mut h, &[b'i', SEP]);
                feed(&mut h, &i.to_be_bytes());
            }
            VarValue::Float(f) => {
                feed(&mut h, &[b'f', SEP]);
                feed(&mut h, &f.to_bits().to_be_bytes());
            }
            VarValue::Bool(b) => {
                feed(&mut h, &[b'b', SEP, u8::from(*b)]);
            }
        }
        feed(&mut h, &[SEP]);
    }
    format!("{:08x}", h as u32)
}

pub fn new_run_id() -> String {
    format!("{:08x}", rand::random::<u32>())
}

enum Mode<'a> {
    Render(&'a dyn Resolver),
    Validate(&'a [&'a str]),
}

enum Lookup<'a> {
    Found(&'a str),
    Allowed,
    Unknown,
}

impl Mode<'_> {
    fn lookup(&self, name: &str) -> Lookup<'_> {
        match self {
            Mode::Render(r) => match r.resolve(name) {
                Some(v) => Lookup::Found(v),
                None => Lookup::Unknown,
            },
            Mode::Validate(allowed) => {
                if allowed.contains(&name) {
                    Lookup::Allowed
                } else {
                    Lookup::Unknown
                }
            }
        }
    }
}

fn process(template: &str, mode: Mode<'_>) -> Result<String, TemplateError> {
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
                match mode.lookup(name) {
                    Lookup::Found(v) => out.push_str(v),
                    Lookup::Allowed => {}
                    Lookup::Unknown => {
                        return Err(TemplateError::UnknownVariable(name.to_string()))
                    }
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

    fn resolver() -> SimpleResolver {
        let mut r = SimpleResolver::new();
        r.insert("run_id", "abcd1234");
        r.insert("cell_id", "deadbeef");
        r
    }

    #[test]
    fn renders_known_variables() {
        let out = render("run={run_id} cell={cell_id}", &resolver()).unwrap();
        assert_eq!(out, "run=abcd1234 cell=deadbeef");
    }

    #[test]
    fn brace_doubling_escapes_to_literals() {
        let r = resolver();
        assert_eq!(render("{{a}}", &r).unwrap(), "{a}");
        assert_eq!(render("{{", &r).unwrap(), "{");
        assert_eq!(render("}}", &r).unwrap(), "}");
    }

    #[test]
    fn unknown_variable_is_error() {
        let err = render("hi {foobar}", &resolver()).unwrap_err();
        assert_eq!(err, TemplateError::UnknownVariable("foobar".into()));
    }

    #[test]
    fn unmatched_open_brace_is_error() {
        let err = render("hello {run_id", &resolver()).unwrap_err();
        assert_eq!(err, TemplateError::UnmatchedOpenBrace);
    }

    #[test]
    fn unmatched_close_brace_is_error() {
        let err = render("hello }", &resolver()).unwrap_err();
        assert_eq!(err, TemplateError::UnmatchedCloseBrace);
    }

    #[test]
    fn empty_variable_is_error() {
        let err = render("hi {}", &resolver()).unwrap_err();
        assert_eq!(err, TemplateError::EmptyVariable);
        assert_eq!(
            validate("hi {}", &["run_id"]).unwrap_err(),
            TemplateError::EmptyVariable
        );
    }

    #[test]
    fn empty_string_passes_through() {
        assert_eq!(render("", &resolver()).unwrap(), "");
    }

    #[test]
    fn mixed_escape_and_var() {
        let out = render("See {{example}} with id={run_id}", &resolver()).unwrap();
        assert_eq!(out, "See {example} with id=abcd1234");
    }

    #[test]
    fn validate_with_empty_slice_rejects_all_vars() {
        assert_eq!(
            validate("oops {bogus}", &[]).unwrap_err(),
            TemplateError::UnknownVariable("bogus".into()),
        );
        assert_eq!(
            validate("hi {run_id}", &[]).unwrap_err(),
            TemplateError::UnknownVariable("run_id".into()),
        );
    }

    #[test]
    fn validate_accepts_listed_names() {
        let allowed = ["run_id", "cell_id"];
        assert!(validate("run={run_id} cell={cell_id}", &allowed).is_ok());
        assert!(validate("plain text with no vars", &allowed).is_ok());
        assert!(validate("escaped {{braces}}", &allowed).is_ok());
    }

    #[test]
    fn validate_rejects_var_not_in_allowed() {
        assert_eq!(
            validate("{model}", &["run_id"]).unwrap_err(),
            TemplateError::UnknownVariable("model".into()),
        );
    }

    #[test]
    fn resolver_trait_is_dyn_dispatched() {
        struct Always(&'static str);
        impl Resolver for Always {
            fn resolve(&self, _name: &str) -> Option<&str> {
                Some(self.0)
            }
        }
        let r: &dyn Resolver = &Always("x");
        assert_eq!(render("{anything}", r).unwrap(), "x");
    }

    #[test]
    fn render_typed_returns_some_for_exact_var() {
        let mut r = SimpleResolver::new();
        r.insert("max_tokens", 2048i64);
        let v = render_typed("{max_tokens}", &r).unwrap();
        assert_eq!(v, VarValue::from(2048i64));
    }

    #[test]
    fn render_typed_returns_some_for_each_typed_value() {
        let mut r = SimpleResolver::new();
        r.insert("s", "hi");
        r.insert("i", 7i64);
        r.insert("f", VarValue::float(1.5).unwrap());
        r.insert("b", true);
        assert_eq!(render_typed("{s}", &r), Some(VarValue::from("hi")));
        assert_eq!(render_typed("{i}", &r), Some(VarValue::from(7i64)));
        assert_eq!(render_typed("{f}", &r), Some(VarValue::float(1.5).unwrap()));
        assert_eq!(render_typed("{b}", &r), Some(VarValue::from(true)));
    }

    #[test]
    fn render_typed_returns_none_for_mixed_template() {
        let mut r = SimpleResolver::new();
        r.insert("max_tokens", 2048i64);
        assert_eq!(render_typed("prefix {max_tokens}", &r), None);
        assert_eq!(render_typed("{max_tokens} suffix", &r), None);
        assert_eq!(render_typed("{max_tokens}{other}", &r), None);
        assert_eq!(render_typed("plain", &r), None);
        assert_eq!(render_typed("", &r), None);
    }

    #[test]
    fn render_typed_returns_none_for_escaped_braces() {
        let mut r = SimpleResolver::new();
        r.insert("x", 1i64);
        assert_eq!(render_typed("{{x}}", &r), None);
        assert_eq!(render_typed("{{}}", &r), None);
        assert_eq!(render_typed("}}", &r), None);
        assert_eq!(render_typed("{{", &r), None);
    }

    #[test]
    fn render_typed_returns_none_for_unknown_var() {
        let r = SimpleResolver::new();
        assert_eq!(render_typed("{nope}", &r), None);
    }

    #[test]
    fn simple_resolver_stringifies_typed_values() {
        let mut r = SimpleResolver::new();
        r.insert("max_tokens", 2048i64);
        r.insert("temperature", VarValue::float(0.5).unwrap());
        r.insert("flag", true);
        assert_eq!(
            render("mt={max_tokens} t={temperature} f={flag}", &r).unwrap(),
            "mt=2048 t=0.5 f=true"
        );
    }

    fn dim(scenario: &str, provider: &str, vars: &[(&str, VarValue)]) -> Dimensions {
        let mut m = indexmap::IndexMap::new();
        for (k, v) in vars {
            m.insert((*k).to_owned(), v.clone());
        }
        Dimensions {
            scenario: scenario.to_owned(),
            provider: provider.to_owned(),
            vars: m,
        }
    }

    #[test]
    fn cell_id_for_is_stable_and_distinct() {
        let a = cell_id_for(&dim(
            "decode",
            "p",
            &[
                ("model", VarValue::from("m1")),
                ("prompt", VarValue::from("short")),
            ],
        ));
        let b = cell_id_for(&dim(
            "decode",
            "p",
            &[
                ("model", VarValue::from("m1")),
                ("prompt", VarValue::from("short")),
            ],
        ));
        let c = cell_id_for(&dim(
            "decode",
            "p",
            &[
                ("model", VarValue::from("m2")),
                ("prompt", VarValue::from("short")),
            ],
        ));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 8);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn cell_id_for_separator_prevents_collisions() {
        let a = cell_id_for(&dim(
            "ab",
            "p",
            &[
                ("model", VarValue::from("c")),
                ("prompt", VarValue::from("d")),
            ],
        ));
        let b = cell_id_for(&dim(
            "a",
            "p",
            &[
                ("model", VarValue::from("bc")),
                ("prompt", VarValue::from("d")),
            ],
        ));
        assert_ne!(a, b);
    }

    #[test]
    fn cell_id_for_invariant_to_var_insertion_order() {
        let a = cell_id_for(&dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("gpt")),
                ("max_tokens", VarValue::from(2048i64)),
                ("topic", VarValue::from("a")),
            ],
        ));
        let b = cell_id_for(&dim(
            "decode",
            "litellm",
            &[
                ("topic", VarValue::from("a")),
                ("model", VarValue::from("gpt")),
                ("max_tokens", VarValue::from(2048i64)),
            ],
        ));
        assert_eq!(a, b);
    }

    #[test]
    fn cell_id_for_changes_with_provider() {
        let a = cell_id_for(&dim("decode", "p1", &[("model", VarValue::from("m1"))]));
        let b = cell_id_for(&dim("decode", "p2", &[("model", VarValue::from("m1"))]));
        assert_ne!(a, b);
    }

    #[test]
    fn cell_id_for_distinguishes_int_from_float_and_string() {
        let i = cell_id_for(&dim("s", "p", &[("x", VarValue::from(1i64))]));
        let f = cell_id_for(&dim("s", "p", &[("x", VarValue::float(1.0).unwrap())]));
        let s = cell_id_for(&dim("s", "p", &[("x", VarValue::from("1"))]));
        let b = cell_id_for(&dim("s", "p", &[("x", VarValue::from(true))]));
        let bs = cell_id_for(&dim("s", "p", &[("x", VarValue::from("true"))]));
        assert_ne!(i, f);
        assert_ne!(i, s);
        assert_ne!(f, s);
        assert_ne!(b, bs);
        assert_ne!(b, i);
    }

    #[test]
    fn cell_id_for_golden() {
        let d = dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("vllm/qwen3.6-27b")),
                ("prompt", VarValue::from("short")),
                ("max_tokens", VarValue::from(2048i64)),
                ("temperature", VarValue::float(0.0).unwrap()),
            ],
        );
        assert_eq!(cell_id_for(&d), "bacccb11");
    }
}

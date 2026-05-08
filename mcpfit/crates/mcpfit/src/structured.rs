/// Marker trait for types usable as structured tool output.
///
/// The trait carries no methods: it exists purely so the type system can
/// require structured-content payloads to be objects at the source level. It
/// does not validate the runtime JSON shape beyond what `schemars` produces.
pub trait StructuredObject {}

/// Wrapper that carries a [`StructuredObject`] value plus an optional text
/// override. When no override is supplied, response conversion renders the
/// text block as compact JSON; [`Structured::with_text`] supplies a custom
/// human-readable form instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Structured<T> {
    value: T,
    text: Option<String>,
}

impl<T> Structured<T>
where
    T: StructuredObject,
{
    pub fn new(value: T) -> Self {
        Self { value, text: None }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn text_override(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn into_parts(self) -> (T, Option<String>) {
        (self.value, self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::{Structured, StructuredObject};

    #[derive(Debug, PartialEq, Eq)]
    struct Sum {
        total: i64,
    }

    impl StructuredObject for Sum {}

    fn assert_structured<T: StructuredObject>() {}

    #[test]
    fn manual_impl_satisfies_marker() {
        assert_structured::<Sum>();
    }

    #[test]
    fn new_stores_value_without_text_override() {
        let s = Structured::new(Sum { total: 5 });
        assert_eq!(s.value(), &Sum { total: 5 });
        assert_eq!(s.text_override(), None);
    }

    #[test]
    fn with_text_sets_override() {
        let s = Structured::new(Sum { total: 7 }).with_text("seven");
        assert_eq!(s.text_override(), Some("seven"));
    }

    #[test]
    fn into_parts_returns_value_and_text() {
        let (value, text) = Structured::new(Sum { total: 3 })
            .with_text("three")
            .into_parts();
        assert_eq!(value, Sum { total: 3 });
        assert_eq!(text.as_deref(), Some("three"));
    }
}

/// Marker trait for types usable as structured tool output.
///
/// The trait carries no methods: it exists purely so the type system can
/// require structured-content payloads to be objects at the source level. It
/// does not validate the runtime JSON shape beyond what `schemars` produces.
pub trait StructuredObject {}

#[cfg(test)]
mod tests {
    use super::StructuredObject;

    struct Manual {
        _a: i64,
    }

    impl StructuredObject for Manual {}

    fn assert_structured<T: StructuredObject>() {}

    #[test]
    fn manual_impl_satisfies_marker() {
        assert_structured::<Manual>();
    }
}

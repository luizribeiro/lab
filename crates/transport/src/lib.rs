pub mod stdio;
pub mod tcp;

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn smoke_compiles() {
        assert_eq!(CRATE_NAME, "fittings-transport");
    }
}

#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod matrix;
#[allow(dead_code)]
mod provider;

fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn main() {
    println!("tempo {}", version());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }
}

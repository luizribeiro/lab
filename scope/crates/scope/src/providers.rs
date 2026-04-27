use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, clap::ValueEnum)]
#[clap(rename_all = "lower")]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Read,
    Search,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            ProviderKind::Read => "read",
            ProviderKind::Search => "search",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderSource {
    Builtin,
    External,
}

impl ProviderSource {
    pub fn label(self) -> &'static str {
        match self {
            ProviderSource::Builtin => "built-in",
            ProviderSource::External => "external",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderInfo {
    pub kind: ProviderKind,
    pub name: String,
    pub source: ProviderSource,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_label() {
        assert_eq!(ProviderKind::Read.label(), "read");
        assert_eq!(ProviderKind::Search.label(), "search");
    }

    #[test]
    fn provider_source_label() {
        assert_eq!(ProviderSource::Builtin.label(), "built-in");
        assert_eq!(ProviderSource::External.label(), "external");
    }
}

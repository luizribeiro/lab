use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "scope", about = "Non-interactive CLI web browser for AI agents")]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Search {
        #[arg(long)]
        provider: Option<String>,

        #[arg(long)]
        limit: Option<usize>,

        query: String,
    },
    Read {
        #[arg(long)]
        reader: Option<String>,

        url: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("parse should succeed")
    }

    #[test]
    fn parses_search_query() {
        let cli = parse(&["scope", "search", "rust async"]);
        match cli.command {
            Command::Search { query, provider, limit } => {
                assert_eq!(query, "rust async");
                assert!(provider.is_none());
                assert!(limit.is_none());
            }
            _ => panic!("expected search"),
        }
    }

    #[test]
    fn parses_read_url() {
        let cli = parse(&["scope", "read", "https://example.com"]);
        match cli.command {
            Command::Read { url, reader } => {
                assert_eq!(url, "https://example.com");
                assert!(reader.is_none());
            }
            _ => panic!("expected read"),
        }
    }

    #[test]
    fn accepts_global_config_flag() {
        let cli = parse(&["scope", "--config", "/tmp/scope.toml", "read", "https://example.com"]);
        assert_eq!(cli.config.as_deref(), Some(std::path::Path::new("/tmp/scope.toml")));
    }

    #[test]
    fn parses_search_provider_and_limit() {
        let cli = parse(&["scope", "search", "--provider", "ddg", "--limit", "5", "hello"]);
        match cli.command {
            Command::Search { provider, limit, query } => {
                assert_eq!(provider.as_deref(), Some("ddg"));
                assert_eq!(limit, Some(5));
                assert_eq!(query, "hello");
            }
            _ => panic!("expected search"),
        }
    }

    #[test]
    fn parses_read_reader_override() {
        let cli = parse(&["scope", "read", "--reader", "html", "https://example.com"]);
        match cli.command {
            Command::Read { reader, url } => {
                assert_eq!(reader.as_deref(), Some("html"));
                assert_eq!(url, "https://example.com");
            }
            _ => panic!("expected read"),
        }
    }

    #[test]
    fn rejects_unknown_format_flag() {
        let err = Cli::try_parse_from(["scope", "--format", "json", "search", "x"]);
        assert!(err.is_err(), "--format should no longer be recognized");
    }
}

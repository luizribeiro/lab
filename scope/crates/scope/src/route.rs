use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteMatch {
    pub priority: i32,
    pub specificity: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
}

impl Route {
    pub fn matches(&self, url: &Url) -> Option<RouteMatch> {
        let mut specificity = 0u32;

        if let Some(scheme) = &self.scheme {
            if !scheme.eq_ignore_ascii_case(url.scheme()) {
                return None;
            }
            specificity += 1;
        }

        let url_host = url.host_str().map(|h| h.to_ascii_lowercase());

        if let Some(host) = &self.host {
            let expected = host.to_ascii_lowercase();
            if url_host.as_deref() != Some(expected.as_str()) {
                return None;
            }
            specificity += 1;
        }

        if let Some(suffix) = &self.host_suffix {
            let suffix = suffix.to_ascii_lowercase();
            let host = url_host.as_deref()?;
            let matches = host == suffix || host.ends_with(&format!(".{suffix}"));
            if !matches {
                return None;
            }
            specificity += 1;
        }

        if let Some(prefix) = &self.path_prefix {
            if !url.path().starts_with(prefix.as_str()) {
                return None;
            }
            specificity += 1;
        }

        Some(RouteMatch { priority: 0, specificity })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn empty_route_matches_anything() {
        let route = Route::default();
        let m = route.matches(&url("https://example.com/")).unwrap();
        assert_eq!(m.specificity, 0);
        assert_eq!(m.priority, 0);
    }

    #[test]
    fn exact_host_match() {
        let route = Route {
            host: Some("example.com".into()),
            ..Default::default()
        };
        assert!(route.matches(&url("https://example.com/x")).is_some());
        assert!(route.matches(&url("https://other.com/")).is_none());
        assert_eq!(
            route.matches(&url("https://example.com/")).unwrap().specificity,
            1
        );
    }

    #[test]
    fn host_case_insensitive() {
        let route = Route {
            host: Some("Example.COM".into()),
            ..Default::default()
        };
        assert!(route.matches(&url("https://EXAMPLE.com/")).is_some());
    }

    #[test]
    fn host_suffix_match() {
        let route = Route {
            host_suffix: Some("example.com".into()),
            ..Default::default()
        };
        assert!(route.matches(&url("https://example.com/")).is_some());
        assert!(route.matches(&url("https://api.example.com/")).is_some());
        assert!(route.matches(&url("https://notexample.com/")).is_none());
        assert!(route.matches(&url("https://example.org/")).is_none());
    }

    #[test]
    fn path_prefix_match() {
        let route = Route {
            path_prefix: Some("/api/".into()),
            ..Default::default()
        };
        assert!(route.matches(&url("https://x.com/api/v1")).is_some());
        assert!(route.matches(&url("https://x.com/other")).is_none());
    }

    #[test]
    fn scheme_mismatch_returns_none() {
        let route = Route {
            scheme: Some("https".into()),
            ..Default::default()
        };
        assert!(route.matches(&url("http://example.com/")).is_none());
        assert!(route.matches(&url("https://example.com/")).is_some());
    }

    #[test]
    fn all_four_fields_combined() {
        let route = Route {
            scheme: Some("https".into()),
            host: Some("api.example.com".into()),
            host_suffix: Some("example.com".into()),
            path_prefix: Some("/v1".into()),
        };
        let m = route
            .matches(&url("https://api.example.com/v1/users"))
            .unwrap();
        assert_eq!(m.specificity, 4);
        assert!(route.matches(&url("https://api.example.com/v2/x")).is_none());
        assert!(route.matches(&url("http://api.example.com/v1/x")).is_none());
    }

    #[test]
    fn deny_unknown_fields_in_toml() {
        let bad = toml::from_str::<Route>("host = \"x\"\nbogus = \"y\"\n");
        assert!(bad.is_err());
    }
}

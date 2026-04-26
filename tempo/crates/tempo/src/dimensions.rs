use std::hash::{Hash, Hasher};

use indexmap::IndexMap;

use crate::var::VarValue;

#[derive(Debug, Clone)]
pub struct Dimensions {
    pub scenario: String,
    pub provider: String,
    pub vars: IndexMap<String, VarValue>,
}

impl PartialEq for Dimensions {
    fn eq(&self, other: &Self) -> bool {
        self.scenario == other.scenario
            && self.provider == other.provider
            && self.vars == other.vars
    }
}

impl Eq for Dimensions {}

impl Hash for Dimensions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.scenario.hash(state);
        self.provider.hash(state);
        let mut entries: Vec<(&String, &VarValue)> = self.vars.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        entries.len().hash(state);
        for (k, v) in entries {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl Dimensions {
    /// Returns the names of axes whose values differ across `slice`, in order:
    /// scenario, provider, then vars in the insertion order of `slice[0]`.
    ///
    /// Precondition: every element shares the same var key set. The matrix
    /// expander is the only producer of these slices and guarantees this.
    pub fn varying(slice: &[Dimensions]) -> Vec<&str> {
        let Some(first) = slice.first() else {
            return Vec::new();
        };
        debug_assert!(
            slice.iter().all(|d| d.vars.len() == first.vars.len()
                && d.vars.keys().all(|k| first.vars.contains_key(k))),
            "Dimensions::varying requires all elements to share the same var key set"
        );
        let mut out: Vec<&str> = Vec::new();
        if slice.iter().any(|d| d.scenario != first.scenario) {
            out.push("scenario");
        }
        if slice.iter().any(|d| d.provider != first.provider) {
            out.push("provider");
        }
        for (name, value) in &first.vars {
            let differs = slice
                .iter()
                .any(|d| d.vars.get(name.as_str()) != Some(value));
            if differs {
                out.push(name.as_str());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash(d: &Dimensions) -> u64 {
        let mut s = DefaultHasher::new();
        d.hash(&mut s);
        s.finish()
    }

    fn dim(scenario: &str, provider: &str, vars: &[(&str, VarValue)]) -> Dimensions {
        let mut m = IndexMap::new();
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
    fn equality_and_hash_invariant_to_var_insertion_order() {
        let a = dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("gpt")),
                ("max_tokens", VarValue::from(2048i64)),
            ],
        );
        let b = dim(
            "decode",
            "litellm",
            &[
                ("max_tokens", VarValue::from(2048i64)),
                ("model", VarValue::from("gpt")),
            ],
        );
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
    }

    #[test]
    fn varying_empty_when_all_constant() {
        let d = dim("decode", "litellm", &[("model", VarValue::from("gpt"))]);
        let slice = vec![d.clone(), d.clone(), d];
        assert!(Dimensions::varying(&slice).is_empty());
    }

    #[test]
    fn varying_identifies_one_axis() {
        let a = dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("gpt")),
                ("max_tokens", VarValue::from(2048i64)),
            ],
        );
        let b = dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("claude")),
                ("max_tokens", VarValue::from(2048i64)),
            ],
        );
        assert_eq!(Dimensions::varying(&[a, b]), vec!["model"]);
    }

    #[test]
    fn varying_identifies_multiple_axes_in_indexmap_order() {
        let a = dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("gpt")),
                ("topic", VarValue::from("a")),
                ("max_tokens", VarValue::from(2048i64)),
            ],
        );
        let b = dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("claude")),
                ("topic", VarValue::from("a")),
                ("max_tokens", VarValue::from(4096i64)),
            ],
        );
        assert_eq!(Dimensions::varying(&[a, b]), vec!["model", "max_tokens"]);
    }

    #[test]
    fn varying_reports_scenario_and_provider_when_they_differ() {
        let a = dim("decode", "litellm", &[("model", VarValue::from("gpt"))]);
        let b = dim("encode", "anthropic", &[("model", VarValue::from("gpt"))]);
        assert_eq!(Dimensions::varying(&[a, b]), vec!["scenario", "provider"]);
    }

    #[test]
    fn varying_empty_slice() {
        let v: Vec<Dimensions> = Vec::new();
        assert!(Dimensions::varying(&v).is_empty());
    }

    #[test]
    fn varying_single_element_slice_is_empty() {
        let d = dim("decode", "litellm", &[("model", VarValue::from("gpt"))]);
        assert!(Dimensions::varying(&[d]).is_empty());
    }
}

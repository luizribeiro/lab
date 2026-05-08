//! Closed-set placeholder substitution (scope §M8).
//!
//! Recognises exactly `${project}`, `${home}`, `${plugin}`,
//! `${cache}`, and `${state}`. No env-var interpolation, no
//! `${secret:...}`. Unknown placeholders →
//! [`ManifestError::UnknownPlaceholder`].

use crate::error::{ManifestError, PathError};
use crate::paths::PathContext;

pub fn expand(input: &str, ctx: &PathContext) -> Result<String, ManifestError> {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while !rest.is_empty() {
        if let Some(after_open) = rest.strip_prefix("${") {
            let close = after_open
                .find('}')
                .ok_or(ManifestError::MalformedPlaceholder)?;
            let name = &after_open[..close];
            let replacement = match name {
                "project" => &ctx.project_root,
                "home" => &ctx.home,
                "plugin" => &ctx.plugin_dir,
                "cache" => &ctx.cache_dir,
                "state" => &ctx.state_dir,
                _ => return Err(ManifestError::UnknownPlaceholder),
            };
            let replacement_str = replacement
                .to_str()
                .ok_or(ManifestError::MalformedPlaceholder)?;
            out.push_str(replacement_str);
            rest = &after_open[close + 1..];
        } else {
            let ch = rest.chars().next().unwrap();
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    Ok(out)
}

pub(crate) fn expand_to_path_error(input: &str, ctx: &PathContext) -> Result<String, PathError> {
    expand(input, ctx).map_err(|e| match e {
        ManifestError::UnknownPlaceholder => PathError::UnknownPlaceholder,
        ManifestError::MalformedPlaceholder => PathError::MalformedPlaceholder,
        _ => PathError::MalformedPlaceholder,
    })
}

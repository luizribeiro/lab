//! Topic / pattern grammar helpers per security RFC §5.1.
//!
//! Public so V2 (c16) and V3 (c22+) can reuse the same grammar
//! and pattern-match logic without copying it.

use crate::error::ValidationError;

/// True for chars allowed inside a topic segment: `[a-z0-9_-]`.
fn is_segment_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
}

/// True for the tool-name / manifest-name / provider-id grammar
/// `[a-z0-9_][a-z0-9_-]*` (single segment, no dots, no leading hyphen).
pub fn is_tool_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let first_ok = first.is_ascii_lowercase() || first.is_ascii_digit() || first == '_';
    first_ok && chars.all(is_segment_char)
}

/// True for the topic segment grammar `[a-z0-9_-]+` per §5.1.
pub fn is_topic_segment(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_segment_char)
}

/// True for the custom sink-class grammar `[a-z0-9_]+` (no hyphen).
pub fn is_custom_sink_class(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// True for renderer vendor / kind part grammar `[a-z][a-z0-9_-]*`.
pub fn is_vendor_or_kind_part(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase() && chars.all(is_segment_char)
}

/// Validate a topic literal: at least two segments, every segment
/// matches `[a-z0-9_-]+`. No `*` / `**` permitted.
pub fn validate_topic(topic: &str) -> Result<(), ValidationError> {
    let segs: Vec<&str> = topic.split('.').collect();
    if segs.len() < 2 {
        return Err(ValidationError::TopicTooFewSegments {
            topic: topic.to_string(),
        });
    }
    for seg in &segs {
        if !is_topic_segment(seg) {
            return Err(ValidationError::IllegalTopicSegment {
                topic: topic.to_string(),
                segment: (*seg).to_string(),
            });
        }
    }
    Ok(())
}

/// Validate a subscribe pattern: at least two segments, every
/// non-wildcard segment matches `[a-z0-9_-]+`, `**` permitted only
/// as the final segment.
pub fn validate_pattern(pattern: &str) -> Result<(), ValidationError> {
    let segs: Vec<&str> = pattern.split('.').collect();
    if segs.len() < 2 {
        return Err(ValidationError::TopicTooFewSegments {
            topic: pattern.to_string(),
        });
    }
    for (i, seg) in segs.iter().enumerate() {
        let last = i + 1 == segs.len();
        if *seg == "*" {
            continue;
        }
        if *seg == "**" {
            if !last {
                return Err(ValidationError::InvalidPatternSegment {
                    pattern: pattern.to_string(),
                    segment: (*seg).to_string(),
                });
            }
            continue;
        }
        if !is_topic_segment(seg) {
            return Err(ValidationError::InvalidPatternSegment {
                pattern: pattern.to_string(),
                segment: (*seg).to_string(),
            });
        }
    }
    Ok(())
}

/// True iff `pattern` matches `topic` under the §5.1 grammar:
/// `*` matches one segment, `**` (final only) matches one or more
/// trailing segments, other segments match by exact equality.
///
/// Inputs are assumed to be well-formed (`validate_pattern` /
/// `validate_topic` checked); a malformed pattern simply yields
/// `false`.
pub fn pattern_matches_topic(pattern: &str, topic: &str) -> bool {
    let psegs: Vec<&str> = pattern.split('.').collect();
    let tsegs: Vec<&str> = topic.split('.').collect();
    let mut pi = 0;
    let mut ti = 0;
    while pi < psegs.len() && ti < tsegs.len() {
        if psegs[pi] == "**" {
            return pi + 1 == psegs.len() && ti < tsegs.len();
        }
        if psegs[pi] == "*" || psegs[pi] == tsegs[ti] {
            pi += 1;
            ti += 1;
        } else {
            return false;
        }
    }
    pi == psegs.len() && ti == tsegs.len()
}

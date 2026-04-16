#![allow(dead_code)]

/// Shell-style pattern match for env var names. Byte-level; correct
/// for ASCII names only.
pub fn matches(pattern: &str, input: &str) -> bool {
    match_bytes(pattern.as_bytes(), input.as_bytes())
}

fn match_bytes(pat: &[u8], inp: &[u8]) -> bool {
    if pat.is_empty() {
        return inp.is_empty();
    }
    match pat[0] {
        b'*' => (0..=inp.len()).any(|i| match_bytes(&pat[1..], &inp[i..])),
        b'?' => !inp.is_empty() && match_bytes(&pat[1..], &inp[1..]),
        b'[' if !inp.is_empty() => match class_match(pat, inp[0]) {
            Some((true, len)) => match_bytes(&pat[len..], &inp[1..]),
            Some((false, _)) => false,
            None => inp[0] == b'[' && match_bytes(&pat[1..], &inp[1..]),
        },
        b'[' => false,
        c => !inp.is_empty() && inp[0] == c && match_bytes(&pat[1..], &inp[1..]),
    }
}

fn class_match(pat: &[u8], c: u8) -> Option<(bool, usize)> {
    let mut i = 1;
    let negated = i < pat.len() && (pat[i] == b'!' || pat[i] == b'^');
    if negated {
        i += 1;
    }
    let start = i;
    let mut matched = false;
    while i < pat.len() && pat[i] != b']' {
        if i + 2 < pat.len() && pat[i + 1] == b'-' && pat[i + 2] != b']' {
            if c >= pat[i] && c <= pat[i + 2] {
                matched = true;
            }
            i += 3;
        } else {
            if pat[i] == c {
                matched = true;
            }
            i += 1;
        }
    }
    if i >= pat.len() || i == start {
        return None;
    }
    Some((matched ^ negated, i + 1))
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn literal_match() {
        assert!(matches("FOO", "FOO"));
        assert!(!matches("FOO", "BAR"));
        assert!(!matches("FOO", "FOOBAR"));
        assert!(!matches("FOO", "BARFOO"));
    }

    #[test]
    fn star_prefix() {
        assert!(matches("AWS_*", "AWS_SECRET"));
        assert!(matches("AWS_*", "AWS_"));
        assert!(!matches("AWS_*", "AW"));
        assert!(!matches("AWS_*", "AWS"));
    }

    #[test]
    fn star_suffix() {
        assert!(matches("*_TOKEN", "GITHUB_TOKEN"));
        assert!(matches("*_TOKEN", "_TOKEN"));
        assert!(!matches("*_TOKEN", "TOKEN"));
    }

    #[test]
    fn star_matches_empty() {
        assert!(matches("*", ""));
        assert!(matches("*", "anything"));
        assert!(matches("FOO*", "FOO"));
    }

    #[test]
    fn star_in_middle() {
        assert!(matches("AWS_*_KEY", "AWS_SECRET_KEY"));
        assert!(matches("AWS_*_KEY", "AWS__KEY"));
        assert!(!matches("AWS_*_KEY", "AWS_KEY"));
    }

    #[test]
    fn multiple_stars() {
        assert!(matches("*_*_*", "A_B_C"));
        assert!(matches("**", "anything"));
        assert!(matches("***", ""));
    }

    #[test]
    fn question_mark() {
        assert!(matches("FOO?", "FOOA"));
        assert!(!matches("FOO?", "FOO"));
        assert!(!matches("FOO?", "FOOAB"));
        assert!(matches("?OO", "FOO"));
    }

    #[test]
    fn character_class_literal() {
        assert!(matches("[AB]WS", "AWS"));
        assert!(matches("[AB]WS", "BWS"));
        assert!(!matches("[AB]WS", "CWS"));
    }

    #[test]
    fn character_class_range() {
        assert!(matches("[A-Z]OO", "FOO"));
        assert!(!matches("[A-Z]OO", "foo"));
        assert!(matches("[0-9]", "5"));
    }

    #[test]
    fn character_class_negated() {
        assert!(matches("[!AB]OO", "COO"));
        assert!(!matches("[!AB]OO", "AOO"));
        assert!(matches("[^AB]OO", "COO"));
    }

    #[test]
    fn empty_pattern() {
        assert!(matches("", ""));
        assert!(!matches("", "anything"));
    }

    #[test]
    fn malformed_class_is_literal() {
        assert!(matches("[unclosed", "[unclosed"));
        assert!(matches("[]empty", "[]empty"));
        assert!(matches("[a-", "[a-"));
    }

    #[test]
    fn anchored_match() {
        assert!(!matches("FOO", "xFOOx"));
        assert!(!matches("*FOO", "FOOx"));
        assert!(!matches("FOO*", "xFOO"));
    }
}

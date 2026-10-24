//! E014: two elements' `at` patterns match the same file.

use saxl_core::{Diagnostic, ElementKind, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E014"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "two elements' `at` patterns match the same file"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let globs: Vec<_> = cx
            .model
            .elements
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Api | ElementKind::Module))
            .filter_map(|e| e.at.as_ref().map(|g| (e, g)))
            .collect();
        for (i, (element, glob)) in globs.iter().enumerate() {
            let Some((first, first_glob)) = globs[..i]
                .iter()
                .find(|(_, g)| overlaps(&g.text, &glob.text))
            else {
                continue;
            };
            out.push(
                Diagnostic::error(
                    "E014",
                    format!(
                        "`{}` and `{}` claim the same files",
                        first.name.text, element.name.text
                    ),
                    glob.span,
                    format!("overlaps `{}`", first_glob.text),
                )
                .with_secondary(first_glob.span, format!("`{}` is here", first.name.text))
                .with_help("narrow one pattern so each file belongs to one element"),
            );
        }
    }
}

/// Whether two patterns can match the same path, judged from the patterns
/// alone. A literal path is matched against the other pattern; two globs
/// overlap when they are equal or one is `dir/**` covering the other's
/// directory. Other glob pairs are assumed not to overlap.
fn overlaps(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (is_literal(a), is_literal(b)) {
        (true, true) => false,
        (true, false) => glob_match(b, a),
        (false, true) => glob_match(a, b),
        (false, false) => covers(a, b) || covers(b, a),
    }
}

fn is_literal(pattern: &str) -> bool {
    !pattern.contains(['*', '?', '[', '{'])
}

/// Whether `outer` is `dir/**` and `inner` starts with `dir/`.
fn covers(outer: &str, inner: &str) -> bool {
    outer
        .strip_suffix("**")
        .filter(|dir| is_literal(dir) && (dir.is_empty() || dir.ends_with('/')))
        .is_some_and(|dir| inner.starts_with(dir))
}

/// Matches `path` against `pattern`, where `*` and `?` stay within one path
/// segment and `**` matches any number of segments.
fn glob_match(pattern: &str, path: &str) -> bool {
    fn go(p: &[u8], s: &[u8]) -> bool {
        match p {
            [] => s.is_empty(),
            [b'*', b'*', b'/', rest @ ..] => {
                go(rest, s) || (0..s.len()).any(|i| s[i] == b'/' && go(rest, &s[i + 1..]))
            }
            [b'*', b'*', rest @ ..] => (0..=s.len()).any(|i| go(rest, &s[i..])),
            [b'*', rest @ ..] => (0..=s.len())
                .take_while(|&i| i == 0 || s[i - 1] != b'/')
                .any(|i| go(rest, &s[i..])),
            [b'?', rest @ ..] => s.first().is_some_and(|&c| c != b'/') && go(rest, &s[1..]),
            [c, rest @ ..] => s.first() == Some(c) && go(rest, &s[1..]),
        }
    }
    go(pattern.as_bytes(), path.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching() {
        assert!(glob_match("src/**", "src/a/b.rs"));
        assert!(glob_match("src/*.rs", "src/a.rs"));
        assert!(!glob_match("src/*.rs", "src/a/b.rs"));
        assert!(glob_match("src/**/mod.rs", "src/mod.rs"));
        assert!(glob_match("src/**/mod.rs", "src/a/b/mod.rs"));
        assert!(glob_match("src/?.rs", "src/a.rs"));
        assert!(!glob_match("src/?.rs", "src/ab.rs"));
    }

    #[test]
    fn overlapping() {
        assert!(overlaps("src/**", "src/escape.rs"));
        assert!(overlaps("src/html/**", "src/html/*.rs"));
        assert!(overlaps("src/a.rs", "src/a.rs"));
        assert!(!overlaps("src/a.rs", "src/b.rs"));
        assert!(!overlaps("src/blocks/**", "src/inlines.rs"));
        assert!(!overlaps("src/html/**", "src/*.rs"));
    }
}

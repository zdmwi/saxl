//! Lexer and parser for saxl. Syntax only: names are resolved by the validator.

mod ast;
mod format;
pub mod lexer;
mod lower;
mod parser;

pub use format::format;
use saxl_core::{Diagnostic, FileId, Model};

/// The parser's output. `model` always exists; statements that failed to parse
/// are left out of it.
#[derive(Debug)]
pub struct Parsed {
    /// The lowered model.
    pub model: Model,
    /// Parse diagnostics (`P` codes), in source order.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses `source`. Never panics.
pub fn parse(source: &str, file: FileId) -> Parsed {
    let lexed = lexer::lex(source, file);
    let len = u32::try_from(source.len()).unwrap_or(u32::MAX);
    let (stmts, parse_diags) = parser::Parser::new(&lexed.tokens, file, len).file();
    let mut diagnostics = lexed.diagnostics;
    diagnostics.extend(parse_diags);
    diagnostics.sort_by_key(Diagnostic::sort_key);
    Parsed {
        model: lower::lower(stmts),
        diagnostics,
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod tests {
    use super::*;

    fn codes(src: &str) -> Vec<&'static str> {
        parse(src, FileId(0))
            .diagnostics
            .iter()
            .map(|d| d.code.0)
            .collect()
    }

    #[test]
    fn snip_parses_cleanly() {
        let parsed = parse(include_str!("../../../examples/snip.saxl"), FileId(0));
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let m = &parsed.model;
        assert_eq!(m.systems.len(), 1);
        assert_eq!(m.elements.len(), 11);
        assert_eq!(m.relations.len(), 12);
        assert_eq!(m.data.len(), 3);
        assert_eq!(m.regions.len(), 1);
        assert_eq!(m.scenarios.len(), 2);
        assert_eq!(m.allows.len(), 1);
    }

    #[test]
    fn recovery_reports_every_error() {
        let src = "system S\nservice\nA -> B \"x\" \"y\"\nstore DB\nqueue Q Q\nB -> DB\n";
        let parsed = parse(src, FileId(0));
        assert_eq!(codes(src), vec!["P001", "P001", "P001"]);
        assert_eq!(parsed.model.elements.len(), 1);
        assert_eq!(parsed.model.relations.len(), 1);
    }

    #[test]
    fn recovery_inside_blocks() {
        let src = "scenario s {\n  A -> B\n  A B\n  parallel {\n    -> C\n    B --> A\n  }\n}\n";
        let parsed = parse(src, FileId(0));
        assert_eq!(codes(src), vec!["P001", "P001"]);
        let steps = &parsed.model.scenarios[0].steps;
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn unclosed_brace() {
        assert_eq!(
            codes("deploy region r {\n  cluster k {\n    A\n  }\n"),
            vec!["P003"]
        );
    }

    #[test]
    fn chain_description() {
        assert_eq!(codes("A -> B -> C \"d\"\n"), vec!["P004"]);
    }

    #[test]
    fn rule_codes() {
        assert_eq!(codes("allow W1 \"r\"\n"), vec!["P005"]);
        assert_eq!(codes("allow \"r\"\n"), vec!["P005"]);
        assert!(codes("allow P001 \"r\"\n").is_empty());
    }

    #[test]
    fn replica_counts() {
        let src = "deploy region r {\n  managed {\n    A x0 autoscale\n    B x01\n    C x12 foo bar+baz\n  }\n}\n";
        assert_eq!(codes(src), vec!["P007", "P007"]);
    }

    #[test]
    fn chains_expand_and_merge() {
        let parsed = parse("A ~> B -> C\nB -> C\nA ~> B \"d\"\nA -> B\n", FileId(0));
        let rels: Vec<_> = parsed
            .model
            .relations
            .iter()
            .map(|r| format!("{}{}{}", r.from.text, r.style.arrow(), r.to.text))
            .collect();
        assert_eq!(rels, vec!["A~>B", "B->C", "A->B"]);
    }

    #[test]
    fn missing_final_newline() {
        assert!(codes("system S").is_empty());
        assert!(codes("scenario s {\n  A -> B : x\n}").is_empty());
    }

    #[test]
    fn stray_tokens_terminate() {
        assert_eq!(codes("}\n} }\n"), vec!["P001", "P001"]);
        assert_eq!(codes("-> ->\n->"), vec!["P001", "P001"]);
    }
}

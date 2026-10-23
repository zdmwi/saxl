//! Canonical formatting. Works on the token stream plus the lexer's comment
//! table, so every token and comment survives; only whitespace changes.
//!
//! - One space between tokens, none before `,` or around `+`.
//! - Two spaces of indentation per open brace.
//! - Runs of consecutive top-level elements, relations (including `impl`
//!   lines), and single-line data statements are aligned in columns (a blank
//!   or comment line ends a run). An element's `at` clause gets its own column.
//! - At most one blank line in a row, none just inside braces, and none at
//!   the start or end of the file. A trailing comment follows one space.

use saxl_core::{Diagnostic, FileId, Span};

use crate::lexer::{self, Tok};

/// Formats `source`. Fails with the parse diagnostics if it does not parse.
pub fn format(source: &str) -> Result<String, Vec<Diagnostic>> {
    let parsed = crate::parse(source, FileId(0));
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics);
    }
    let lexed = lexer::lex(source, FileId(0));
    let lines = source_lines(source, &lexed.tokens, &lexed.comments);

    let mut out: Vec<Line> = Vec::new();
    let mut depth: usize = 0;
    for line in &lines {
        let closes = matches!(line.tokens.first(), Some(Tok::RBrace));
        let indent = depth.saturating_sub(usize::from(closes));
        for tok in &line.tokens {
            match tok {
                Tok::LBrace => depth += 1,
                Tok::RBrace => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if line.tokens.is_empty() && line.comment.is_none() {
            // Blank: keep one, but not at the start, twice, or after `{`.
            let after_open = out.last().is_some_and(|l| l.opens);
            if !out.is_empty() && !out.last().is_some_and(Line::is_blank) && !after_open {
                out.push(Line::blank());
            }
            continue;
        }
        if closes && out.last().is_some_and(Line::is_blank) {
            out.pop();
        }
        out.push(Line {
            indent,
            cells: if line.tokens.is_empty() {
                (Kind::Other, Vec::new())
            } else {
                cells(&line.tokens, indent)
            },
            comment: line.comment.clone(),
            opens: matches!(line.tokens.last(), Some(Tok::LBrace)),
        });
    }
    while out.last().is_some_and(Line::is_blank) {
        out.pop();
    }
    align(&mut out);

    let mut text = String::new();
    for line in &out {
        text.push_str(&line.render());
        text.push('\n');
    }
    Ok(text)
}

/// One source line: its tokens and its comment, if any.
struct SourceLine {
    tokens: Vec<Tok>,
    comment: Option<String>,
}

fn source_lines(source: &str, tokens: &[(Tok, Span)], comments: &[Span]) -> Vec<SourceLine> {
    let mut lines = vec![SourceLine {
        tokens: Vec::new(),
        comment: None,
    }];
    let mut comments = comments.iter().peekable();
    for (tok, span) in tokens {
        // A comment ends its line, so it belongs to the line before the next newline.
        while let Some(c) = comments.next_if(|c| c.start < span.start) {
            if let Some(line) = lines.last_mut() {
                line.comment = Some(comment_text(source, c));
            }
        }
        if *tok == Tok::Newline {
            lines.push(SourceLine {
                tokens: Vec::new(),
                comment: None,
            });
        } else if let Some(line) = lines.last_mut() {
            line.tokens.push(tok.clone());
        }
    }
    for c in comments {
        if let Some(line) = lines.last_mut() {
            line.comment = Some(comment_text(source, c));
        }
    }
    lines
}

fn comment_text(source: &str, span: &Span) -> String {
    source
        .get(span.start as usize..span.end as usize)
        .unwrap_or("#")
        .trim_end()
        .to_owned()
}

/// What a line is, for alignment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Element,
    Relation,
    Data,
    Other,
}

/// A formatted line, split into cells for alignment.
struct Line {
    indent: usize,
    /// `(kind, cells)`. Aligned kinds pad every cell but the last.
    cells: (Kind, Vec<String>),
    comment: Option<String>,
    opens: bool,
}

impl Line {
    fn blank() -> Self {
        Self {
            indent: 0,
            cells: (Kind::Other, Vec::new()),
            comment: None,
            opens: false,
        }
    }

    fn is_blank(&self) -> bool {
        self.cells.1.is_empty() && self.comment.is_none()
    }

    fn render(&self) -> String {
        let mut s = "  ".repeat(self.indent);
        s.push_str(&self.cells.1.join(" "));
        let s = s.trim_end().to_owned();
        match &self.comment {
            Some(c) if self.cells.1.is_empty() => format!("{}{c}", "  ".repeat(self.indent)),
            Some(c) => format!("{s} {c}"),
            None => s,
        }
    }
}

fn token_text(tok: &Tok) -> String {
    match tok {
        Tok::Ident(s) => s.clone(),
        Tok::Str(s) => format!("\"{s}\""),
        Tok::Label(s) if s.is_empty() => ":".to_owned(),
        Tok::Label(s) => format!(": {s}"),
        other => other.text().to_owned(),
    }
}

/// Joins tokens with canonical spacing.
fn join(tokens: &[Tok]) -> String {
    let mut out = String::new();
    for (i, tok) in tokens.iter().enumerate() {
        let glued = matches!(tok, Tok::Comma | Tok::Plus) || (i > 0 && tokens[i - 1] == Tok::Plus);
        if i > 0 && !glued {
            out.push(' ');
        }
        out.push_str(&token_text(tok));
    }
    out
}

/// Splits a line into alignment cells. Only top-level lines are aligned.
fn cells(tokens: &[Tok], indent: usize) -> (Kind, Vec<String>) {
    let other = || (Kind::Other, vec![join(tokens)]);
    if indent > 0 {
        return other();
    }
    match tokens {
        [
            kind @ (Tok::Actor
            | Tok::External
            | Tok::Service
            | Tok::Store
            | Tok::Queue
            | Tok::Consumer
            | Tok::Package
            | Tok::Api
            | Tok::Module
            | Tok::Hook),
            name,
            rest @ ..,
        ] => {
            // Every element line has four cells so a run aligns even when only
            // some lines have a description or an `at` clause.
            let at = rest
                .iter()
                .position(|t| matches!(t, Tok::Ident(w) if w == "at"))
                .unwrap_or(rest.len());
            (
                Kind::Element,
                vec![
                    token_text(kind),
                    token_text(name),
                    join(&rest[..at]),
                    join(&rest[at..]),
                ],
            )
        }
        [Tok::Ident(first), Tok::Arrow | Tok::AsyncArrow, ..] => {
            (Kind::Relation, vec![first.clone(), join(&tokens[1..])])
        }
        [Tok::Ident(first), Tok::Ident(word), ..] if word == "impl" => {
            (Kind::Relation, vec![first.clone(), join(&tokens[1..])])
        }
        [Tok::Data, name, rest @ ..] if rest.last() == Some(&Tok::RBrace) => {
            let Some(open) = rest.iter().position(|t| *t == Tok::LBrace) else {
                return other();
            };
            (
                Kind::Data,
                vec![
                    "data".to_owned(),
                    token_text(name),
                    join(&rest[..open]),
                    join(&rest[open..]),
                ],
            )
        }
        _ => other(),
    }
}

/// Pads the cells of each run of same-kind lines to common widths.
fn align(lines: &mut [Line]) {
    let mut start = 0;
    while start < lines.len() {
        let kind = lines[start].cells.0;
        let mut end = start + 1;
        while end < lines.len() && lines[end].cells.0 == kind && kind != Kind::Other {
            end += 1;
        }
        if kind != Kind::Other {
            let columns = lines[start].cells.1.len();
            for col in 0..columns.saturating_sub(1) {
                let width = lines[start..end]
                    .iter()
                    .map(|l| l.cells.1[col].chars().count())
                    .max()
                    .unwrap_or(0);
                for line in &mut lines[start..end] {
                    let cell = &mut line.cells.1[col];
                    let pad = width - cell.chars().count();
                    cell.extend(std::iter::repeat_n(' ', pad));
                }
            }
        }
        start = end;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::format;

    #[test]
    fn spacing_indentation_and_alignment() {
        let messy = "\n\n# Shop\nsystem   Shop\nactor U \"user\"\nservice   Web   \"frontend\"\nstore DB\n\n\n\nU->Web\nWeb  ~>DB\"writes\"\ndeploy region eu {\n\n cluster main {\n   Web,DB x2   primary +replica # managed\n\n }\n}\nscenario s {\n  U->Web:  GET /\n  parallel {\n Web-->U :\n}\n}\n\n";
        let want = "# Shop\nsystem Shop\nactor   U   \"user\"\nservice Web \"frontend\"\nstore   DB\n\nU   -> Web\nWeb ~> DB \"writes\"\ndeploy region eu {\n  cluster main {\n    Web, DB x2 primary+replica # managed\n  }\n}\nscenario s {\n  U -> Web : GET /\n  parallel {\n    Web --> U :\n  }\n}\n";
        assert_eq!(format(messy).unwrap(), want);
        assert_eq!(format(want).unwrap(), want);
    }

    #[test]
    fn data_alignment_and_wrapping() {
        let src = "system S\nstore Db\nstore C\nqueue Q\ndata Link in Db cached C { code, url }\ndata User in Db { id }\ndata Click via Q -> Db {\n  ts,   # when\n  ref,\n}\n";
        let want = "system S\nstore Db\nstore C\nqueue Q\ndata Link in Db cached C { code, url }\ndata User in Db          { id }\ndata Click via Q -> Db {\n  ts, # when\n  ref,\n}\n";
        assert_eq!(format(src).unwrap(), want);
    }

    #[test]
    fn library_alignment() {
        let src = "library L \"lib\"\napi parse \"text\" at \"src/parse.rs\"\nmodule lexer at \"src/lexer.rs\"\nhook Highlighter \"colors code\"\nparse -> lexer\nlexer impl Highlighter\ndata Doc in lexer pub { blocks }\ndata Token in lexer { kind }\npublish \"crates.io\" {\n  crate l   target wasm32 {\n    parse,lexer  feature   syntax\n  }\n}\n";
        let want = "library L \"lib\"\napi    parse       \"text\"        at \"src/parse.rs\"\nmodule lexer                     at \"src/lexer.rs\"\nhook   Highlighter \"colors code\"\nparse -> lexer\nlexer impl Highlighter\ndata Doc   in lexer pub { blocks }\ndata Token in lexer     { kind }\npublish \"crates.io\" {\n  crate l target wasm32 {\n    parse, lexer feature syntax\n  }\n}\n";
        assert_eq!(format(src).unwrap(), want);
        assert_eq!(format(want).unwrap(), want);
    }

    #[test]
    fn refuses_parse_errors() {
        let err = format("system\n").unwrap_err();
        assert_eq!(err[0].code.0, "P001");
    }
}

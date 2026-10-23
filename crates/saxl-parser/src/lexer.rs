//! Tokens and the lexer wrapper.

use logos::Logos;
use saxl_core::{Diagnostic, FileId, Span};

/// A saxl token.
#[derive(Logos, Debug, Clone, PartialEq, Eq)]
#[logos(skip r"[ \t\r]+")]
pub enum Tok {
    /// A line break. Statements end here.
    #[token("\n")]
    Newline,
    /// A `#` comment. Removed by [`lex`] and kept as trivia.
    #[regex(r"#[^\n]*", allow_greedy = true)]
    Comment,
    /// `system`
    #[token("system")]
    System,
    /// `actor`
    #[token("actor")]
    Actor,
    /// `external`
    #[token("external")]
    External,
    /// `service`
    #[token("service")]
    Service,
    /// `store`
    #[token("store")]
    Store,
    /// `queue`
    #[token("queue")]
    Queue,
    /// `data`
    #[token("data")]
    Data,
    /// `in`
    #[token("in")]
    In,
    /// `cached`
    #[token("cached")]
    Cached,
    /// `via`
    #[token("via")]
    Via,
    /// `deploy`
    #[token("deploy")]
    Deploy,
    /// `region`
    #[token("region")]
    Region,
    /// `cdn`
    #[token("cdn")]
    Cdn,
    /// `cluster`
    #[token("cluster")]
    Cluster,
    /// `managed`
    #[token("managed")]
    Managed,
    /// `scenario`
    #[token("scenario")]
    Scenario,
    /// `parallel`
    #[token("parallel")]
    Parallel,
    /// `allow`
    #[token("allow")]
    Allow,
    /// `->`
    #[token("->")]
    Arrow,
    /// `~>`
    #[token("~>")]
    AsyncArrow,
    /// `-->`
    #[token("-->")]
    Reply,
    /// `{`
    #[token("{")]
    LBrace,
    /// `}`
    #[token("}")]
    RBrace,
    /// `,`
    #[token(",")]
    Comma,
    /// `+`
    #[token("+")]
    Plus,
    /// An identifier. Hyphens may join alphanumeric segments.
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*(-[A-Za-z0-9_]+)*", |l| l.slice().to_owned())]
    Ident(String),
    /// A one-line string, without its quotes.
    #[regex(r#""[^"\n]*""#, |l| { let s = l.slice(); s[1..s.len() - 1].to_owned() })]
    Str(String),
    /// `:` and the rest of the line, trimmed.
    #[regex(r":[^\n]*", |l| l.slice()[1..].trim().to_owned(), allow_greedy = true)]
    Label(String),
}

impl Tok {
    /// How the token is described in "expected …, found …" messages.
    pub fn describe(&self) -> String {
        match self {
            Tok::Newline => "end of line".to_owned(),
            Tok::Comment => "comment".to_owned(),
            Tok::Ident(s) => format!("name `{s}`"),
            Tok::Str(_) => "string".to_owned(),
            Tok::Label(_) => "label".to_owned(),
            other => format!("`{}`", other.text()),
        }
    }

    /// The fixed text of a keyword or punctuation token.
    pub fn text(&self) -> &'static str {
        match self {
            Tok::System => "system",
            Tok::Actor => "actor",
            Tok::External => "external",
            Tok::Service => "service",
            Tok::Store => "store",
            Tok::Queue => "queue",
            Tok::Data => "data",
            Tok::In => "in",
            Tok::Cached => "cached",
            Tok::Via => "via",
            Tok::Deploy => "deploy",
            Tok::Region => "region",
            Tok::Cdn => "cdn",
            Tok::Cluster => "cluster",
            Tok::Managed => "managed",
            Tok::Scenario => "scenario",
            Tok::Parallel => "parallel",
            Tok::Allow => "allow",
            Tok::Arrow => "->",
            Tok::AsyncArrow => "~>",
            Tok::Reply => "-->",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::Comma => ",",
            Tok::Plus => "+",
            Tok::Newline | Tok::Comment | Tok::Ident(_) | Tok::Str(_) | Tok::Label(_) => "",
        }
    }
}

/// The lexer's output.
#[derive(Debug, Default)]
pub struct Lexed {
    /// Tokens, without comments.
    pub tokens: Vec<(Tok, Span)>,
    /// Comment spans, for the formatter.
    pub comments: Vec<Span>,
    /// P002 and P006 diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Lexes `source`. Never fails: bad input becomes diagnostics.
pub fn lex(source: &str, file: FileId) -> Lexed {
    let mut out = Lexed::default();
    let mut lexer = Tok::lexer(source);
    while let Some(result) = lexer.next() {
        let range = lexer.span();
        let span = Span::new(file, to_u32(range.start), to_u32(range.end));
        match result {
            Ok(Tok::Comment) => out.comments.push(span),
            Ok(tok) => out.tokens.push((tok, span)),
            Err(()) if source[range.start..].starts_with('"') => {
                // Unterminated string: swallow the rest of the line so it does not
                // produce follow-on errors.
                let eol = source[range.start..]
                    .find('\n')
                    .map_or(source.len(), |i| range.start + i);
                lexer.bump(eol.saturating_sub(range.end));
                out.diagnostics.push(Diagnostic::error(
                    "P002",
                    "unterminated string",
                    Span::new(file, span.start, to_u32(eol)),
                    "string is not closed before the end of the line",
                ));
            }
            Err(()) => {
                let ch = source[range.start..].chars().next().unwrap_or('?');
                let end = range.start + ch.len_utf8();
                if end > range.end {
                    lexer.bump(end - range.end);
                }
                out.diagnostics.push(Diagnostic::error(
                    "P006",
                    format!("unrecognized character `{}`", ch.escape_debug()),
                    Span::new(file, span.start, to_u32(end.max(range.end))),
                    "not valid here",
                ));
            }
        }
    }
    out
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        let lexed = lex(src, FileId(0));
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        lexed.tokens.into_iter().map(|(t, _)| t).collect()
    }

    fn id(s: &str) -> Tok {
        Tok::Ident(s.to_owned())
    }

    #[test]
    fn keywords() {
        let src = "system actor external service store queue data in cached via deploy region cdn cluster managed scenario parallel allow";
        assert_eq!(
            toks(src),
            vec![
                Tok::System,
                Tok::Actor,
                Tok::External,
                Tok::Service,
                Tok::Store,
                Tok::Queue,
                Tok::Data,
                Tok::In,
                Tok::Cached,
                Tok::Via,
                Tok::Deploy,
                Tok::Region,
                Tok::Cdn,
                Tok::Cluster,
                Tok::Managed,
                Tok::Scenario,
                Tok::Parallel,
                Tok::Allow,
            ]
        );
    }

    #[test]
    fn keyword_prefix_is_identifier() {
        assert_eq!(toks("services index"), vec![id("services"), id("index")]);
    }

    #[test]
    fn punctuation_and_arrows() {
        assert_eq!(
            toks("{ } , + -> ~> -->\n"),
            vec![
                Tok::LBrace,
                Tok::RBrace,
                Tok::Comma,
                Tok::Plus,
                Tok::Arrow,
                Tok::AsyncArrow,
                Tok::Reply,
                Tok::Newline,
            ]
        );
    }

    #[test]
    fn probe_cases() {
        assert_eq!(toks("A->B"), vec![id("A"), Tok::Arrow, id("B")]);
        assert_eq!(toks("A-->B"), vec![id("A"), Tok::Reply, id("B")]);
        assert_eq!(toks("A~>B"), vec![id("A"), Tok::AsyncArrow, id("B")]);
        assert_eq!(toks("us-east-1"), vec![id("us-east-1")]);
        assert_eq!(toks("Edge-1->B"), vec![id("Edge-1"), Tok::Arrow, id("B")]);
        assert_eq!(
            toks("primary+replica"),
            vec![id("primary"), Tok::Plus, id("replica")]
        );
    }

    #[test]
    fn strings_labels_comments() {
        assert_eq!(
            toks(r#""code → URL cache""#),
            vec![Tok::Str("code → URL cache".to_owned())]
        );
        assert_eq!(
            toks("A -> B :  get #abc \nC"),
            vec![
                id("A"),
                Tok::Arrow,
                id("B"),
                Tok::Label("get #abc".to_owned()),
                Tok::Newline,
                id("C")
            ]
        );
        let lexed = lex("A # comment\nB", FileId(0));
        assert_eq!(lexed.comments, vec![Span::new(FileId(0), 2, 11)]);
        assert_eq!(lexed.tokens.len(), 3);
    }

    #[test]
    fn unterminated_string() {
        let lexed = lex("service A \"abc def\nstore B", FileId(0));
        assert_eq!(lexed.diagnostics.len(), 1);
        assert_eq!(lexed.diagnostics[0].code.0, "P002");
        assert_eq!(
            lexed.diagnostics[0].labels[0].span,
            Span::new(FileId(0), 10, 18)
        );
        let kinds: Vec<_> = lexed.tokens.into_iter().map(|(t, _)| t).collect();
        assert_eq!(
            kinds,
            vec![Tok::Service, id("A"), Tok::Newline, Tok::Store, id("B")]
        );
    }

    #[test]
    fn unrecognized_character() {
        let lexed = lex("A @ é B", FileId(0));
        let codes: Vec<_> = lexed.diagnostics.iter().map(|d| d.code.0).collect();
        assert_eq!(codes, vec!["P006", "P006"]);
        assert_eq!(
            lexed.diagnostics[1].labels[0].span,
            Span::new(FileId(0), 4, 6)
        );
        assert_eq!(lexed.tokens.len(), 2);
    }

    #[test]
    fn snip_lexes_cleanly() {
        let src = include_str!("../../../examples/snip.saxl");
        let lexed = lex(src, FileId(0));
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    }
}

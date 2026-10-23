//! Recursive descent over the token vector, with line-based error recovery.

use saxl_core::{Attribute, Diagnostic, ElementKind, FileId, GroupKind, Span, StepArrow, Style};

use crate::ast::{DeployItem, Ident, Placement, Step, Stmt, Workload};
use crate::lexer::Tok;

/// The diagnostic is already recorded when `Err` is returned.
type PResult<T> = Result<T, ()>;

pub(crate) struct Parser<'t> {
    tokens: &'t [(Tok, Span)],
    pos: usize,
    /// Spans of `{` tokens not yet closed.
    open: Vec<Span>,
    /// Span of the last consumed token other than a newline.
    prev: Span,
    eof: Span,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'t> Parser<'t> {
    pub fn new(tokens: &'t [(Tok, Span)], file: FileId, len: u32) -> Self {
        let eof = Span::new(file, len, len);
        Self {
            tokens,
            pos: 0,
            open: Vec::new(),
            prev: eof,
            eof,
            diagnostics: Vec::new(),
        }
    }

    /// Parses the whole file.
    pub fn file(mut self) -> (Vec<Stmt>, Vec<Diagnostic>) {
        let mut stmts = Vec::new();
        while let Some(tok) = self.peek() {
            if *tok == Tok::Newline {
                self.bump();
                continue;
            }
            let mark = self.mark();
            match self.statement() {
                Ok(stmt) => stmts.push(stmt),
                Err(()) => self.recover(mark),
            }
        }
        for span in std::mem::take(&mut self.open) {
            self.diagnostics.push(
                Diagnostic::error(
                    "P003",
                    "unclosed `{` at end of file",
                    span,
                    "this `{` is never closed",
                )
                .with_help("add a matching `}` on its own line"),
            );
        }
        (stmts, self.diagnostics)
    }

    // ---- token helpers ----

    fn peek(&self) -> Option<&'t Tok> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn peek_span(&self) -> Span {
        self.tokens.get(self.pos).map_or(self.eof, |(_, s)| *s)
    }

    fn at(&self, tok: &Tok) -> bool {
        self.peek() == Some(tok)
    }

    fn bump(&mut self) -> Span {
        let Some((tok, span)) = self.tokens.get(self.pos) else {
            return self.eof;
        };
        self.pos += 1;
        match tok {
            Tok::LBrace => self.open.push(*span),
            Tok::RBrace => {
                self.open.pop();
            }
            _ => {}
        }
        if *tok != Tok::Newline {
            self.prev = *span;
        }
        *span
    }

    fn found(&self) -> String {
        self.peek()
            .map_or_else(|| "end of file".to_owned(), Tok::describe)
    }

    fn error<T>(&mut self, expected: &str) -> PResult<T> {
        let found = self.found();
        let span = self.peek_span();
        self.diagnostics.push(Diagnostic::error(
            "P001",
            format!("expected {expected}, found {found}"),
            span,
            format!("expected {expected}"),
        ));
        Err(())
    }

    fn expect(&mut self, tok: Tok) -> PResult<Span> {
        if self.at(&tok) {
            Ok(self.bump())
        } else {
            self.error(&format!("`{}`", tok.text()))
        }
    }

    fn ident(&mut self, what: &str) -> PResult<Ident> {
        match self.peek() {
            Some(Tok::Ident(text)) => {
                let span = self.bump();
                Ok(Ident {
                    text: text.clone(),
                    span,
                })
            }
            _ => self.error(what),
        }
    }

    fn opt_str(&mut self) -> Option<String> {
        match self.peek() {
            Some(Tok::Str(s)) => {
                self.bump();
                Some(s.clone())
            }
            _ => None,
        }
    }

    /// A statement ends at a newline or the end of the file.
    fn end_line(&mut self) -> PResult<()> {
        match self.peek() {
            None => Ok(()),
            Some(Tok::Newline) => {
                self.bump();
                Ok(())
            }
            Some(_) => self.error("end of line"),
        }
    }

    // ---- recovery ----

    fn mark(&self) -> (usize, usize) {
        (self.pos, self.open.len())
    }

    /// Skips to the end of the line at the brace depth where the failed
    /// statement started. Stops before a `}` that closes the enclosing block.
    /// Always consumes at least one token.
    fn recover(&mut self, (start_pos, start_depth): (usize, usize)) {
        loop {
            match self.peek() {
                None => break,
                Some(Tok::Newline) if self.open.len() <= start_depth => {
                    self.bump();
                    break;
                }
                Some(Tok::RBrace) if start_depth > 0 && self.open.len() <= start_depth => break,
                Some(_) => {
                    self.bump();
                }
            }
        }
        if self.pos == start_pos {
            self.bump();
        }
    }

    /// Parses `{ NEWLINE { NEWLINE | item } }`, recovering per item. Returns the
    /// items and the span of the closing brace (or the end of file).
    fn block<T>(
        &mut self,
        mut item: impl FnMut(&mut Self) -> PResult<T>,
    ) -> PResult<(Vec<T>, Span)> {
        self.expect(Tok::LBrace)?;
        self.end_line()?;
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None => return Ok((items, self.prev)),
                Some(Tok::Newline) => {
                    self.bump();
                }
                Some(Tok::RBrace) => return Ok((items, self.bump())),
                Some(_) => {
                    let mark = self.mark();
                    match item(self) {
                        Ok(it) => items.push(it),
                        Err(()) => self.recover(mark),
                    }
                }
            }
        }
    }

    // ---- statements ----

    fn statement(&mut self) -> PResult<Stmt> {
        match self.peek() {
            Some(Tok::System) => self.system(),
            Some(Tok::Actor) => self.element(ElementKind::Actor),
            Some(Tok::External) => self.element(ElementKind::External),
            Some(Tok::Service) => self.element(ElementKind::Service),
            Some(Tok::Store) => self.element(ElementKind::Store),
            Some(Tok::Queue) => self.element(ElementKind::Queue),
            Some(Tok::Data) => self.data(),
            Some(Tok::Deploy) => self.deploy(),
            Some(Tok::Scenario) => self.scenario(),
            Some(Tok::Allow) => self.allow(),
            Some(Tok::Ident(_)) => self.relation(),
            _ => self.error("a statement"),
        }
    }

    fn system(&mut self) -> PResult<Stmt> {
        let start = self.bump();
        let name = self.ident("system name")?;
        let description = self.opt_str();
        let span = start.to(self.prev);
        self.end_line()?;
        Ok(Stmt::System {
            name,
            description,
            span,
        })
    }

    fn element(&mut self, kind: ElementKind) -> PResult<Stmt> {
        let start = self.bump();
        let name = self.ident(&format!("{} name", kind.keyword()))?;
        let description = self.opt_str();
        let span = start.to(self.prev);
        self.end_line()?;
        Ok(Stmt::Element {
            kind,
            name,
            description,
            span,
        })
    }

    fn relation(&mut self) -> PResult<Stmt> {
        let mut hops = vec![self.ident("name")?];
        let mut arrows = Vec::new();
        loop {
            let style = match self.peek() {
                Some(Tok::Arrow) => Style::Sync,
                Some(Tok::AsyncArrow) => Style::Async,
                Some(Tok::Reply) => {
                    let span = self.peek_span();
                    self.diagnostics.push(
                        Diagnostic::error(
                            "P001",
                            "expected `->` or `~>`, found `-->`",
                            span,
                            "replies are not relations",
                        )
                        .with_help("`-->` is only allowed in scenario steps"),
                    );
                    return Err(());
                }
                _ if arrows.is_empty() => return self.error("`->` or `~>`"),
                _ => break,
            };
            self.bump();
            arrows.push(style);
            hops.push(self.ident("name")?);
        }
        let description = match self.peek() {
            Some(Tok::Str(s)) => {
                let span = self.bump();
                if hops.len() > 2 {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "P004",
                            "description string on a relation chain",
                            span,
                            "a chain can't have a description",
                        )
                        .with_help("split the chain into single-hop relations and describe the one you mean"),
                    );
                    return Err(());
                }
                Some(s.clone())
            }
            _ => None,
        };
        self.end_line()?;
        Ok(Stmt::Relation {
            hops,
            arrows,
            description,
        })
    }

    fn data(&mut self) -> PResult<Stmt> {
        let start = self.bump();
        let name = self.ident("data entity name")?;
        let placement = match self.peek() {
            Some(Tok::In) => {
                self.bump();
                let store = self.ident("store name")?;
                let mut cached = Vec::new();
                if self.at(&Tok::Cached) {
                    self.bump();
                    cached.push(self.ident("store name")?);
                    while self.at(&Tok::Comma) {
                        self.bump();
                        cached.push(self.ident("store name")?);
                    }
                }
                Placement::In { store, cached }
            }
            Some(Tok::Via) => {
                self.bump();
                let queue = self.ident("queue name")?;
                self.expect(Tok::Arrow)?;
                let store = self.ident("store name")?;
                Placement::Via { queue, store }
            }
            _ => return self.error("`in` or `via`"),
        };
        self.expect(Tok::LBrace)?;
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&Tok::RBrace) && !fields.is_empty() {
                break;
            }
            fields.push(self.ident("field name")?);
            self.skip_newlines();
            if self.at(&Tok::Comma) {
                self.bump();
                continue;
            }
            if !self.at(&Tok::RBrace) {
                return self.error("`,` or `}`");
            }
        }
        let end = self.bump();
        self.end_line()?;
        Ok(Stmt::Data {
            name,
            placement,
            fields,
            span: start.to(end),
        })
    }

    fn skip_newlines(&mut self) {
        while self.at(&Tok::Newline) {
            self.bump();
        }
    }

    fn deploy(&mut self) -> PResult<Stmt> {
        let start = self.bump();
        self.expect(Tok::Region)?;
        let name = self.ident("region name")?;
        let (items, end) = self.block(Self::deploy_item)?;
        self.end_line()?;
        Ok(Stmt::Deploy {
            name,
            items,
            span: start.to(end),
        })
    }

    fn deploy_item(&mut self) -> PResult<DeployItem> {
        match self.peek() {
            Some(Tok::Cdn) => {
                let start = self.bump();
                let name = self.ident("cdn name")?;
                let span = start.to(self.prev);
                self.end_line()?;
                Ok(DeployItem::Cdn { name, span })
            }
            Some(Tok::Cluster) => {
                let start = self.bump();
                let name = self.ident("cluster name")?;
                let (workloads, end) = self.block(Self::workload)?;
                self.end_line()?;
                Ok(DeployItem::Group {
                    kind: GroupKind::Cluster,
                    name: Some(name),
                    workloads,
                    span: start.to(end),
                })
            }
            Some(Tok::Managed) => {
                let start = self.bump();
                let (workloads, end) = self.block(Self::workload)?;
                self.end_line()?;
                Ok(DeployItem::Group {
                    kind: GroupKind::Managed,
                    name: None,
                    workloads,
                    span: start.to(end),
                })
            }
            _ => self.error("`cdn`, `cluster`, `managed`, or `}`"),
        }
    }

    fn workload(&mut self) -> PResult<Workload> {
        let first = self.ident("workload name")?;
        let start = first.span;
        let mut names = vec![first];
        while self.at(&Tok::Comma) {
            self.bump();
            names.push(self.ident("workload name")?);
        }
        let mut attributes = Vec::new();
        while let Some(Tok::Ident(_)) = self.peek() {
            if let Some(attr) = self.attribute()? {
                attributes.push(attr);
            }
        }
        let span = start.to(self.prev);
        self.end_line()?;
        Ok(Workload {
            names,
            attributes,
            span,
        })
    }

    /// `attribute = REPLICAS | IDENT [ "+" IDENT ]`. A malformed replica count
    /// is reported (P007) and dropped without failing the line.
    fn attribute(&mut self) -> PResult<Option<Attribute>> {
        let first = self.ident("attribute")?;
        if self.at(&Tok::Plus) {
            self.bump();
            let second = self.ident("attribute after `+`")?;
            let span = first.span.to(second.span);
            return Ok(Some(
                if first.text == "primary" && second.text == "replica" {
                    Attribute::PrimaryReplica { span }
                } else {
                    Attribute::Unknown {
                        text: format!("{}+{}", first.text, second.text),
                        span,
                    }
                },
            ));
        }
        let span = first.span;
        if first.text == "autoscale" {
            return Ok(Some(Attribute::Autoscale { span }));
        }
        if let Some(digits) = first.text.strip_prefix('x')
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
        {
            return Ok(match digits.parse::<u32>() {
                Ok(count) if !digits.starts_with('0') => Some(Attribute::Replicas { count, span }),
                _ => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "P007",
                            format!("malformed replica count `{}`", first.text),
                            span,
                            "not a valid replica count",
                        )
                        .with_help("write `x` followed by a positive number without leading zeros, for example `x3`"),
                    );
                    None
                }
            });
        }
        Ok(Some(Attribute::Unknown {
            text: first.text,
            span,
        }))
    }

    fn scenario(&mut self) -> PResult<Stmt> {
        let start = self.bump();
        let name = self.ident("scenario name")?;
        let (steps, end) = self.block(Self::step)?;
        self.end_line()?;
        Ok(Stmt::Scenario {
            name,
            steps,
            span: start.to(end),
        })
    }

    fn step(&mut self) -> PResult<Step> {
        if self.at(&Tok::Parallel) {
            let start = self.bump();
            let (steps, end) = self.block(Self::step)?;
            self.end_line()?;
            return Ok(Step::Parallel {
                steps,
                span: start.to(end),
            });
        }
        if !matches!(self.peek(), Some(Tok::Ident(_))) {
            return self.error("a step, `parallel`, or `}`");
        }
        let first = self.ident("name")?;
        let start = first.span;
        let mut hops = vec![first];
        let mut arrows = Vec::new();
        loop {
            let arrow = match self.peek() {
                Some(Tok::Arrow) => StepArrow::Call,
                Some(Tok::AsyncArrow) => StepArrow::Async,
                Some(Tok::Reply) => StepArrow::Reply,
                _ if arrows.is_empty() => return self.error("`->`, `~>`, or `-->`"),
                _ => break,
            };
            self.bump();
            arrows.push(arrow);
            hops.push(self.ident("name")?);
        }
        let label = match self.peek() {
            Some(Tok::Label(text)) => {
                self.bump();
                Some(text.clone())
            }
            _ => None,
        };
        let span = start.to(self.prev);
        self.end_line()?;
        Ok(Step::Message {
            hops,
            arrows,
            label,
            span,
        })
    }

    fn allow(&mut self) -> PResult<Stmt> {
        let start = self.bump();
        let code = match self.peek() {
            Some(Tok::Ident(text)) if is_rule_code(text) => self.ident("rule code")?,
            _ => {
                let span = self.peek_span();
                let found = self.found();
                self.diagnostics.push(
                    Diagnostic::error(
                        "P005",
                        format!("malformed rule code after `allow`, found {found}"),
                        span,
                        "expected a rule code",
                    )
                    .with_help("rule codes are a letter and three digits, for example `W102`"),
                );
                return Err(());
            }
        };
        let reason = match self.peek() {
            Some(Tok::Str(s)) => {
                self.bump();
                s.clone()
            }
            _ => return self.error("a reason string"),
        };
        let span = start.to(self.prev);
        self.end_line()?;
        Ok(Stmt::Allow { code, reason, span })
    }
}

/// `[EWIP][0-9]{3}`. Parse codes are accepted here so that E010 can report them.
fn is_rule_code(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 4
        && matches!(b[0], b'E' | b'W' | b'I' | b'P')
        && b[1..].iter().all(u8::is_ascii_digit)
}

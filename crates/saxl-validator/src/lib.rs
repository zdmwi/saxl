//! Semantic validation for saxl models: name resolution plus the rules in
//! `rules/`. Only this crate can construct a [`ValidatedModel`].

mod resolve;
mod rules;

pub use resolve::{BrokenRefs, DeployedElement, ElementId, InfraId, Resolved, Target};
pub use rules::{Context, Rule, rules};
use saxl_core::{Diagnostic, Model, Severity};

/// Parse diagnostic codes and their summaries. They have rule documentation but
/// no validator rule.
pub static PARSE_CODES: &[(&str, &str)] = &[
    ("P001", "unexpected token"),
    ("P002", "unterminated string"),
    ("P003", "unclosed `{` at end of file"),
    ("P004", "description string on a relation chain"),
    ("P005", "malformed rule code after `allow`"),
    ("P006", "unrecognized character"),
    ("P007", "malformed replica count"),
];

/// The validator's output.
#[derive(Debug)]
pub struct Validation {
    /// `Some` only if there are no errors.
    pub model: Option<ValidatedModel>,
    /// Diagnostics with `allow`ed codes removed, sorted by primary span start.
    pub diagnostics: Vec<Diagnostic>,
}

/// A model with no errors, together with its resolved names.
#[derive(Debug)]
pub struct ValidatedModel {
    model: Model,
    resolved: Resolved,
}

impl ValidatedModel {
    /// The model.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Name resolution results.
    pub fn resolved(&self) -> &Resolved {
        &self.resolved
    }
}

/// Checks `model` against every rule.
pub fn validate(model: Model) -> Validation {
    let (resolved, broken) = resolve::resolve(&model);
    let mut diagnostics = Vec::new();
    {
        let cx = Context {
            model: &model,
            resolved: &resolved,
            broken: &broken,
        };
        for rule in rules() {
            rule.check(&cx, &mut diagnostics);
        }
    }
    let allowed: Vec<&str> = model
        .allows
        .iter()
        .map(|a| a.code.as_str())
        .filter(|code| {
            rules()
                .iter()
                .any(|r| r.code() == *code && r.severity() != Severity::Error)
        })
        .collect();
    diagnostics.retain(|d| d.severity == Severity::Error || !allowed.contains(&d.code.0));
    diagnostics.sort_by_key(Diagnostic::sort_key);
    let ok = !diagnostics.iter().any(|d| d.severity == Severity::Error);
    Validation {
        model: ok.then_some(ValidatedModel { model, resolved }),
        diagnostics,
    }
}

macro_rules! docs {
    ($($code:literal),* $(,)?) => {
        &[$(($code, include_str!(concat!("../rules/", $code, ".md")))),*]
    };
}

static DOCS: &[(&str, &str)] = docs![
    "P001", "P002", "P003", "P004", "P005", "P006", "P007", "E001", "E002", "E003", "E004", "E005",
    "E006", "E007", "E008", "E009", "E010", "E011", "W101", "W102", "W103", "W104", "W105", "W106",
    "W107", "W108", "I201",
];

/// The markdown documentation for `code`, if the code exists.
pub fn explain(code: &str) -> Option<&'static str> {
    DOCS.iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
        .map(|(_, doc)| *doc)
}

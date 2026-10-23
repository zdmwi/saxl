//! E010: `allow` names an error code or a code that does not exist.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E010"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "`allow` names an error or unknown code"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for allow in &cx.model.allows {
            let code = allow.code.as_str();
            let severity = super::rules()
                .iter()
                .find(|r| r.code() == code)
                .map(|r| r.severity())
                .or_else(|| {
                    crate::PARSE_CODES
                        .iter()
                        .any(|(c, _)| *c == code)
                        .then_some(Severity::Error)
                });
            match severity {
                Some(Severity::Warning | Severity::Info) => {}
                Some(Severity::Error) => out.push(
                    Diagnostic::error(
                        "E010",
                        format!("`{code}` is an error and can't be allowed"),
                        allow.code_span,
                        "error code",
                    )
                    .with_help(
                        "only warnings (W) and infos (I) can be allowed; fix the error instead",
                    ),
                ),
                None => out.push(
                    Diagnostic::error(
                        "E010",
                        format!("unknown rule code `{code}`"),
                        allow.code_span,
                        "no rule has this code",
                    )
                    .with_help("run `saxl explain` to list every code"),
                ),
            }
        }
    }
}

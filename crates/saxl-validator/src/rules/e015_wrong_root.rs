//! E015: a construct is used in the wrong kind of file, or on the wrong
//! element.

use saxl_core::{Attribute, Diagnostic, ElementKind, Severity, Span};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E015"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "a construct is used in the wrong kind of file, or on the wrong element"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        // With no root, E003 reports the file instead.
        if cx.model.roots.is_empty() {
            return;
        }
        if cx.is_library() {
            library(cx, out);
        } else {
            system(cx, out);
        }
    }
}

fn library(cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
    for e in &cx.model.elements {
        if !is_library_kind(e.kind) {
            out.push(
                Diagnostic::error(
                    "E015",
                    format!("`{}` is not allowed in a library", e.kind.keyword()),
                    e.span,
                    format!(
                        "`{}` is {} {}",
                        e.name.text,
                        e.kind.article(),
                        e.kind.keyword()
                    ),
                )
                .with_help("a library uses `consumer`, `package`, `api`, `module`, and `hook`"),
            );
        } else if let (Some(glob), false) = (
            &e.at,
            matches!(e.kind, ElementKind::Api | ElementKind::Module),
        ) {
            out.push(
                Diagnostic::error(
                    "E015",
                    format!(
                        "`at` is not allowed on {} {}",
                        e.kind.article(),
                        e.kind.keyword()
                    ),
                    glob.span,
                    format!("`{}` has no code in this library", e.name.text),
                )
                .with_help("only an api or a module says where its code lives"),
            );
        }
    }
    for region in &cx.model.regions {
        out.push(
            Diagnostic::error(
                "E015",
                "`deploy` is not allowed in a library",
                keyword(region.span, "deploy"),
                "a library is published, not deployed",
            )
            .with_help("list the library's packages in a `publish` block instead"),
        );
    }
    let attributes = cx
        .model
        .registries
        .iter()
        .flat_map(|r| &r.packages)
        .flat_map(|p| &p.workloads)
        .flat_map(|w| &w.attributes);
    for attr in attributes {
        let text = match attr {
            Attribute::Replicas { count, .. } => format!("x{count}"),
            Attribute::Autoscale { .. } => "autoscale".to_owned(),
            Attribute::PrimaryReplica { .. } => "primary+replica".to_owned(),
            Attribute::Feature { .. } | Attribute::Unknown { .. } => continue,
        };
        out.push(
            Diagnostic::error(
                "E015",
                format!("`{text}` is not allowed in a `publish` block"),
                attr.span(),
                "a deployment attribute",
            )
            .with_help("`publish` workloads take only `feature <name>`"),
        );
    }
}

fn system(cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
    let help = "describe the library in its own file, starting with `library <Name>`";
    for e in &cx.model.elements {
        if is_library_kind(e.kind) {
            out.push(
                Diagnostic::error(
                    "E015",
                    format!("`{}` is not allowed in a system", e.kind.keyword()),
                    e.span,
                    format!(
                        "`{}` is {} {}",
                        e.name.text,
                        e.kind.article(),
                        e.kind.keyword()
                    ),
                )
                .with_help(help),
            );
        } else if let Some(glob) = &e.at {
            out.push(
                Diagnostic::error(
                    "E015",
                    "`at` is not allowed in a system",
                    glob.span,
                    "only library elements say where their code lives",
                )
                .with_help("remove the `at` clause"),
            );
        }
    }
    for registry in &cx.model.registries {
        out.push(
            Diagnostic::error(
                "E015",
                "`publish` is not allowed in a system",
                keyword(registry.span, "publish"),
                "a system is deployed, not published",
            )
            .with_help("list the system's workloads in a `deploy region` block instead"),
        );
    }
    for stmt in &cx.model.impls {
        out.push(
            Diagnostic::error(
                "E015",
                "`impl` is not allowed in a system",
                stmt.span,
                "only libraries have hooks",
            )
            .with_help(help),
        );
    }
    for data in cx.model.data.iter().filter(|d| d.public) {
        out.push(
            Diagnostic::error(
                "E015",
                "`pub` is not allowed in a system",
                data.name.span,
                "marked `pub`",
            )
            .with_help("remove `pub`; only library types are public or internal"),
        );
    }
    let attributes = cx
        .model
        .regions
        .iter()
        .flat_map(|r| &r.groups)
        .flat_map(|g| &g.workloads)
        .flat_map(|w| &w.attributes);
    for attr in attributes {
        if let Attribute::Feature { span, .. } = attr {
            out.push(
                Diagnostic::error(
                    "E015",
                    "`feature` is not allowed in a `deploy` block",
                    *span,
                    "a publish attribute",
                )
                .with_help("feature flags belong to a library's `publish` block"),
            );
        }
    }
}

fn is_library_kind(kind: ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::Consumer
            | ElementKind::Package
            | ElementKind::Api
            | ElementKind::Module
            | ElementKind::Hook
    )
}

/// The span of the keyword that starts `block`.
fn keyword(block: Span, word: &str) -> Span {
    let len = u32::try_from(word.len()).unwrap_or(0);
    Span::new(block.file, block.start, block.start + len)
}

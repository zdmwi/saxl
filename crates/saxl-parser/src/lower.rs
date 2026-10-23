//! Syntax tree to IR: expands relation chains, merges duplicate relations, and
//! assembles deployment regions.

use saxl_core::{
    Allow, DataEntity, Element, Group, InfraKind, InfraNode, Model, Region, Relation, Scenario,
    System, Workload,
};

use crate::ast::{self, DeployItem, Stmt};

pub(crate) fn lower(stmts: Vec<Stmt>) -> Model {
    let mut model = Model::default();
    for stmt in stmts {
        match stmt {
            Stmt::System {
                name,
                description,
                span,
            } => model.systems.push(System {
                name: name.name(),
                description,
                span,
            }),
            Stmt::Element {
                kind,
                name,
                description,
                span,
            } => model.elements.push(Element {
                name: name.name(),
                kind,
                description,
                span,
            }),
            Stmt::Relation {
                hops,
                arrows,
                description,
            } => {
                for (pair, style) in hops.windows(2).zip(arrows) {
                    let (from, to) = (&pair[0], &pair[1]);
                    let duplicate = model.relations.iter().any(|r| {
                        r.from.text == from.text && r.to.text == to.text && r.style == style
                    });
                    if !duplicate {
                        model.relations.push(Relation {
                            from: from.name_ref(),
                            to: to.name_ref(),
                            style,
                            description: description.clone(),
                            span: from.span.to(to.span),
                        });
                    }
                }
            }
            Stmt::Data {
                name,
                placement,
                fields,
                span,
            } => model.data.push(DataEntity {
                name: name.name(),
                placement: match placement {
                    ast::Placement::In { store, cached } => saxl_core::Placement::In {
                        store: store.name_ref(),
                        cached: cached.iter().map(ast::Ident::name_ref).collect(),
                    },
                    ast::Placement::Via { queue, store } => saxl_core::Placement::Via {
                        queue: queue.name_ref(),
                        store: store.name_ref(),
                    },
                },
                fields: fields.iter().map(ast::Ident::name).collect(),
                span,
            }),
            Stmt::Deploy { name, items, span } => {
                let mut region = Region {
                    name: name.name(),
                    infra: Vec::new(),
                    groups: Vec::new(),
                    span,
                };
                for item in items {
                    match item {
                        DeployItem::Cdn { name, span } => region.infra.push(InfraNode {
                            name: name.name(),
                            kind: InfraKind::Cdn,
                            span,
                        }),
                        DeployItem::Group {
                            kind,
                            name,
                            workloads,
                            span,
                        } => region.groups.push(Group {
                            kind,
                            name: name.as_ref().map(ast::Ident::name),
                            workloads: workloads.into_iter().map(lower_workload).collect(),
                            span,
                        }),
                    }
                }
                model.regions.push(region);
            }
            Stmt::Scenario { name, steps, span } => model.scenarios.push(Scenario {
                name: name.name(),
                steps: steps.into_iter().map(lower_step).collect(),
                span,
            }),
            Stmt::Allow { code, reason, span } => model.allows.push(Allow {
                code: code.text,
                code_span: code.span,
                reason,
                span,
            }),
        }
    }
    model
}

fn lower_workload(w: ast::Workload) -> Workload {
    Workload {
        names: w.names.iter().map(ast::Ident::name_ref).collect(),
        attributes: w.attributes,
        span: w.span,
    }
}

fn lower_step(step: ast::Step) -> saxl_core::Step {
    match step {
        ast::Step::Message {
            hops,
            arrows,
            label,
            span,
        } => saxl_core::Step::Message {
            hops: hops.iter().map(ast::Ident::name_ref).collect(),
            arrows,
            label,
            span,
        },
        ast::Step::Parallel { steps, span } => saxl_core::Step::Parallel {
            steps: steps.into_iter().map(lower_step).collect(),
            span,
        },
    }
}

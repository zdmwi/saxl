//! Turns a validated saxl model into scenes and HTML, SVG, or JSON output.
//!
//! [`scene`] runs two stages: a projection (`project`, a pure function from
//! the model to a view-specific graph) and a layout (`layout`, which assigns
//! geometry). Only a [`ValidatedModel`] can be rendered.

mod html;
mod layout;
mod project;
mod scene;
mod svg;

pub use html::{HtmlOptions, to_html};
use saxl_validator::ValidatedModel;
pub use scene::*;
pub use svg::to_svg;

/// The bundled browser viewer, embedded into HTML output.
pub const VIEWER_JS: &str = include_str!("../assets/viewer.js");

/// Why a scene could not be produced.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// `ViewKind::Runtime` named a scenario that does not exist.
    #[error("no scenario named `{0}`")]
    UnknownScenario(String),
    /// SVG output was requested for the 3D tower.
    #[error("the tower view is 3D and has no SVG form")]
    NotTwoDimensional,
    /// The view is not implemented yet.
    #[error("the {0} view is not implemented yet")]
    Unsupported(&'static str),
}

/// Projects and lays out one view.
pub fn scene(model: &ValidatedModel, view: &ViewKind) -> Result<Scene, RenderError> {
    let system = project::system_name(model.model());
    let mut scene = Scene {
        view: view.clone(),
        title: String::new(),
        size: Size2::default(),
        nodes: Vec::new(),
        edges: Vec::new(),
        groups: Vec::new(),
        timelines: Vec::new(),
    };
    match view {
        ViewKind::Context => {
            let l = layout::context::layout(project::context(model));
            scene.title = format!("Context: {system}");
            (scene.nodes, scene.edges, scene.size) = (l.nodes, l.edges, l.size);
        }
        ViewKind::Containers => {
            let p = project::containers(model);
            let l = layout::layered::layout(p.nodes, &p.edges);
            scene.title = format!("Containers: {system}");
            (scene.nodes, scene.edges, scene.size) = (l.nodes, l.edges, l.size);
        }
        ViewKind::Runtime { scenario } => {
            let mut p = project::runtime(model, scenario)?;
            scene.title = std::mem::take(&mut p.title);
            scene.timelines = vec![p.timeline.clone()];
            let l = layout::runtime::layout(p);
            (scene.nodes, scene.edges, scene.groups, scene.size) =
                (l.nodes, l.edges, l.groups, l.size);
        }
        ViewKind::Data => {
            let l = layout::data::layout(project::data(model));
            scene.title = format!("Data: {system}");
            (scene.nodes, scene.edges, scene.size) = (l.nodes, l.edges, l.size);
        }
        ViewKind::Deployment => {
            let l = layout::deployment::layout(project::deployment(model));
            scene.title = format!("Deployment: {system}");
            (scene.nodes, scene.groups, scene.size) = (l.nodes, l.groups, l.size);
        }
        ViewKind::Tower => return Err(RenderError::Unsupported("tower")),
    }
    Ok(scene)
}

/// Every view the model supports, in a fixed order: context, containers, one
/// runtime view per scenario, data, deployment.
pub fn views(model: &ValidatedModel) -> Vec<ViewKind> {
    let mut views = vec![ViewKind::Context, ViewKind::Containers];
    views.extend(model.model().scenarios.iter().map(|s| ViewKind::Runtime {
        scenario: s.name.text.clone(),
    }));
    views.push(ViewKind::Data);
    views.push(ViewKind::Deployment);
    views
}

/// Scenes for every view and every scenario.
pub fn scenes(model: &ValidatedModel) -> Vec<Scene> {
    views(model)
        .iter()
        .filter_map(|v| scene(model, v).ok())
        .collect()
}

/// The scene as pretty JSON.
pub fn to_json(scene: &Scene) -> String {
    serde_json::to_string_pretty(scene).unwrap_or_else(|_| "{}".to_owned())
}

//! WebAssembly bindings for the saxl playground.

use saxl_core::{ColumnUnit, FileId, LineIndex, LocatedDiagnostic, Model, Severity};
use saxl_renderer::Scene;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// The result of compiling one source text.
#[derive(Debug, Serialize)]
pub struct CompileResult {
    /// Parse and validation diagnostics. Columns count UTF-16 code units, as
    /// browser editors do.
    pub diagnostics: Vec<LocatedDiagnostic>,
    /// The IR, when parsing succeeded.
    pub ir: Option<Model>,
    /// Every view's scene, when there are no errors.
    pub scenes: Option<Vec<Scene>>,
}

/// Parses, validates, and renders `source` (the pipeline behind [`compile`]).
pub fn compile_result(source: &str) -> CompileResult {
    let file = FileId(0);
    let parsed = saxl_parser::parse(source, file);
    let mut diagnostics = parsed.diagnostics;
    let mut ir = None;
    let mut scenes = None;
    if diagnostics.is_empty() {
        ir = Some(parsed.model.clone());
        let validation = saxl_validator::validate(parsed.model);
        diagnostics = validation.diagnostics;
        let clean = !diagnostics.iter().any(|d| d.severity == Severity::Error);
        if let (true, Some(model)) = (clean, validation.model) {
            scenes = Some(saxl_renderer::scenes(&model));
        }
    }
    let index = LineIndex::new(source);
    CompileResult {
        diagnostics: diagnostics
            .iter()
            .map(|d| d.locate(source, &index, ColumnUnit::Utf16))
            .collect(),
        ir,
        scenes,
    }
}

/// Compiles `source`. Returns `{ diagnostics, ir, scenes }`; `scenes` is null
/// when there are errors and `ir` is null when parsing failed.
#[wasm_bindgen]
pub fn compile(source: &str) -> JsValue {
    compile_result(source)
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap_or(JsValue::NULL)
}

/// The markdown documentation for a diagnostic code.
#[wasm_bindgen]
pub fn explain(code: &str) -> Option<String> {
    saxl_validator::explain(code).map(str::to_owned)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn snip_compiles_to_scenes() {
        let r = compile_result(include_str!("../../../examples/snip.saxl"));
        assert!(r.diagnostics.is_empty());
        assert!(r.ir.is_some());
        assert_eq!(r.scenes.unwrap().len(), 7);
    }

    #[test]
    fn errors_withhold_scenes_but_keep_ir() {
        let r = compile_result("system S\nactor A\nexternal B\nA -> B\n");
        assert_eq!(r.diagnostics[0].code.0, "E004");
        assert!(r.ir.is_some());
        assert!(r.scenes.is_none());
    }

    #[test]
    fn parse_errors_withhold_ir() {
        let r = compile_result("system\n");
        assert_eq!(r.diagnostics[0].code.0, "P001");
        assert!(r.ir.is_none());
    }

    #[test]
    fn columns_are_utf16() {
        // `𝄞` is two UTF-16 units, so `Wbe` starts at column 15 in UTF-16 and 14 in chars.
        let r = compile_result("system S \"𝄞\"\nactor A\nservice B\nA -> Wbe\n");
        let e002 = r.diagnostics.iter().find(|d| d.code.0 == "E002").unwrap();
        assert_eq!(e002.labels[0].range.start.line, 4);
        let r = compile_result("system S\nactor A \"𝄞\" \nA -> 𝄞B\n");
        let p006 = r.diagnostics.iter().find(|d| d.code.0 == "P006").unwrap();
        assert_eq!(p006.labels[0].range.start.col, 6);
        assert_eq!(p006.labels[0].range.end.col, 8);
    }
}

//! Reading a file and running the pipeline over it.

use std::path::Path;

use saxl_core::{Diagnostic, FileId, SourceFile};
use saxl_parser::Parsed;
use saxl_validator::Validation;

use crate::CliError;

/// One loaded file. `validation` is `None` when parsing reported errors, since
/// the validator never runs on a model with parse errors.
pub struct Loaded {
    pub source: SourceFile,
    pub parsed: Parsed,
    pub validation: Option<Validation>,
}

impl Loaded {
    /// Parse diagnostics followed by validation diagnostics.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut all = self.parsed.diagnostics.clone();
        if let Some(v) = &self.validation {
            all.extend(v.diagnostics.iter().cloned());
        }
        all
    }
}

pub fn read(path: &Path, id: FileId) -> Result<SourceFile, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError(format!("can't read {}: {e}", path.display())))?;
    Ok(SourceFile {
        id,
        name: path.display().to_string(),
        text,
    })
}

pub fn load(path: &Path, id: FileId) -> Result<Loaded, CliError> {
    let source = read(path, id)?;
    let parsed = saxl_parser::parse(&source.text, id);
    let validation = parsed
        .diagnostics
        .is_empty()
        .then(|| saxl_validator::validate(parsed.model.clone()));
    Ok(Loaded {
        source,
        parsed,
        validation,
    })
}

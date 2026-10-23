//! Shared vocabulary for the saxl toolchain: source spans, diagnostics, and the
//! model IR produced by the parser and checked by the validator.

mod diagnostic;
mod ir;
mod span;

pub use diagnostic::{
    ByteRange, Code, ColumnUnit, Diagnostic, Label, LocatedDiagnostic, LocatedLabel, Range,
    Severity,
};
pub use ir::*;
pub use span::{FileId, LineIndex, Position, SourceFile, Span};

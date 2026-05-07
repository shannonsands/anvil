pub mod ast;
pub mod diagnostic;
pub mod reader;
pub mod repl;
pub mod source;

pub use ast::{
    AstDiagnostic, AstKind, AstLiteral, AstMapEntry, SpannedAst, format_ast, lower_datums,
    lower_source, lower_source_text,
};
pub use diagnostic::{
    Diagnostic, DiagnosticCodeFrame, DiagnosticLabel, DiagnosticPhase, DiagnosticSeverity,
    DiagnosticSuggestion,
};
pub use reader::{
    Datum, ReaderDiagnostic, SpannedDatum, format_datums, read_source, read_source_text,
};
pub use repl::{ReplInteraction, ReplResponse, ReplSession, read_repl_input};
pub use source::{SourceLocation, SourceSpan, SourceText};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectShape {
    pub name: &'static str,
    pub status: &'static str,
    pub vm_first: bool,
    pub mightygrad_external: bool,
}

pub fn project_shape() -> ProjectShape {
    ProjectShape {
        name: "Anvil",
        status: "phase 0 planning scaffold",
        vm_first: true,
        mightygrad_external: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_current_project_shape() {
        let shape = project_shape();

        assert_eq!(shape.name, "Anvil");
        assert!(shape.vm_first);
        assert!(shape.mightygrad_external);
    }
}

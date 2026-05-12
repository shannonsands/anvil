pub mod ast;
pub mod capability;
pub mod diagnostic;
pub mod draft;
pub mod host;
pub mod manifest;
pub mod module;
pub mod module_session;
pub mod project;
pub mod reader;
pub mod repl;
pub mod resource;
pub mod response;
pub mod source;
pub mod syntax;
pub mod vm;

pub use ast::{
    AstDiagnostic, AstKind, AstLiteral, AstMapEntry, RequireImport, SpannedAst, format_ast,
    lower_datums, lower_datums_with_resolver, lower_source, lower_source_text,
    lower_source_text_with_resolver, lower_source_with_resolver,
};
pub use capability::CapabilityProfile;
pub use diagnostic::{
    Diagnostic, DiagnosticCodeFrame, DiagnosticLabel, DiagnosticPhase, DiagnosticSeverity,
    DiagnosticSuggestion,
};
pub use draft::{DraftModule, DraftOverlay, DraftStatus};
pub use host::{
    HostCallContext, HostCallFailure, HostCallResult, HostFunction, HostFunctionArity,
    HostFunctionRegistry, HostFunctionSpec, RegisteredHostFunction,
};
pub use manifest::{
    AnvilManifest, LibraryManifest, ManifestDiagnostic, PackageManifest, SourceRoots,
    WorkspaceManifest, parse_manifest, parse_manifest_text,
};
pub use module::{
    ModuleCandidate, ModuleDiagnostic, ModuleResolution, ModuleResolver, ModuleRootKind,
    ModuleSource,
};
pub use module_session::{ModuleExecutionDiagnostic, ModuleExecutionResult, ModuleSession};
pub use project::{
    PackageSnapshot, PackageSourceFile, ProjectDiagnostic, ProjectResult, WorkspaceMemberSnapshot,
    WorkspaceSnapshot, load_package_snapshot, load_workspace_snapshot, package_module_sources,
};
pub use reader::{
    Datum, ReaderDiagnostic, SpannedDatum, format_datums, read_source, read_source_text,
};
pub use repl::{EvaluationStatus, ReplInteraction, ReplResponse, ReplSession, read_repl_input};
pub use resource::{
    HandleDelegationPolicy, HandleDisplayPolicy, HandleEntry, HandleRevocationState, HandleTable,
    ResourceAdapter, ResourceAdapterFailure, ResourceAdapterOutcome, ResourceAdapterRequest,
    ResourceAdapterResult, ResourceAdapterStatus, ResourceAuditDecision, ResourceAuditEvent,
    ResourceAuditKind, ResourceAuditPolicy, ResourceBudgetPolicy, ResourceDebugPolicy,
    ResourceDelegationRequest, ResourceDenial, ResourceDenialReason, ResourceDiagnostic,
    ResourceEffect, ResourceEffectRecord, ResourceEntry, ResourceError, ResourceExecutionMode,
    ResourceLifetime, ResourceOpenRequest, ResourceOperationAuthorization,
    ResourceOperationOutcome, ResourceOperationPayload, ResourceOperationRequest,
    ResourceOperationSchema, ResourcePolicy, ResourceRedactionPolicy, ResourceRegistry,
};
pub use response::{
    EvalResponse, RESPONSE_PROTOCOL, ResponseDetail, ResponseEffect, ResponseEnvelope,
    ResponseFacet, ResponseKind, ResponseMetadata, ResponseNextAction, ResponseNotice,
    ResponseOptions, ResponseStatus, ResponseValue, ResponseValueData, ResponseValueMapEntry,
};
pub use source::{SourceLocation, SourceSpan, SourceText};
pub use syntax::{
    SyntaxContext, SyntaxDiagnostic, SyntaxObject, format_syntax_objects, syntax_from_datums,
    syntax_from_source, syntax_from_source_text,
};
pub use vm::{
    BytecodeInstruction, BytecodeProgram, Instruction, MapRegisterEntry, Value, ValueMapEntry, Vm,
    VmBudget, VmDiagnostic, VmOutput, VmSession, compile_ast, compile_ast_with_host_functions,
    compile_source, compile_source_text, compile_source_text_with_host_functions,
    compile_source_with_host_functions, run_source, run_source_response, run_source_text,
    run_source_text_response,
};

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

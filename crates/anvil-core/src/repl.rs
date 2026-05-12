use crate::{
    diagnostic::Diagnostic,
    module_session::ModuleSession,
    reader::{ReaderDiagnostic, SpannedDatum, read_source},
    response::{EvalResponse, ResponseEnvelope},
    vm::{Value, VmDiagnostic, VmOutput, VmSession},
};

use serde::{Serialize, ser::SerializeStruct};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EvaluationStatus {
    NotImplemented,
    Value { output: VmOutput },
    Error { diagnostic: Box<VmDiagnostic> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReplResponse {
    Read {
        datums: Vec<SpannedDatum>,
        evaluation: EvaluationStatus,
    },
    Error {
        diagnostic: Box<ReaderDiagnostic>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplInteraction {
    Complete(ReplResponse),
    Pending {
        diagnostic: Box<ReaderDiagnostic>,
        buffered_lines: usize,
    },
}

impl Serialize for ReplInteraction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Complete(response) => response.serialize(serializer),
            Self::Pending {
                diagnostic,
                buffered_lines,
            } => {
                let mut state = serializer.serialize_struct("ReplInteraction", 3)?;
                state.serialize_field("status", "pending")?;
                state.serialize_field("diagnostic", diagnostic)?;
                state.serialize_field("buffered_lines", buffered_lines)?;
                state.end()
            }
        }
    }
}

impl ReplInteraction {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub fn response(&self) -> Option<&ReplResponse> {
        match self {
            Self::Complete(response) => Some(response),
            Self::Pending { .. } => None,
        }
    }

    pub fn diagnostic(&self) -> Option<&ReaderDiagnostic> {
        match self {
            Self::Complete(response) => response.diagnostic(),
            Self::Pending { diagnostic, .. } => Some(diagnostic.as_ref()),
        }
    }

    pub fn eval_response(&self) -> Option<EvalResponse> {
        match self {
            Self::Complete(response) => response.eval_response(),
            Self::Pending {
                diagnostic,
                buffered_lines,
            } => Some(ResponseEnvelope::pending(
                diagnostic.as_ref(),
                *buffered_lines,
            )),
        }
    }
}

impl ReplResponse {
    pub fn datums(&self) -> &[SpannedDatum] {
        match self {
            Self::Read { datums, .. } => datums,
            Self::Error { .. } => &[],
        }
    }

    pub fn diagnostic(&self) -> Option<&ReaderDiagnostic> {
        match self {
            Self::Read { evaluation, .. } => evaluation.diagnostic(),
            Self::Error { diagnostic } => Some(diagnostic.as_ref()),
        }
    }

    pub fn evaluation(&self) -> Option<&EvaluationStatus> {
        match self {
            Self::Read { evaluation, .. } => Some(evaluation),
            Self::Error { .. } => None,
        }
    }

    pub fn eval_response(&self) -> Option<EvalResponse> {
        match self {
            Self::Read { evaluation, .. } => evaluation.eval_response(),
            Self::Error { diagnostic } => Some(ResponseEnvelope::error(diagnostic.as_ref())),
        }
    }
}

#[derive(Debug, Default)]
pub struct ReplSession {
    buffer: String,
    pending: Option<ReaderDiagnostic>,
    buffered_lines: usize,
    evaluator: ReplEvaluator,
}

#[derive(Debug)]
enum ReplEvaluator {
    Vm(VmSession),
    Module(ModuleSession),
}

impl Default for ReplEvaluator {
    fn default() -> Self {
        Self::Vm(VmSession::new())
    }
}

impl ReplEvaluator {
    fn eval_source(&mut self, source: &str) -> Result<VmOutput, Box<VmDiagnostic>> {
        match self {
            Self::Vm(session) => session.eval_source(source),
            Self::Module(session) => session.eval_source(source),
        }
    }

    fn vm(&self) -> &VmSession {
        match self {
            Self::Vm(session) => session,
            Self::Module(session) => session.vm(),
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Vm(session) => session.reset(),
            Self::Module(session) => session.reset(),
        }
    }
}

impl ReplSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_module_session(module_session: ModuleSession) -> Self {
        Self {
            evaluator: ReplEvaluator::Module(module_session),
            ..Self::default()
        }
    }

    pub fn push_line(&mut self, line: &str) -> ReplInteraction {
        self.buffer.push_str(line);
        self.buffered_lines += 1;

        match read_source(&self.buffer) {
            Ok(datums) => {
                let evaluation = match self.evaluator.eval_source(&self.buffer) {
                    Ok(output) => EvaluationStatus::Value { output },
                    Err(diagnostic) => EvaluationStatus::Error { diagnostic },
                };
                self.buffer.clear();
                self.pending = None;
                self.buffered_lines = 0;
                ReplInteraction::Complete(ReplResponse::Read { datums, evaluation })
            }
            Err(diagnostic) => {
                if diagnostic.is_incomplete_input() {
                    self.pending = Some((*diagnostic).clone());
                    ReplInteraction::Pending {
                        diagnostic,
                        buffered_lines: self.buffered_lines,
                    }
                } else {
                    self.buffer.clear();
                    self.pending = None;
                    self.buffered_lines = 0;
                    ReplInteraction::Complete(ReplResponse::Error { diagnostic })
                }
            }
        }
    }

    pub fn finish(self) -> Option<ReplResponse> {
        self.pending.map(|diagnostic| ReplResponse::Error {
            diagnostic: Box::new(diagnostic),
        })
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn vm(&self) -> &VmSession {
        self.evaluator.vm()
    }

    pub fn reset_vm(&mut self) {
        self.evaluator.reset();
    }
}

pub fn read_repl_input(source: &str) -> ReplResponse {
    match read_source(source) {
        Ok(datums) => ReplResponse::Read {
            datums,
            evaluation: EvaluationStatus::NotImplemented,
        },
        Err(diagnostic) => ReplResponse::Error { diagnostic },
    }
}

impl EvaluationStatus {
    pub fn diagnostic(&self) -> Option<&Diagnostic> {
        match self {
            Self::Error { diagnostic } => Some(diagnostic.as_ref()),
            Self::NotImplemented | Self::Value { .. } => None,
        }
    }

    pub fn value(&self) -> Option<&Value> {
        match self {
            Self::Value { output } => Some(&output.value),
            Self::NotImplemented | Self::Error { .. } => None,
        }
    }

    pub fn eval_response(&self) -> Option<EvalResponse> {
        match self {
            Self::Value { output } => Some(ResponseEnvelope::ok(output)),
            Self::Error { diagnostic } => Some(ResponseEnvelope::error(diagnostic.as_ref())),
            Self::NotImplemented => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_waits_for_unclosed_form() {
        let mut session = ReplSession::new();

        let interaction = session.push_line("(define answer\n");

        assert!(interaction.is_pending());
        assert!(session.is_pending());
    }

    #[test]
    fn session_completes_after_continuation_line() {
        let mut session = ReplSession::new();

        assert!(session.push_line("(define answer\n").is_pending());
        let interaction = session.push_line("42)\n");
        let response = interaction.response().expect("complete response");

        assert_eq!(response.datums()[0].to_string(), "(define answer 42)");
        assert_eq!(
            response.evaluation().and_then(EvaluationStatus::value),
            Some(&Value::Integer(42))
        );
        assert!(!session.is_pending());
    }

    #[test]
    fn session_evaluates_against_persistent_vm_state() {
        let mut session = ReplSession::new();

        session.push_line("(define answer 42)\n");
        let interaction = session.push_line("answer\n");
        let response = interaction.response().expect("complete response");

        assert_eq!(
            response.evaluation().and_then(EvaluationStatus::value),
            Some(&Value::Integer(42))
        );
        assert_eq!(session.vm().binding("answer"), Some(&Value::Integer(42)));
    }

    #[test]
    fn session_can_evaluate_with_module_loader() {
        let mut module_session = ModuleSession::new();
        module_session.add_module_source(
            crate::ModuleSource::new(
                crate::ModuleRootKind::Package,
                "planner-tools",
                "planner.search",
                "src/planner/search.anv",
            ),
            "(define answer 42)",
        );
        let mut session = ReplSession::with_module_session(module_session);

        session.push_line("(require planner.search)\n");
        let interaction = session.push_line("answer\n");
        let response = interaction.response().expect("complete response");

        assert_eq!(
            response.evaluation().and_then(EvaluationStatus::value),
            Some(&Value::Integer(42))
        );
    }

    #[test]
    fn session_flushes_pending_error_on_finish() {
        let mut session = ReplSession::new();

        assert!(session.push_line("(define answer\n").is_pending());
        let response = session.finish().expect("pending response");

        assert_eq!(
            response.diagnostic().map(|diagnostic| diagnostic.code),
            Some("ANVIL_READER_UNCLOSED_DELIMITER")
        );
    }
}

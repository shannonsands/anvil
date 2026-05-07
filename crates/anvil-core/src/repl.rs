use crate::reader::{ReaderDiagnostic, SpannedDatum, read_source};

use serde::{Serialize, ser::SerializeStruct};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStatus {
    NotImplemented,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReplResponse {
    Read {
        datums: Vec<SpannedDatum>,
        evaluation: EvaluationStatus,
    },
    Error {
        diagnostic: ReaderDiagnostic,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplInteraction {
    Complete(ReplResponse),
    Pending {
        diagnostic: ReaderDiagnostic,
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
            Self::Pending { diagnostic, .. } => Some(diagnostic),
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
            Self::Read { .. } => None,
            Self::Error { diagnostic } => Some(diagnostic),
        }
    }
}

#[derive(Debug, Default)]
pub struct ReplSession {
    buffer: String,
    pending: Option<ReaderDiagnostic>,
    buffered_lines: usize,
}

impl ReplSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_line(&mut self, line: &str) -> ReplInteraction {
        self.buffer.push_str(line);
        self.buffered_lines += 1;

        match read_source(&self.buffer) {
            Ok(datums) => {
                self.buffer.clear();
                self.pending = None;
                self.buffered_lines = 0;
                ReplInteraction::Complete(ReplResponse::Read {
                    datums,
                    evaluation: EvaluationStatus::NotImplemented,
                })
            }
            Err(diagnostic) => {
                let diagnostic = *diagnostic;
                if diagnostic.is_incomplete_input() {
                    self.pending = Some(diagnostic.clone());
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
        self.pending
            .map(|diagnostic| ReplResponse::Error { diagnostic })
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

pub fn read_repl_input(source: &str) -> ReplResponse {
    match read_source(source) {
        Ok(datums) => ReplResponse::Read {
            datums,
            evaluation: EvaluationStatus::NotImplemented,
        },
        Err(diagnostic) => ReplResponse::Error {
            diagnostic: *diagnostic,
        },
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
        assert!(!session.is_pending());
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

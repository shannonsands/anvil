use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    diagnostic::Diagnostic,
    source::SourceText,
    vm::{Value, ValueMapEntry, VmOutput},
};

pub const RESPONSE_PROTOCOL: &str = "anvil.response.v1";

pub type EvalResponse = ResponseEnvelope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Ok,
    Error,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseKind {
    EvalResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseDetail {
    Summary,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseOptions {
    pub detail: ResponseDetail,
}

impl ResponseOptions {
    pub fn summary() -> Self {
        Self {
            detail: ResponseDetail::Summary,
        }
    }

    pub fn debug() -> Self {
        Self {
            detail: ResponseDetail::Debug,
        }
    }

    fn includes_debug(self) -> bool {
        matches!(self.detail, ResponseDetail::Debug)
    }
}

impl Default for ResponseOptions {
    fn default() -> Self {
        Self::summary()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResponseEnvelope {
    pub protocol: &'static str,
    pub status: ResponseStatus,
    pub kind: ResponseKind,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<ResponseValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<ResponseNotice>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<ResponseEffect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facets: Vec<ResponseFacet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next: Vec<ResponseNextAction>,
    #[serde(default, skip_serializing_if = "ResponseMetadata::is_empty")]
    pub metadata: ResponseMetadata,
}

impl ResponseEnvelope {
    pub fn ok(output: &VmOutput) -> Self {
        Self::ok_with_options(output, None, ResponseOptions::default())
    }

    pub fn ok_with_source(output: &VmOutput, source: &SourceText) -> Self {
        Self::ok_with_options(output, Some(source), ResponseOptions::default())
    }

    pub fn ok_with_options(
        output: &VmOutput,
        source: Option<&SourceText>,
        options: ResponseOptions,
    ) -> Self {
        let value = ResponseValue::from(&output.value);
        let mut metadata = ResponseMetadata::from_output(output);
        metadata.merge_source(source);

        let mut response = Self {
            protocol: RESPONSE_PROTOCOL,
            status: ResponseStatus::Ok,
            kind: ResponseKind::EvalResult,
            summary: value.display.clone(),
            id: None,
            value: Some(value),
            diagnostics: Vec::new(),
            notices: Vec::new(),
            effects: Vec::new(),
            facets: Vec::new(),
            next: Vec::new(),
            metadata,
        };

        if options.includes_debug() {
            response.facets.push(ResponseFacet::vm_metrics(output));
        }

        response
    }

    pub fn error(diagnostic: &Diagnostic) -> Self {
        Self::error_with_options(diagnostic, ResponseOptions::default())
    }

    pub fn error_with_options(diagnostic: &Diagnostic, options: ResponseOptions) -> Self {
        let mut response = Self {
            protocol: RESPONSE_PROTOCOL,
            status: ResponseStatus::Error,
            kind: ResponseKind::EvalResult,
            summary: diagnostic.message.clone(),
            id: None,
            value: None,
            diagnostics: vec![diagnostic.clone()],
            notices: Vec::new(),
            effects: Vec::new(),
            facets: Vec::new(),
            next: Vec::new(),
            metadata: ResponseMetadata::from_diagnostic(diagnostic),
        };

        if options.includes_debug() {
            response
                .facets
                .push(ResponseFacet::diagnostic_summary(diagnostic));
        }

        response
    }

    pub fn pending(diagnostic: &Diagnostic, buffered_lines: usize) -> Self {
        Self {
            protocol: RESPONSE_PROTOCOL,
            status: ResponseStatus::Pending,
            kind: ResponseKind::EvalResult,
            summary: diagnostic.message.clone(),
            id: None,
            value: None,
            diagnostics: vec![diagnostic.clone()],
            notices: Vec::new(),
            effects: Vec::new(),
            facets: Vec::new(),
            next: Vec::new(),
            metadata: ResponseMetadata::from_diagnostic(diagnostic)
                .with_buffered_lines(buffered_lines),
        }
    }

    pub fn value(&self) -> Option<&ResponseValue> {
        self.value.as_ref()
    }

    pub fn primary_diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostics.first()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResponseValue {
    pub display: String,
    #[serde(flatten)]
    pub data: ResponseValueData,
}

impl From<&Value> for ResponseValue {
    fn from(value: &Value) -> Self {
        Self {
            display: value.to_string(),
            data: ResponseValueData::from(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResponseValueData {
    Nil,
    Bool { value: bool },
    Integer { value: i64 },
    Float64 { value: f64 },
    String { value: String },
    Symbol { value: String },
    Keyword { value: String },
    List { items: Vec<ResponseValueData> },
    Vector { items: Vec<ResponseValueData> },
    Map { entries: Vec<ResponseValueMapEntry> },
    Function { opaque: bool },
}

impl From<&Value> for ResponseValueData {
    fn from(value: &Value) -> Self {
        match value {
            Value::Nil => Self::Nil,
            Value::Bool(value) => Self::Bool { value: *value },
            Value::Integer(value) => Self::Integer { value: *value },
            Value::Float64(value) => Self::Float64 { value: *value },
            Value::String(value) => Self::String {
                value: value.clone(),
            },
            Value::Symbol(value) => Self::Symbol {
                value: value.clone(),
            },
            Value::Keyword(value) => Self::Keyword {
                value: value.clone(),
            },
            Value::List(items) => Self::List {
                items: items.iter().map(Self::from).collect(),
            },
            Value::Vector(items) => Self::Vector {
                items: items.iter().map(Self::from).collect(),
            },
            Value::Map(entries) => Self::Map {
                entries: entries.iter().map(ResponseValueMapEntry::from).collect(),
            },
            Value::Function(_) => Self::Function { opaque: true },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResponseValueMapEntry {
    pub key: ResponseValueData,
    pub value: ResponseValueData,
}

impl From<&ValueMapEntry> for ResponseValueMapEntry {
    fn from(entry: &ValueMapEntry) -> Self {
        Self {
            key: ResponseValueData::from(&entry.key),
            value: ResponseValueData::from(&entry.value),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResponseMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions_executed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_call_depth: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffered_lines: Option<usize>,
}

impl ResponseMetadata {
    pub fn is_empty(&self) -> bool {
        self.source_id.is_none()
            && self.source_path.is_none()
            && self.instructions_executed.is_none()
            && self.max_call_depth.is_none()
            && self.buffered_lines.is_none()
    }

    fn from_output(output: &VmOutput) -> Self {
        Self {
            instructions_executed: Some(output.instructions_executed),
            max_call_depth: Some(output.max_call_depth),
            ..Self::default()
        }
    }

    fn from_diagnostic(diagnostic: &Diagnostic) -> Self {
        Self {
            source_id: Some(diagnostic.source_id.clone()),
            ..Self::default()
        }
    }

    fn with_buffered_lines(mut self, buffered_lines: usize) -> Self {
        self.buffered_lines = Some(buffered_lines);
        self
    }

    fn merge_source(&mut self, source: Option<&SourceText>) {
        let Some(source) = source else {
            return;
        };

        self.source_id = Some(source.id().to_string());
        self.source_path = source.path().map(ToString::to_string);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponseNotice {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponseEffect {
    pub kind: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponseFacet {
    pub name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entries: BTreeMap<String, String>,
}

impl ResponseFacet {
    fn vm_metrics(output: &VmOutput) -> Self {
        Self {
            name: "vm.metrics".to_string(),
            entries: BTreeMap::from([
                (
                    "instructions_executed".to_string(),
                    output.instructions_executed.to_string(),
                ),
                (
                    "max_call_depth".to_string(),
                    output.max_call_depth.to_string(),
                ),
            ]),
        }
    }

    fn diagnostic_summary(diagnostic: &Diagnostic) -> Self {
        Self {
            name: "diagnostic.summary".to_string(),
            entries: BTreeMap::from([
                ("code".to_string(), diagnostic.code.to_string()),
                ("phase".to_string(), format!("{:?}", diagnostic.phase)),
            ]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponseNextAction {
    pub command: String,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::FunctionValue;

    #[test]
    fn ok_responses_include_safe_value_and_vm_metadata() {
        let output = VmOutput {
            value: Value::Integer(42),
            instructions_executed: 7,
            max_call_depth: 2,
        };
        let response = ResponseEnvelope::ok(&output);

        assert_eq!(response.protocol, RESPONSE_PROTOCOL);
        assert_eq!(response.status, ResponseStatus::Ok);
        assert_eq!(response.summary, "42");
        assert_eq!(response.metadata.instructions_executed, Some(7));
        assert_eq!(response.metadata.max_call_depth, Some(2));
        assert!(response.facets.is_empty());
        assert_eq!(
            response.value().map(|value| &value.data),
            Some(&ResponseValueData::Integer { value: 42 })
        );
    }

    #[test]
    fn debug_responses_include_vm_metric_facets() {
        let output = VmOutput {
            value: Value::Bool(true),
            instructions_executed: 3,
            max_call_depth: 1,
        };
        let response = ResponseEnvelope::ok_with_options(
            &output,
            None,
            ResponseOptions {
                detail: ResponseDetail::Debug,
            },
        );

        assert_eq!(response.facets.len(), 1);
        assert_eq!(response.facets[0].name, "vm.metrics");
        assert_eq!(
            response.facets[0].entries.get("instructions_executed"),
            Some(&"3".to_string())
        );
    }

    #[test]
    fn response_values_do_not_expose_function_internals() {
        let value = ResponseValue::from(&Value::Function(FunctionValue::new(9)));

        assert_eq!(value.display, "#<fn:9>");
        assert_eq!(value.data, ResponseValueData::Function { opaque: true });
    }
}

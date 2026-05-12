use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use serde::Serialize;

use crate::{capability::CapabilityProfile, vm::Value};

pub type HostCallResult = Result<Value, HostCallFailure>;

pub trait HostFunction: Send + Sync + 'static {
    fn call(&self, context: &HostCallContext, args: &[Value]) -> HostCallResult;
}

#[derive(Clone)]
pub struct HostFunctionRegistry {
    functions: BTreeMap<String, RegisteredHostFunction>,
}

impl HostFunctionRegistry {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
        }
    }

    pub fn with_function<F>(mut self, spec: HostFunctionSpec, function: F) -> Self
    where
        F: Fn(&HostCallContext, &[Value]) -> HostCallResult + Send + Sync + 'static,
    {
        self.register_function(spec, function);
        self
    }

    pub fn register_function<F>(&mut self, spec: HostFunctionSpec, function: F)
    where
        F: Fn(&HostCallContext, &[Value]) -> HostCallResult + Send + Sync + 'static,
    {
        self.register_host_function(spec, CallbackHostFunction { function });
    }

    pub fn register_host_function<F>(&mut self, spec: HostFunctionSpec, function: F)
    where
        F: HostFunction,
    {
        self.functions.insert(
            spec.name.clone(),
            RegisteredHostFunction {
                spec,
                function: Arc::new(function),
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredHostFunction> {
        self.functions.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    pub fn names(&self) -> BTreeSet<String> {
        self.functions.keys().cloned().collect()
    }

    pub fn specs(&self) -> Vec<&HostFunctionSpec> {
        self.functions
            .values()
            .map(|function| &function.spec)
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

impl Default for HostFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for HostFunctionRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostFunctionRegistry")
            .field("functions", &self.specs())
            .finish()
    }
}

impl PartialEq for HostFunctionRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.specs() == other.specs()
    }
}

impl Eq for HostFunctionRegistry {}

#[derive(Clone)]
pub struct RegisteredHostFunction {
    spec: HostFunctionSpec,
    function: Arc<dyn HostFunction>,
}

impl RegisteredHostFunction {
    pub fn spec(&self) -> &HostFunctionSpec {
        &self.spec
    }

    pub fn call(&self, context: &HostCallContext, args: &[Value]) -> HostCallResult {
        self.function.call(context, args)
    }
}

impl fmt::Debug for RegisteredHostFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredHostFunction")
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}

struct CallbackHostFunction<F> {
    function: F,
}

impl<F> HostFunction for CallbackHostFunction<F>
where
    F: Fn(&HostCallContext, &[Value]) -> HostCallResult + Send + Sync + 'static,
{
    fn call(&self, context: &HostCallContext, args: &[Value]) -> HostCallResult {
        (self.function)(context, args)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostFunctionSpec {
    pub name: String,
    pub arity: HostFunctionArity,
    pub required_capability: Option<String>,
    pub trust_zone: Option<String>,
}

impl HostFunctionSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arity: HostFunctionArity::any(),
            required_capability: None,
            trust_zone: None,
        }
    }

    pub fn with_exact_arity(mut self, arity: usize) -> Self {
        self.arity = HostFunctionArity::exact(arity);
        self
    }

    pub fn with_min_arity(mut self, min: usize) -> Self {
        self.arity = HostFunctionArity::at_least(min);
        self
    }

    pub fn with_required_capability(mut self, capability: impl Into<String>) -> Self {
        self.required_capability = Some(capability.into());
        self
    }

    pub fn with_trust_zone(mut self, trust_zone: impl Into<String>) -> Self {
        self.trust_zone = Some(trust_zone.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HostFunctionArity {
    pub min: usize,
    pub max: Option<usize>,
}

impl HostFunctionArity {
    pub fn any() -> Self {
        Self { min: 0, max: None }
    }

    pub fn exact(arity: usize) -> Self {
        Self {
            min: arity,
            max: Some(arity),
        }
    }

    pub fn at_least(min: usize) -> Self {
        Self { min, max: None }
    }

    pub fn allows(self, actual: usize) -> bool {
        actual >= self.min && self.max.is_none_or(|max| actual <= max)
    }

    pub fn description(self) -> String {
        match self.max {
            Some(max) if max == self.min => format!("{} argument(s)", self.min),
            Some(max) => format!("{} to {} argument(s)", self.min, max),
            None if self.min == 0 => "any number of arguments".to_string(),
            None => format!("at least {} argument(s)", self.min),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostCallContext {
    pub function: String,
    pub profile_id: Option<String>,
    pub principal: Option<String>,
    pub trust_zone: Option<String>,
    pub required_capability: Option<String>,
}

impl HostCallContext {
    pub fn new(spec: &HostFunctionSpec, profile: Option<&CapabilityProfile>) -> Self {
        Self {
            function: spec.name.clone(),
            profile_id: profile.map(|profile| profile.profile_id.clone()),
            principal: profile.map(|profile| profile.principal.clone()),
            trust_zone: spec.trust_zone.clone(),
            required_capability: spec.required_capability.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostCallFailure {
    pub message: String,
    pub expected: Vec<String>,
    pub actual: Option<String>,
    pub suggestion: Option<String>,
}

impl HostCallFailure {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            expected: Vec::new(),
            actual: None,
            suggestion: None,
        }
    }

    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected.push(expected.into());
        self
    }

    pub fn with_actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = Some(actual.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_and_calls_host_functions() {
        let mut registry = HostFunctionRegistry::new();
        registry.register_function(
            HostFunctionSpec::new("host/answer").with_exact_arity(0),
            |_context, _args| Ok(Value::Integer(42)),
        );

        let function = registry.get("host/answer").expect("host function");
        let context = HostCallContext::new(function.spec(), None);

        assert_eq!(function.call(&context, &[]), Ok(Value::Integer(42)));
        assert!(registry.contains("host/answer"));
        assert_eq!(
            registry.names().into_iter().collect::<Vec<_>>(),
            ["host/answer"]
        );
    }

    #[test]
    fn arity_descriptions_are_agent_readable() {
        assert!(HostFunctionArity::exact(2).allows(2));
        assert!(!HostFunctionArity::exact(2).allows(1));
        assert!(HostFunctionArity::at_least(1).allows(3));
        assert_eq!(HostFunctionArity::exact(2).description(), "2 argument(s)");
        assert_eq!(
            HostFunctionArity::at_least(1).description(),
            "at least 1 argument(s)"
        );
    }

    #[test]
    fn registry_metadata_is_inspectable() {
        let empty = HostFunctionRegistry::default();
        assert!(empty.is_empty());
        assert_eq!(empty.specs(), Vec::<&HostFunctionSpec>::new());

        let spec = HostFunctionSpec::new("host/sum").with_min_arity(1);
        let registry = HostFunctionRegistry::new()
            .with_function(spec.clone(), |_context, _args| Ok(Value::Nil));
        let same = HostFunctionRegistry::new()
            .with_function(spec.clone(), |_context, _args| Ok(Value::Nil));
        let other = HostFunctionRegistry::new()
            .with_function(HostFunctionSpec::new("host/other"), |_context, _args| {
                Ok(Value::Nil)
            });

        assert_eq!(registry, same);
        assert_ne!(registry, other);
        assert_eq!(registry.specs(), vec![&spec]);
        assert!(format!("{registry:?}").contains("host/sum"));
        assert!(format!("{:?}", registry.get("host/sum").unwrap()).contains("host/sum"));
        assert_eq!(
            HostFunctionArity {
                min: 1,
                max: Some(3),
            }
            .description(),
            "1 to 3 argument(s)"
        );
    }

    #[test]
    fn host_call_context_includes_profile_metadata() {
        let spec = HostFunctionSpec::new("host/secret")
            .with_required_capability("host/secret")
            .with_trust_zone("project.markodb");
        let profile = CapabilityProfile::new("dev", "agent.alpha", "project.markodb")
            .with_capability("host/secret");

        let context = HostCallContext::new(&spec, Some(&profile));

        assert_eq!(context.function, "host/secret");
        assert_eq!(context.profile_id.as_deref(), Some("dev"));
        assert_eq!(context.principal.as_deref(), Some("agent.alpha"));
        assert_eq!(context.trust_zone.as_deref(), Some("project.markodb"));
        assert_eq!(context.required_capability.as_deref(), Some("host/secret"));
    }
}

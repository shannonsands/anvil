use std::path::Path;

use anvil_core::{ReplInteraction, ReplResponse, ReplSession, format_datums, read_repl_input};
use cucumber::{World as _, gherkin::Step, given, then, when};

#[derive(Debug, Default, cucumber::World)]
struct AnvilWorld {
    response: Option<String>,
    source: String,
    repl_response: Option<ReplResponse>,
    repl_session: ReplSession,
    repl_interaction: Option<ReplInteraction>,
    json_response: Option<serde_json::Value>,
}

#[given("a fresh Anvil planning scaffold")]
async fn fresh_scaffold(world: &mut AnvilWorld) {
    world.response = None;
}

#[when("the agent asks for the project shape")]
async fn asks_for_project_shape(world: &mut AnvilWorld) {
    let shape = anvil_core::project_shape();
    world.response = Some(format!("{}: {}", shape.name, shape.status));
}

#[then("the response says Anvil is in phase 0 planning")]
async fn response_says_phase_zero(world: &mut AnvilWorld) {
    assert_eq!(
        world.response.as_deref(),
        Some("Anvil: phase 0 planning scaffold"),
    );
}

#[given(expr = "the agent input {string}")]
async fn agent_input(world: &mut AnvilWorld, source: String) {
    world.source = source;
    world.repl_response = None;
    world.json_response = None;
}

#[given("the agent input")]
async fn agent_doc_input(world: &mut AnvilWorld, #[step] step: &Step) {
    world.source = trim_docstring(step.docstring().expect("agent input docstring")).to_string();
    world.repl_response = None;
    world.json_response = None;
}

#[when("the reader-backed REPL reads the input")]
async fn reader_repl_reads_input(world: &mut AnvilWorld) {
    world.repl_response = Some(read_repl_input(&world.source));
}

#[given("an empty REPL session")]
async fn empty_repl_session(world: &mut AnvilWorld) {
    world.repl_session = ReplSession::new();
    world.repl_interaction = None;
    world.repl_response = None;
}

#[when(expr = "the REPL session reads the line {string}")]
async fn repl_session_reads_line(world: &mut AnvilWorld, source: String) {
    let mut line = source;
    line.push('\n');

    let interaction = world.repl_session.push_line(&line);
    if let Some(response) = interaction.response() {
        world.repl_response = Some(response.clone());
    } else {
        world.repl_response = None;
    }
    world.repl_interaction = Some(interaction);
}

#[when("the REPL response is serialized as JSON")]
async fn repl_response_is_serialized_as_json(world: &mut AnvilWorld) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");

    world.json_response = Some(serde_json::to_value(response).expect("JSON response"));
}

#[when("the REPL interaction is serialized as JSON")]
async fn repl_interaction_is_serialized_as_json(world: &mut AnvilWorld) {
    let interaction = world
        .repl_interaction
        .as_ref()
        .expect("REPL session interaction");

    world.json_response = Some(serde_json::to_value(interaction).expect("JSON interaction"));
}

#[then("the REPL session is waiting for more input")]
async fn repl_session_waiting_for_more_input(world: &mut AnvilWorld) {
    let interaction = world
        .repl_interaction
        .as_ref()
        .expect("REPL session interaction");

    assert!(interaction.is_pending());
    assert!(world.repl_session.is_pending());
}

#[then("the response contains one datum")]
async fn response_contains_one_datum(world: &mut AnvilWorld) {
    assert_response_datum_count(world, 1);
}

#[then(expr = "the response contains {int} datums")]
async fn response_contains_n_datums(world: &mut AnvilWorld, expected: usize) {
    assert_response_datum_count(world, expected);
}

fn assert_response_datum_count(world: &mut AnvilWorld, expected: usize) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");

    assert_eq!(response.datums().len(), expected);
}

#[then(expr = "the first datum prints as {string}")]
async fn first_datum_prints_as(world: &mut AnvilWorld, expected: String) {
    assert_first_datum_prints_as(world, &expected);
}

#[then("the first datum prints as")]
async fn first_datum_prints_as_docstring(world: &mut AnvilWorld, #[step] step: &Step) {
    let expected = trim_docstring(step.docstring().expect("expected datum docstring"));

    assert_first_datum_prints_as(world, expected);
}

fn assert_first_datum_prints_as(world: &mut AnvilWorld, expected: &str) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");
    let first = response.datums().first().expect("first datum");

    assert_eq!(first.to_string(), expected);
}

#[then("the datums print as")]
async fn datums_print_as_docstring(world: &mut AnvilWorld, #[step] step: &Step) {
    let expected = trim_docstring(step.docstring().expect("expected datums docstring"));
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");

    assert_eq!(format_datums(response.datums()), expected);
}

fn trim_docstring(value: &str) -> &str {
    value.trim_matches('\n')
}

#[then("the response is a reader error")]
async fn response_is_reader_error(world: &mut AnvilWorld) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");

    assert!(response.diagnostic().is_some());
}

#[then(expr = "the diagnostic code is {string}")]
async fn diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");
    let diagnostic = response.diagnostic().expect("reader diagnostic");

    assert_eq!(diagnostic.code, expected);
}

#[then(expr = "the JSON status is {string}")]
async fn json_status_is(world: &mut AnvilWorld, expected: String) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["status"], expected);
}

#[then(expr = "the JSON diagnostic code is {string}")]
async fn json_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["diagnostic"]["code"], expected);
}

#[then(expr = "the JSON buffered line count is {int}")]
async fn json_buffered_line_count_is(world: &mut AnvilWorld, expected: usize) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["buffered_lines"], expected);
}

#[tokio::main]
async fn main() {
    let specs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs");
    AnvilWorld::run(specs).await;
}

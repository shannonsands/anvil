use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

fn run_anvil(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_anvil-cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn anvil-cli");

    child
        .stdin
        .as_mut()
        .expect("stdin pipe")
        .write_all(stdin.as_bytes())
        .expect("write stdin");

    child.wait_with_output().expect("wait for anvil-cli")
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout utf8")
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr utf8")
}

#[test]
fn default_command_prints_project_shape() {
    let output = run_anvil(&[], "");

    assert!(output.status.success());
    let stdout = stdout_text(&output);
    assert!(stdout.contains("Anvil: phase 0 planning scaffold"));
    assert!(stdout.contains("Run `anvil-cli repl`"));
}

#[test]
fn help_command_prints_commands() {
    let output = run_anvil(&["help"], "");

    assert!(output.status.success());
    let stdout = stdout_text(&output);
    assert!(stdout.contains("Commands:"));
    assert!(stdout.contains("read [SOURCE]"));
    assert!(stdout.contains("--json"));
}

#[test]
fn unknown_command_fails() {
    let output = run_anvil(&["missing"], "");

    assert!(!output.status.success());
    assert!(stderr_text(&output).contains("unknown command missing"));
    assert!(stdout_text(&output).contains("Commands:"));
}

#[test]
fn read_command_reads_source_argument_as_text() {
    let output = run_anvil(&["read", "(define answer 42)"], "");

    assert!(output.status.success());
    assert!(stdout_text(&output).contains("ok (define answer 42)"));
}

#[test]
fn read_command_reads_stdin_as_json() {
    let output = run_anvil(&["read", "--json"], "(define answer 42)");

    assert!(output.status.success());
    let stdout = stdout_text(&output);
    assert!(stdout.contains(r#""status":"read""#));
    assert!(stdout.contains(r#""evaluation":"not_implemented""#));
}

#[test]
fn syntax_command_reports_diagnostics_as_json() {
    let output = run_anvil(&["syntax", "--json", "(define answer 42"], "");

    assert!(output.status.success());
    let stdout = stdout_text(&output);
    assert!(stdout.contains(r#""status":"error""#));
    assert!(stdout.contains("ANVIL_READER_UNCLOSED_DELIMITER"));
}

#[test]
fn ast_command_prints_lowered_expression() {
    let output = run_anvil(&["ast", "(if ready? :yes :no)"], "");

    assert!(output.status.success());
    assert!(stdout_text(&output).contains("ast (if ready? :yes :no)"));
}

#[test]
fn repl_reads_noninteractive_multiline_input() {
    let output = run_anvil(&["repl"], "(define answer\n42)\n");

    assert!(output.status.success());
    let stdout = stdout_text(&output);
    assert!(stdout.contains("ok (define answer 42)"));
    assert!(!stdout.contains("anvil>"));
}

#[test]
fn repl_rejects_unknown_options() {
    let output = run_anvil(&["repl", "--wat"], "");

    assert!(!output.status.success());
    assert!(stderr_text(&output).contains("unknown option for repl: --wat"));
}

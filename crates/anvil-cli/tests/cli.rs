use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

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
    assert!(stdout.contains("run [SOURCE]"));
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
    assert!(stdout.contains(r#""evaluation":{"status":"not_implemented"}"#));
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
fn run_command_evaluates_in_bootstrap_vm() {
    let output = run_anvil(&["run", "(if false :yes :no)"], "");

    assert!(output.status.success());
    assert!(stdout_text(&output).contains("value :no"));
}

#[test]
fn run_command_reports_compile_diagnostics_as_json() {
    let output = run_anvil(&["run", "--json", "(require planner.search)"], "");

    assert!(output.status.success());
    let stdout = stdout_text(&output);
    assert!(stdout.contains(r#""status":"error""#));
    assert!(stdout.contains("ANVIL_COMPILE_UNSUPPORTED_FORM"));
    assert!(stdout.contains(r#""phase":"compile""#));
}

#[test]
fn run_command_can_require_package_modules() {
    let package = TempPackage::new();
    package.write(
        "Anvil.toml",
        r#"
        [package]
        name = "planner-tools"
        version = "0.1.0"

        [lib]
        module = "planner.tools"
        path = "src/lib.anv"
        "#,
    );
    package.write("src/lib.anv", "(define root true)");
    package.write("src/planner/search.anv", "(define answer 42)");

    let output = run_anvil(
        &[
            "run",
            "--package",
            package.path_str(),
            "(require planner.search) answer",
        ],
        "",
    );

    assert!(output.status.success(), "{}", stderr_text(&output));
    assert!(stdout_text(&output).contains("value 42"));
}

#[test]
fn repl_reads_noninteractive_multiline_input() {
    let output = run_anvil(&["repl"], "(define answer\n42)\nanswer\n");

    assert!(output.status.success());
    let stdout = stdout_text(&output);
    assert_eq!(stdout.matches("value 42").count(), 2);
    assert!(!stdout.contains("anvil>"));
}

#[test]
fn repl_preserves_state_in_json_mode() {
    let output = run_anvil(&["repl", "--json"], "(define answer 42)\nanswer\n");

    assert!(output.status.success());
    let stdout = stdout_text(&output);
    assert_eq!(stdout.matches(r#""status":"value""#).count(), 2);
    assert!(stdout.contains(r#""value":{"kind":"integer","value":42}"#));
}

#[test]
fn repl_can_require_package_modules() {
    let package = TempPackage::new();
    package.write(
        "Anvil.toml",
        r#"
        [package]
        name = "planner-tools"
        version = "0.1.0"

        [lib]
        module = "planner.tools"
        path = "src/lib.anv"
        "#,
    );
    package.write("src/lib.anv", "(define root true)");
    package.write("src/planner/search.anv", "(define answer 42)");

    let output = run_anvil(
        &["repl", "--package", package.path_str()],
        "(require planner.search)\nanswer\n",
    );

    assert!(output.status.success(), "{}", stderr_text(&output));
    assert!(stdout_text(&output).contains("value 42"));
}

#[test]
fn repl_rejects_unknown_options() {
    let output = run_anvil(&["repl", "--wat"], "");

    assert!(!output.status.success());
    assert!(stderr_text(&output).contains("unknown option for repl: --wat"));
}

struct TempPackage {
    path: PathBuf,
    path_str: String,
}

impl TempPackage {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("anvil-cli-test-{nanos}-{id}"));
        fs::create_dir_all(&path).expect("create temp package");
        let path_str = path.to_str().expect("temp path utf8").to_string();

        Self { path, path_str }
    }

    fn path_str(&self) -> &str {
        &self.path_str
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.path.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, unindent(contents)).expect("write package file");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unindent(contents: &str) -> String {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

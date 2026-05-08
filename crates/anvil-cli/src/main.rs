use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
};

use anvil_core::{
    ReaderDiagnostic, ReplInteraction, ReplResponse, ReplSession, SpannedAst, SyntaxObject,
    VmOutput, lower_source, project_shape, read_repl_input, run_source, syntax_from_source,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("repl") => match parse_output_format(args.collect()).and_then(run_repl) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("anvil: {error}");
                ExitCode::FAILURE
            }
        },
        Some("read") => match read_command(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("anvil: {error}");
                ExitCode::FAILURE
            }
        },
        Some("syntax") => match syntax_command(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("anvil: {error}");
                ExitCode::FAILURE
            }
        },
        Some("ast") => match ast_command(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("anvil: {error}");
                ExitCode::FAILURE
            }
        },
        Some("run") => match run_command(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("anvil: {error}");
                ExitCode::FAILURE
            }
        },
        Some("-h" | "--help" | "help") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(command) => {
            eprintln!("anvil: unknown command {command}");
            print_help();
            ExitCode::FAILURE
        }
        None => {
            print_project_shape();
            println!("Run `anvil-cli repl` for the reader-backed REPL.");
            ExitCode::SUCCESS
        }
    }
}

fn print_project_shape() {
    let shape = project_shape();
    println!("{}: {}", shape.name, shape.status);
}

fn print_help() {
    print_project_shape();
    println!();
    println!("Commands:");
    println!("  repl          Start the reader-backed REPL. Evaluation is not implemented yet.");
    println!("  read [SOURCE] Read SOURCE as Anvil datums, or read stdin when SOURCE is omitted.");
    println!(
        "  syntax [SOURCE] Wrap SOURCE as syntax objects, or read stdin when SOURCE is omitted."
    );
    println!(
        "  ast [SOURCE]  Lower SOURCE to the initial core AST, or read stdin when SOURCE is omitted."
    );
    println!("  run [SOURCE]  Compile and run SOURCE in the bootstrap bytecode VM.");
    println!();
    println!("Options:");
    println!("  --json        Emit one JSON response object per input.");
}

fn parse_output_format(args: Vec<String>) -> io::Result<OutputFormat> {
    let mut format = OutputFormat::Text;
    for arg in args {
        match arg.as_str() {
            "--json" => format = OutputFormat::Json,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown option for repl: {other}"),
                ));
            }
        }
    }
    Ok(format)
}

fn run_repl(format: OutputFormat) -> io::Result<()> {
    let stdin = io::stdin();
    let interactive = stdin.is_terminal();
    print_repl_banner(interactive, format);

    let mut session = ReplSession::new();
    while let Some(line) = read_repl_line(&stdin, &mut session, interactive, format)? {
        if is_repl_quit(&line) {
            break;
        }
        if should_skip_repl_line(interactive, &line) {
            continue;
        }

        let interaction = session.push_line(&line);
        print_interaction(&interaction, format)?;
    }

    Ok(())
}

fn print_repl_banner(interactive: bool, format: OutputFormat) {
    if interactive && format == OutputFormat::Text {
        println!("Anvil reader REPL. Evaluation is not implemented yet.");
        println!("Use :quit to exit.");
    }
}

fn read_repl_line(
    stdin: &io::Stdin,
    session: &mut ReplSession,
    interactive: bool,
    format: OutputFormat,
) -> io::Result<Option<String>> {
    print_repl_prompt(interactive, format, session)?;

    let mut line = String::new();
    if stdin.read_line(&mut line)? == 0 {
        print_pending_repl_response(std::mem::take(session), format)?;
        return Ok(None);
    }

    Ok(Some(line))
}

fn print_repl_prompt(
    interactive: bool,
    format: OutputFormat,
    session: &ReplSession,
) -> io::Result<()> {
    if interactive && format == OutputFormat::Text {
        print!("{}", repl_prompt(session));
        io::stdout().flush()?;
    }
    Ok(())
}

fn repl_prompt(session: &ReplSession) -> &'static str {
    if session.is_pending() {
        "....> "
    } else {
        "anvil> "
    }
}

fn print_pending_repl_response(session: ReplSession, format: OutputFormat) -> io::Result<()> {
    if let Some(response) = session.finish() {
        print_response(&response, format)?;
    }
    Ok(())
}

fn is_repl_quit(line: &str) -> bool {
    matches!(line.trim(), ":quit" | ":q" | ":exit")
}

fn should_skip_repl_line(interactive: bool, line: &str) -> bool {
    interactive && line.trim().is_empty()
}

fn read_command(args: Vec<String>) -> io::Result<()> {
    let (format, args) = split_read_args(args)?;
    let source = if args.is_empty() {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        source
    } else {
        args.join(" ")
    };

    print_response(&read_repl_input(&source), format)?;
    Ok(())
}

fn syntax_command(args: Vec<String>) -> io::Result<()> {
    let (format, args) = split_source_args(args)?;
    let source = if args.is_empty() {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        source
    } else {
        args.join(" ")
    };

    match syntax_from_source(&source) {
        Ok(objects) => print_syntax_response(&objects, format)?,
        Err(diagnostic) => print_syntax_diagnostic(&diagnostic, format)?,
    }
    Ok(())
}

fn ast_command(args: Vec<String>) -> io::Result<()> {
    let (format, args) = split_source_args(args)?;
    let source = if args.is_empty() {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        source
    } else {
        args.join(" ")
    };

    match lower_source(&source) {
        Ok(expressions) => print_ast_response(&expressions, format)?,
        Err(diagnostic) => print_command_diagnostic(&diagnostic, format)?,
    }
    Ok(())
}

fn run_command(args: Vec<String>) -> io::Result<()> {
    let (format, args) = split_source_args(args)?;
    let source = if args.is_empty() {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        source
    } else {
        args.join(" ")
    };

    match run_source(&source) {
        Ok(output) => print_run_response(&output, format)?,
        Err(diagnostic) => print_run_diagnostic(&diagnostic, format)?,
    }
    Ok(())
}

fn split_read_args(args: Vec<String>) -> io::Result<(OutputFormat, Vec<String>)> {
    split_source_args(args)
}

fn split_source_args(args: Vec<String>) -> io::Result<(OutputFormat, Vec<String>)> {
    let mut format = OutputFormat::Text;
    let mut source_args = Vec::new();

    for arg in args {
        if arg == "--json" {
            format = OutputFormat::Json;
        } else {
            source_args.push(arg);
        }
    }

    Ok((format, source_args))
}

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum AstCommandResponse<'a> {
    Ast { expressions: &'a [SpannedAst] },
    Error { diagnostic: &'a ReaderDiagnostic },
}

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum SyntaxCommandResponse<'a> {
    Syntax { objects: &'a [SyntaxObject] },
    Error { diagnostic: &'a ReaderDiagnostic },
}

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum RunCommandResponse<'a> {
    Value { output: &'a VmOutput },
    Error { diagnostic: &'a ReaderDiagnostic },
}

fn print_syntax_response(objects: &[SyntaxObject], format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Text => {
            if objects.is_empty() {
                println!("ok syntax");
            }
            for object in objects {
                println!("syntax {object}");
            }
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(&SyntaxCommandResponse::Syntax { objects })?
            );
        }
    }

    Ok(())
}

fn print_syntax_diagnostic(diagnostic: &ReaderDiagnostic, format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Text => print_diagnostic(diagnostic),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(&SyntaxCommandResponse::Error { diagnostic })?
            );
        }
    }

    Ok(())
}

fn print_ast_response(expressions: &[SpannedAst], format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Text => {
            if expressions.is_empty() {
                println!("ok ast");
            }
            for expression in expressions {
                println!("ast {expression}");
            }
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(&AstCommandResponse::Ast { expressions })?
            );
        }
    }

    Ok(())
}

fn print_run_response(output: &VmOutput, format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Text => {
            println!("value {}", output.value);
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(&RunCommandResponse::Value { output })?
            );
        }
    }

    Ok(())
}

fn print_run_diagnostic(diagnostic: &ReaderDiagnostic, format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Text => print_diagnostic(diagnostic),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(&RunCommandResponse::Error { diagnostic })?
            );
        }
    }

    Ok(())
}

fn print_command_diagnostic(diagnostic: &ReaderDiagnostic, format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Text => print_diagnostic(diagnostic),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(&AstCommandResponse::Error { diagnostic })?
            );
        }
    }

    Ok(())
}

fn print_response(response: &ReplResponse, format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Text => print_text_response(response),
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(response)?);
            Ok(())
        }
    }
}

fn print_interaction(interaction: &ReplInteraction, format: OutputFormat) -> io::Result<()> {
    match interaction {
        ReplInteraction::Complete(response) => print_response(response, format),
        ReplInteraction::Pending { .. } => {
            if format == OutputFormat::Json {
                println!("{}", serde_json::to_string(interaction)?);
            }
            Ok(())
        }
    }
}

fn print_text_response(response: &ReplResponse) -> io::Result<()> {
    match response {
        ReplResponse::Read { datums, .. } => {
            if datums.is_empty() {
                println!("ok");
            }
            for datum in datums {
                println!("ok {datum}");
            }
        }
        ReplResponse::Error { diagnostic } => print_diagnostic(diagnostic),
    }
    Ok(())
}

fn print_diagnostic(diagnostic: &ReaderDiagnostic) {
    println!("{}", diagnostic.render_code_frame());
}

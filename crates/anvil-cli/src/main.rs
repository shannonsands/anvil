use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
};

use anvil_core::{
    ReaderDiagnostic, ReplInteraction, ReplResponse, ReplSession, SpannedAst, lower_source,
    project_shape, read_repl_input,
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
        Some("ast") => match ast_command(args.collect()) {
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
        "  ast [SOURCE]  Lower SOURCE to the initial core AST, or read stdin when SOURCE is omitted."
    );
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

    if interactive && format == OutputFormat::Text {
        println!("Anvil reader REPL. Evaluation is not implemented yet.");
        println!("Use :quit to exit.");
    }

    let mut session = ReplSession::new();

    loop {
        if interactive && format == OutputFormat::Text {
            if session.is_pending() {
                print!("....> ");
            } else {
                print!("anvil> ");
            }
            io::stdout().flush()?;
        }

        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            if let Some(response) = session.finish() {
                print_response(&response, format)?;
            }
            break;
        }

        let trimmed = line.trim();
        if matches!(trimmed, ":quit" | ":q" | ":exit") {
            break;
        }

        if interactive && trimmed.is_empty() {
            continue;
        }

        let interaction = session.push_line(&line);
        print_interaction(&interaction, format)?;
    }

    Ok(())
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

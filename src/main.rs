use anyhow::Result;
use clap::Parser;
use spctr::{diag, interp, jit, parser, resolver, stdlib::imports, typeck};

use std::fs;
use std::process::ExitCode;
use std::thread;

const INTERP_STACK_SIZE: usize = 64 * 1024 * 1024;

#[derive(Parser)]
#[command(name = "spctr")]
struct Cli {
    /// Source file to evaluate.
    file: Option<String>,
    /// Inline source.
    #[arg(short = 'c', long = "input")]
    input: Option<String>,
    /// Start the REPL.
    #[arg(long)]
    repl: bool,
    /// Print the inferred program type then evaluate.
    #[arg(long = "type")]
    show_type: bool,
    /// Type-check only — skip evaluation.
    #[arg(long)]
    check: bool,
    /// Run via the Cranelift JIT.
    #[arg(long)]
    jit: bool,
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let show_type = cli.show_type;
    let only_check = cli.check;
    let use_jit = cli.jit;

    let mode = if cli.repl {
        Mode::Repl
    } else if let Some(s) = cli.input {
        Mode::Source {
            filename: "<inline>".to_string(),
            source: s,
        }
    } else if let Some(path) = cli.file {
        Mode::Source {
            source: fs::read_to_string(&path)?,
            filename: path,
        }
    } else {
        Mode::Repl
    };

    let handle = thread::Builder::new()
        .stack_size(INTERP_STACK_SIZE)
        .spawn(move || run(mode, show_type, only_check, use_jit))?;
    handle.join().expect("interpreter thread panicked")
}

enum Mode {
    Source { filename: String, source: String },
    Repl,
}

fn run(mode: Mode, show_type: bool, only_check: bool, use_jit: bool) -> Result<ExitCode> {
    match mode {
        Mode::Source { filename, source } => {
            run_source(&filename, &source, show_type, only_check, use_jit)
        }
        Mode::Repl => run_repl(),
    }
}

fn run_source(
    filename: &str,
    source: &str,
    show_type: bool,
    only_check: bool,
    use_jit: bool,
) -> Result<ExitCode> {
    if let Some(parent) = std::path::Path::new(filename).parent() {
        if !parent.as_os_str().is_empty() {
            imports::set_current_dir(parent.to_path_buf());
        }
    }
    let ast = match parser::parse(source) {
        Ok(ast) => ast,
        Err(diags) => {
            for d in &diags {
                diag::report(filename, source, d);
            }
            return Ok(ExitCode::FAILURE);
        }
    };

    if let Err(d) = resolver::resolve(&ast, &interp::ROOT_NAMES) {
        diag::report(filename, source, &d);
        return Ok(ExitCode::FAILURE);
    }

    if show_type || only_check {
        let root_types = interp::root_types();
        let result = typeck::check(&ast, &root_types);
        for w in &result.warnings {
            diag::report(filename, source, w);
        }
        if show_type {
            println!("type: {}", result.program_type);
        }
        if only_check {
            return Ok(if result.warnings.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            });
        }
    }

    if use_jit {
        // JIT prints the value internally via `spctr_print` so it can render
        // any program type (record / list / string / etc.) without forcing
        // `__spctr_main` to vary its return ABI.
        return match jit::run_with_display(&ast) {
            Ok(()) => Ok(ExitCode::SUCCESS),
            Err(d) => {
                diag::report(filename, source, &d);
                Ok(ExitCode::FAILURE)
            }
        };
    }

    match interp::run(&ast) {
        Ok(v) => {
            println!("{}", v);
            Ok(ExitCode::SUCCESS)
        }
        Err(d) => {
            diag::report(filename, source, &d);
            Ok(ExitCode::FAILURE)
        }
    }
}

fn run_repl() -> Result<ExitCode> {
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;

    println!("spctr REPL — Ctrl-D to exit");

    let mut rl = DefaultEditor::new()?;
    let history = dirs_history();
    if let Some(p) = &history {
        let _ = rl.load_history(p);
    }

    let mut accumulated: Vec<spctr::ast::Bind> = Vec::new();

    loop {
        match rl.readline("spctr> ") {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);
                evaluate_repl_line(&line, &mut accumulated);
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("readline error: {}", e);
                break;
            }
        }
    }

    if let Some(p) = &history {
        let _ = rl.save_history(p);
    }

    Ok(ExitCode::SUCCESS)
}

fn dirs_history() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".spctr_history"))
}

fn evaluate_repl_line(line: &str, accumulated: &mut Vec<spctr::ast::Bind>) {
    let stmt = match parser::parse(line) {
        Ok(s) => s,
        Err(diags) => {
            for d in &diags {
                diag::report("<repl>", line, d);
            }
            return;
        }
    };

    let combined = spctr::ast::Statement {
        definitions: {
            let mut d = accumulated.clone();
            d.extend(stmt.definitions.clone());
            d
        },
        body: stmt.body.clone(),
    };

    if let Err(d) = resolver::resolve(&combined, &interp::ROOT_NAMES) {
        diag::report("<repl>", line, &d);
        return;
    }

    match interp::run(&combined) {
        Ok(v) => println!("{}", v),
        Err(d) => diag::report("<repl>", line, &d),
    }

    accumulated.extend(stmt.definitions);
}

use clap::Parser;
use langulo::repl::Repl;
use std::fs;
use std::path::PathBuf;
use colored::Colorize;
use langulo::errors::LanguloResult;
use langulo::parser::parse;
use langulo::runtime::{eval_python, init_python};
use langulo::transpile::transpile;

#[derive(Parser)]
#[command(name = "langulo")]
#[command(about = "a programming language")]
struct Cli {
    #[arg(short, long)]
    debug: bool,

    /// Path to a .lgl file to execute
    file: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    let show_python = cli.debug || cfg!(debug_assertions);

    if let Some(file_path) = cli.file {
        let source = match fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file '{}': {}", file_path.display(), e);
                std::process::exit(1);
            }
        };

        if let Err(err) = run_file(&source) {
            let mut buf = String::new();
            let report_handler = miette::GraphicalReportHandler::new_themed(miette::GraphicalTheme::unicode());
            if report_handler.render_report(&mut buf, &err).is_ok() {
                eprint!("{}", buf);
            } else {
                eprintln!("{}: {}", "Error".red().bold(), &err);
            }
        }
    } else {
        if let Err(e) = Repl::new(show_python).run() {
            eprintln!("Fatal error: {:?}", e);
            std::process::exit(1);
        }
    }
}

fn run_file(source: &String) -> LanguloResult<()> {
    init_python()?;
    let ast = parse(source)?;
    let statements = transpile(&ast, source)?;
    let result = eval_python(&statements)?;
    println!(
        "\t{} {} {}",
        "=>".green(),
        result.display.green().bold(),
        format!(": {}", result.py_type).dimmed()
    );
    Ok(())
}

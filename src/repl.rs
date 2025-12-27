use crate::errors::{LanguriaError, LanguriaResult};
use crate::parser::parse;
use crate::runtime::{eval_python, init_python};
use crate::transpile::transpile;
use colored::Colorize;
use miette::{GraphicalReportHandler, GraphicalTheme};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

pub struct Repl {
    show_python: bool,
    report_handler: GraphicalReportHandler,
}

impl Repl {
    pub fn new(show_python: bool) -> Self {
        Self {
            show_python,
            report_handler: GraphicalReportHandler::new_themed(GraphicalTheme::unicode()),
        }
    }

    pub fn run(&mut self) -> LanguriaResult<()> {
        init_python()?;

        println!(
            "{} {}",
            "Languria REPL".magenta().bold(),
            "v0.1.0".dimmed() // todo read version from cargo
        );
        println!("{}", "Type expressions to evaluate. Ctrl+D to exit.".dimmed());
        if self.show_python {
            println!("{}", "(Showing transpiled Python)".yellow().dimmed());
        }
        println!();

        self.repl_loop().map_err(|e| LanguriaError::InternalError {
            message: format!("REPL error: {}", e),
        })
    }

    fn repl_loop(&mut self) -> Result<(), ReadlineError> {
        let mut rl = DefaultEditor::new()?;

        let history_path = std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".languria_history"));

        if let Some(ref path) = history_path {
            let _ = rl.load_history(path);
        }

        loop {
            let prompt = ">>> ".cyan().bold().to_string();
            match rl.readline(&prompt) {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let _ = rl.add_history_entry(line);
                    self.eval_line(line);
                }
                Err(ReadlineError::Interrupted) => {
                    println!("{}", "^C".dimmed());
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    println!("{}", "Goodbye!".dimmed());
                    break;
                }
                Err(err) => {
                    eprintln!("{}: {:?}", "Error".red(), err);
                    break;
                }
            }
        }

        if let Some(ref path) = history_path {
            let _ = rl.save_history(path);
        }

        Ok(())
    }

    fn eval_line(&mut self, input: &str) {
        let ast = match parse(input) {
            Ok(ast) => ast,
            Err(e) => {
                self.print_error(&e);
                return;
            }
        };

        let python_code = match transpile(&ast) {
            Ok(code) => code,
            Err(e) => {
                self.print_error(&e);
                return;
            }
        };

        if self.show_python {
            println!("{} {}", "→".dimmed(), python_code.yellow());
        }

        match eval_python(&python_code) {
            Ok(result) => {
                println!(
                    "{} {} {}",
                    "=".green(),
                    result.display.green().bold(),
                    format!(": {}", result.py_type).dimmed()
                );
            }
            Err(e) => {
                self.print_error(&e);
            }
        }
    }

    fn print_error(&self, error: &LanguriaError) {
        let mut buf = String::new();
        if self.report_handler.render_report(&mut buf, error).is_ok() {
            eprint!("{}", buf);
        } else {
            eprintln!("{}: {}", "Error".red().bold(), error);
        }
    }
}
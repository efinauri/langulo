use crate::errors::{LanguloError, LanguloResult};
use crate::parser::parse;
use crate::runtime::{eval_python, init_python};
use crate::transpile::transpile;
use colored::Colorize;
use logos::Logos;
use miette::{GraphicalReportHandler, GraphicalTheme};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use crate::lexer::Tok;

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

    pub fn run(&mut self) -> LanguloResult<()> {
        init_python()?;

        println!(
            "{} {}",
            "Langulo REPL".magenta().bold(),
            "v0.1.0".dimmed() // todo read version from cargo
        );
        println!("{}", "Type expressions to evaluate. Ctrl+D to exit.".dimmed());
        if self.show_python {
            println!("{}", "(Showing transpiled Python)".yellow().dimmed());
        }
        println!();

        self.repl_loop().map_err(|e| LanguloError::InternalError {
            message: format!("REPL error: {}", e),
        })
    }

    fn repl_loop(&mut self) -> Result<(), ReadlineError> {
        let mut rl = DefaultEditor::new()?;

        let history_path = std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".langulo_history"));

        if let Some(ref path) = history_path {
            let _ = rl.load_history(path);
        }

        loop {
            match self.read_complete_input(&mut rl) {
                Ok(Some(input)) => {
                    let _ = rl.add_history_entry(&input);
                    self.eval_line(&input);
                }
                Ok(None) => {
                    continue;
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

    fn read_complete_input(&self, rl: &mut DefaultEditor) -> Result<Option<String>, ReadlineError> {
        let prompt = ">>> ".cyan().bold().to_string();
        let first_line = rl.readline(&prompt)?;

        let trimmed = first_line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let mut input = first_line;

        loop {
            let (indent_level, has_continuation) = Self::analyze_input(&input);
            if indent_level <= 0 && !has_continuation {
                break;
            } // else need more input
            let continuation_prompt = self.make_continuation_prompt(indent_level.max(0) as usize, has_continuation);
            let next_line = rl.readline(&continuation_prompt)?;

            input.push('\n');
            input.push_str(&next_line);
        }

        Ok(Some(input))
    }

    fn analyze_input(input: &str) -> (i32, bool) {
        let mut indent_level = 0i32;
        let mut has_continuation = false;

        for token in Tok::lexer(input) {
            match token {
                Ok(Tok::LBrace) => indent_level += 1,
                Ok(Tok::RBrace) => indent_level -= 1,
                Ok(Tok::LineContinuation(_)) => has_continuation = true,
                Ok(Tok::Newline(_)) => has_continuation = false,
                Ok(_) => has_continuation = false,
                Err(_) => {} // parser will catch this later anyways
            }
        }
        (indent_level, has_continuation)
    }

    fn make_continuation_prompt(&self, indent_level: usize, has_continuation: bool) -> String {
        let dots = "... ".cyan().to_string();
        let effective_indent = if has_continuation && indent_level == 0 {
            1
        } else {
            indent_level
        };
        let indent = dots.repeat(effective_indent);
        format!("{}{}", dots, indent)
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

    fn print_error(&self, error: &LanguloError) {
        let mut buf = String::new();
        if self.report_handler.render_report(&mut buf, error).is_ok() {
            eprint!("{}", buf);
        } else {
            eprintln!("{}: {}", "Error".red().bold(), error);
        }
    }
}
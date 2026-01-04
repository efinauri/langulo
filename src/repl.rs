use crate::errors::{LanguloError, LanguloResult};
use crate::lexer::Tok;
use crate::lexer::Tok::TrailingBackslash;
use crate::parser::parse;
use crate::runtime::{eval_python, init_python};
use crate::transpile::transpile;
use colored::Colorize;
use logos::Logos;
use miette::{GraphicalReportHandler, GraphicalTheme};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::cmp::max;
use std::process::exit;

const COMMAND_PREFIX: &'static str = "::";

struct ReplCommand {
    name: &'static str,
    alias: &'static str,
    description: &'static str,
}

const REPL_COMMANDS: &[ReplCommand] = &[
    ReplCommand {
        name: "help",
        alias: "h",
        description: "Show this help message",
    },
    ReplCommand {
        name: "transpile",
        alias: "t",
        description: "Toggle transpiled code display",
    },
    ReplCommand {
        name: "quit",
        alias: "q",
        description: "Exit the REPL",
    },
];

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
            format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
        );
        println!(
            "{}",
            "Type expressions to evaluate. Ctrl+D to exit.".dimmed()
        );
        if self.show_python {
            println!("{}", "(Showing transpiled Python)".yellow().dimmed());
        }
        println!();

        self.repl_loop().map_err(|e| LanguloError::InternalError {
            _message: format!("REPL error: {}", e),
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
                    if let Some(cmd) = input.strip_prefix(COMMAND_PREFIX) {
                        self.handle_command(cmd);
                        continue;
                    }

                    let _ = rl.add_history_entry(&input);
                    if !self.eval_line(&input) {
                        println!("{}", "Goodbye!".dimmed());
                        break;
                    }
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
        let mut user_input = rl.readline(&prompt)?;

        if user_input.trim().is_empty() {
            return Ok(None);
        }

        loop {
            let (indent_level, has_continuation) = Self::analyze_input(&user_input);
            if indent_level <= 0 && !has_continuation {
                break;
            } // else grow partial user input
            let continuation_prompt = self.make_continuation_prompt(indent_level);
            user_input.push('\n');
            user_input.push_str(&rl.readline(&continuation_prompt)?);
        }

        Ok(Some(user_input))
    }

    /// determines if the user is passing a partial input they intend to grow with other lines.
    /// this includes opening and not closing a multiline string, a grouping expression, explicitly
    /// adding a line break, etc. etc.
    fn analyze_input(input: &str) -> (usize, bool) {
        let mut indent_level = 0i32;
        let mut has_continuation = false;
        let mut in_unclosed_string = false;

        let lex = Tok::lexer(input);

        for token in lex.clone() {
            match token {
                Ok(Tok::LBrace) => {
                    indent_level += 1;
                    has_continuation = false;
                }
                Ok(Tok::RBrace) => {
                    indent_level -= 1;
                    has_continuation = false;
                }
                Ok(Tok::LineContinuation(_)) => {
                    has_continuation = true;
                }
                Ok(Tok::Newline(_)) => {
                    has_continuation = false;
                }
                Ok(Tok::TripleDoubleQuote(s)) => {
                    in_unclosed_string = !s.ends_with("\"\"\"") || s.len() < 6;
                    has_continuation = false;
                }
                Ok(Tok::TripleSingleQuote(s)) => {
                    in_unclosed_string = !s.ends_with("'''") || s.len() < 6;
                    has_continuation = false;
                }
                Ok(_) => {
                    has_continuation = false;
                }
                Err(_) => {}
            }
        }
        if let Some(Ok(TrailingBackslash(_))) = lex.last() {
            has_continuation = true;
        }

        let needs_more = indent_level > 0 || has_continuation || in_unclosed_string;

        if needs_more {
            indent_level = max(indent_level, 1);
        }
        if in_unclosed_string {
            indent_level = 0
        }
        (max(indent_level, 0i32) as usize, needs_more)
    }

    fn make_continuation_prompt(&self, indent_level: usize) -> String {
        let dots = "... ".cyan().to_string();
        let indent = dots.repeat(indent_level);
        format!("{}{}", dots, indent)
    }

    fn eval_line(&mut self, input: &str) -> bool {
        let ast = match parse(input) {
            Ok(ast) => ast,
            Err(e) => {
                self.print_error(&e);
                return true;
            }
        };

        let statements = match transpile(&ast, input) {
            Ok(stmts) => stmts,
            Err(e) => {
                self.print_error(&e);
                return true;
            }
        };

        if self.show_python {
            for stmt in &statements {
                println!("{} {}", "→".dimmed(), stmt.yellow());
            }
        }

        match eval_python(&statements) {
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

        true
    }

    fn handle_command(&mut self, cmd: &str) {
        match cmd {
            "help"|"h" => {
                self.print_help();
            }
            "transpile"|"t" => {
                self.show_python = !self.show_python;
                let status = if self.show_python {
                    "enabled"
                } else {
                    "disabled"
                };
                println!("{} {}", "Transpiled code display".cyan(), status.yellow());
            }
            "quit"|"q" => exit(0),
            _ => {
                println!("{} {}{}", "Unknown command:".red(), COMMAND_PREFIX, cmd);
                println!("Type {}help for available commands", COMMAND_PREFIX);
            }
        }
    }

    fn print_help(&self) {
        println!("{}", "REPL Commands:".cyan().bold());
        for cmd in REPL_COMMANDS {
            println!(
                "  {}{}  {}  {}",
                COMMAND_PREFIX,
                cmd.name.yellow(),
                format!("({}{})", COMMAND_PREFIX, cmd.alias).dimmed(),
                cmd.description
            );
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

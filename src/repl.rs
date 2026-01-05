use crate::errors::{LanguloError, LanguloResult};
use crate::lexer::Tok;
use crate::lexer::Tok::TrailingBackslash;
use crate::parser::parse;
use crate::runtime::{eval_python, init_python};
use crate::transpile::transpile;
use colored::Colorize;
use logos::Logos;
use miette::{GraphicalReportHandler, GraphicalTheme};
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use std::borrow::Cow;
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
        let h = LanguloHighLighter::default();
        let mut rl = Editor::new()?;
        rl.set_helper(Some(h));

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

    fn read_complete_input(
        &self,
        rl: &mut Editor<LanguloHighLighter, DefaultHistory>,
    ) -> Result<Option<String>, ReadlineError> {
        let prompt = ">>> ".cyan().bold().to_string();
        let mut user_input = rl.readline(&prompt)?;

        if user_input.trim().is_empty() {
            return Ok(None);
        }

        loop {
            let state = ReplInputState::from_input(&user_input);
            rl.helper_mut().unwrap().is_in_unclosed_string = state.is_in_unclosed_string;
            if state.indent_level <= 0 && !state.is_partial {
                break;
            } // else grow partial user input
            let continuation_prompt = self.make_continuation_prompt(state.indent_level);
            user_input.push('\n');
            user_input.push_str(&rl.readline(&continuation_prompt)?);
        }

        Ok(Some(user_input))
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
            "help" | "h" => {
                self.print_help();
            }
            "transpile" | "t" => {
                self.show_python = !self.show_python;
                let status = if self.show_python {
                    "enabled"
                } else {
                    "disabled"
                };
                println!("{} {}", "Transpiled code display".cyan(), status.yellow());
            }
            "quit" | "q" => exit(0),
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

#[derive(Default)]
struct LanguloHighLighter {
    pub is_in_unclosed_string: bool,
}

impl Hinter for LanguloHighLighter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        None
    }
}
impl Validator for LanguloHighLighter {}
impl Completer for LanguloHighLighter {
    type Candidate = String;
}
impl Helper for LanguloHighLighter {}

impl Highlighter for LanguloHighLighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Owned(colorize_line(line, self.is_in_unclosed_string))
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Owned(prompt.cyan().bold().to_string())
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(hint.dimmed().to_string())
    }

    fn highlight_char(&self, line: &str, pos: usize, kind: CmdKind) -> bool {
        true
    }
}

fn colorize_line(line: &str, mut in_string: bool) -> String {
    use colored::*;

    let mut out = String::new();
    let mut buf = String::new();

    let mut chars = line.chars().peekable();
    let mut delim: Option<&'static str> = if in_string { Some("\"\"\"") } else { None };

    while let Some(c) = chars.next() {
        if !in_string {
            // ─── Opening quotes ───
            if (c == '"' || c == '\'')
                && chars.peek() == Some(&c)
                && chars.clone().nth(1) == Some(c)
            {
                flush_lexed(&mut out, &mut buf);

                let d = if c == '"' { "\"\"\"" } else { "'''" };
                out.push_str(&d.green().to_string());
                chars.next();
                chars.next();

                in_string = true;
                delim = Some(d);
            } else if c == '"' || c == '\'' {
                flush_lexed(&mut out, &mut buf);

                out.push_str(&c.to_string().green().to_string());
                in_string = true;
                delim = Some(if c == '"' { "\"" } else { "'" });
            } else {
                buf.push(c);
            }
        } else {
            // ─── Inside string ───
            out.push_str(&c.to_string().green().to_string());

            if let Some(d) = delim {
                if d.len() == 1 && c.to_string() == d {
                    in_string = false;
                    delim = None;
                } else if d.len() == 3
                    && c == d.chars().last().unwrap()
                    && chars.peek() == Some(&d.chars().nth(1).unwrap())
                    && chars.clone().nth(1) == Some(d.chars().nth(0).unwrap())
                {
                    out.push_str(&chars.next().unwrap().to_string().green().to_string());
                    out.push_str(&chars.next().unwrap().to_string().green().to_string());
                    in_string = false;
                    delim = None;
                }
            }
        }
    }

    flush_lexed(&mut out, &mut buf);
    out
}


fn flush_lexed(out: &mut String, buf: &mut String) {
    if buf.is_empty() {
        return;
    }

    for tok in Tok::lexer(buf) {
        match tok {
            Ok(t) => out.push_str(&color_token(t)),
            Err(_) => out.push_str(buf),
        }
    }

    buf.clear();
}


fn color_token(tok: Tok) -> String {
    use Tok::*;
    use colored::*;
    match tok {
        Num(_) | Literal(_) | True(_) | False(_) | StringLitSingle(_) | StringLitDouble(_)
        | TripleSingleQuote(_) | Key(_) | Value(_) | Index(_) | QuestionMark | At
        | TripleDoubleQuote(_) => tok.info().bright_yellow().to_string(),
        Plus | Minus | Star | Slash | Percent | Caret | Eq(_) | Neq(_) | Lt | Gt | Leq(_)
        | Geq(_) | And(_) | Or(_) | Xor(_) | DotDot(_) | Assign | If(_) | Else(_) | Dot
        | Iter(_) | Dot => tok.info().bright_red().to_string(),
        Not(_) | Ask(_) | Dollar | ExclamationMark | Return(_) | Del(_) => {
            tok.info().bright_cyan().to_string()
        }
        Pipe | LParen | RParen | LBrace | RBrace | LBracket | RBracket | Comma | Colon => {
            tok.info().to_string()
        }
        Set(txt)
        | List(txt)
        | Whitespace(txt)
        | Newline(txt)
        | LineComment(txt)
        | BlockComment(txt)
        | LineContinuation(txt)
        | TrailingBackslash(txt) => txt.to_string(),
        __Test_Eof => String::new(),
    }
}

pub struct ReplInputState {
    is_in_unclosed_string: bool,
    has_continuation: bool,
    indent_level: usize,
    is_partial: bool,
}
impl ReplInputState {
    /// determines if the user is passing a partial input they intend to grow with other lines.
    /// this includes opening and not closing a multiline string, a grouping expression, explicitly
    /// adding a line break, etc. etc.
    pub fn from_input(input: &str) -> Self {
        let mut indent_level = 0i32;
        let mut has_continuation = false;
        let mut is_in_unclosed_string = false;

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
                    is_in_unclosed_string = !s.ends_with("\"\"\"") || s.len() < 6;
                    has_continuation = false;
                }
                Ok(Tok::TripleSingleQuote(s)) => {
                    is_in_unclosed_string = !s.ends_with("'''") || s.len() < 6;
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

        let is_partial = indent_level > 0 || has_continuation || is_in_unclosed_string;

        if is_partial {
            indent_level = max(indent_level, 1);
        }
        if is_in_unclosed_string {
            indent_level = 0
        }
        Self {
            is_in_unclosed_string,
            has_continuation,
            is_partial,
            indent_level: max(indent_level, 0i32) as usize,
        }
    }
}
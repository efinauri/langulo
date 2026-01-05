use crate::errors::{LanguloError, LanguloResult};
use crate::lexer::Tok;
use crate::lexer::Tok::TrailingBackslash;
use crate::parser::parse;
use crate::runtime::{eval_python, init_python};
use crate::transpile::transpile;
use colored::{Colorize, CustomColor};
use logos::Logos;
use miette::{GraphicalReportHandler, GraphicalTheme};
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{CompletionType, Config, Context, Editor, Helper};
use std::borrow::Cow;
use std::cmp::max;
use std::process::exit;
use std::string::ToString;

const COMMAND_PREFIX: &'static str = "::";

struct ReplCommand {
    name: &'static str,
    aliases: &'static [&'static str],
    help_description: fn(&mut Repl) -> String,
    handler: fn(&mut Repl) -> (),
}

impl ReplCommand {
    /// the command prefix isn't taken into account
    fn matches(&self, input: &str) -> bool {
        self.name == input || self.aliases.contains(&input)
    }

    fn display_aliases(&self) -> String {
        if self.aliases.is_empty() {
            String::new()
        } else {
            format!(
                "({})",
                self.aliases
                    .iter()
                    .map(|a| format!("{}{}", COMMAND_PREFIX, a))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

const REPL_COMMANDS: &[ReplCommand] = &[
    ReplCommand {
        name: "help",
        aliases: &["h"],
        help_description: |_| "Show this help message".to_string(),
        handler: |repl| {
            println!("{}", "REPL Commands:".cyan().bold());
            for cmd in REPL_COMMANDS {
                println!(
                    "  {}{}  {}  {}",
                    COMMAND_PREFIX,
                    cmd.name.yellow(),
                    format!("{}", cmd.display_aliases()).dimmed(),
                    (cmd.help_description)(repl)
                );
            }
        },
    },
    ReplCommand {
        name: "clear",
        aliases: &["cls", "c"],
        help_description: |_| "Clear the screen".to_string(),
        handler: |_| {
            print!("\x1B[2J\x1B[1;1H");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        },
    },
    ReplCommand {
        name: "showpython",
        aliases: &["sp", "py"],
        help_description: |repl| {
            format!(
                "Toggle display of generated Python code (currently {})",
                if repl.show_python {
                    "enabled".green()
                } else {
                    "disabled".red()
                }
            )
        },
        handler: |repl| {
            repl.show_python = !repl.show_python;
            let status = if repl.show_python {
                "enabled".green()
            } else {
                "disabled".red()
            };
            println!("{} {}", "Transpiled code display:".cyan(), status);
        },
    },
    ReplCommand {
        name: "quit",
        aliases: &["q", "exit"],
        help_description: |_| "Exit the REPL".to_string(),
        handler: |_| exit(0),
    },
];

const COMMAND_INPUTS_SIZE: usize = {
    let mut tot = 0;
    let mut i = 0;
    while i < REPL_COMMANDS.len() {
        tot += 1;
        tot += REPL_COMMANDS[i].aliases.len();
        i += 1;
    }
    tot
};

const fn COMMAND_INPUTS() -> [&'static str; COMMAND_INPUTS_SIZE] {
    let mut result = [""; COMMAND_INPUTS_SIZE];
    let mut result_i = 0;
    let mut i = 0;
    while i < REPL_COMMANDS.len() {
        let cmd = &REPL_COMMANDS[i];
        result[result_i] = cmd.name;
        result_i += 1;
        let mut j = 0;
        while j < cmd.aliases.len() {
            result[result_i] = cmd.aliases[j];
            result_i += 1;
            j += 1;
        }
        i += 1;
    }
    result
}

fn find_command(input: &str) -> Option<&'static ReplCommand> {
    REPL_COMMANDS.iter().find(|cmd| cmd.matches(input))
}

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
            "Type expressions to evaluate. {} for commands.",
            format!("{}help", COMMAND_PREFIX).cyan()
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
        let config = Config::builder()
            .completion_type(CompletionType::List)
            .build();
        let h = LanguloReplHelper::default();
        let mut rl = Editor::with_config(config)?;
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
                    if let Some(cmd_txt) = input.strip_prefix(COMMAND_PREFIX) {
                        match find_command(cmd_txt) {
                            Some(cmd) => {
                                (cmd.handler)(self);
                            }
                            None => println!("{}", format!("Unknown command: {}", cmd_txt).red()),
                        }
                        continue;
                    }

                    let _ = rl.add_history_entry(&input);
                    if let Err(err) = self.eval_line(&input) {
                        let mut buf = String::new();
                        if self.report_handler.render_report(&mut buf, &err).is_ok() {
                            eprint!("{}", buf);
                        } else {
                            eprintln!("{}: {}", "Error".red().bold(), &err);
                        }
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
        rl: &mut Editor<LanguloReplHelper, DefaultHistory>,
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

    fn eval_line(&mut self, input: &str) -> LanguloResult<()> {
        let ast = parse(input)?;
        let statements = transpile(&ast, input)?;

        if self.show_python {
            for stmt in &statements {
                println!("\t{} {}", "→".dimmed(), stmt.yellow());
            }
        }

        let result = eval_python(&statements)?;
        println!(
            "\t{} {} {}",
            "=>".green(),
            result.display.green().bold(),
            format!(": {}", result.py_type).dimmed()
        );
        Ok(())
    }
}

#[derive(Default)]
struct LanguloReplHelper {
    pub is_in_unclosed_string: bool,
}

impl Hinter for LanguloReplHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        if !line.starts_with(COMMAND_PREFIX) {
            return None;
        }

        let partial = &line[..pos];

        COMMAND_INPUTS()
            .into_iter()
            .map(|name| format!("{}{}", COMMAND_PREFIX, name))
            .find(|cmd| cmd.starts_with(partial) && cmd != partial)
            .map(|cmd| cmd[pos..].to_string())
    }
}
impl Validator for LanguloReplHelper {}
impl Completer for LanguloReplHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        if !line.starts_with(COMMAND_PREFIX) {
            return Ok((0, vec![]));
        }

        let partial = &line[COMMAND_PREFIX.len()..pos];

        let candidates: Vec<Pair> = COMMAND_INPUTS()
            .iter()
            .find(|name| name.starts_with(partial))
            .map(|name| Pair {
                display: name.to_string(),
                replacement: format!("{}{}", COMMAND_PREFIX, name),
            })
            .into_iter()
            .collect();

        Ok((0, candidates))
    }
}
impl Helper for LanguloReplHelper {}

impl Highlighter for LanguloReplHelper {
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

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
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
                out.push_str(&d.custom_color(CYAN).to_string());
                chars.next();
                chars.next();

                in_string = true;
                delim = Some(d);
            } else if c == '"' || c == '\'' {
                flush_lexed(&mut out, &mut buf);

                out.push_str(&c.to_string().custom_color(CYAN).to_string());
                in_string = true;
                delim = Some(if c == '"' { "\"" } else { "'" });
            } else {
                buf.push(c);
            }
        } else {
            // ─── Inside string ───
            out.push_str(&c.to_string().custom_color(CYAN).to_string());

            if let Some(d) = delim {
                if d.len() == 1 && c.to_string() == d {
                    in_string = false;
                    delim = None;
                } else if d.len() == 3
                    && c == d.chars().last().unwrap()
                    && chars.peek() == Some(&d.chars().nth(1).unwrap())
                    && chars.clone().nth(1) == Some(d.chars().nth(0).unwrap())
                {
                    out.push_str(
                        &chars
                            .next()
                            .unwrap()
                            .to_string()
                            .custom_color(CYAN)
                            .to_string(),
                    );
                    out.push_str(
                        &chars
                            .next()
                            .unwrap()
                            .to_string()
                            .custom_color(CYAN)
                            .to_string(),
                    );
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
        Literal(_) => tok.info().custom_color(YELLOW).underline().to_string(),
        Num(_) | True(_) | False(_) | StringLitSingle(_) | StringLitDouble(_)
        | TripleSingleQuote(_) | Key(_) | Value(_) | Index(_) | QuestionMark | At
        | TripleDoubleQuote(_) => tok.info().custom_color(YELLOW).to_string(),
        Plus | Minus | Star | Slash | Percent | Caret | Eq(_) | Neq(_) | Lt | Gt | Leq(_)
        | Geq(_) | And(_) | Or(_) | Xor(_) | DotDot(_) | Assign | If(_) | Else(_) | Iter(_)
        => tok.info().custom_color(ORANGE).to_string(),
        Dot => {
            let mut str = tok.info().custom_color(ORANGE);
            str.bgcolor = Some(Color::BrightBlack);
            str.to_string()
        },
        Not(_) | Ask(_) | Dollar | ExclamationMark | Return(_) | Del(_) => tok.info().custom_color(RED).to_string(),

        Pipe | LParen | RParen | LBrace | RBrace | LBracket | RBracket | Comma | Colon => tok.info().bold().to_string(),
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
            is_partial,
            indent_level: max(indent_level, 0i32) as usize,
        }
    }
}

#[allow(dead_code)]
const YELLOW: CustomColor = CustomColor {
    r: 230,
    g: 218,
    b: 41,
};
#[allow(dead_code)]
const CYAN: CustomColor = CustomColor {
    r: 45,
    g: 147,
    b: 221,
};
#[allow(dead_code)]
const GREEN: CustomColor = CustomColor {
    r: 40,
    g: 198,
    b: 65,
};
#[allow(dead_code)]
const RED: CustomColor = CustomColor {
    r: 211,
    g: 39,
    b: 52,
};
#[allow(dead_code)]
const PURPLE: CustomColor = CustomColor {
    r: 123,
    g: 83,
    b: 173,
};
#[allow(dead_code)]
const ORANGE: CustomColor = CustomColor {
    r: 218,
    g: 125,
    b: 34,
};
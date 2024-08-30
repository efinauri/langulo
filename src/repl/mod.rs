use crate::parser::Parser;
use codespan_reporting::files::SimpleFile;
use colored::Colorize;
use rustyline::DefaultEditor;
use std::io;
use std::io::Write;
use std::string::String;

pub fn serve_repl() {
    let mut stdout = io::stdout();
    let mut input_reader = DefaultEditor::new().unwrap();
    let mut source = String::new();

    loop {
        println!();
        stdout.flush().unwrap();
        let input = match input_reader.readline(">> ") {
            Ok(inp) => inp,
            Err(_) => {
                eprintln!("Could not read input.");
                continue;
            }
        };

        input_reader.add_history_entry(input.as_str()).unwrap();
        match input.trim() {
            "exit" => break,
            "help" => {
                println!(
                    r#"
    {} - terminates the REPL session
    {} - shows this message
"#,
                    "exit".underline(),
                    "help".underline()
                );
                continue;
            }
            _ => {}
        }

        source.push_str(&input);
        source.push('\n');
        let sf = SimpleFile::new("repl.rs", &source);

        let mut parser = Parser::new(&*input);
        if let Err(err) = parser.parse() {
            err.emit(&sf);
        };

        println!("{:#?}", parser.to_ast())
    }
}

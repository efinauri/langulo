use codespan_reporting::files::SimpleFile;
use colored::Colorize;
use rustyline::DefaultEditor;
use std::io;
use std::io::{Cursor, Write};
use std::string::String;
use crate::emitter::Emitter;
use crate::vm::VM;

macro_rules! ok_or_printerr {
    ($sf:expr, $action:expr) => {
        match $action {
            Ok(v) => v,
            Err(e) => {
                e.emit($sf);
                continue;
            }
        }
    };
}

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

        // todo extend with incremental compilation. for now, it recompiles everything each input
        let mut emitter = ok_or_printerr!(&sf, Emitter::new(source.as_str()));
        let mut buf = vec![];
        ok_or_printerr!(&sf, emitter.emit());
        emitter.write_to_stream(&mut buf).expect("could not write to stream");
        let mut cursor = Cursor::new(buf);
        let mut vm = VM::from_compiled_stream(&mut cursor).expect("could not create VM");
        ok_or_printerr!(&sf, vm.run());
        let result = vm.finalize();
        println!("{}", result)
    }
}

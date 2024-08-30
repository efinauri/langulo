use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use logos::Span;

#[derive(Debug)]
pub struct LanguloErr {
    diagnostic: Diagnostic<()>,
}

impl LanguloErr {
    pub(crate) fn vm(p0: &str) -> LanguloErr {
        Self {
            diagnostic: Diagnostic::error()
                .with_message(format!("VMError - {p0}"))
                .with_labels(vec![]),
        }
    }
}

impl LanguloErr {
    pub fn emit(&self, file: &SimpleFile<&str, &String>) {
        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();
        term::emit(&mut writer.lock(), &config, file, &self.diagnostic)
            .expect("failed to write diagnostic");
    }
    pub fn lexical(msg: &str, span: &Span) -> Self {
        Self {
            diagnostic: Diagnostic::error()
                .with_message(format!("LexicalError - {msg}"))
                .with_labels(vec![Label::primary((), span.start..span.end)]),
        }
    }

    pub fn _runtime(msg: &str, span: &Span) -> Self {
        Self {
            diagnostic: Diagnostic::error()
                .with_message(format!("RuntimeError - {msg}"))
                .with_labels(vec![Label::primary((), span.start..span.end)]),
        }
    }

    pub fn semantic(msg: &str /*span: &Span*/) -> Self {
        Self {
            diagnostic: Diagnostic::error()
                .with_message(format!("SemanticError - {msg}"))
                .with_labels(vec![Label::primary((), 0..0)]),
        }
    }

    pub fn typecheck(msg: String) -> Self {
        Self {
            diagnostic: Diagnostic::error()
                .with_message(format!("TypeError - {msg}"))
                .with_labels(vec![Label::primary((), 0..0)]),
        }
    }
}

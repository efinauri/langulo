use logos::Logos;
use crate::errors::err::LanguloErr;
use crate::lexer::tok::Tok;

pub mod tok;

/// wrapper for logos' lexer that supports peek
pub struct Lexer<'a> {
    logos: logos::Lexer<'a, Tok>,
    buffer: Option<(Tok, &'a str)>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            logos: Tok::lexer(input),
            buffer: None
        }
    }

    fn inner_next(&mut self) -> Result<Option<(Tok, &'a str)>, LanguloErr> {
        let maybe_tok = self.logos.next()
            .transpose()
            .map_err(|_| LanguloErr::lexical("Invalid Token", &self.logos.span()))?;

        match maybe_tok {
            None => Ok(None),
            Some(tok) => Ok(Some((tok, self.logos.slice())))
        }
    }

    pub fn next(&mut self) -> Result<Option<(Tok, &'a str)>, LanguloErr> {
        if let Some(buf) = self.buffer.take() {
            self.buffer = None;
            return Ok(Some(buf));
        }
        self.inner_next()
    }

    pub fn peek(&mut self) -> Result<&Option<(Tok, &'a str)>, LanguloErr> {
        if self.buffer.is_none() {
            self.buffer = self.inner_next()?
        }
        Ok(&self.buffer)
    }
}
use crate::lexer::tok::Tok;

impl Tok {
    pub fn precedence(&self) -> u8 {
        match self {
            Tok::Plus | Tok::Minus => 10,
            Tok::Star | Tok::Slash => 20,
            _ => unimplemented!("must assign precedence to tok {:?}", self),
        }
    }
}
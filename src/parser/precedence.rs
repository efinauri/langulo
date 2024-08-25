use crate::lexer::tok::Tok;

impl Tok {
    pub fn precedence(&self) -> u8 {
        match self {
            Tok::Int
            | Tok::Semicolon => 0,
            Tok::Assign => 10,
            Tok::And
            | Tok::Or
            | Tok::Xor
            | Tok::Not => 20,
            Tok::Equals
            | Tok::NotEquals => 30,
            Tok::GreaterThan
            | Tok::LessThan => 40,
            Tok::Range => 50,
            Tok::Plus
            | Tok::Minus => 60,
            Tok::Star
            | Tok::Slash => 70,
            Tok::Caret => 80,
            Tok::Dollar => 90,
            Tok::As => 100,
            Tok::Bang
            | Tok::Question
            | Tok::If
            | Tok::Else => 110,
            Tok::Iter => 120,
            Tok::At => 130,
            _ => unimplemented!("must explicitly set a precedence for operator {:?} ", self)
        }
    }
}
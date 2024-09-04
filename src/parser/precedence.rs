use crate::lexer::tok::Tok;

impl Tok {
    pub fn precedence(&self) -> u8 {
        // a > b -> a must finish evaluating before b
        match self {
            // tokens to not consider as operators
            Tok::Semicolon
            | Tok::RBrace
            | Tok::RParen
            | Tok::Colon
            | Tok::Comma
            // start of expressions
            | Tok::Int
            | Tok::Bool
            | Tok::Str
            | Tok::Char
            | Tok::Float
            | Tok::Pipe
            | Tok::LParen
            | Tok::LBrace
            | Tok::RBracket => 0,
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
            Tok::Else => 109,
            Tok::Bang
            | Tok::Question
            | Tok::If => 110,
            Tok::Iter => 120,
            Tok::LBracket => 130, // indexing
            Tok::At => 130,
            _ => unimplemented!("must explicitly set a precedence for operator {:?} ", self)
        }
    }
}

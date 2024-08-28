use std::fmt::{Debug, Formatter};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

#[derive(Debug, Clone, PartialEq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Return,
    Constant,
    Add,
    Sub,
    Mul,
    Div,
    Print,
    Negate,
}




pub type Value = i32;

#[derive(PartialEq)]
pub struct Instruction(u64);
impl Debug for Instruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,
               "raw: {:064b}\n\
               {:32} {:8}\n\
                {:032b} {:08b}",
               self.0,
               "value", "opcode",
               self.value(),
               self.opcode().to_u8().unwrap()
        )
    }
}

impl Instruction {
    pub fn new(opcode: OpCode, value: Value) -> Self {
        Self(opcode as u64 | (value << 8) as u64)
    }

    pub fn value(&self) -> Value {
        (self.0 >> 8 & 0xffff) as Value
    }

    pub fn opcode(&self) -> OpCode {
        OpCode::from_u8((self.0 & 0xff) as u8).unwrap()
    }
}

#[cfg(test)]
pub mod tests {
    use crate::vm::instruction::{Instruction, OpCode};

    #[test]
    fn test_instruction_debug() {
        let instruction = Instruction::new(OpCode::Return, 9);
        println!("{:?}", instruction);

        assert_eq!(instruction.value(), 9);
        assert_eq!(instruction.opcode(), OpCode::Return);
    }
}
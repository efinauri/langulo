use std::collections::VecDeque;
use crate::errors::err::LanguloErr;
use crate::vm::instruction::{Instruction, OpCode, Value};

pub mod instruction;

macro_rules! run_binary {
    ($vm:expr, $op:expr) => {{
        let b = $vm.pop_value();
        let a = $vm.pop_value();
        $vm.stack.push_back($op(a, b));
    }};
}



pub struct VM {
    instructions: Vec<Instruction>,
    stack: VecDeque<Value>,
    ip: usize,
}

impl VM {
    pub fn new(instructions: Vec<Instruction>) -> Self {
        VM {
            instructions,
            stack: VecDeque::new(),
            ip: 0,
        }
    }

    pub fn pop_value(&mut self) -> Value {
        self.stack.pop_back().expect("stack underflow")
    }

    pub fn run(&mut self) -> Result<(), LanguloErr> {
        loop {
            let current = &self.instructions[self.ip];
            self.ip += 1;
            match current.opcode() {
                OpCode::Return => break,
                OpCode::Negate => {
                    let v = self.pop_value();
                    self.stack.push_back(-v);
                }
                OpCode::Constant => self.stack.push_back(current.value()),
                OpCode::Add => run_binary!(self, |a, b| a + b),
                OpCode::Sub => run_binary!(self, |a, b| a - b),
                OpCode::Mul => run_binary!(self, |a, b| a * b),
                OpCode::Div => unimplemented!(),
                OpCode::Print => println!("{}", self.stack.back().unwrap()),
            }
        }
        Ok(())
    }

    pub fn finalize(mut self) -> Value { self.pop_value() }
}

#[cfg(test)]
mod tests {
    use crate::vm::instruction::Instruction;
    use crate::vm::VM;
    use crate::vm::instruction::OpCode::*;

    #[test]
    fn negate() {
        let mut vm = VM::new(vec![
            Instruction::new(Constant, 5),
            Instruction::new(Negate, 0),
            Instruction::new(Return, 0)]);
        vm.run().unwrap();
        assert_eq!(vm.stack.pop_back().unwrap(), -5);
    }

    #[test]
    fn add() {
        let mut vm = VM::new(vec![
            Instruction::new(Constant, 3),
            Instruction::new(Constant, 2),
            Instruction::new(Add, 0),
            Instruction::new(Return, 0)]);
        vm.run().unwrap();
        assert_eq!(vm.stack.pop_back().unwrap(), 5);
    }
}

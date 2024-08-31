use crate::errors::err::LanguloErr;
use crate::vm::garbage_collector::GarbageCollector;
use crate::word::structure::{OpCode, Word};
use std::collections::VecDeque;
use std::io;
use std::io::Read;

pub mod garbage_collector;

macro_rules! branch_binary {
    ($opcode:ident, $method:ident) => {
        paste! {
            OpCode::$opcode => run_binary!(self, $method),
            OpCode::[<$opcode This>] => self.stack.back_mut().unwrap().$method(current),
        }
    };
}

macro_rules! run_binary {
    ($vm:expr, $op:ident) => {{
        debug_assert!($vm.stack.len() >= 1);
        let lhs = $vm.pop_value();
        $vm.stack.back_mut().unwrap().$op(&lhs)?;
    }}
}

pub struct VM {
    bytecode: Vec<Word>,
    stack: VecDeque<Word>,
    gc: GarbageCollector,
    ip: usize,
}

impl VM {
    pub fn new(bytecode: Vec<Word>) -> Self {
        VM {
            bytecode,
            stack: VecDeque::new(),
            gc: GarbageCollector::new(),
            ip: 0,
        }
    }

    pub fn from_compiled_stream<R: Read>(mut reader: R) -> io::Result<Self> {
        let mut bytecode = Vec::new();
        let mut heap_floats = Vec::new();
        let mut heap_tables = Vec::new();
        let mut heap_strings = Vec::new();
        loop {
            let mut section_id = [0u8; 1];
            if reader.read_exact(&mut section_id).is_err() {
                break;
            }

            match section_id[0] {
                0x01 => {
                    let mut word_buf = [0u8; 8];
                    while reader.read_exact(&mut word_buf).is_ok() {
                        let word = u64::from_le_bytes(word_buf);
                        bytecode.push(Word::from_u64(word));
                    }
                }
                0x02 => {
                    let mut float_buf = [0u8; 8];
                    while reader.read_exact(&mut float_buf).is_ok() {
                        let float = f64::from_le_bytes(float_buf);
                        heap_floats.push(float);
                    }
                }
                0x03 => {
                    let mut num_tables_buf = [0u8; 4];
                    reader.read_exact(&mut num_tables_buf)?;
                    let num_tables = u32::from_le_bytes(num_tables_buf);

                    for _ in 0..num_tables {
                        let mut table_len_buf = [0u8; 4];
                        reader.read_exact(&mut table_len_buf)?;
                        let table_len = u32::from_le_bytes(table_len_buf) as usize;

                        let mut table = vec![0u8; table_len];
                        reader.read_exact(&mut table)?;
                        heap_tables.push(table);
                    }
                }
                0x04 => {
                    let mut num_strings_buf = [0u8; 4];
                    reader.read_exact(&mut num_strings_buf)?;
                    let num_strings = u32::from_le_bytes(num_strings_buf);

                    for _ in 0..num_strings {
                        let mut str_len_buf = [0u8; 4];
                        reader.read_exact(&mut str_len_buf)?;
                        let str_len = u32::from_le_bytes(str_len_buf) as usize;

                        let mut str_buf = vec![0u8; str_len];
                        reader.read_exact(&mut str_buf)?;

                        let string = String::from_utf8(str_buf).expect("Invalid UTF-8 data");
                        heap_strings.push(string);
                    }
                }
                _ => panic!("malformed compiled stream")
            }
        }
        println!("heap floats: {:?}", heap_floats);
        println!("heap tables: {:?}", heap_tables);
        println!("heap strings: {:?}", heap_strings);
        Ok(Self {
            bytecode,
            stack: VecDeque::new(),
            gc: GarbageCollector::new(),
            ip: 0,
        })
    }

    pub fn pop_value(&mut self) -> Word {
        let back = self.stack.pop_back().expect("stack underflow");
        debug_assert_eq!(back.opcode(), OpCode::Value);
        back
    }

    pub fn run(&mut self) -> Result<(), LanguloErr> {
        loop {
            let current = &self.bytecode[self.ip];
            #[feature(debug)] {
                println!("running bytecode [{}]: {:?}", self.ip, current);
            }
            self.ip += 1;
            debug_assert!(self.ip <= self.bytecode.len());
            match current.opcode() {
                OpCode::Stop => break,
                OpCode::Value => self.stack.push_back(*current),
                OpCode::PrintThis => println!("(TEMPORARY PRINT) {:?}", self.stack.back().unwrap()), // todo impl Display and show that

                OpCode::Add => run_binary!(self, add_inplace),
                OpCode::AddThis => self.stack.back_mut().unwrap().add_inplace(current)?,
                OpCode::Subtract => run_binary!(self, subtract_inplace),
                OpCode::SubtractThis => self.stack.back_mut().unwrap().subtract_inplace(current)?,
                OpCode::Multiply => run_binary!(self, multiply_inplace),
                OpCode::MultiplyThis => self.stack.back_mut().unwrap().multiply_inplace(current)?,
                OpCode::Divide => run_binary!(self, divide_inplace),
                OpCode::DivideThis => self.stack.back_mut().unwrap().divide_inplace(current)?,
                OpCode::Modulo => run_binary!(self, modulo_inplace),
                OpCode::ModuloThis => self.stack.back_mut().unwrap().modulo_inplace(current)?,

                OpCode::LogicalAnd => run_binary!(self, logical_and_inplace),
                OpCode::LogicalAndThis => self.stack.back_mut().unwrap().logical_and_inplace(current)?,
                OpCode::LogicalOr => run_binary!(self, logical_or_inplace),
                OpCode::LogicalOrThis => self.stack.back_mut().unwrap().logical_or_inplace(current)?,
                OpCode::LogicalXor => run_binary!(self, logical_xor_inplace),
                OpCode::LogicalXorThis => self.stack.back_mut().unwrap().logical_xor_inplace(current)?,

                OpCode::Equals => run_binary!(self, equals_inplace),
                OpCode::EqualsThis => self.stack.back_mut().unwrap().equals_inplace(current)?,
                OpCode::NotEquals => run_binary!(self, not_equals_inplace),
                OpCode::NotEqualsThis => self.stack.back_mut().unwrap().not_equals_inplace(current)?,

                OpCode::GreaterThan => run_binary!(self, greater_than_inplace),
                OpCode::GreaterThanThis => self.stack.back_mut().unwrap().greater_than_inplace(current)?,
                OpCode::LessThan => run_binary!(self, less_than_inplace),
                OpCode::LessThanThis => self.stack.back_mut().unwrap().less_than_inplace(current)?,
                OpCode::GreaterThanEq => run_binary!(self, greater_than_eq_inplace),
                OpCode::GreaterThanEqThis => self.stack.back_mut().unwrap().greater_than_eq_inplace(current)?,
                OpCode::LessThanEq => run_binary!(self, less_than_eq_inplace),
                OpCode::LessThanEqThis => self.stack.back_mut().unwrap().less_than_eq_inplace(current)?,

                OpCode::Power => {
                    debug_assert!(self.stack.len() >= 1);
                    let lhs = self.pop_value();
                    self.stack.back_mut().unwrap().exponentiate_inplace(&lhs, &mut self.gc)?;
                },
                OpCode::PowerThis => self.stack.back_mut().unwrap().exponentiate_inplace(current, &mut self.gc)?,
                OpCode::NegateThis => self.stack.push_back(Word::bool(!current.to_bool(), OpCode::Value)),
                // OpCode::SetLocal
                // OpCode::GetLocal

                _ => unimplemented!("opcode not implemented: {:?}", current.opcode()),
            }
        }
        Ok(())
    }

    pub fn finalize(mut self) -> Word {
        debug_assert_eq!(self.stack.len(), 1);
        self.pop_value()
    }
}

#[cfg(test)]
mod tests {
    use crate::emitter::Emitter;
    use super::*;
    use crate::word::structure::Word;
    fn expect_float_vm_execution_approx(lhs: f64, rhs: f64, op: OpCode, expected_output: f64) {
        let mut gc = GarbageCollector::new();
        let bytecode = vec![
            Word::float(lhs, OpCode::Value, &mut gc),
            Word::float(rhs, op, &mut gc),
            Word::int(0, OpCode::Stop)
        ];
        let mut vm = VM::new(bytecode);
        vm.run().unwrap();
        let result = vm.finalize();
        println!("{:?}", result);
        let result_flt = result.to_float();
        assert!((result_flt - expected_output).abs() < 0.00001,
                "Result: {}, Expected: {}", result_flt, expected_output);

    }

    #[test]
    fn from_emitted_stream() {
        let mut emitter = Emitter::new(r#"
        2 + 3 * 4;
        "#).expect("could not emit");
        emitter.emit().unwrap();
        let mut buf = vec![];
        emitter.write_to_stream(&mut buf).expect("could not write to stream");
        let mut cursor = std::io::Cursor::new(buf);
        let mut vm = VM::from_compiled_stream(&mut cursor).expect("failed to spin vm up from stream");
        vm.run().expect("error while running");
        let result = vm.finalize();
        assert_eq!(result, Word::int(14, OpCode::Value));
    }

    #[test]
    fn float_arithmetic_tests() {
        expect_float_vm_execution_approx(3.0, 5.0, OpCode::AddThis, 8.0);
        expect_float_vm_execution_approx(3.0, 5.0, OpCode::SubtractThis, -2.0);
        expect_float_vm_execution_approx(3.0, 5.0, OpCode::MultiplyThis, 15.0);
        expect_float_vm_execution_approx(3.0, 5.0, OpCode::DivideThis, 0.6);
    }

    fn expect_vm_execution(mut bytecode: Vec<Word>, expected_output: Word) {
        bytecode.push(Word::int(0, OpCode::Stop));
        let mut vm = VM::new(bytecode);
        vm.run().unwrap();
        assert_eq!(vm.finalize(), expected_output);
    }

    #[test]
    fn vm_arithmetic() {
        expect_vm_execution(
            vec![Word::bool(true, OpCode::NegateThis)],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![Word::int(3, OpCode::Value), Word::int(5, OpCode::AddThis)],
            Word::int(8, OpCode::Value),
        );
        expect_vm_execution(
            vec![Word::int(3, OpCode::Value), Word::int(5, OpCode::SubtractThis)],
            Word::int(-2, OpCode::Value),
        );
        expect_vm_execution(
            vec![Word::int(3, OpCode::Value), Word::int(5, OpCode::MultiplyThis)],
            Word::int(15, OpCode::Value),
        );
    }

    #[test]
    fn vm_logic() {
        expect_vm_execution(
            vec![
                Word::bool(true, OpCode::Value),
                Word::bool(false, OpCode::LogicalAndThis),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::bool(true, OpCode::Value),
                Word::bool(false, OpCode::LogicalOrThis),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::bool(true, OpCode::Value),
                Word::bool(false, OpCode::LogicalXorThis),
            ],
            Word::bool(true, OpCode::Value),
        );
    }

    #[test]
    fn vm_comparisons() {
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::EqualsThis),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(3, OpCode::EqualsThis),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::NotEqualsThis),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::GreaterThanThis),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(5, OpCode::Value),
                Word::int(3, OpCode::GreaterThanThis),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::LessThanThis),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(5, OpCode::Value),
                Word::int(3, OpCode::LessThanThis),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::GreaterThanEqThis),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(5, OpCode::Value),
                Word::int(3, OpCode::GreaterThanEqThis),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(3, OpCode::GreaterThanEqThis),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::LessThanEqThis),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(5, OpCode::Value),
                Word::int(3, OpCode::LessThanEqThis),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(3, OpCode::LessThanEqThis),
            ],
            Word::bool(true, OpCode::Value),
        );
    }

    #[test]
    fn vm_power() {
        let mut gc = GarbageCollector::new();
        expect_vm_execution(
            vec![
                Word::int(2, OpCode::Value),
                Word::int(3, OpCode::PowerThis),
            ],
            Word::float(8.0, OpCode::Value, &mut gc),
        );
        expect_vm_execution(
            vec![
                Word::float(2.0, OpCode::Value, &mut gc),
                Word::float(3.0, OpCode::PowerThis, &mut gc),
            ],
            Word::float(8.0, OpCode::Value, &mut gc),
        );
        expect_vm_execution(
            vec![
                Word::int(2, OpCode::Value),
                Word::float(3.0, OpCode::PowerThis, &mut gc),
            ],
            Word::float(8.0, OpCode::Value, &mut gc),
        );
        expect_float_vm_execution_approx(8.0, 0.33333333, OpCode::PowerThis, 2.0);
        expect_float_vm_execution_approx(4.0, -2.0, OpCode::PowerThis, 0.0625)
    }

    #[test]
    fn vm_modulo() {
        expect_vm_execution(
            vec![
                Word::int(7, OpCode::Value),
                Word::int(3, OpCode::ModuloThis),
            ],
            Word::int(1, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(10, OpCode::Value),
                Word::int(3, OpCode::ModuloThis),
            ],
            Word::int(1, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(-7, OpCode::Value),
                Word::int(3, OpCode::ModuloThis),
            ],
            Word::int(-1, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(-10, OpCode::Value),
                Word::int(3, OpCode::ModuloThis),
            ],
            Word::int(-1, OpCode::Value),
        );

        expect_float_vm_execution_approx(7.2, 3.3, OpCode::ModuloThis, 0.6);
        expect_float_vm_execution_approx(7.2, -3.3, OpCode::ModuloThis, 0.6);
        expect_float_vm_execution_approx(7.2, 55.4, OpCode::ModuloThis, 7.2);
        expect_float_vm_execution_approx(7.2, -55.4, OpCode::ModuloThis, 7.2);
    }

    #[test]
    fn vm_add_not_embedded() {
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(2, OpCode::Value),
                Word::int(0, OpCode::Add),
            ],
            Word::int(5, OpCode::Value),
        )
    }
}

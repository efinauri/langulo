use crate::errors::err::LanguloErr;
use crate::vm::garbage_collector::GarbageCollector;
use crate::word::heap::Table;
use crate::word::structure::{OpCode, ValueTag, Word};
use std::collections::VecDeque;

pub mod garbage_collector;
mod input;

macro_rules! run_binary {
    ($vm:expr, $op:ident) => {{
        debug_assert!($vm.stack.len() >= 1);
        let lhs = $vm.pop_value();
        $vm.stack.back_mut().unwrap().$op(&lhs)?;
    }}
}

pub struct VM {
    bytecode: Vec<Word>,
    vars: VecDeque<Word>,
    stack: VecDeque<Word>,
    gc: GarbageCollector,
    ip: usize,
    heap_floats: Vec<f64>,
    // wrapped in a option so that the value can be taken without copying
    heap_strings: Vec<Option<String>>,
}

impl VM {
    pub fn pop_value(&mut self) -> Word {
        let back = self.stack.pop_back().expect("stack underflow");
        debug_assert_eq!(back.opcode(), OpCode::Value);
        back
    }

    pub fn run(&mut self) -> Result<(), LanguloErr> {
        loop {
            debug_assert!(self.ip < self.bytecode.len());
            let current = &mut self.bytecode[self.ip];
            #[feature(debug)] {
                println!("running bytecode [{}]:\n{:?}", self.ip, current);
            }
            self.ip += 1;
            match &current.opcode() {
                OpCode::Stop => break,
                OpCode::Value => self.stack.push_back(*current),
                OpCode::Print => println!("{}", self.stack.back().unwrap()),

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
                }
                OpCode::PowerThis => self.stack.back_mut().unwrap().exponentiate_inplace(current, &mut self.gc)?,
                OpCode::NegateThis => self.stack.push_back(Word::bool(!current.to_bool(), OpCode::Value)),

                OpCode::SetLocalThis => {
                    current.set_opcode(OpCode::Value);
                    self.vars.push_back(*current);
                    self.stack.push_back(*current);
                }
                OpCode::SetLocal => {
                    self.vars.push_back(*self.stack.back().unwrap());
                }
                OpCode::GetLocal => {
                    let local_idx = current.aux() as usize;
                    debug_assert!(local_idx < self.vars.len());
                    self.stack.push_back(self.vars[local_idx]);
                }

                OpCode::WrapInOption => {
                    let value = self.pop_value();
                    let value_option = if value.tag() == ValueTag::Special { None } else { Some(value) };
                    let option = Word::option(value_option, OpCode::Value, &mut self.gc);
                    self.stack.push_back(option);
                }
                OpCode::WrapInOptionThis => {
                    current.set_opcode(OpCode::Value);
                    let value_option = if current.tag() == ValueTag::Special { None } else { Some(*current)};
                    self.stack.push_back(Word::option(value_option, OpCode::Value, &mut self.gc));
                }
                OpCode::UnwrapOption => {
                    let value = self.pop_value()
                        .as_option()
                        .ok_or(LanguloErr::vm("unwrap option from non-option value"))?;
                    self.stack.push_back(value);
                }
                OpCode::UnwrapOptionThis => {
                    let current = current
                        .as_option()
                        .ok_or(LanguloErr::vm("unwrap option from non-option value"))?;
                    self.stack.push_back(current);
                }
                OpCode::IndexGet => {
                    let value = self.stack.pop_back().unwrap();
                    let key = self.pop_value();
                    let value = value.as_table();
                    let value  = value
                        .get(&key)
                        .or_else(||value.get(&Word::DEFAULTTABLEARM()))
                        .map(|v|*v);
                    self.stack.push_back(Word::option(value, OpCode::Value, &mut self.gc));
                }
                OpCode::IndexGetThis => {
                    current.set_opcode(OpCode::Value);
                    let value = self.stack.pop_back().unwrap();
                    let value = value.as_table();
                    let value = value
                        .get(&current)
                        .or_else(||value.get(&Word::DEFAULTTABLEARM()))
                        .map(|v|*v);
                    self.stack.push_back(Word::option(value, OpCode::Value, &mut self.gc));
                }

                OpCode::ReadFromMap => {
                    let map_idx = current.value() as usize;
                    debug_assert!(current.is_tag_for_heap());
                    match current.tag() {
                        ValueTag::FloatPtr => {
                            let float = *self.heap_floats.get(map_idx)
                                .expect("readfrommap pointing to invalid raw float");
                            let word = Word::float(float, OpCode::Value, &mut self.gc);
                            self.stack.push_back(word);
                        }
                        ValueTag::StrPtr => {
                            let string = self.heap_strings.get_mut(map_idx)
                                .expect("readfrommap pointing to invalid raw string");
                            let string = string.take().expect("string already taken");
                            let word = Word::str(&*string, OpCode::Value, &mut self.gc);
                            self.stack.push_back(word);
                        }
                        ValueTag::TablePtr => {
                            let num_of_pairs = current.value();
                            let mut tbl = Table::new();
                            for _ in 0..num_of_pairs {
                                let value = self.pop_value();
                                let key = self.pop_value();
                                debug_assert!(key.opcode() == OpCode::Value);
                                debug_assert!(value.opcode() == OpCode::Value);
                                tbl.insert(key, value);
                            }
                            let word = Word::table(tbl, OpCode::Value, &mut self.gc);
                            self.stack.push_back(word);
                        }
                        _ => return Err(LanguloErr::vm("reading from map a nonheap value")),
                    }
                }

                _ => unimplemented!("opcode not implemented: {:?}", current.opcode()),
            }
        }
        Ok(())
    }

    pub fn finalize(mut self) -> Word {
        // no guarantee that this is the last element in the stack. for example this is a valid program: 3; 2; 1;
        self.pop_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::structure::Word;
    fn expect_float_vm_execution_approx(lhs: f64, rhs: f64, op: OpCode, expected_output: f64) {
        let mut gc = GarbageCollector::new();
        let bytecode = vec![
            Word::float(lhs, OpCode::Value, &mut gc),
            Word::float(rhs, op, &mut gc),
            Word::int(0, OpCode::Stop)
        ];
        let mut vm = VM::from_bytecode_only(bytecode);
        vm.run().unwrap();
        let result = vm.finalize();
        println!("{:?}", result);
        let result_flt = result.to_float();
        assert!((result_flt - expected_output).abs() < 0.00001,
                "Result: {}, Expected: {}", result_flt, expected_output);
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
        let mut vm = VM::from_bytecode_only(bytecode);
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

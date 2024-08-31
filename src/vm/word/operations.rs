use crate::errors::err::LanguloErr;
use crate::vm::garbage_collector::GarbageCollector;
use crate::vm::word::heap::HeapFloat;
use crate::vm::word::word_shape::ValueTag::*;
use crate::vm::word::word_shape::{OpCode, Word};

macro_rules! impl_word_cmp {
    ($name:ident, $op:tt) => {
        pub fn $name(&mut self, rhs: &Word,) -> Result<(), LanguloErr> {
            debug_assert_eq!(self.tag(), rhs.tag());
            self.replace_with_stack_value(
                ((self as &Word) $op rhs) as u32,
                OpCode::Value,
                Bool,
            );
            Ok(())
        }
    };
}


/// arithmetic. todo: under under/overflow
impl Word {
    pub fn add_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            Int => self.update_stack_value((self.to_int() + rhs.to_int()) as u32, OpCode::Value),
            FloatPtr => self.update_heap_value::<HeapFloat>(self.to_float() + rhs.to_float(), OpCode::Value),
            _ => return Err(LanguloErr::vm("cannot add")),
        };
        Ok(())
    }

    pub fn subtract_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            Int => self.update_stack_value((self.to_int() - rhs.to_int()) as u32, OpCode::Value),
            FloatPtr => self.update_heap_value::<HeapFloat>(self.to_float() - rhs.to_float(), OpCode::Value),
            _ => return Err(LanguloErr::vm("cannot sub")),
        };
        Ok(())
    }

    pub fn multiply_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            Int => self.update_stack_value((self.to_int() * rhs.to_int()) as u32, OpCode::Value),
            FloatPtr => self.update_heap_value::<HeapFloat>(self.to_float() * rhs.to_float(), OpCode::Value),
            _ => return Err(LanguloErr::vm("cannot mul")),
        };
        Ok(())
    }

    pub fn divide_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            Int => {
                if rhs.to_int() == 0 {
                    return Err(LanguloErr::vm("division by zero"));
                }
                self.update_stack_value((self.to_int() / rhs.to_int()) as u32, OpCode::Value);
            }
            FloatPtr => {
                if rhs.to_float() == 0.0 {
                    return Err(LanguloErr::vm("division by zero"));
                }
                self.update_heap_value::<HeapFloat>(self.to_float() / rhs.to_float(), OpCode::Value)
            }
            _ => return Err(LanguloErr::vm("cannot div")),
        };
        Ok(())
    }

    pub fn exponentiate_inplace(&mut self, rhs: &Word, gc: &mut GarbageCollector) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        match (self.tag(), rhs.tag()) {
            (Int, Int) => {
                let float_ptr = Word::float(
                    (self.to_int() as f32).powf(rhs.to_int() as f32),
                    OpCode::Value,
                    gc
                );
                self.become_word(float_ptr);
            },
            (Int, FloatPtr) => {
                let float_ptr = Word::float(
                    (self.to_int() as f32).powf(rhs.to_float()),
                    OpCode::Value,
                    gc
                );
                self.become_word(float_ptr);
            }
            (FloatPtr, Int) => self.update_heap_value::<HeapFloat>(self.to_float().powf(rhs.to_int() as f32), OpCode::Value),
            (FloatPtr, FloatPtr) => self.update_heap_value::<HeapFloat>(self.to_float().powf(rhs.to_float()), OpCode::Value),
            _ => return Err(LanguloErr::vm("cannot exponentiate")),
        };
        Ok(())
    }

    pub fn modulo_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            Int => {
                if rhs.to_int() == 0 {
                    return Err(LanguloErr::vm("modulo by zero"));
                }
                self.update_stack_value((self.to_int() % rhs.to_int()) as u32, OpCode::Value);
            }
            FloatPtr => {
                if rhs.to_float() == 0.0 {
                    return Err(LanguloErr::vm("modulo by zero"));
                }
                self.update_heap_value::<HeapFloat>(self.to_float() % rhs.to_float(), OpCode::Value)
            }
            _ => return Err(LanguloErr::vm("cannot modulo")),
        };
        Ok(())
    }
}

///logical
impl Word {
    pub fn logical_and_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert_eq!(self.tag(), Bool);
        debug_assert_eq!(rhs.tag(), Bool);
        self.update_stack_value((self.to_bool() && rhs.to_bool()) as u32, OpCode::Value);
        Ok(())
    }

    pub fn logical_or_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert_eq!(self.tag(), Bool);
        debug_assert_eq!(rhs.tag(), Bool);
        self.update_stack_value((self.to_bool() || rhs.to_bool()) as u32, OpCode::Value);
        Ok(())
    }

    pub fn logical_xor_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert_eq!(self.tag(), Bool);
        debug_assert_eq!(rhs.tag(), Bool);
        self.update_stack_value((self.to_bool() ^ rhs.to_bool()) as u32, OpCode::Value);
        Ok(())
    }
}

///comparisons
impl Word {
    pub fn equals_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert_eq!(self.tag(), rhs.tag());
        self.replace_with_stack_value(
            (self == rhs) as u32,
            OpCode::Value,
            Bool,
        );
        Ok(())
    }

    pub fn not_equals_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert_eq!(self.tag(), rhs.tag());
        self.replace_with_stack_value(
            (self != rhs) as u32,
            OpCode::Value,
            Bool,
        );
        Ok(())
    }

    impl_word_cmp!(greater_than_inplace, >);
    impl_word_cmp!(greater_than_eq_inplace, >=);
    impl_word_cmp!(less_than_inplace, <);
    impl_word_cmp!(less_than_eq_inplace, <=);
}

#[cfg(test)]
mod tests {
    use crate::vm::garbage_collector::GarbageCollector;
    use crate::vm::word::word_shape::OpCode;
    use crate::vm::word::Word;

    #[test]
    fn test_add_inplace() {
        let mut w = Word::int(5, OpCode::Add);
        let rhs = Word::int(3, OpCode::Value);
        w.add_inplace(&rhs).unwrap();
        assert_eq!(w.to_int(), 8);

        let rhs2 = Word::int(-18, OpCode::Value);
        assert_eq!(rhs2.to_int(), -18);
        w.add_inplace(&rhs2).unwrap();
        println!("{:?}", w);
        assert_eq!(w.to_int(), -10);
    }

    #[test]
    fn stack_eq_ne() {
        let mut w = Word::int(5, OpCode::Value);
        let w2 = Word::int(5, OpCode::Value);
        w.equals_inplace(&w2).unwrap();
        assert!(w.to_bool());

        let mut w = Word::int(5, OpCode::Value);
        let w3 = Word::int(6, OpCode::Value);
        w.equals_inplace(&w3).unwrap();
        assert!(!w.to_bool());
    }

    #[test]
    fn heap_eq_ne() {
        let mut gc = GarbageCollector::new();
        let mut w = Word::float(5.3, OpCode::Value, &mut gc);
        let w2 = Word::float(5.3, OpCode::Value, &mut gc);
        w.equals_inplace(&w2).unwrap();
        assert!(w.to_bool());

        let mut w = Word::float(5.3, OpCode::Value, &mut gc);
        let w3 = Word::float(5.3000001, OpCode::Value, &mut gc);
        w.equals_inplace(&w3).unwrap();
        assert!(!w.to_bool());
    }
}

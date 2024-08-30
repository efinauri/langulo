use crate::errors::err::LanguloErr;
use crate::vm::word::heap::HeapFloat;
use crate::vm::word::word_shape::ValueTag::*;
use crate::vm::word::word_shape::{OpCode, Word};

/// arithmetic. todo: under under/overflow
impl Word {
    pub fn add_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            Int => self.set_stack_value((self.to_int() + rhs.to_int()) as u32, OpCode::Value),
            FloatPtr => self.set_heap_value::<HeapFloat>(self.to_float() + rhs.to_float(), OpCode::Value),
            _ => return Err(LanguloErr::vm("cannot add")),
        };
        Ok(())
    }

    pub fn subtract_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            Int => self.set_stack_value((self.to_int() - rhs.to_int()) as u32, OpCode::Value),
            FloatPtr => self.set_heap_value::<HeapFloat>(self.to_float() - rhs.to_float(), OpCode::Value),
            _ => return Err(LanguloErr::vm("cannot sub")),
        };
        Ok(())
    }

    pub fn multiply_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert!([Int, FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            Int => self.set_stack_value((self.to_int() * rhs.to_int()) as u32, OpCode::Value),
            FloatPtr => self.set_heap_value::<HeapFloat>(self.to_float() * rhs.to_float(), OpCode::Value),
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
                self.set_stack_value((self.to_int() / rhs.to_int()) as u32, OpCode::Value);
            }
            FloatPtr => {
                if rhs.to_float() == 0.0 {
                    return Err(LanguloErr::vm("division by zero"));
                }
                self.set_heap_value::<HeapFloat>(self.to_float() / rhs.to_float(), OpCode::Value)
            }
            _ => return Err(LanguloErr::vm("cannot div")),
        };
        Ok(())
    }
}

///logical
impl Word {
    pub fn and_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert_eq!(self.tag(), Bool);
        debug_assert_eq!(rhs.tag(), Bool);
        self.set_stack_value((self.to_bool() && rhs.to_bool()) as u32, OpCode::Value);
        Ok(())
    }

    pub fn or_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert_eq!(self.tag(), Bool);
        debug_assert_eq!(rhs.tag(), Bool);
        self.set_stack_value((self.to_bool() || rhs.to_bool()) as u32, OpCode::Value);
        Ok(())
    }

    pub fn xor_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        debug_assert_eq!(self.tag(), Bool);
        debug_assert_eq!(rhs.tag(), Bool);
        self.set_stack_value((self.to_bool() ^ rhs.to_bool()) as u32, OpCode::Value);
        Ok(())
    }
}

///comparisons
impl Word {
    pub fn equals_inplace(&mut self, rhs: &Word) -> Result<(), LanguloErr> {
        self.set_stack_value((self.value() == rhs.value()) as u32, OpCode::Value);
        Ok(())
    }
}


#[cfg(test)]
mod tests {
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


}

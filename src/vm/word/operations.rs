use crate::errors::err::LanguloErr;
use crate::vm::garbage_collector::GarbageCollector;
use crate::vm::word::word_shape::{ValueTag, Word};

impl Word {
    pub fn add_inplace(&mut self, rhs: &Word, gc: &mut GarbageCollector) -> Result<(), LanguloErr> {
        debug_assert!([ValueTag::Int, ValueTag::FloatPtr].contains(&rhs.tag()));
        debug_assert_eq!(self.tag(), rhs.tag());
        match self.tag() {
            ValueTag::Int => self.set_value((self.to_int() + rhs.to_int()) as u64),
            ValueTag::FloatPtr => self.set_value((self.to_float() + rhs.to_float()) as u64),
            _ => return Err(LanguloErr::vm("cannot add")),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::word::word_shape::OpCode;
    use crate::vm::word::Word;

    #[test]
    fn test_add_inplace() {
        let mut gc = GarbageCollector::new();
        let mut w = Word::int(5, OpCode::Add);
        let rhs = Word::int(3, OpCode::Constant);

        assert_eq!(w.to_int(), 5);
        assert_eq!(rhs.to_int(), 3);

        w.add_inplace(&rhs, &mut gc).unwrap();

        assert_eq!(w.to_int(), 8);
    }
}

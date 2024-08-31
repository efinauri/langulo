use crate::vm::word::heap::HeapValue;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use std::fmt::{Debug, Formatter};

const fn bitmask(from: u32, to_excluded: u32) -> u64 {
    let to = to_excluded - 1;
    if from > 63 || to > 63 || from > to {
        0
    } else {
        ((1u64 << (to - from + 1)) - 1) << from
    }
}

#[derive(Copy, Clone, Eq, Ord)]
/// stores VM values in an usize that can either be the direct representation of the value,
/// or a pointer to its location in the heap.
pub struct Word(pub(crate) *mut u8);

// the first 3 bits of the word represent the type of the value (if any)

#[derive(Debug, FromPrimitive, ToPrimitive, PartialEq, PartialOrd, Copy, Clone)]
#[repr(u8)]
pub enum ValueTag {
    Int,
    Bool,
    Char,

    FnPtr,
    FloatPtr,
    StrPtr,
    TablePtr,
}

/// then we have the two option flags: is option (A) and is none (B)
/// the behavior of the flag combination (AB) on a tag T is the following:
/// - 00 -> T
/// - 10 -> None
/// - 11 -> Some<T>
/// - 01 -> Some<next word> // todo lot of wasted space this way, think of a way to make this word more informative, maybe it could be the full type layour of the next word

#[derive(Debug, Clone, PartialEq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Value,
    Stop,
    Return,
    Jump,
    JumpIfFalse,
    Call,
    CallBuiltin,
    SetLocal,
    GetLocal,
    SetGlobal,
    GetGlobal,
    IndexGet,
    IndexSet,
    WrapInOption,
    UnwrapOption,

    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,

    Print,

    Negate,
    LogicalAnd,
    LogicalOr,
    LogicalXor,
    GreaterThan,
    LessThan,
    Equals,
    NotEquals,
    GreaterThanEq,
    LessThanEq,

    Cast,
}

/// for now bits 11..32 are chaff

/// the rest is either the stack value or a heap pointer to it

pub const TAG_START: u64 = 0;
pub const IS_OPTION_FLAG_START: u64 = 3;
pub const IS_NONE_OPTION_FLAG_START: u64 = 4;
pub const OPCODE_START: u64 = 5;
pub const CHAFF_START: u64 = 11;
pub const PTR_START: u64 = 32;

pub const TAG_MASK: u64 = bitmask(TAG_START as u32, IS_OPTION_FLAG_START as u32);
pub const IS_OPTION_FLAG_MASK: u64 =
    bitmask(IS_OPTION_FLAG_START as u32, IS_NONE_OPTION_FLAG_START as u32);
pub const IS_NONE_OPTION_FLAG_MASK: u64 = bitmask(IS_NONE_OPTION_FLAG_START as u32, OPCODE_START as u32);
pub const OPCODE_MASK: u64 = bitmask(OPCODE_START as u32, CHAFF_START as u32);
pub const CHAFF_MASK: u64 = bitmask(CHAFF_START as u32, PTR_START as u32);
pub const PTR_MASK: u64 = bitmask(PTR_START as u32, 64);

impl Debug for Word {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "raw: {:064b}\n\
        {:32} {:21} {:6} {:1} {:1} {:3}\n\
        {:032b} {:021b} {:06b} {:01b} {:01b} {:03b}\n\
        {:?} {:?} {}\n",
            self.0 as usize,
            "ptr",
            "chaff",
            "opcode",
            "B",
            "A",
            "tag",
            self.ptr() as u32,
            self.chaff(),
            self.opcode().to_u8().unwrap(),
            self.is_option() as u8,
            self.is_none() as u8,
            self.tag().to_u8().unwrap(),
            self.opcode(),
            self.tag(),
            self.value()
        )
    }
}

impl Word {
    pub fn ptr(self) -> *mut u8 {
        ((self.0 as u64 & PTR_MASK) >> PTR_START) as _
    }
    pub fn chaff(&self) -> u32 {
        ((self.0 as u64 & CHAFF_MASK) >> CHAFF_START) as _
    }
    pub fn is_option(&self) -> bool {
        ((self.0 as u64 & IS_OPTION_FLAG_MASK) >> IS_OPTION_FLAG_START) == 1
    }
    pub fn is_none(&self) -> bool {
        ((self.0 as u64 & IS_NONE_OPTION_FLAG_MASK) >> IS_NONE_OPTION_FLAG_START) == 1
    }
    pub fn opcode(&self) -> OpCode {
        OpCode::from_u64((self.0 as u64 & OPCODE_MASK) >> OPCODE_START).unwrap()
    }
    pub fn tag(&self) -> ValueTag {
        ValueTag::from_u64(self.0 as u64 & TAG_MASK).unwrap()
    }

    pub fn new(
        ptr: *mut u8,
        is_option: bool,
        is_none: bool,
        op_code: OpCode,
        tag: ValueTag,
    ) -> Self {
        Self(
            ((((ptr as u64) << PTR_START) & PTR_MASK)
                | ((is_option as u64) << IS_OPTION_FLAG_START)
                | ((is_none as u64) << IS_NONE_OPTION_FLAG_START)
                | (((op_code as u64) << OPCODE_START) & OPCODE_MASK)
                | (tag as u64 & TAG_MASK)) as *mut u8,
        )
    }

    pub fn in_heap(&self) -> bool {
        self.tag() > ValueTag::Char
    }

    pub fn get<'a, T>(self) -> &'a T {
        unsafe { &*(self.ptr() as *const T) }
    }

    pub fn get_mut<'a, T>(self) -> &'a mut T {
        unsafe { &mut *(self.ptr() as *mut T) }
    }

    pub fn value(&self) -> u32 {
        ((self.0 as u64 & PTR_MASK) >> PTR_START) as u32
    }

    pub fn update_stack_value(&mut self, value: u32, opcode: OpCode) {
        debug_assert!([ValueTag::Int, ValueTag::Bool, ValueTag::Char].contains(&self.tag()));
        self.0 = (
            ((self.0 as u64 & !PTR_MASK) & !OPCODE_MASK)
                | ((value as u64) << PTR_START)
                | (((opcode as u64) << OPCODE_START) & OPCODE_MASK)
        ) as _;
        debug_assert_eq!(self.value(), value);
    }

    pub fn replace_with_stack_value(&mut self, value: u32, opcode: OpCode, tag: ValueTag) {
        // todo make sure that the replaced value was a heap ptr, the corresponding value is swept
        self.0 = (
            (((self.0 as u64 & !PTR_MASK) & !OPCODE_MASK) & !TAG_MASK)
                | ((value as u64) << PTR_START)
                | (((opcode as u64) << OPCODE_START) & OPCODE_MASK)
                | (((tag as u64) << TAG_START) & TAG_MASK)
        ) as _;
        debug_assert_eq!(self.value(), value);
        debug_assert_eq!(self.tag(), tag);
    }

    pub fn update_heap_value<H>(&mut self, value: H::Inner, opcode: OpCode)
    where
        H: HeapValue,
    {
        let mut current = H::get_inner_mut(self);
        *current = value;
        self.0 = (
            (self.0 as u64 & !OPCODE_MASK)
                | (((opcode as u64) << OPCODE_START) & OPCODE_MASK)
        ) as _;
    }

    pub fn change_tag(&mut self, new_tag: ValueTag, is_option: bool, is_none: bool) {
        self.0 = (
            ((self.0 as u64 & !TAG_MASK) | ((new_tag as u64) << TAG_START))
                | ((self.0 as u64 & !IS_OPTION_FLAG_MASK) | ((is_option as u64) << IS_OPTION_FLAG_START))
                | ((self.0 as u64 & !IS_NONE_OPTION_FLAG_MASK) | ((is_none as u64) << IS_NONE_OPTION_FLAG_START))
        ) as _
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmask_calculator() {
        assert_eq!(bitmask(0, 3), 0b_111);
        assert_eq!(bitmask(4, 8), 0b_1111_0000);
        assert_eq!(bitmask(4, 5), 0b_1_0000);
    }

    #[test]
    fn new_word() {
        let word = Word::new(0x123 as _, true, false, OpCode::Value, ValueTag::Int);
        println!("{:?}", word);

        assert_eq!(word.ptr(), 0x123 as _);
        assert_eq!(word.is_option(), true);
        assert_eq!(word.is_none(), false);
        assert_eq!(word.opcode(), OpCode::Value);
        assert_eq!(word.tag(), ValueTag::Int);

        let word = Word::new(
            0x69420 as _,
            true,
            true,
            OpCode::JumpIfFalse,
            ValueTag::StrPtr,
        );
        println!("{:?}", word);

        assert_eq!(word.ptr(), 0x69420 as _);
        assert_eq!(word.is_option(), true);
        assert_eq!(word.is_none(), true);
        assert_eq!(word.opcode(), OpCode::JumpIfFalse);
        assert_eq!(word.tag(), ValueTag::StrPtr);
    }

    #[test]
    fn set() {
        let mut w = Word::int(2345, OpCode::Value);
        println!("{:?}", w);
        w.update_stack_value(123, OpCode::Add);
        println!("{:?}", w);
        assert_eq!(w.to_int(), 123);
        assert_eq!(w.opcode(), OpCode::Add);
    }
}

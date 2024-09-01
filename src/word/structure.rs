use crate::word::heap::HeapValue;
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

/// the first 4 bits of the word represent the type of the value (if any)

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
    OptionPtr, // todo could this be a flag???
}

/// information about the operation to execute with this value.

#[derive(Debug, Clone, PartialEq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Value,
    ReadFromMap, // given to compile-time, heap-allocated values. the value of a word with this opcode is an index to the value map read by the compiled file
    Stop,
    Return,
    Jump,
    JumpIfFalse,
    Call,
    CallBuiltin,
    SetLocal,
    SetLocalThis,
    GetLocal,
    SetGlobal,
    GetGlobal,
    IndexGet,
    IndexSet,
    WrapInOption,
    UnwrapOption,
    Print, // other ops
    Cast,
    Add, // arithmetic
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
    Negate, // logic
    LogicalAnd,
    LogicalOr,
    LogicalXor,
    GreaterThan,
    LessThan,
    Equals,
    NotEquals,
    GreaterThanEq,
    LessThanEq,
    PrintThis, // same as above, but an operand is embedded in the word directly
    CastThis,
    AddThis, // arithmetic
    SubtractThis,
    MultiplyThis,
    DivideThis,
    ModuloThis,
    PowerThis,
    NegateThis, // logic
    LogicalAndThis,
    LogicalOrThis,
    LogicalXorThis,
    GreaterThanThis,
    LessThanThis,
    EqualsThis,
    NotEqualsThis,
    GreaterThanEqThis,
    LessThanEqThis,
}

/// bits 11..32 are more flexible and store auxiliary information that might be needed by some operations
/// for example, when setting/getting a variable, the pointer to the variable stack is read from this section

/// the rest is either the stack value or a heap pointer to it

pub const TAG_START: u64 = 0;
pub const OPCODE_START: u64 = 4;
pub const AUX_START: u64 = 10;
pub const PTR_START: u64 = 32;

pub const TAG_MASK: u64 = bitmask(TAG_START as u32, OPCODE_START as u32);
pub const OPCODE_MASK: u64 = bitmask(OPCODE_START as u32, AUX_START as u32);
pub const AUX_MASK: u64 = bitmask(AUX_START as u32, PTR_START as u32);
pub const PTR_MASK: u64 = bitmask(PTR_START as u32, 64);

impl Debug for Word {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "raw: {:064b}\n\
        {:32} {:21} {:6} {:4}\n\
        {:032b} {:021b} {:06b} {:04b}\n\
        {:?} {:?} {}\n",
            self.0 as usize,
            "ptr",
            "aux",
            "opcode",
            "tag",
            self.ptr() as u32,
            self.aux(),
            self.opcode().to_u8().unwrap(),
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
    pub fn aux(&self) -> u32 {
        ((self.0 as u64 & AUX_MASK) >> AUX_START) as _
    }
    pub fn opcode(&self) -> OpCode {
        OpCode::from_u64((self.0 as u64 & OPCODE_MASK) >> OPCODE_START).unwrap()
    }
    pub fn tag(&self) -> ValueTag {
        ValueTag::from_u64(self.0 as u64 & TAG_MASK).unwrap()
    }

    pub fn new(
        ptr: *mut u8,
        op_code: OpCode,
        tag: ValueTag,
    ) -> Self {
        Self(
            ((((ptr as u64) << PTR_START) & PTR_MASK)
                | (((op_code as u64) << OPCODE_START) & OPCODE_MASK)
                | (tag as u64 & TAG_MASK)) as *mut u8,
        )
    }

    pub fn is_tag_for_heap(&self) -> bool {
        self.tag() > ValueTag::Char
    }

    pub fn is_embeddable(&self) -> bool { self.opcode() == OpCode::Value }

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

    pub fn set_tag(&mut self, new_tag: ValueTag) {
        self.0 = (
            ((self.0 as u64 & !TAG_MASK) | ((new_tag as u64) << TAG_START))
        ) as _
    }

    pub fn set_opcode(&mut self, new_opcode: OpCode) {
        self.0 = (
            (self.0 as u64 &!OPCODE_MASK) | ((new_opcode as u64) << OPCODE_START)
        ) as _;
    }

    pub fn set_aux(&mut self, new_aux: u32) {
        self.0 = (
            (self.0 as u64 &!AUX_MASK) | ((new_aux as u64) << AUX_START)
        ) as _;
    }

    pub fn become_word(&mut self, new_word: Word) {
        self.0 = new_word.0;
    }

    pub fn from_u64(raw: u64) -> Self { Self(raw as _) }
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
        let word = Word::new(0x123 as _, OpCode::Value, ValueTag::Int);
        println!("{:?}", word);

        assert_eq!(word.ptr(), 0x123 as _);
        assert_eq!(word.opcode(), OpCode::Value);
        assert_eq!(word.tag(), ValueTag::Int);

        let word = Word::new(
            0x69420 as _,
            OpCode::JumpIfFalse,
            ValueTag::StrPtr,
        );
        println!("{:?}", word);

        assert_eq!(word.ptr(), 0x69420 as _);
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

    #[test]
    fn become_word() {
        let mut w = Word::int(2345, OpCode::Value);
        let new_word = Word::new(0x123 as _, OpCode::Value, ValueTag::Int);
        w.become_word(new_word);
        println!("{:?}", w);
        assert_eq!(w.ptr(), 0x123 as _);
        assert_eq!(w.opcode(), OpCode::Value);
        assert_eq!(w.tag(), ValueTag::Int);
    }
}

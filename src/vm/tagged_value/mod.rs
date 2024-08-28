pub mod heap;

use crate::vm::garbage_collector::GarbageCollector;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;
use crate::vm::tagged_value::heap::{HeapFloat, HeapValue};

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

// todo option flags are not yet implemented
/// structure of the pointer
/// | 0..2 |               3 |               4 | 5..   usize |
/// | tag  | option flag A   | option flag B   | bulk        |
/// | 000  |               0 |               0 | 000...      |
///
/// - tag: the type of the value
/// - option flag A: indicates whether the type is T (0) or Option<T> (1)
/// - option flag B: whether the above optional is None.
/// (NOTE): the flag combination (11) signals a nested option
/// - bulk: the stack value or the heap location of the value, depending on the type

const TAG_SPAN: usize = 3;
const TAG_MASK: usize = (1 << TAG_SPAN) - 1;

const PTR_SPAN: usize = usize::BITS as usize - TAG_SPAN;
const PTR_MASK: usize = ((1 << PTR_SPAN >> TAG_SPAN) - 1) << TAG_SPAN;

// todo use these to throw an error if an int exceeds the representable size
const MAX_INT: isize = isize::MAX >> TAG_SPAN;
const MIN_INT: isize = isize::MIN >> TAG_SPAN;


#[derive(Copy, Clone)]
/// stores VM values in an usize that can either be the direct representation of the value,
/// or a pointer to its location in the heap.
pub struct TaggedValue(*mut u8);


// constructors
impl TaggedValue {
    fn new(of: *mut u8, typ: ValueTag) -> Self {
        Self((of as usize | typ as usize) as *mut u8)
    }

    pub fn bool(value: bool) -> Self {
        Self::new(((value as u8) << TAG_SPAN) as _, ValueTag::Bool)
    }

    pub fn int(value: isize) -> Self {
        Self::new(((value) << TAG_SPAN) as _, ValueTag::Int)
    }

    pub fn float(value: f64, gc: &mut GarbageCollector) -> Self {
        let ptr = unsafe { HeapFloat::write(value) };
        gc.trace(ptr);
        ptr
    }
}

// accessors
impl TaggedValue {
    pub fn tag(&self) -> ValueTag { ValueTag::from_usize(self.0 as usize & TAG_MASK).unwrap() }

    pub fn ptr(self) -> *mut u8 { (self.0 as usize & PTR_MASK) as _ }

    pub fn in_heap(&self) -> bool { self.tag() > ValueTag::Char }

    unsafe fn get<'a, T>(self) -> &'a T {
        &*(self.ptr() as *const T)
    }

    unsafe fn get_mut<'a, T>(self) -> &'a mut T { &mut *(self.ptr() as *mut T) }
}

// finalizers
impl TaggedValue {
    pub fn to_bool(self) -> bool {
        debug_assert_eq!(self.tag(), ValueTag::Bool);
        self.0 as u8 >> TAG_SPAN == 1
    }

    pub fn to_int(self) -> isize {
        debug_assert_eq!(self.tag(), ValueTag::Int);
        self.0 as isize >> TAG_SPAN
    }

    pub fn to_float(self) -> f64 {
        debug_assert_eq!(self.tag(), ValueTag::FloatPtr);
        unsafe { self.to_float_unchk() }
    }

    pub unsafe fn to_float_unchk(self) -> f64 { *HeapFloat::read(&self) }
}



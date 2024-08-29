use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use std::collections::BTreeMap;
use std::ptr::drop_in_place;
use crate::vm::word::Word;
use crate::vm::word::word_shape::ValueTag;
use crate::vm::word::word_shape::OpCode;

fn allocate(layout: Layout) -> *mut u8 {
    let ptr = unsafe { alloc(layout) };

    if ptr.is_null() {
        handle_alloc_error(layout);
    } else {
        ptr
    }
}

macro_rules! init {
    ($field: expr => $value: expr) => {
        unsafe {
            std::ptr::addr_of_mut!($field).write($value);
        }
    };
}

macro_rules! heap_read {
    ($tv:expr) => { &$tv.get::<Self>().0 }
}

macro_rules! heap_write {
    ($input:expr, $tag:expr) => {{
        let tv = Word::new(
            allocate(Layout::new::<Self>()),
            false,
            false,
            OpCode::Value,
            $tag,
        );
        let container = unsafe { tv.get_mut::<Self>() };
        init!(container.0 => $input);
        tv
    }}
}

macro_rules! heap_destroy {
    ($tv:expr) => {
        drop_in_place($tv.ptr() as *mut Self);
        dealloc($tv.ptr(), Layout::new::<Self>());
    }
}

pub trait HeapValue<T> {
    unsafe fn read(tv: &Word) -> &T;
    unsafe fn write(value: T) -> Word;
    unsafe fn destroy(tv: Word);
}

pub struct HeapFloat(pub f64);
impl HeapValue<f64> for HeapFloat {
    unsafe fn read(tv: &Word) -> &f64 { heap_read!(tv) }
    unsafe fn write(value: f64) -> Word { heap_write!(value, ValueTag::FloatPtr) }
    unsafe fn destroy(tv: Word) { heap_destroy!(tv); }
}

pub struct HeapStr(pub String);
impl HeapValue<String> for HeapStr {
    unsafe fn read(tv: &Word) -> &String { heap_read!(tv) }
    unsafe fn write(value: String) -> Word { heap_write!(value, ValueTag::StrPtr) }
    unsafe fn destroy(tv: Word) { heap_destroy!(tv); }
}

pub struct HeapTable(pub BTreeMap<Word, Word>);
impl HeapValue<BTreeMap<Word, Word>> for HeapTable {
    unsafe fn read(tv: &Word) -> &BTreeMap<Word, Word> { heap_read!(tv) }
    unsafe fn write(value: BTreeMap<Word, Word>) -> Word { heap_write!(value, ValueTag::TablePtr) }
    unsafe fn destroy(tv: Word) { heap_destroy!(tv); }
}
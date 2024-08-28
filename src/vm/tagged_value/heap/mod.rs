use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use std::ptr::drop_in_place;
use crate::vm::tagged_value::{TaggedValue, ValueTag};

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
        let tv = TaggedValue::new(
            allocate(Layout::new::<Self>()),
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
    unsafe fn read(tv: &TaggedValue) -> &T;
    unsafe fn write(value: T) -> TaggedValue;
    unsafe fn destroy(tv: &TaggedValue);
}

pub struct HeapFloat(f64);
impl HeapValue<f64> for HeapFloat {
    unsafe fn read(tv: &TaggedValue) -> &f64 { heap_read!(tv) }
    unsafe fn write(value: f64) -> TaggedValue { heap_write!(value, ValueTag::FloatPtr) }
    unsafe fn destroy(tv: &TaggedValue) { heap_destroy!(tv); }
}

struct HeapStr(String);
impl HeapValue<String> for HeapStr {
    unsafe fn read(tv: &TaggedValue) -> &String { heap_read!(tv) }
    unsafe fn write(value: String) -> TaggedValue { heap_write!(value, ValueTag::StrPtr) }
    unsafe fn destroy(tv: &TaggedValue) { heap_destroy!(tv); }
}

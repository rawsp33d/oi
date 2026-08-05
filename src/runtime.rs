//! Backend-agnostic functions a compiled Oi program calls at runtime.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::mem::size_of;
use std::sync::atomic::{AtomicI64, Ordering};

pub const STR_CONCAT: &str = "oi_str_concat";
pub const STR_MARK: &str = "oi_str_mark";
pub const STR_TAKE: &str = "oi_str_take";
pub const ALLOC: &str = "oi_alloc";
pub const ARRAY_SHARE: &str = "oi_array_share";
pub const ARRAY_COW: &str = "oi_array_cow";
pub const ARRAY_RELEASE: &str = "oi_array_release";
pub const MAP_RELEASE: &str = "oi_map_release";
pub const WRITE: &str = "oi_write";
pub const WRITE_SEP: &str = "oi_write_sep";
pub const SLICE: &str = "oi_slice";
pub const ARRAY_WRITE_BACK: &str = "oi_array_write_back";
pub const PANIC_OOB: &str = "oi_panic_oob";
pub const ARRAY_RESERVE: &str = "oi_array_reserve";
pub const ARRAY_EXTEND: &str = "oi_array_extend";
pub const STR_EQ: &str = "oi_str_eq";
pub const STR_CONTAINS: &str = "oi_str_contains";
pub const ASSERT_FAIL: &str = "oi_assert_fail";
pub const PANIC: &str = "oi_panic";
pub const MAP_NEW: &str = "oi_map_new";
pub const MAP_GET: &str = "oi_map_get";
pub const MAP_SET: &str = "oi_map_set";
pub const MAP_DELETE: &str = "oi_map_delete";
pub const MAP_SHARE: &str = "oi_map_share";
pub const REF_SHARE: &str = "oi_ref_share";
pub const REF_RELEASE: &str = "oi_ref_release";

// Type tag shared with the compiler.
#[repr(i64)]
#[derive(Clone, Copy)]
pub enum Tag {
	Bool,
	Int,
	UInt,
	Float,
	Str,
	Raw,
}

impl Tag {
	// Checked conversion from the raw i64 the JIT passes across the ABI.
	fn from_i64(v: i64) -> Tag {
		match v {
			0 => Tag::Bool,
			1 => Tag::Int,
			2 => Tag::UInt,
			3 => Tag::Float,
			4 => Tag::Str,
			5 => Tag::Raw,
			_ => {
				eprintln!("invalid tag: {v}");
				std::process::abort();
			}
		}
	}
}

// Output sink for writing.
#[repr(i64)]
#[derive(Clone, Copy)]
pub enum Sink {
	Out,
	Err,
	Buf,
}

impl Sink {
	fn from_i64(v: i64) -> Sink {
		match v {
			0 => Sink::Out,
			1 => Sink::Err,
			2 => Sink::Buf,
			_ => {
				eprintln!("invalid sink: {v}");
				std::process::abort();
			}
		}
	}
}

thread_local! {
	static BUF: RefCell<String> = const { RefCell::new(String::new()) };
}

// Route a fragment to its sink.
fn emit(sink: i64, s: &str) {
	match Sink::from_i64(sink) {
		Sink::Out => print!("{s}"),
		Sink::Err => eprint!("{s}"),
		Sink::Buf => BUF.with(|b| b.borrow_mut().push_str(s)),
	}
}

unsafe fn cstr<'a>(ptr: *const u8) -> &'a CStr {
	unsafe { CStr::from_ptr(ptr as *const c_char) }
}

// Render one value to a string.
fn render(tag: Tag, bits: i64, width: i64, quote: bool) -> String {
	match tag {
		Tag::Bool => (bits == 1).to_string(),
		Tag::Int => bits.to_string(),
		Tag::UInt => (bits as u64).to_string(),
		Tag::Float => match width {
			16 => format!("{:?}", f16::from_bits(bits as u16)),
			32 => format!("{:?}", f32::from_bits(bits as u32)),
			_ => format!("{:?}", f64::from_bits(bits as u64)),
		},
		Tag::Str | Tag::Raw => {
			let s = unsafe { cstr(bits as *const u8) }.to_string_lossy();
			if quote && matches!(tag, Tag::Str) {
				format!("{s:?}")
			} else {
				s.into_owned()
			}
		}
	}
}

// Write a rendered value fragment.
pub extern "C" fn write(tag: i64, bits: i64, width: i64, quote: i64, sink: i64) {
	let s = render(Tag::from_i64(tag), bits, width, quote != 0);
	emit(sink, &s);
}

// Write the ", " separator before every element but the first.
pub extern "C" fn write_sep(i: i64, sink: i64) {
	if i > 0 {
		emit(sink, ", ");
	}
}

// Panic with an out-of-bounds message.
pub extern "C" fn panic_oob(index: i64, len: i64) {
	eprintln!("index out of range: the length is {len} but the index is {index}");
	std::process::abort();
}

// Print `{prefix}{msg}` and abort.
unsafe fn abort_with(prefix: &str, msg: *const u8) -> ! {
	let msg = unsafe { cstr(msg) }.to_string_lossy();
	eprintln!("{prefix}{msg}");
	std::process::abort();
}

/// Print an assertion failure message and abort.
/// # Safety
/// `msg` must be a valid NUL-terminated C string.
pub unsafe extern "C" fn assert_fail(msg: *const u8) {
	unsafe { abort_with("assertion failed: ", msg) }
}

/// Print a panic message and abort.
/// # Safety
/// `msg` must be a valid NUL-terminated C string.
pub unsafe extern "C" fn panic(msg: *const u8) {
	unsafe { abort_with("panic: ", msg) }
}

/// # Safety
/// `collection` and `value` must be valid NUL-terminated C strings.
pub unsafe extern "C" fn str_contains(collection: *const u8, value: *const u8) -> i64 {
	let h = unsafe { cstr(collection) }.to_string_lossy();
	let n = unsafe { cstr(value) }.to_string_lossy();
	h.contains(n.as_ref()) as i64
}

/// Compare two 0-terminated strings.
/// # Safety
/// `a` and `b` must be valid NUL-terminated C strings.
pub unsafe extern "C" fn str_eq(a: *const u8, b: *const u8) -> i64 {
	let a = unsafe { cstr(a) };
	let b = unsafe { cstr(b) };
	(a == b) as i64
}

/// Concatenate two 0-terminated strings into a fresh one.
/// # Safety
/// `a` and `b` must be valid NUL-terminated C strings.
pub unsafe extern "C" fn str_concat(a: *const u8, b: *const u8) -> *const u8 {
	let a = unsafe { cstr(a) }.to_bytes();
	let b = unsafe { cstr(b) }.to_bytes();
	let mut out = Vec::with_capacity(a.len() + b.len() + 1);
	out.extend_from_slice(a);
	out.extend_from_slice(b);
	out.push(0);
	Box::leak(out.into_boxed_slice()).as_ptr()
}

// Current buffer length.
pub extern "C" fn str_mark() -> i64 {
	BUF.with(|b| b.borrow().len() as i64)
}

// Split the buffer tail from `mark` into a fresh NUL-terminated heap string.
pub extern "C" fn str_take(mark: i64) -> *const u8 {
	BUF.with(|b| {
		let mut out = b.borrow_mut().split_off(mark as usize).into_bytes();
		out.push(0);
		Box::leak(out.into_boxed_slice()).as_ptr()
	})
}

// Active managed allocations, for leak checks.
static LIVE: AtomicI64 = AtomicI64::new(0);

pub fn leaked() -> i64 {
	LIVE.load(Ordering::Relaxed)
}

// Allocate `size` zeroed bytes for a composite value (e.g. a tuple's field slots).
pub extern "C" fn alloc(size: i64) -> *mut u8 {
	let size = size.max(1) as usize + 8;
	let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
	LIVE.fetch_add(1, Ordering::Relaxed);
	unsafe {
		let base = std::alloc::alloc_zeroed(layout);
		*(base as *mut i64) = size as i64;
		base.add(8)
	}
}

// Free an `alloc` result.
unsafe fn free(ptr: *mut u8) {
	if ptr.is_null() {
		return;
	}
	LIVE.fetch_sub(1, Ordering::Relaxed);
	unsafe {
		let base = ptr.sub(8);
		let size = *(base as *const i64) as usize;
		std::alloc::dealloc(base, std::alloc::Layout::from_size_align_unchecked(size, 8));
	}
}

// Allocate an element buffer with its refcount at data[-8], count starting at 1.
fn buffer_alloc(bytes: i64) -> *mut u8 {
	let base = alloc(bytes + 8);
	unsafe {
		*(base as *mut i64) = 1;
		base.add(8)
	}
}

// Array header layout shared with the compiler (lower/array.rs, offsets 0/8/16).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Header {
	data: i64,
	len: i64,
	cap: i64,
}

/// Clone a header, sharing its buffer with a refcount bump.
/// # Safety
/// `header` must point to a valid array header.
pub unsafe extern "C" fn array_share(header: *const Header) -> *const Header {
	let h = unsafe { *header };
	if h.data != 0 {
		unsafe { *((h.data - 8) as *mut i64) += 1 };
	}
	let out = alloc(size_of::<Header>() as i64) as *mut Header;
	unsafe { *out = h };
	out
}

/// Drop one ref to an array.
/// The buffer frees at zero.
/// # Safety
/// `header` must be null or point to a valid array header.
pub unsafe extern "C" fn array_release(header: *mut Header) {
	if header.is_null() {
		return;
	}
	let Header { data, .. } = unsafe { *header };
	if data != 0 {
		let rc = (data - 8) as *mut i64;
		unsafe { *rc -= 1 };
		if unsafe { *rc } == 0 {
			unsafe { free((data - 8) as *mut u8) };
		}
	}
	unsafe { free(header as *mut u8) };
}

/// Give a shared array its own buffer before a write.
/// No-op when the buffer is null or unshared.
/// # Safety
/// `header` must point to a valid array header.
pub unsafe extern "C" fn array_cow(header: *mut Header, elem_size: i64) {
	let Header { data, len, .. } = unsafe { *header };
	if data == 0 || unsafe { *((data - 8) as *const i64) } <= 1 {
		return;
	}
	let new_data = buffer_alloc(len * elem_size);
	unsafe {
		std::ptr::copy_nonoverlapping(data as *const u8, new_data, (len * elem_size) as usize);
		*((data - 8) as *mut i64) -= 1;
		(*header).data = new_data as i64;
		(*header).cap = len;
	}
}

/// Copy the range of an array into a fresh array.
/// Panics if out of range.
/// # Safety
/// `header` must point to a valid array header.
pub unsafe extern "C" fn slice(header: *const Header, start: i64, end: i64, elem_size: i64) -> *const Header {
	let Header { data, len, .. } = unsafe { *header };
	if start < 0 || start > end || end > len {
		eprintln!("slice range {start}..{end} out of bounds for array of length {len}");
		std::process::abort();
	}
	let view_len = end - start;
	let new_data = buffer_alloc(view_len * elem_size);
	let out = alloc(size_of::<Header>() as i64) as *mut Header;
	unsafe {
		let src = (data + start * elem_size) as *const u8;
		std::ptr::copy_nonoverlapping(src, new_data, (view_len * elem_size) as usize);
		*out = Header {
			data: new_data as i64,
			len: view_len,
			cap: view_len,
		};
	}
	out
}

/// Write a `mut` slice projection back into its parent buffer at `lo`.
/// # Safety
/// `parent` and `src` must point to valid array headers.
pub unsafe extern "C" fn array_write_back(parent: *mut Header, lo: i64, len: i64, src: *const Header, elem_size: i64) {
	let Header { data, len: slen, .. } = unsafe { *src };
	if slen != len {
		eprintln!("projection changed length: expected {len} elements, got {slen}");
		std::process::abort();
	}
	unsafe {
		let dst = ((*parent).data + lo * elem_size) as *mut u8;
		std::ptr::copy_nonoverlapping(data as *const u8, dst, (len * elem_size) as usize);
	}
}

/// Ensure the array has capacity for at least `min_cap` elements.
/// Grows by doubling, at least to `min_cap`. Updates data and cap in place.
/// # Safety
/// `header` must point to a valid array header.
pub unsafe extern "C" fn array_reserve(header: *mut Header, min_cap: i64, elem_size: i64) {
	let Header { data, len, cap } = unsafe { *header };
	if min_cap <= cap {
		return;
	}
	let new_cap = (cap.max(1) * 2).max(min_cap);
	let new_data = buffer_alloc(new_cap * elem_size);
	unsafe {
		std::ptr::copy_nonoverlapping(data as *const u8, new_data, (len * elem_size) as usize);
		(*header).data = new_data as i64;
		(*header).cap = new_cap;
		if data != 0 {
			free((data - 8) as *mut u8);
		}
	}
}

/// Append all elements of `src` to `dst`, growing dst's buffer as needed.
/// # Safety
/// `dst` and `src` must point to valid array headers.
pub unsafe extern "C" fn array_extend(dst: *mut Header, src: *const Header, elem_size: i64) {
	let dst_len = unsafe { (*dst).len };
	let Header {
		data: src_data,
		len: src_len,
		..
	} = unsafe { *src };
	unsafe { array_reserve(dst, dst_len + src_len, elem_size) };
	unsafe {
		let dst_data = (*dst).data as *mut u8;
		let dst_tail = dst_data.add((dst_len * elem_size) as usize);
		std::ptr::copy_nonoverlapping(src_data as *const u8, dst_tail, (src_len * elem_size) as usize);
		(*dst).len = dst_len + src_len;
	}
}

/// Share a `&T`, bumping its refcount.
/// # Safety
/// `ptr` must point to a valid boxed struct's field slots (or be null).
pub unsafe extern "C" fn ref_share(ptr: *mut u8) -> *mut u8 {
	if ptr.is_null() {
		return ptr;
	}
	unsafe { *(ptr.sub(8) as *mut i64) += 1 };
	ptr
}

// Walk a box's trace descriptor, calling `visit` on each ref slot.
unsafe fn trace(fields: *mut u8, desc: *const i64, visit: unsafe extern "C" fn(*mut u8)) {
	if desc.is_null() {
		return;
	}
	unsafe {
		let mut p = desc.add(1);
		for _ in 0..*desc {
			let e = *p;
			p = p.add(1);
			let slot = *(fields.add((e >> 1) as usize) as *const *mut u8);
			if e & 1 == 0 {
				visit(slot);
				continue;
			}
			if !slot.is_null() {
				trace(slot, *p as *const i64, visit);
			}
			p = p.add(1);
		}
	}
}

/// Drop one ref to a boxed struct.
/// # Safety
/// `ptr` must be null or point to a valid boxed struct's field slots.
pub unsafe extern "C" fn ref_release(ptr: *mut u8) {
	if ptr.is_null() {
		return;
	}
	unsafe {
		let rc = ptr.sub(8) as *mut i64;
		*rc -= 1;
		if *rc == 0 {
			let desc = *(ptr.sub(16) as *const *const i64);
			trace(ptr, desc, ref_release);
			free(ptr.sub(16));
		}
	}
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum MapKey {
	Raw(i64),
	Str(Vec<u8>),
}

fn map_key(tag: Tag, bits: i64) -> MapKey {
	match tag {
		Tag::Str => MapKey::Str(unsafe { cstr(bits as *const u8) }.to_bytes().to_vec()),
		_ => MapKey::Raw(bits),
	}
}

pub struct OiMap {
	entries: HashMap<MapKey, i64>,
	rc: i64,
}

pub extern "C" fn map_new() -> *mut OiMap {
	LIVE.fetch_add(1, Ordering::Relaxed);
	Box::into_raw(Box::new(OiMap {
		entries: HashMap::new(),
		rc: 1,
	}))
}

/// Drop one ref to a map.
/// The box frees at zero.
/// # Safety
/// `map` must be null or a valid live `OiMap` pointer.
pub unsafe extern "C" fn map_release(map: *mut OiMap) {
	if map.is_null() {
		return;
	}
	unsafe { (*map).rc -= 1 };
	if unsafe { (*map).rc } == 0 {
		LIVE.fetch_sub(1, Ordering::Relaxed);
		drop(unsafe { Box::from_raw(map) });
	}
}

/// Share a map handle.
/// RC bump.
/// # Safety
/// `map` must be a valid, live `OiMap` pointer.
pub unsafe extern "C" fn map_share(map: *mut OiMap) -> *mut OiMap {
	unsafe { (*map).rc += 1 };
	map
}

// Give a shared map its own entries before a write.
unsafe fn map_cow(map: *mut OiMap) -> *mut OiMap {
	if unsafe { (*map).rc } <= 1 {
		return map;
	}
	unsafe { (*map).rc -= 1 };
	let entries = unsafe { (*map).entries.clone() };
	LIVE.fetch_add(1, Ordering::Relaxed);
	Box::into_raw(Box::new(OiMap { entries, rc: 1 }))
}

/// # Safety
/// `map` must be a valid, live `OiMap` pointer.
pub unsafe extern "C" fn map_get(map: *mut OiMap, tag: i64, bits: i64) -> i64 {
	let map = unsafe { &*map };
	match map.entries.get(&map_key(Tag::from_i64(tag), bits)) {
		Some(v) => *v,
		None => {
			eprintln!("key not found in map");
			std::process::abort();
		}
	}
}

/// Set a map entry, cloning shared entries first.
/// # Safety
/// `map` must be a valid, live `OiMap` pointer.
pub unsafe extern "C" fn map_set(map: *mut OiMap, tag: i64, bits: i64, value: i64) -> *mut OiMap {
	let map = unsafe { map_cow(map) };
	unsafe { &mut *map }.entries.insert(map_key(Tag::from_i64(tag), bits), value);
	map
}

/// Remove a map entry if present, cloning shared entries first.
/// # Safety
/// `map` must be a valid, live `OiMap` pointer.
pub unsafe extern "C" fn map_delete(map: *mut OiMap, tag: i64, bits: i64) -> *mut OiMap {
	let map = unsafe { map_cow(map) };
	unsafe { &mut *map }.entries.remove(&map_key(Tag::from_i64(tag), bits));
	map
}

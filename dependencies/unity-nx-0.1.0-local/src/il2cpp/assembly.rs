use std::ffi::CStr;

use super::api;

#[repr(C)]
pub struct Il2CppImage {
    pub name: *const u8,
    name_no_ext: *const u8,
    assembly: &'static Il2CppAssembly,
}

impl Il2CppImage {
    pub fn get_name(&self) -> String {
        unsafe {
            String::from_utf8_lossy(CStr::from_ptr(self.name as _).to_bytes()).to_string()
        }
    }
}

#[repr(C)]
pub struct Il2CppAssembly {
    pub image: &'static Il2CppImage,
    token: u32,
    referenced_assembly_start: i32,
    referenced_assembly_count: i32,
}

// std::vector layout, read-only from unity
#[repr(C)]
pub(crate) struct CppVector<T> {
    start: *const T,
    end: *const T,
    eos: *const T,
}

impl<T> CppVector<T> {
    pub(crate) fn as_slice(&self) -> &[T] {
        if self.start.is_null() {
            return &[];
        }
        let len = unsafe { self.end.offset_from(self.start) as usize };
        unsafe { std::slice::from_raw_parts(self.start, len) }
    }
}

// Iterate in reverse to get game assemblies before Unity and mscorlib
pub fn get_assemblies() -> &'static [&'static Il2CppAssembly] {
    unsafe { api::assembly_getallassemblies() }.as_slice()
}

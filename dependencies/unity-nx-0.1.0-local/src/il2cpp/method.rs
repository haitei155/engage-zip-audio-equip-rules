use std::ffi::CStr;

use super::{class::Il2CppClass, Il2CppType};

pub type OptionalMethod = Option<&'static MethodInfo>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MethodInfo {
    pub method_ptr: *mut u8,
    pub invoker_method: *const u8,
    pub name: *const u8,
    pub class: Option<&'static Il2CppClass>,
    pub return_type: *const u8,
    pub parameters: *const ParameterInfo,
    pub info_or_definition: *const u8,
    pub generic_method_or_container: *const u8,
    pub token: u32,
    pub flags: u16,
    pub iflags: u16,
    pub slot: u16,
    pub parameters_count: u8,
    pub bitflags: u8,
}

unsafe impl Send for MethodInfo {}
unsafe impl Sync for MethodInfo {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ParameterInfo {
    pub name: *const u8,
    pub position: i32,
    pub token: u32,
    pub parameter_type: &'static Il2CppType,
}

unsafe impl Send for ParameterInfo {}
unsafe impl Sync for ParameterInfo {}

impl Default for MethodInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl MethodInfo {
    pub fn new() -> Self {
        unsafe { ::core::mem::zeroed() }
    }

    #[inline]
    pub fn is_generic_definition(&self) -> bool {
        self.bitflags & 1 != 0
    }

    pub fn get_name(&self) -> Option<String> {
        if self.name.is_null() {
            None
        } else {
            Some(unsafe {
                String::from_utf8_lossy(CStr::from_ptr(self.name as _).to_bytes()).to_string()
            })
        }
    }

    pub fn get_parameters(&self) -> &[ParameterInfo] {
        unsafe { std::slice::from_raw_parts(self.parameters, self.parameters_count as _) }
    }
}

impl ParameterInfo {
    pub fn get_name(&self) -> Option<String> {
        if self.name.is_null() {
            None
        } else {
            Some(unsafe {
                String::from_utf8_lossy(CStr::from_ptr(self.name as _).to_bytes()).to_string()
            })
        }
    }
}

//! Bindings for the il2cpp_* runtime functions.
//!
//! Cobalt scans for these and exports them as named symbols. We just
//! declare them as extern and the loader hooks them up at plugin load.
//! Rust names stay the same as before so callers don't have to change,
//! link_name maps each one onto the real il2cpp_* symbol.
//!
//! See cobalt::api::il2cpp for the why.

use super::{
    assembly::{CppVector, Il2CppAssembly, Il2CppImage},
    class::Il2CppClass,
    method::MethodInfo,
    Il2CppType,
};
use crate::system::SystemType;

extern "C" {
    #[allow(dead_code)]
    #[link_name = "il2cpp_init"]
    pub(crate) fn init(domain_name: *const i8) -> i32;

    #[link_name = "il2cpp_class_from_name"]
    pub(crate) fn class_from_name(
        image: &Il2CppImage,
        namespace: *const u8,
        name: *const u8,
    ) -> Option<&'static mut Il2CppClass>;

    #[link_name = "il2cpp_class_get_method_from_name"]
    pub(crate) fn get_method_from_name_flags(
        class: &Il2CppClass,
        method_name: *const u8,
        args_count: usize,
        flags: u32,
    ) -> Option<&'static mut MethodInfo>;

    #[link_name = "il2cpp_type_get_object"]
    pub(crate) fn type_get_object(ty: &Il2CppType) -> SystemType;

    #[link_name = "il2cpp_class_from_il2cpptype"]
    pub fn class_from_il2cpptype(ty: &Il2CppType) -> Option<&'static mut Il2CppClass>;

    // Run after resolving a class from a generic Il2CppType, finalizes field/method/vtable metadata
    #[link_name = "il2cpp_class_init"]
    pub(crate) fn class_init(class: &Il2CppClass);

    // Sets the class header only, callers still need a .ctor
    #[link_name = "il2cpp_object_new"]
    pub fn object_new(klass: &Il2CppClass) -> crate::IlInstance;

    #[link_name = "il2cpp_string_new"]
    pub(crate) fn string_new(c_str: *const u8) -> crate::Il2CppString;

    // extern blocks can't be generic, so this is the plain version that
    // the generic array_new<T> below calls into. Array<T> is laid out
    // the same as IlInstance no matter what T is, so the transmute is safe
    #[link_name = "il2cpp_array_new"]
    fn array_new_extern(element_typeinfo: &Il2CppClass, length: usize) -> crate::IlInstance;
}

#[skyline::from_offset(0x42911c)]
pub(crate) fn assembly_getallassemblies() -> &'static CppVector<&'static Il2CppAssembly>;

// kind 0 is Normal (scanned), 1 is Atomic (not scanned), class cloning uses Normal
#[skyline::from_offset(0x474370)]
pub(crate) fn gc_malloc_kind(size: usize, kind: u32) -> *mut u8;

// length 0 returns the s_EmptyString static instance, do NOT write into it
#[skyline::from_offset(0x44a168)]
pub(crate) fn string_new_size(length: i32, method_info: crate::OptionalMethod) -> crate::Il2CppString;

pub(crate) unsafe fn array_new<T: Copy>(element_typeinfo: &Il2CppClass, length: usize) -> crate::Array<T> {
    let inst = array_new_extern(element_typeinfo, length);
    std::mem::transmute_copy(&inst)
}

// Iterating in reverse so game assemblies are found before Unity and mscorlib
pub(crate) fn get_class_from_name(
    namespace: impl AsRef<str>,
    name: impl AsRef<str>,
) -> Option<&'static mut Il2CppClass> {
    super::assembly::get_assemblies().iter().rev().find_map(|assembly| {
        let namespace = std::ffi::CString::new(namespace.as_ref()).ok()?;
        let name = std::ffi::CString::new(name.as_ref()).ok()?;
        unsafe {
            class_from_name(
                assembly.image,
                namespace.as_ptr() as _,
                name.as_ptr() as _,
            )
        }
    })
}

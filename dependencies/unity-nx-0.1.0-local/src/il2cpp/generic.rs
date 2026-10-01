use crate::il2cpp::{Il2CppType, MethodInfo};

#[repr(C)]
#[derive(Clone, Copy)]
pub union GenericMethodInfo {
    pub generic_method: &'static Il2CppGenericMethod,
    pub generic_container: crate::il2cpp::Il2CppGenericContainer,
}

#[repr(C)]
pub struct Il2CppGenericMethod {
    pub method_definition: &'static MethodInfo,
    pub generic_context: Il2CppGenericContext,
}

#[repr(C)]
pub struct Il2CppGenericContext {
    pub class_inst: Option<&'static Il2CppGenericInst>,
    pub method_inst: Option<&'static Il2CppGenericInst>,
}

#[repr(C)]
pub struct Il2CppGenericInst {
    pub type_argc: u32,
    pub type_argv: *const &'static Il2CppType,
}

impl Il2CppGenericInst {
    pub fn get_types(&self) -> &[&'static Il2CppType] {
        unsafe { ::std::slice::from_raw_parts(self.type_argv, self.type_argc as usize) }
    }
}

pub fn create_generic_method_info(
    open: &MethodInfo,
    types: &[&'static Il2CppType],
) -> &'static MethodInfo {
    let method_inst = unsafe { create_generic_inst(types.as_ptr(), types.len()) };
    let class_inst: Option<&Il2CppGenericInst> = if open.bitflags & 2 == 0 {
        None
    } else {
        open.class
            .and_then(|k| k._1.generic_class)
            .and_then(|gc| gc.context.class_inst)
    };
    let gm = unsafe { create_generic_method(open, class_inst, Some(method_inst)) };
    unsafe { generic_method_create_method_info(gm, 0) }
}

#[skyline::from_offset(0x43e2bc)]
fn create_generic_inst(
    types: *const &'static Il2CppType,
    len: usize,
) -> &'static Il2CppGenericInst;

#[skyline::from_offset(0x439a48)]
fn create_generic_method(
    open: &MethodInfo,
    class_inst: Option<&Il2CppGenericInst>,
    method_inst: Option<&Il2CppGenericInst>,
) -> &'static Il2CppGenericMethod;

#[skyline::from_offset(0x47c0d4)]
fn generic_method_create_method_info(
    gm: &Il2CppGenericMethod,
    flags: i32,
) -> &'static MethodInfo;

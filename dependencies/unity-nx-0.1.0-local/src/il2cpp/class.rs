use std::ffi::CStr;

use super::{
    api,
    assembly::Il2CppImage,
    method::MethodInfo,
    Il2CppGenericClass, Il2CppGenericContainer, Il2CppRGCTXData, Il2CppType,
};

// Il2CppObject header plus *const Il2CppType, prefer the SystemType wrapper
#[repr(C)]
pub struct Il2CppReflectionType {
    _klass: *mut Il2CppClass,
    _monitor: *const (),
    pub ty: *const Il2CppType,
}

#[repr(C)]
pub struct Il2CppClass1 {
    pub image: &'static Il2CppImage,
    pub gc_desc: *const u8,
    name: *const u8,
    namespace: *const u8,
    pub byval_arg: Il2CppType,
    this_arg: Il2CppType,
    pub element_class: &'static Il2CppClass,
    _1_start: [u8; 0x10],
    pub parent: &'static Il2CppClass,
    pub generic_class: Option<&'static Il2CppGenericClass>,
    _1_end: [u8; 0x18],
    pub fields: *const FieldInfo,
    pub events: *const u8,
    pub properties: *const PropertyInfo,
    pub methods: *const &'static MethodInfo,
    pub nested_types: *const &'static Il2CppClass,
    // Length is Il2CppClass2::interfaces_count
    implemented_interfaces: *const &'static Il2CppClass,
    interface_offsets: *const u8,
}

#[repr(C)]
pub struct Il2CppClass2 {
    pub type_hierarchy: *const &'static Il2CppClass,
    _2_start: [u8; 0x20],
    pub generic_handle: Option<&'static Il2CppGenericContainer>,
    pub instance_size: u32,
    pub actual_size: u32,
    __: [u8; 0x18],
    pub token: u32,
    pub method_count: u16,
    property_count: u16,
    pub field_count: u16,
    event_count: u16,
    pub nested_type_count: u16,
    pub vtable_count: u16,
    interfaces_count: u16,
    interface_offsets_count: u16,
    pub type_hierarchy_depth: u8,
    generic_recursion_depth: u8,
    pub rank: u8,
    minimum_alignment: u8,
    natural_alignment: u8,
    packing_size: u8,
    // packed flags, bit 5 is has_references (the GC scans this type's instances
    // for pointers). offset 0x132 from the class base, confirmed in ghidra
    pub bitflags1: u8,
    bitflags2: u8,
    _2_end: [u8; 0x4],
}

#[repr(C)]
pub struct Il2CppClass {
    pub _1: Il2CppClass1,
    pub static_fields: *mut (),
    pub rgctx_data: &'static mut Il2CppRGCTXData,
    pub _2: Il2CppClass2,
    vtable: [VirtualInvoke; 0],
}

unsafe impl Send for Il2CppClass {}
unsafe impl Sync for Il2CppClass {}

impl Il2CppClass1 {
    pub unsafe fn set_name_ptrs_via_ptr(
        class1: *mut Il2CppClass1,
        name: *const u8,
        namespace: *const u8,
    ) {
        ::core::ptr::write_volatile(::core::ptr::addr_of_mut!((*class1).name), name);
        ::core::ptr::write_volatile(::core::ptr::addr_of_mut!((*class1).namespace), namespace);
    }

    pub unsafe fn repoint_type_args_to_self(class: *mut Il2CppClass, template: *const Il2CppClass) {
        let self_ptr = class as *const u8;
        let template_ptr = template as *const u8;
        let byval = ::core::ptr::addr_of_mut!((*class)._1.byval_arg.data.data);
        let this = ::core::ptr::addr_of_mut!((*class)._1.this_arg.data.data);
        if *byval == template_ptr {
            *byval = self_ptr;
        }
        if *this == template_ptr {
            *this = self_ptr;
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FieldInfo {
    name: *const u8,
    ty: &'static Il2CppType,
    parent: &'static Il2CppClass,
    pub offset: i32,
    token: u32,
}

unsafe impl Send for FieldInfo {}
unsafe impl Sync for FieldInfo {}

impl FieldInfo {
    pub fn new_synthetic(
        name: *const u8,
        ty: &'static Il2CppType,
        parent: &'static Il2CppClass,
        offset: i32,
        token: u32,
    ) -> Self {
        Self {
            name,
            ty,
            parent,
            offset,
            token,
        }
    }

    pub fn is_instance(&self) -> bool {
        (self.ty.bits >> 16) & 0x0010 == 0
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
}

#[repr(C)]
pub struct PropertyInfo {
    pub class: &'static Il2CppClass,
    pub name: *const u8,
    pub get: &'static MethodInfo,
    pub set: &'static MethodInfo,
    pub attrs: i32,
    pub token: u32,
}

impl PropertyInfo {
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtualInvoke {
    pub method_ptr: *mut u8,
    pub method_info: &'static MethodInfo,
}

impl VirtualInvoke {
    pub fn get_name(&self) -> Option<String> {
        self.method_info.get_name()
    }
}

unsafe impl Send for VirtualInvoke {}
unsafe impl Sync for VirtualInvoke {}

impl Il2CppClass {
    pub fn from_name(
        namespace: impl AsRef<str>,
        name: impl AsRef<str>,
    ) -> Option<&'static mut Self> {
        api::get_class_from_name(namespace, name)
    }

    pub fn get_type(&self) -> &Il2CppType {
        &self._1.byval_arg
    }

    pub fn get_name(&self) -> String {
        unsafe {
            String::from_utf8_lossy(CStr::from_ptr(self._1.name as _).to_bytes()).to_string()
        }
    }

    pub fn get_namespace(&self) -> String {
        unsafe {
            String::from_utf8_lossy(CStr::from_ptr(self._1.namespace as _).to_bytes()).to_string()
        }
    }

    pub fn get_vtable(&self) -> &[VirtualInvoke] {
        unsafe { std::slice::from_raw_parts(self.vtable.as_ptr(), self._2.vtable_count as _) }
    }

    pub fn get_vtable_mut(&mut self) -> &mut [VirtualInvoke] {
        unsafe {
            std::slice::from_raw_parts_mut(self.vtable.as_mut_ptr(), self._2.vtable_count as _)
        }
    }

    pub fn get_virtual_method(&self, name: impl AsRef<str>) -> Option<&VirtualInvoke> {
        self.get_vtable()
            .iter()
            .find(|m| m.get_name().unwrap_or_default() == name.as_ref())
    }

    pub fn get_virtual_method_mut(
        &mut self,
        name: impl AsRef<str>,
    ) -> Option<&mut VirtualInvoke> {
        self.get_vtable_mut()
            .iter_mut()
            .find(|v| v.get_name().as_deref() == Some(name.as_ref()))
    }

    pub fn override_virtual_method(
        &mut self,
        name: impl AsRef<str>,
        method_info: &'static MethodInfo,
    ) -> Option<VirtualInvoke> {
        let entry = self.get_virtual_method_mut(name)?;
        let old = *entry;
        entry.method_ptr = method_info.method_ptr;
        entry.method_info = method_info;
        Some(old)
    }

    // Declared only, inherited fields live on the parent
    pub fn get_fields(&self) -> &[FieldInfo] {
        unsafe { std::slice::from_raw_parts(self._1.fields, self._2.field_count as _) }
    }

    pub fn get_instance_fields(&self) -> impl Iterator<Item = &FieldInfo> {
        self.get_fields()
            .iter()
            .filter(|f| f.is_instance() && f.offset != 0)
    }

    pub fn get_properties(&self) -> &[PropertyInfo] {
        unsafe {
            std::slice::from_raw_parts(self._1.properties, self._2.property_count as _)
        }
    }

    pub fn get_methods(&self) -> &[&'static MethodInfo] {
        unsafe { std::slice::from_raw_parts(self._1.methods, self._2.method_count as _) }
    }

    pub fn get_nested_types(&self) -> &[&'static Il2CppClass] {
        unsafe {
            std::slice::from_raw_parts(self._1.nested_types, self._2.nested_type_count as _)
        }
    }

    pub fn get_class_hierarchy(&self) -> &[&'static Il2CppClass] {
        unsafe {
            std::slice::from_raw_parts(
                self._2.type_hierarchy,
                self._2.type_hierarchy_depth as _,
            )
        }
    }

    pub fn get_implemented_interfaces(&self) -> &[&'static Il2CppClass] {
        if self._1.implemented_interfaces.is_null() {
            return &[];
        }
        unsafe {
            std::slice::from_raw_parts(
                self._1.implemented_interfaces,
                self._2.interfaces_count as _,
            )
        }
    }

    pub fn get_method_from_name(
        &self,
        name: impl AsRef<str>,
        args_count: usize,
    ) -> Option<&'static mut MethodInfo> {
        self.get_method_from_name_with_flag(name, args_count, 0)
    }

    pub fn get_method_from_name_with_flag(
        &self,
        name: impl AsRef<str>,
        args_count: usize,
        flag: u32,
    ) -> Option<&'static mut MethodInfo> {
        let name = std::ffi::CString::new(name.as_ref()).ok()?;
        unsafe { api::get_method_from_name_flags(self, name.as_ptr() as _, args_count, flag) }
    }
}

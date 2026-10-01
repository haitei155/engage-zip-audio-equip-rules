use std::ffi::CStr;
use std::sync::OnceLock;

use crate::error::{Il2CppError, Il2CppResult, InjectionReason};
use crate::il2cpp::method::ParameterInfo;
use crate::il2cpp::{FieldInfo, Il2CppClass, Il2CppType, MethodInfo};
use crate::{Class, ClassIdentity, FromIlInstance};

pub trait InjectedClass: ClassIdentity + FromIlInstance {
    type Parent: ClassIdentity;
    const EXTRA_BYTES: u32 = 0;

    const NAME_CSTR: &'static CStr;
    const NAMESPACE_CSTR: &'static CStr;

    fn cache() -> &'static OnceLock<Class>;

    fn injected_fields() -> Vec<InjectedFieldDescriptor> {
        Vec::new()
    }

    fn injected_methods() -> Vec<InjectedMethodDescriptor> {
        Vec::new()
    }

    fn injected_overrides() -> Vec<InjectedOverrideDescriptor> {
        Vec::new()
    }

    fn fill_cache(class: Class) {
        let _ = Self::cache().set(class);
    }
}

pub trait DefaultInjectedMembers {
    fn __injected_fields() -> Vec<InjectedFieldDescriptor> {
        Vec::new()
    }

    fn __injected_methods() -> Vec<InjectedMethodDescriptor> {
        Vec::new()
    }

    fn __injected_overrides() -> Vec<InjectedOverrideDescriptor> {
        Vec::new()
    }
}

impl<T> DefaultInjectedMembers for T {}

pub struct InjectedFieldDescriptor {
    pub name: &'static CStr,
    pub ty: &'static Il2CppType,
    pub offset: u32,
}

pub struct InjectedParameterDescriptor {
    pub name: &'static CStr,
    pub ty: &'static Il2CppType,
}

pub struct InjectedMethodDescriptor {
    pub name: &'static CStr,
    pub method_ptr: *mut u8,
    pub return_type: &'static Il2CppType,
    pub parameters: Vec<InjectedParameterDescriptor>,
}

unsafe impl Send for InjectedMethodDescriptor {}
unsafe impl Sync for InjectedMethodDescriptor {}

pub struct InjectedOverrideDescriptor {
    pub name: &'static CStr,
    pub method_ptr: *mut u8,
}

unsafe impl Send for InjectedOverrideDescriptor {}
unsafe impl Sync for InjectedOverrideDescriptor {}

fn force_conservative_gc(class: Class) {
    unsafe {
        let raw = class.raw() as *const Il2CppClass as *mut Il2CppClass;
        ::core::ptr::write_volatile(::core::ptr::addr_of_mut!((*raw)._1.gc_desc), ::core::ptr::null());
        let flags = ::core::ptr::addr_of_mut!((*raw)._2.bitflags1);
        ::core::ptr::write_volatile(flags, ::core::ptr::read_volatile(flags) | 0x20);
    }
}

fn install_type_hierarchy(class: Class, parent: Class) {
    let parent_raw = parent.raw();
    let parent_depth = parent_raw._2.type_hierarchy_depth as usize;
    let parent_hierarchy: &[&'static Il2CppClass] = unsafe {
        if parent_raw._2.type_hierarchy.is_null() || parent_depth == 0 {
            &[]
        } else {
            ::core::slice::from_raw_parts(parent_raw._2.type_hierarchy, parent_depth)
        }
    };

    let mut new_hierarchy: Vec<&'static Il2CppClass> = parent_hierarchy.to_vec();
    new_hierarchy.push(class.raw());

    let leaked: &'static [&'static Il2CppClass] = Box::leak(new_hierarchy.into_boxed_slice());
    unsafe {
        let raw_ptr = class.raw() as *const Il2CppClass as *mut Il2CppClass;
        ::core::ptr::write_volatile(
            ::core::ptr::addr_of_mut!((*raw_ptr)._2.type_hierarchy),
            leaked.as_ptr(),
        );
        ::core::ptr::write_volatile(
            ::core::ptr::addr_of_mut!((*raw_ptr)._2.type_hierarchy_depth),
            leaked.len() as u8,
        );
    }
}

fn install_fields(class: Class, descriptors: Vec<InjectedFieldDescriptor>) {
    const FIELD_ATTRIBUTE_PUBLIC: u32 = 0x0006;

    let parent_fields: &[FieldInfo] = class.raw().get_fields();
    let mut all_fields: Vec<FieldInfo> = parent_fields.to_vec();

    for desc in descriptors {
        let mut ty: Il2CppType =
            unsafe { ::core::ptr::read(desc.ty as *const Il2CppType) };
        ty.bits = (ty.bits & !0xFFFF) | FIELD_ATTRIBUTE_PUBLIC;
        let leaked_ty: &'static Il2CppType = Box::leak(Box::new(ty));

        let fi = FieldInfo::new_synthetic(
            desc.name.as_ptr() as *const u8,
            leaked_ty,
            class.raw(),
            desc.offset as i32,
            0,
        );
        all_fields.push(fi);
    }

    let leaked_array: &'static [FieldInfo] = Box::leak(all_fields.into_boxed_slice());
    unsafe {
        let raw_ptr = class.raw() as *const Il2CppClass as *mut Il2CppClass;
        ::core::ptr::write_volatile(
            ::core::ptr::addr_of_mut!((*raw_ptr)._1.fields),
            leaked_array.as_ptr(),
        );
        ::core::ptr::write_volatile(
            ::core::ptr::addr_of_mut!((*raw_ptr)._2.field_count),
            leaked_array.len() as u16,
        );
    }
}

fn install_override(class: Class, slot_name: &str, method_ptr: *mut u8) -> Il2CppResult<()> {
    let donor: &MethodInfo = {
        let vi = class.raw().get_virtual_method(slot_name).ok_or_else(|| {
            Il2CppError::InjectionFailed {
                class: format!("{}.{}", class.namespace(), class.name()),
                method: slot_name.to_string(),
                reason: InjectionReason::MissingVirtualSlot,
            }
        })?;
        vi.method_info
    };

    let mut new_mi: MethodInfo = *donor;
    new_mi.method_ptr = method_ptr;
    let leaked: &'static MethodInfo = Box::leak(Box::new(new_mi));

    class.override_virtual_method(slot_name, leaked);
    Ok(())
}

fn install_methods(
    class: Class,
    descriptors: Vec<InjectedMethodDescriptor>,
) -> Il2CppResult<()> {
    let parent_methods: &[&'static MethodInfo] = class.raw().get_methods();
    let mut all_methods: Vec<&'static MethodInfo> = parent_methods.to_vec();

    fn pick_invoker_donor(
        parent_methods: &[&'static MethodInfo],
        params_count: u8,
        return_is_void: bool,
    ) -> Option<*const u8> {
        if let Some(m) = parent_methods.iter().find(|m| {
            m.parameters_count == params_count
                && method_is_void_return(m) == return_is_void
                && !m.invoker_method.is_null()
        }) {
            return Some(m.invoker_method);
        }
        parent_methods
            .iter()
            .find(|m| !m.invoker_method.is_null())
            .map(|m| m.invoker_method)
    }

    fn method_is_void_return(m: &MethodInfo) -> bool {
        if m.return_type.is_null() {
            return false;
        }
        unsafe {
            let ty = &*(m.return_type as *const Il2CppType);
            ty.type_enum() == crate::il2cpp::TYPE_VOID
        }
    }

    for desc in descriptors {
        let parameters_count = desc.parameters.len() as u8;

        let parameters_ptr: *const ParameterInfo = if desc.parameters.is_empty() {
            ::core::ptr::null()
        } else {
            let infos: Vec<ParameterInfo> = desc
                .parameters
                .iter()
                .enumerate()
                .map(|(i, p)| ParameterInfo {
                    name: p.name.as_ptr() as *const u8,
                    position: i as i32,
                    token: 0,
                    parameter_type: p.ty,
                })
                .collect();
            let leaked: &'static [ParameterInfo] = Box::leak(infos.into_boxed_slice());
            leaked.as_ptr()
        };

        let return_is_void = desc.return_type.type_enum() == crate::il2cpp::TYPE_VOID;
        let invoker_donor = pick_invoker_donor(parent_methods, parameters_count, return_is_void)
            .ok_or_else(|| Il2CppError::InjectionFailed {
                class: format!("{}.{}", class.namespace(), class.name()),
                method: desc.name.to_string_lossy().into_owned(),
                reason: InjectionReason::NoInvokerDonor { params: parameters_count },
            })?;

        let mut mi = MethodInfo::new();
        mi.method_ptr = desc.method_ptr;
        mi.invoker_method = invoker_donor;
        mi.name = desc.name.as_ptr() as *const u8;
        mi.class = Some(class.raw());
        mi.return_type = desc.return_type as *const Il2CppType as *const u8;
        mi.parameters = parameters_ptr;
        mi.parameters_count = parameters_count;
        mi.flags = 0x0006;
        mi.slot = u16::MAX;

        let leaked: &'static MethodInfo = Box::leak(Box::new(mi));
        all_methods.push(leaked);
    }

    let leaked_array: &'static [&'static MethodInfo] = Box::leak(all_methods.into_boxed_slice());
    unsafe {
        let raw_ptr = class.raw() as *const Il2CppClass as *mut Il2CppClass;
        ::core::ptr::write_volatile(
            ::core::ptr::addr_of_mut!((*raw_ptr)._1.methods),
            leaked_array.as_ptr(),
        );
        ::core::ptr::write_volatile(
            ::core::ptr::addr_of_mut!((*raw_ptr)._2.method_count),
            leaked_array.len() as u16,
        );
    }
    Ok(())
}

pub fn build<T: InjectedClass>() -> Class {
    let parent = <T::Parent as ClassIdentity>::class();
    let class = parent.clone_for_override();

    class.set_name(T::NAME_CSTR, T::NAMESPACE_CSTR);
    class.set_instance_size(parent.instance_size() + T::EXTRA_BYTES);
    install_type_hierarchy(class, parent);

    for o in T::injected_overrides() {
        let slot = o.name.to_string_lossy();
        install_override(class, &slot, o.method_ptr).unwrap_or_else(|e| panic!("{}", e));
    }

    let fields = T::injected_fields();
    let has_reference_field = fields.iter().any(|f| !f.ty.valuetype());
    if !fields.is_empty() {
        install_fields(class, fields);
    }

    let methods = T::injected_methods();
    if !methods.is_empty() {
        install_methods(class, methods).unwrap_or_else(|e| panic!("{}", e));
    }

    if has_reference_field {
        force_conservative_gc(class);
    }

    class
}

pub fn instantiate<T: InjectedClass>() -> Option<T> {
    let class = T::class();
    let inst: crate::IlInstance = unsafe { crate::il2cpp::api::object_new(class.raw()) };
    if inst.is_null() {
        return None;
    }
    Some(T::from_il_instance(inst))
}

pub fn instantiate_with_ctor<T: InjectedClass>() -> Option<T> {
    let class = T::class();
    let inst: crate::IlInstance = unsafe { crate::il2cpp::api::object_new(class.raw()) };
    if inst.is_null() {
        return None;
    }

    if let Some(ctor) = T::Parent::class().raw().get_method_from_name(".ctor", 0) {
        invoke_ctor(inst, &*ctor);
    }

    Some(T::from_il_instance(inst))
}

fn invoke_ctor(inst: crate::IlInstance, ctor: &'static MethodInfo) {
    if ctor.method_ptr.is_null() {
        return;
    }
    type CtorFn = unsafe extern "C" fn(*mut (), *const MethodInfo);
    let f: CtorFn = unsafe { ::core::mem::transmute(ctor.method_ptr) };
    unsafe { f(inst.as_ptr(), ctor as *const _) };
}

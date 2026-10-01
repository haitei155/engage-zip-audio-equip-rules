use crate::il2cpp::{Il2CppType, MethodInfo};
use crate::{Class, Il2CppError, Il2CppResult};

pub fn method_offset_by_name(
    namespace: &str,
    class_name: &str,
    method_name: &str,
    param_count: usize,
) -> Il2CppResult<usize> {
    let class = Class::try_lookup(namespace, class_name)?;
    method_offset_on_class(class, method_name, param_count)
}

pub fn method_offset_on_class(
    class: Class,
    method_name: &str,
    param_count: usize,
) -> Il2CppResult<usize> {
    let raw = class.raw();
    let method = raw.get_method_from_name(method_name, param_count).ok_or_else(|| {
        Il2CppError::MissingMethod {
            class: format!("{}.{}", raw.get_namespace(), raw.get_name()),
            method: method_name.to_string(),
            param_count,
        }
    })?;

    Ok(method_ptr_to_offset(method.method_ptr))
}

pub fn method_offset_by_vtable_index(
    namespace: &str,
    class_name: &str,
    vtable_index: usize,
) -> Il2CppResult<usize> {
    let class = Class::try_lookup(namespace, class_name)?;
    method_offset_by_vtable_index_on_class(class, vtable_index)
}

pub fn method_offset_by_vtable_index_on_class(
    class: Class,
    vtable_index: usize,
) -> Il2CppResult<usize> {
    let raw = class.raw();
    let vtable = raw.get_vtable();
    let slot = vtable.get(vtable_index).ok_or_else(|| Il2CppError::VtableIndexOutOfRange {
        class: format!("{}.{}", raw.get_namespace(), raw.get_name()),
        index: vtable_index,
        vtable_len: vtable.len(),
    })?;

    Ok(method_ptr_to_offset(slot.method_ptr))
}

fn method_ptr_to_offset(method_ptr: *mut u8) -> usize {
    let text = crate::scan::get_text();
    unsafe { (method_ptr as *const u8).offset_from(text.as_ptr()) as usize }
}

const METHOD_ATTRIBUTE_STATIC: u16 = 0x0010;

pub fn method_info_on_class(
    class: Class,
    method_name: &str,
    param_count: usize,
) -> Il2CppResult<&'static MethodInfo> {
    let raw = class.raw();
    let mut generic_hit: Option<&'static MethodInfo> = None;
    for_each_in_parent_chain(class, |c| {
        for m in c.get_methods() {
            if m.parameters_count as usize == param_count
                && m.is_generic_definition()
                && m.get_name().as_deref() == Some(method_name)
            {
                generic_hit = Some(*m);
                return false;
            }
        }
        true
    });
    if let Some(mi) = generic_hit {
        return Ok(mi);
    }
    raw.get_method_from_name(method_name, param_count)
        .map(|mi| &*mi)
        .ok_or_else(|| Il2CppError::MissingMethod {
            class: format!("{}.{}", raw.get_namespace(), raw.get_name()),
            method: method_name.to_string(),
            param_count,
        })
}

fn for_each_in_parent_chain<F: FnMut(&'static crate::il2cpp::Il2CppClass) -> bool>(
    class: Class,
    mut visit: F,
) {
    const MAX_DEPTH: usize = 16;
    const PARENT_OFFSET: usize = 0x58;
    let mut cur: *const crate::il2cpp::Il2CppClass = class.raw() as *const _;
    let mut steps = 0usize;
    while !cur.is_null() && steps < MAX_DEPTH {
        let c: &'static crate::il2cpp::Il2CppClass = unsafe { &*cur };
        if !visit(c) {
            return;
        }
        let parent_ptr_loc = (cur as *const u8).wrapping_add(PARENT_OFFSET)
            as *const *const crate::il2cpp::Il2CppClass;
        let parent_ptr = unsafe { *parent_ptr_loc };
        if parent_ptr.is_null() || std::ptr::eq(parent_ptr, cur) {
            return;
        }
        cur = parent_ptr;
        steps += 1;
    }
}

pub fn method_info_on_class_with_signature(
    class: Class,
    method_name: &str,
    param_count: usize,
    param_types: &[&'static Il2CppType],
    is_static: bool,
) -> Il2CppResult<&'static MethodInfo> {
    debug_assert_eq!(param_count, param_types.len(), "param_count must match param_types.len()");
    let raw = class.raw();
    let mut strict_hits: Vec<&'static MethodInfo> = Vec::new();
    let mut loose_hits: Vec<&'static MethodInfo> = Vec::new();
    for_each_in_parent_chain(class, |c| {
        let before_strict = strict_hits.len();
        let before_loose = loose_hits.len();
        for m in c.get_methods() {
            if m.parameters_count as usize != param_count {
                continue;
            }
            if m.get_name().as_deref() != Some(method_name) {
                continue;
            }
            let mi_static = (m.flags & METHOD_ATTRIBUTE_STATIC) != 0;
            if mi_static != is_static {
                continue;
            }
            let actual = m.get_parameters();
            let mut strict = true;
            let mut loose = true;
            for (i, want) in param_types.iter().enumerate() {
                let got = actual[i].parameter_type;
                if !il2cpp_type_eq_strict(got, *want) {
                    strict = false;
                }
                if !il2cpp_type_eq(got, *want) {
                    loose = false;
                    break;
                }
            }
            if !loose {
                continue;
            }
            if strict {
                strict_hits.push(*m);
            } else {
                loose_hits.push(*m);
            }
        }
        strict_hits.len() == before_strict && loose_hits.len() == before_loose
    });
    let hits = if !strict_hits.is_empty() {
        strict_hits
    } else {
        loose_hits
    };

    match hits.len() {
        0 => Err(Il2CppError::MissingMethod {
            class: format!("{}.{}", raw.get_namespace(), raw.get_name()),
            method: method_name.to_string(),
            param_count,
        }),
        1 => Ok(hits[0]),
        n => Err(Il2CppError::AmbiguousMethod {
            class: format!("{}.{}", raw.get_namespace(), raw.get_name()),
            method: method_name.to_string(),
            param_count,
            overload_count: n,
        }),
    }
}

pub fn method_info_disambiguated(
    class: Class,
    method_name: &str,
    param_count: usize,
    param_types: &[&'static Il2CppType],
    is_static: bool,
) -> Option<&'static MethodInfo> {
    if let Ok(mi) = method_info_on_class_with_signature(class, method_name, param_count, param_types, is_static) {
        return Some(mi);
    }
    class.raw().get_method_from_name(method_name, param_count).map(|m| &*m)
}

fn il2cpp_type_eq(a: &Il2CppType, b: &Il2CppType) -> bool {
    if std::ptr::eq(a as *const _, b as *const _) {
        return true;
    }
    if matches!(a.type_enum(), crate::il2cpp::TYPE_VAR | crate::il2cpp::TYPE_MVAR)
        || matches!(b.type_enum(), crate::il2cpp::TYPE_VAR | crate::il2cpp::TYPE_MVAR)
    {
        return true;
    }
    if (a.type_enum() == crate::il2cpp::TYPE_OBJECT && is_reference_type_enum(b.type_enum()))
        || (b.type_enum() == crate::il2cpp::TYPE_OBJECT && is_reference_type_enum(a.type_enum()))
    {
        return true;
    }
    if a.type_enum() != b.type_enum() {
        return false;
    }
    match a.type_enum() {
        crate::il2cpp::TYPE_VOID
        | crate::il2cpp::TYPE_BOOLEAN
        | crate::il2cpp::TYPE_CHAR
        | crate::il2cpp::TYPE_I1
        | crate::il2cpp::TYPE_U1
        | crate::il2cpp::TYPE_I2
        | crate::il2cpp::TYPE_U2
        | crate::il2cpp::TYPE_I4
        | crate::il2cpp::TYPE_U4
        | crate::il2cpp::TYPE_I8
        | crate::il2cpp::TYPE_U8
        | crate::il2cpp::TYPE_R4
        | crate::il2cpp::TYPE_R8
        | crate::il2cpp::TYPE_STRING
        | crate::il2cpp::TYPE_OBJECT
        | crate::il2cpp::TYPE_I
        | crate::il2cpp::TYPE_U => true,
        crate::il2cpp::TYPE_CLASS | crate::il2cpp::TYPE_VALUETYPE => {
            a.data_as_usize() == b.data_as_usize()
        }
        crate::il2cpp::TYPE_SZARRAY => {
            let ae = unsafe { &*(a.data_as_usize() as *const Il2CppType) };
            let be = unsafe { &*(b.data_as_usize() as *const Il2CppType) };
            il2cpp_type_eq(ae, be)
        }
        _ => a.data_as_usize() == b.data_as_usize(),
    }
}

fn il2cpp_type_eq_strict(a: &Il2CppType, b: &Il2CppType) -> bool {
    if std::ptr::eq(a as *const _, b as *const _) {
        return true;
    }
    if a.byref() != b.byref() {
        return false;
    }
    if a.type_enum() != b.type_enum() {
        return false;
    }
    match a.type_enum() {
        crate::il2cpp::TYPE_VOID
        | crate::il2cpp::TYPE_BOOLEAN
        | crate::il2cpp::TYPE_CHAR
        | crate::il2cpp::TYPE_I1
        | crate::il2cpp::TYPE_U1
        | crate::il2cpp::TYPE_I2
        | crate::il2cpp::TYPE_U2
        | crate::il2cpp::TYPE_I4
        | crate::il2cpp::TYPE_U4
        | crate::il2cpp::TYPE_I8
        | crate::il2cpp::TYPE_U8
        | crate::il2cpp::TYPE_R4
        | crate::il2cpp::TYPE_R8
        | crate::il2cpp::TYPE_STRING
        | crate::il2cpp::TYPE_OBJECT
        | crate::il2cpp::TYPE_I
        | crate::il2cpp::TYPE_U => true,
        crate::il2cpp::TYPE_CLASS | crate::il2cpp::TYPE_VALUETYPE => {
            a.data_as_usize() == b.data_as_usize()
        }
        crate::il2cpp::TYPE_SZARRAY => {
            let ae = unsafe { &*(a.data_as_usize() as *const Il2CppType) };
            let be = unsafe { &*(b.data_as_usize() as *const Il2CppType) };
            il2cpp_type_eq_strict(ae, be)
        }
        _ => a.data_as_usize() == b.data_as_usize(),
    }
}

fn is_reference_type_enum(t: u8) -> bool {
    matches!(
        t,
        crate::il2cpp::TYPE_STRING
            | crate::il2cpp::TYPE_CLASS
            | crate::il2cpp::TYPE_OBJECT
            | crate::il2cpp::TYPE_SZARRAY
            | crate::il2cpp::TYPE_ARRAY
    )
}

pub fn method_info_by_vtable_index_on_class(
    class: Class,
    vtable_index: usize,
) -> Il2CppResult<&'static MethodInfo> {
    let raw = class.raw();
    let vtable = raw.get_vtable();
    let slot = vtable.get(vtable_index).ok_or_else(|| Il2CppError::VtableIndexOutOfRange {
        class: format!("{}.{}", raw.get_namespace(), raw.get_name()),
        index: vtable_index,
        vtable_len: vtable.len(),
    })?;
    Ok(slot.method_info)
}

use crate::{get_fields, Class, Il2CppResult};

pub fn peek_method(
    namespace: &str,
    class_name: &str,
    method_name: &str,
    param_count: usize,
) -> Il2CppResult<String> {
    let class = Class::try_lookup(namespace, class_name)?.raw();
    let method = class
        .get_method_from_name(method_name, param_count)
        .ok_or_else(|| crate::Il2CppError::MissingMethod {
            class: format!("{}.{}", namespace, class_name),
            method: method_name.to_string(),
            param_count,
        })?;

    let method_ptr = method.method_ptr;
    let text = crate::scan::get_text();
    let offset = unsafe { (method_ptr as *const u8).offset_from(text.as_ptr()) as isize };

    let mut insns = [0u32; 4];
    unsafe {
        core::ptr::copy_nonoverlapping(
            method_ptr as *const u32,
            insns.as_mut_ptr(),
            insns.len(),
        );
    }

    Ok(format!(
        "[unity::diag::peek_method] {ns}.{cls}::{m}({argc})\n  \
         method_ptr = {ptr:p}\n  \
         offset from .text = 0x{off:x}\n  \
         first 4 instructions (LE) = [0x{a:08x}, 0x{b:08x}, 0x{c:08x}, 0x{d:08x}]\n  \
         parameters_count = {pc}, flags = 0x{flags:04x}",
        ns = namespace,
        cls = class_name,
        m = method_name,
        argc = param_count,
        ptr = method_ptr,
        off = offset,
        a = insns[0],
        b = insns[1],
        c = insns[2],
        d = insns[3],
        pc = method.parameters_count,
        flags = method.flags,
    ))
}

pub fn peek_fields(namespace: &str, class_name: &str) -> Il2CppResult<String> {
    let class = Class::try_lookup(namespace, class_name)?.raw();
    let mut out = format!("[unity::diag::peek_fields] {}.{}\n", namespace, class_name);
    for c in class.get_class_hierarchy() {
        out.push_str(&format!(
            "  class `{}` declares {} field(s):\n",
            c.get_name(),
            c.get_fields().len(),
        ));
        for f in c.get_fields() {
            out.push_str(&format!(
                "    name={:?} offset=0x{:x}\n",
                f.get_name().unwrap_or_default(),
                f.offset,
            ));
        }
    }
    out.push_str(&format!("  (total via get_fields hierarchy walk: {})\n", get_fields(class).len()));
    Ok(out)
}

pub fn peek_class_layout(namespace: &str, class_name: &str) -> Il2CppResult<String> {
    let class = Class::try_lookup(namespace, class_name)?.raw();
    let class_ptr = class as *const crate::il2cpp::Il2CppClass as *const u8;

    let raw: &[u8] = unsafe { core::slice::from_raw_parts(class_ptr, 0x200) };

    let mut out = format!(
        "[unity::diag::peek_class_layout] {}.{}\n  class ptr = {:p}\n",
        namespace, class_name, class_ptr,
    );

    let hier = class.get_class_hierarchy();
    out.push_str(&format!(
        "  unity reads: fields={:p} field_count={} methods={:p} method_count={} hierarchy_depth={} (ptr={:p})\n",
        class.get_fields().as_ptr() as *const _,
        class.get_fields().len(),
        class.get_methods().as_ptr() as *const _,
        class.get_methods().len(),
        hier.len(),
        hier.as_ptr() as *const _,
    ));

    out.push_str("  hierarchy chain (via type_hierarchy + type_hierarchy_depth):\n");
    for (i, parent) in hier.iter().enumerate() {
        out.push_str(&format!(
            "    [{}] ptr={:p} name={:?} field_count={}\n",
            i,
            *parent as *const _,
            parent.get_name(),
            parent.get_fields().len(),
        ));
    }

    out.push_str("  parent chain (via Il2CppClass1.parent ptr):\n");
    let mut cur: Option<&'static crate::il2cpp::Il2CppClass> = Some(class);
    let mut steps = 0;
    while let Some(c) = cur {
        out.push_str(&format!(
            "    [{}] ptr={:p} name={:?} field_count={}\n",
            steps,
            c as *const _,
            c.get_name(),
            c.get_fields().len(),
        ));
        steps += 1;
        if steps > 8 { break; }
        let parent_ptr: *const crate::il2cpp::Il2CppClass = c._1.parent;
        if parent_ptr.is_null() || core::ptr::eq(parent_ptr, c as *const _) {
            break;
        }
        cur = Some(unsafe { &*parent_ptr });
    }

    out.push_str("  bytes (hex qwords from offset 0):\n");
    for off in (0..0x180).step_by(0x20) {
        let chunk: &[u64] = unsafe {
            core::slice::from_raw_parts(raw.as_ptr().add(off) as *const u64, 4)
        };
        out.push_str(&format!(
            "    0x{:03x}: {:016x} {:016x} {:016x} {:016x}\n",
            off, chunk[0], chunk[1], chunk[2], chunk[3],
        ));
    }
    Ok(out)
}

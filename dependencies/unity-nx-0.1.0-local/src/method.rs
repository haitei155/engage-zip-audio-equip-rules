use std::marker::PhantomData;

use crate::class::Class;
use crate::il2cpp::MethodInfo;

/// # Safety
///
/// Caller must ensure that `method_info.invoker_method` is non-null and matches the
/// signature `R, this, args` expected by the IL2CPP invoker thunk. `args.len()` must
/// equal `method_info.parameters_count`, and each pointer in `args` must outlive the call.
pub unsafe fn invoke_via_invoker<R: Copy>(
    method_info: &'static MethodInfo,
    this: *const (),
    args: &[*const ()],
) -> R {
    debug_assert!(
        !method_info.invoker_method.is_null(),
        "invoke_via_invoker called on a MethodInfo with null invoker_method",
    );
    let invoker: extern "C" fn(
        *mut u8,
        &'static MethodInfo,
        *const (),
        *const *const (),
    ) -> R = std::mem::transmute(method_info.invoker_method);
    invoker(method_info.method_ptr, method_info, this, args.as_ptr())
}

const METHOD_ATTRIBUTE_STATIC: u16 = 0x0010;

pub struct Method<Sig> {
    method_ptr: *mut u8,
    method_info: &'static MethodInfo,
    _sig: PhantomData<fn() -> Sig>,
}

impl<Sig> Clone for Method<Sig> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Sig> Copy for Method<Sig> {}

unsafe impl<Sig> Send for Method<Sig> {}
unsafe impl<Sig> Sync for Method<Sig> {}

impl<Sig> Method<Sig> {
    pub fn info(self) -> &'static MethodInfo {
        self.method_info
    }

    pub fn raw_ptr(self) -> *mut u8 {
        self.method_ptr
    }
}

pub trait MethodSignature {
    const PARAM_COUNT: usize;
}

impl Class {
    pub fn method<Sig: MethodSignature>(self, name: &str) -> Option<Method<Sig>> {
        for ancestor in self.hierarchy() {
            let cls = ancestor.raw();
            let matches: Vec<&'static MethodInfo> = cls
                .get_methods()
                .iter()
                .copied()
                .filter(|mi| {
                    if mi.get_name().as_deref() != Some(name) {
                        return false;
                    }
                    let is_static = (mi.flags & METHOD_ATTRIBUTE_STATIC) != 0;
                    let effective_param_count =
                        mi.parameters_count as usize + if is_static { 0 } else { 1 };
                    effective_param_count == Sig::PARAM_COUNT
                })
                .collect();

            if matches.len() > 1 {
                panic!(
                    "{}",
                    crate::Il2CppError::AmbiguousMethod {
                        class: format!("{}.{}", ancestor.namespace(), ancestor.name()),
                        method: name.to_string(),
                        param_count: Sig::PARAM_COUNT,
                        overload_count: matches.len(),
                    }
                );
            }

            if let Some(&mi) = matches.first() {
                return Some(Method {
                    method_ptr: mi.method_ptr,
                    method_info: mi,
                    _sig: PhantomData,
                });
            }
        }
        None
    }
}

macro_rules! impl_method {
    ($param_count:literal $(, $arg:ident : $a:ident)*) => {
        impl<$($a,)* R> MethodSignature for fn($($a),*) -> R {
            const PARAM_COUNT: usize = $param_count;
        }

        impl<$($a,)* R> Method<fn($($a),*) -> R> {
            #[inline]
            #[allow(clippy::too_many_arguments)]
            pub fn call(self $(, $arg: $a)*) -> R {
                let f: extern "C" fn($($a,)* Option<&'static MethodInfo>) -> R =
                    unsafe { std::mem::transmute(self.method_ptr) };
                f($($arg,)* Some(self.method_info))
            }
        }
    };
}

impl_method!(0);
impl_method!(1, a1: A1);
impl_method!(2, a1: A1, a2: A2);
impl_method!(3, a1: A1, a2: A2, a3: A3);
impl_method!(4, a1: A1, a2: A2, a3: A3, a4: A4);
impl_method!(5, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5);
impl_method!(6, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5, a6: A6);
impl_method!(7, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5, a6: A6, a7: A7);
impl_method!(8, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5, a6: A6, a7: A7, a8: A8);
impl_method!(9, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5, a6: A6, a7: A7, a8: A8, a9: A9);
impl_method!(10, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5, a6: A6, a7: A7, a8: A8, a9: A9, a10: A10);
impl_method!(11, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5, a6: A6, a7: A7, a8: A8, a9: A9, a10: A10, a11: A11);
impl_method!(12, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5, a6: A6, a7: A7, a8: A8, a9: A9, a10: A10, a11: A11, a12: A12);

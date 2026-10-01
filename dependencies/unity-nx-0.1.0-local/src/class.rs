use std::ffi::CStr;
use std::sync::OnceLock;

use crate::il2cpp::{api, FieldInfo, Il2CppClass, MethodInfo, PropertyInfo};
use crate::system::SystemType;
use crate::{Array, ClassIdentity};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Class {
    inner: *mut Il2CppClass,
}

unsafe impl Send for Class {}
unsafe impl Sync for Class {}

impl Class {
    #[inline]
    pub fn from_raw(inner: &'static Il2CppClass) -> Self {
        Self { inner: inner as *const Il2CppClass as *mut Il2CppClass }
    }

    // Dotted names like `MapUnitCommandMenu.DanceMenuItem` split on the last `.` and walk nested_types
    pub fn lookup(namespace: &str, name: &str) -> Self {
        Self::try_lookup(namespace, name)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    pub fn try_lookup(namespace: &str, name: &str) -> crate::Il2CppResult<Self> {
        if let Some((outer, inner)) = name.rsplit_once('.') {
            let outer_class = Self::try_lookup(namespace, outer)?;
            return outer_class
                .raw()
                .get_nested_types()
                .iter()
                .find(|nt| nt.get_name() == inner)
                .map(|&c| Self::from_raw(c))
                .inspect(|class| {
                    unsafe { api::class_init(class.raw()) };
                })
                .ok_or_else(|| crate::Il2CppError::MissingClass {
                    namespace: namespace.to_string(),
                    name: name.to_string(),
                });
        }
        Il2CppClass::from_name(namespace, name)
            .map(|c| Self::from_raw(c))
            .inspect(|class| {
                unsafe { api::class_init(class.raw()) };
            })
            .ok_or_else(|| crate::Il2CppError::MissingClass {
                namespace: namespace.to_string(),
                name: name.to_string(),
            })
    }

    #[inline]
    pub fn raw(self) -> &'static Il2CppClass {
        unsafe { &*self.inner }
    }

    #[inline]
    pub fn raw_mut(self) -> &'static mut Il2CppClass {
        unsafe { &mut *self.inner }
    }

    pub fn name(self) -> String {
        self.raw().get_name()
    }

    pub fn namespace(self) -> String {
        self.raw().get_namespace()
    }

    pub fn instance_size(self) -> u32 {
        self.raw()._2.instance_size
    }

    pub fn set_name(self, name: &'static CStr, namespace: &'static CStr) {
        unsafe {
            let class_ptr = self.inner as *const Il2CppClass as *mut Il2CppClass;
            let class1_ptr = ::core::ptr::addr_of_mut!((*class_ptr)._1)
                as *mut crate::il2cpp::class::Il2CppClass1;
            crate::il2cpp::class::Il2CppClass1::set_name_ptrs_via_ptr(
                class1_ptr,
                name.as_ptr() as *const u8,
                namespace.as_ptr() as *const u8,
            );
        }
    }

    pub fn set_instance_size(self, size: u32) {
        unsafe {
            let raw_ptr = self.inner as *const Il2CppClass as *mut Il2CppClass;
            ::core::ptr::write_volatile(
                ::core::ptr::addr_of_mut!((*raw_ptr)._2.instance_size),
                size,
            );
            ::core::ptr::write_volatile(
                ::core::ptr::addr_of_mut!((*raw_ptr)._2.actual_size),
                size,
            );
        }
    }

    pub fn parent(self) -> Option<Class> {
        let parent = self.raw()._1.parent;

        if std::ptr::eq(parent as *const _, self.inner as *const _) {
            None
        } else {
            Some(Class::from_raw(parent))
        }
    }

    pub fn hierarchy(self) -> impl Iterator<Item = Class> {
        self.raw()
            .get_class_hierarchy()
            .iter()
            .map(|&c| Class::from_raw(c))
    }

    pub fn is<T: ClassIdentity>(self) -> bool {
        self == T::class()
    }

    // Immediate parent only, does not walk the hierarchy
    pub fn parent_is<T: ClassIdentity>(self) -> bool {
        self.parent().is_some_and(|p| p.is::<T>())
    }

    pub fn interfaces(self) -> impl Iterator<Item = Class> {
        self.raw()
            .get_implemented_interfaces()
            .iter()
            .map(|&c| Class::from_raw(c))
    }

    // Matches il2cpp_class_is_assignable_from, hierarchy walk plus flattened interfaces
    pub fn is_subclass_of<T: ClassIdentity>(self) -> bool {
        let target = T::class();
        self.hierarchy().any(|c| c == target) || self.interfaces().any(|i| i == target)
    }

    // Declared only, walk hierarchy().flat_map(|c| c.declared_fields()) for inherited
    pub fn declared_fields(self) -> &'static [FieldInfo] {
        self.raw().get_fields()
    }

    pub fn declared_methods(self) -> &'static [&'static MethodInfo] {
        self.raw().get_methods()
    }

    pub fn declared_properties(self) -> &'static [PropertyInfo] {
        self.raw().get_properties()
    }
}

impl Class {
    #[inline]
    pub fn of<T: ClassIdentity>() -> Class {
        T::class()
    }

    pub fn clone_for_override(self) -> Class {
        // Class header ends at 0x138, vtable entries follow, 16 bytes each
        const HEADER_SIZE: usize = 0x138;
        const VIRTUAL_INVOKE_SIZE: usize = ::core::mem::size_of::<crate::il2cpp::VirtualInvoke>();

        let src = self.raw();
        let size = HEADER_SIZE + VIRTUAL_INVOKE_SIZE * src._2.vtable_count as usize;

        unsafe {
            let layout = ::std::alloc::Layout::from_size_align(size, 8).unwrap();
            let dest = ::std::alloc::alloc(layout);
            ::core::ptr::copy_nonoverlapping(
                src as *const Il2CppClass as *const u8,
                dest,
                size,
            );

            // the memcpy left byval_arg/this_arg pointing at the template, fix
            // them so type checks resolve to this clone and not the template
            crate::il2cpp::class::Il2CppClass1::repoint_type_args_to_self(dest as *mut Il2CppClass, src as *const Il2CppClass);

            Class::from_raw(&*(dest as *const Il2CppClass))
        }
    }

    #[inline]
    pub fn override_virtual_method(
        self,
        name: impl AsRef<str>,
        method_info: &'static crate::il2cpp::MethodInfo,
    ) -> Option<crate::il2cpp::VirtualInvoke> {
        self.raw_mut().override_virtual_method(name, method_info)
    }

    pub fn init(self) {
        unsafe { api::class_init(self.raw()) }
    }

    pub fn array_class(self) -> Class {
        let arr: Array<u8> = unsafe { api::array_new::<u8>(self.raw(), 0) };
        // Il2CppArraySize header's first field is *const Il2CppClass regardless of T
        let class_ptr: *const Il2CppClass =
            unsafe { *(crate::IlInstance::from(arr).as_ptr() as *const *const Il2CppClass) };
        let class_ref: &'static Il2CppClass = unsafe { &*class_ptr };
        unsafe { api::class_init(class_ref) };
        Self::from_raw(class_ref)
    }

    pub fn make_generic(self, type_args: &[Class]) -> Option<Class> {
        let args_array: Array<SystemType> =
            Array::new(SystemType::class().raw(), type_args.len())?;

        for (i, arg) in type_args.iter().enumerate() {
            let ty = SystemType::from_il2cpp_type(arg.raw().get_type())?;
            args_array.set(i, ty);
        }

        let generic_type = SystemType::from_il2cpp_type(self.raw().get_type())?;

        static MAKE_GENERIC_TYPE: OnceLock<&'static crate::il2cpp::MethodInfo> = OnceLock::new();

        let method_info = *MAKE_GENERIC_TYPE.get_or_init(|| {
            let cls = Class::lookup("System", "RuntimeType");
            cls.raw()
                .get_method_from_name("MakeGenericType", 2)
                .expect("System.RuntimeType.MakeGenericType(Type, Type[]) not found in IL2CPP metadata")
        });

        if method_info.invoker_method.is_null() {
            panic!("System.RuntimeType.MakeGenericType has null invoker_method");
        }

        let args: [*const (); 2] = [
            crate::IlInstance::from(generic_type).as_ptr() as *const (),
            crate::IlInstance::from(args_array).as_ptr() as *const (),
        ];

        let invoker: extern "C" fn(
            *const u8,
            *const crate::il2cpp::MethodInfo,
            *const (),
            *const *const (),
        ) -> *mut crate::IlObject = unsafe { std::mem::transmute(method_info.invoker_method) };

        let method_info_ptr: *const crate::il2cpp::MethodInfo = method_info;
        let result_ptr = invoker(
            method_info.method_ptr as *const u8,
            method_info_ptr,
            core::ptr::null(),
            args.as_ptr(),
        );

        let result = <SystemType as crate::FromIlInstance>::from_il_instance(
            crate::IlInstance::from_raw(result_ptr as *mut ()),
        );

        if result.is_null() {
            return None;
        }

        let result_type = result.il2cpp_type();
        let result_class = unsafe { api::class_from_il2cpptype(result_type) }?;

        unsafe { api::class_init(result_class) };

        Some(Class::from_raw(result_class))
    }
}

impl PartialEq for Class {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.inner as *const _, other.inner as *const _)
    }
}
impl Eq for Class {}

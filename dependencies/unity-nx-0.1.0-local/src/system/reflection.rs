use std::sync::OnceLock;

use crate::class::Class;
use crate::il2cpp::{api, Il2CppReflectionType, Il2CppType};
use crate::{ClassIdentity, FromIlInstance, IlInstance, IlType};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct SystemType(IlInstance);

impl ClassIdentity for SystemType {
    const NAMESPACE: &'static str = "System";
    const NAME: &'static str = "Type";

    fn class() -> Class {
        static CACHE: OnceLock<Class> = OnceLock::new();
        *CACHE.get_or_init(|| Class::lookup(Self::NAMESPACE, Self::NAME))
    }
}

impl FromIlInstance for SystemType {
    fn from_il_instance(instance: IlInstance) -> Self {
        Self(instance)
    }
}

impl IlType for SystemType {
    fn il_type() -> &'static Il2CppType {
        &<Self as ClassIdentity>::class().raw()._1.byval_arg
    }
}

impl From<SystemType> for IlInstance {
    fn from(s: SystemType) -> Self {
        s.0
    }
}

impl AsRef<IlInstance> for SystemType {
    fn as_ref(&self) -> &IlInstance {
        &self.0
    }
}

impl SystemType {
    pub fn from_il2cpp_type(ty: &Il2CppType) -> Option<Self> {
        let st = unsafe { api::type_get_object(ty) };
        (!st.0.is_null()).then_some(st)
    }

    pub fn of<T: ClassIdentity>() -> Option<Self> {
        Self::from_il2cpp_type(T::class().raw().get_type())
    }

    #[inline]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }

    pub fn il2cpp_type(self) -> &'static Il2CppType {
        assert!(!self.is_null(), "SystemType::il2cpp_type on null");
        unsafe {
            let rt = self.0.as_ptr() as *const Il2CppReflectionType;
            &*(*rt).ty
        }
    }
}

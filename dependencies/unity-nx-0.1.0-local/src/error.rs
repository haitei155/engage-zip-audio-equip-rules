pub type Il2CppResult<T> = ::core::result::Result<T, Il2CppError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Il2CppError {
    #[error("class `{namespace}.{name}` not found")]
    MissingClass { namespace: String, name: String },

    #[error("could not resolve Il2CppType to an Il2CppClass")]
    MissingClassForType,

    #[error("method `{class}::{method}` with {param_count} parameters not found")]
    MissingMethod {
        class: String,
        method: String,
        param_count: usize,
    },

    #[error("method `{class}::{method}` has {overload_count} overloads with {param_count} parameters, disambiguate via offset or vtable_index")]
    AmbiguousMethod {
        class: String,
        method: String,
        param_count: usize,
        overload_count: usize,
    },

    #[error("field `{field}` not found on class `{class}`")]
    MissingField { class: String, field: String },

    #[error("vtable index {index} out of range for `{class}`, vtable has {vtable_len} slots")]
    VtableIndexOutOfRange {
        class: String,
        index: usize,
        vtable_len: usize,
    },

    #[error("IL2CPP allocator returned null for `{class}`")]
    FailedInstantiation { class: String },

    #[error("IL2CPP array allocator returned null")]
    FailedArrayInstantiation,

    #[error("IL2CPP generic instantiation failed for `{class}`")]
    FailedGenericInstantiation { class: String },

    #[error("IL2CPP method `{method}` returned null")]
    FailedMethodInvocation { method: String },

    #[error("could not construct a System.Type reflection object")]
    FailedReflectionQuerying,

    #[error("injection failed for `{class}::{method}`: {reason}")]
    InjectionFailed {
        class: String,
        method: String,
        #[source]
        reason: InjectionReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InjectionReason {
    // pick_invoker_donor couldn't find any parent method with a non-null invoker
    #[error("no invoker donor in parent class for a method with {params} param(s)")]
    NoInvokerDonor { params: u8 },
    // override_virtual was given a slot name the parent class doesn't declare
    #[error("parent class does not declare this virtual slot")]
    MissingVirtualSlot,
}

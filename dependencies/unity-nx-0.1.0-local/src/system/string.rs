use std::fmt::{self, Display, Formatter, Write};
use std::sync::OnceLock;

use crate::class::Class;
use crate::{ClassIdentity, FromIlInstance, IlInstance, IlType};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Il2CppString(IlInstance);

impl ClassIdentity for Il2CppString {
    const NAMESPACE: &'static str = "System";
    const NAME: &'static str = "String";

    fn class() -> Class {
        static CACHE: OnceLock<Class> = OnceLock::new();
        *CACHE.get_or_init(|| Class::lookup(Self::NAMESPACE, Self::NAME))
    }
}

impl FromIlInstance for Il2CppString {
    fn from_il_instance(instance: IlInstance) -> Self {
        Self(instance)
    }
}

impl IlType for Il2CppString {
    fn il_type() -> &'static crate::il2cpp::Il2CppType {
        &<Self as ClassIdentity>::class().raw()._1.byval_arg
    }
}

// Mirrors what `#[unity::class]` would emit, needed because Il2CppString is declared by hand
pub trait IIl2CppString: crate::SystemObject {}

impl IIl2CppString for Il2CppString {}

impl Il2CppString {
    #[doc(hidden)]
    #[inline]
    pub fn __unity2_from_il_instance(instance: IlInstance) -> Self {
        Self(instance)
    }
}

impl From<Il2CppString> for IlInstance {
    fn from(s: Il2CppString) -> Self {
        s.0
    }
}

impl AsRef<IlInstance> for Il2CppString {
    fn as_ref(&self) -> &IlInstance {
        &self.0
    }
}

// Il2CppObject header is 0x10 (klass + monitor)
const LENGTH_OFFSET: usize = 0x10;
const CHARS_OFFSET: usize = 0x14;

impl Il2CppString {
    // Returns null if the input contains interior NULs
    pub fn new(s: impl AsRef<str>) -> Self {
        let s = s.as_ref();
        
        if let Some(handle) = ascii_fast_new(s.as_bytes()) {
            return handle;
        }

        // Fallback, let il2cpp handle the UTF-8 to UTF-16 conversion
        let c = match std::ffi::CString::new(s) {
            Ok(c) => c,
            Err(_) => return Self::null(),
        };

        unsafe { crate::il2cpp::api::string_new(c.as_bytes_with_nul().as_ptr()) }
    }

    #[inline]
    pub fn null() -> Self {
        Self(IlInstance::null())
    }

    #[inline]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn len(self) -> usize {
        assert!(!self.is_null(), "Il2CppString::len on null");
        unsafe { *(self.0.field_ptr(LENGTH_OFFSET) as *const i32) as usize }
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn chars(self) -> &'static [u16] {
        if self.is_null() {
            return &[];
        }

        let len = self.len();

        if len == 0 {
            return &[];
        }
        
        let ptr = self.0.field_ptr(CHARS_OFFSET) as *const u16;
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    pub fn to_rust_string(self) -> String {
        let chars = self.chars();

        // Fast path, NEON narrows u16 to u8 when every char is ASCII
        if let Some(bytes) = utf16_ascii_to_bytes(chars) {
            // ascii_to_bytes only returns Some when every byte is <= 0x7F, which is valid UTF-8
            return unsafe { String::from_utf8_unchecked(bytes) };
        }

        String::from_utf16_lossy(chars)
    }
}

impl Display for Il2CppString {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            return f.write_str("<null>");
        }
        for c in std::char::decode_utf16(self.chars().iter().copied()) {
            f.write_char(c.unwrap_or('\u{fffd}'))?;
        }
        Ok(())
    }
}

impl PartialEq for Il2CppString {
    fn eq(&self, other: &Self) -> bool {
        let a_null = self.is_null();
        let b_null = other.is_null();
        if a_null || b_null {
            return a_null == b_null;
        }
        self.chars() == other.chars()
    }
}

impl Eq for Il2CppString {}

impl From<&str> for Il2CppString {
    fn from(s: &str) -> Self {
        Il2CppString::new(s)
    }
}

impl From<String> for Il2CppString {
    fn from(s: String) -> Self {
        Il2CppString::new(s.as_str())
    }
}

#[unity::methods]
impl Il2CppString {
    #[method(name = "StartsWith", args = 1)]
    fn starts_with_raw(self, value: Il2CppString) -> bool;

    #[method(name = "Contains", args = 1)]
    fn contains_raw(self, value: Il2CppString) -> bool;

    #[method(name = "Replace", args = 2)]
    fn replace_raw(self, old_value: Il2CppString, new_value: Il2CppString) -> Il2CppString;

    #[method(name = "ToLower", args = 0)]
    fn to_lowercase(self) -> Il2CppString;

    #[method(name = "GetHashCode", args = 0)]
    fn get_hash_code(self) -> i32;
}

impl Il2CppString {
    #[inline]
    pub fn starts_with<S: Into<Il2CppString>>(self, value: S) -> bool {
        self.starts_with_raw(value.into())
    }

    #[inline]
    pub fn contains<S: Into<Il2CppString>>(self, value: S) -> bool {
        self.contains_raw(value.into())
    }

    #[inline]
    pub fn replace<A: Into<Il2CppString>, B: Into<Il2CppString>>(
        self,
        old_value: A,
        new_value: B,
    ) -> Il2CppString {
        self.replace_raw(old_value.into(), new_value.into())
    }
}

fn ascii_fast_new(bytes: &[u8]) -> Option<Il2CppString> {
    if bytes.is_empty() {
        return None;
    }
    if !ascii_check_neon(bytes) {
        return None;
    }

    let handle = unsafe { crate::il2cpp::api::string_new_size(bytes.len() as i32, ::core::option::Option::None) };

    if handle.is_null() {
        return None;
    }

    let dst = handle.0.field_ptr(CHARS_OFFSET) as *mut u16;
    unsafe { ascii_bytes_to_utf16_into(bytes, dst) };

    Some(handle)
}

fn ascii_check_neon(bytes: &[u8]) -> bool {
    let len = bytes.len();
    let mut i = 0;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        let src = bytes.as_ptr();
        while i + 16 <= len {
            let chunk = vld1q_u8(src.add(i));
            let hi = vcgtq_u8(chunk, vdupq_n_u8(0x7F));
            if vmaxvq_u8(hi) != 0 {
                return false;
            }
            i += 16;
        }
    }

    while i < len {
        if bytes[i] > 0x7F {
            return false;
        }
        i += 1;
    }
    true
}

unsafe fn ascii_bytes_to_utf16_into(bytes: &[u8], dst: *mut u16) {
    let len = bytes.len();
    let src = bytes.as_ptr();
    let mut i = 0;

    #[cfg(target_arch = "aarch64")]
    {
        use core::arch::aarch64::*;
        while i + 16 <= len {
            let a = vld1_u8(src.add(i));
            let b = vld1_u8(src.add(i + 8));
            vst1q_u16(dst.add(i), vmovl_u8(a));
            vst1q_u16(dst.add(i + 8), vmovl_u8(b));
            i += 16;
        }
    }

    while i < len {
        *dst.add(i) = *src.add(i) as u16;
        i += 1;
    }
}

fn utf16_ascii_to_bytes(input: &[u16]) -> Option<Vec<u8>> {
    let len = input.len();
    let mut out: Vec<u8> = Vec::with_capacity(len);
    let dst = out.as_mut_ptr();
    let src = input.as_ptr();
    let mut i = 0;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        while i + 8 <= len {
            let chunk = vld1q_u16(src.add(i));
            let hi = vcgtq_u16(chunk, vdupq_n_u16(0x7F));
            if vmaxvq_u16(hi) != 0 {
                return None;
            }
            vst1_u8(dst.add(i), vmovn_u16(chunk));
            i += 8;
        }
    }

    while i < len {
        let c = unsafe { *src.add(i) };
        if c > 0x7F {
            return None;
        }
        unsafe { *dst.add(i) = c as u8 };
        i += 1;
    }

    unsafe { out.set_len(len) };
    Some(out)
}

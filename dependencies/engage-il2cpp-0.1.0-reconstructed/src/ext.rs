// The extension traits/iterators here mirror the generated `List_1` / `Dictionary_2` names on
// purpose, so they carry the same non-camel-case `_1`/`_2`.
#![allow(non_camel_case_types)]

use unity::{IntPtr, MethodInfo};

fn method_info_intptr(callback_ptr: *mut u8, args: u8) -> IntPtr {
    let mi = unity::method_info_for_fn(callback_ptr, args);
    IntPtr(mi as *const MethodInfo as *mut ())
}

#[cfg(feature = "system-collections-generic-list_1")]
mod list_1_ext {
    use crate::system::collections::generic::list_1::{IList_1Methods, List_1};

    pub trait List_1Ext<T: Copy + ::unity::ClassIdentity> {
        fn with_capacity(capacity: i32) -> Self;
        fn get(self, index: i32) -> T;
        fn iter(self) -> List_1Iter<T>;
        fn count(self) -> i32;
    }

    impl<T: Copy + ::unity::ClassIdentity> List_1Ext<T> for List_1<T> {
        fn with_capacity(capacity: i32) -> Self {
            let list = Self::new();
            list.ensure_capacity(capacity);
            list
        }

        fn get(self, index: i32) -> T {
            self.get_item(index)
        }

        fn iter(self) -> List_1Iter<T> {
            let len = self.get_count();
            List_1Iter { list: self, index: 0, len }
        }

        fn count(self) -> i32 {
            self.get_count()
        }
    }

    pub struct List_1Iter<T: Copy + ::unity::ClassIdentity> {
        list: List_1<T>,
        index: i32,
        len: i32,
    }

    impl<T: Copy + ::unity::ClassIdentity> Iterator for List_1Iter<T> {
        type Item = T;

        fn next(&mut self) -> Option<T> {
            if self.index < self.len {
                let v = self.list.get_item(self.index);
                self.index += 1;
                Some(v)
            } else {
                None
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            let r = (self.len - self.index) as usize;
            (r, Some(r))
        }
    }

    impl<T: Copy + ::unity::ClassIdentity> ExactSizeIterator for List_1Iter<T> {}
}
#[cfg(feature = "system-collections-generic-list_1")]
pub use list_1_ext::*;

#[cfg(feature = "system-collections-generic-dictionary_2")]
mod dictionary_2_ext {
    use ::unity::Array;

    use crate::system::collections::generic::dictionary_2::{
        Dictionary_2, IDictionary_2Methods, IDictionary_2_KeyCollectionMethods, IDictionary_2_ValueCollectionMethods,
    };

    pub trait Dictionary_2Ext<K: Copy + ::unity::ClassIdentity, V: Copy + ::unity::ClassIdentity> {
        fn iter(self) -> Dictionary_2Iter<K, V>;
    }

    impl<K: Copy + ::unity::ClassIdentity, V: Copy + ::unity::ClassIdentity> Dictionary_2Ext<K, V> for Dictionary_2<K, V> {
        fn iter(self) -> Dictionary_2Iter<K, V> {
            let len = self.get_count();
            let keys = Array::<K>::of_len(len as usize).expect("Dictionary_2Iter: keys alloc");
            let values = Array::<V>::of_len(len as usize).expect("Dictionary_2Iter: values alloc");
            self.get_keys().copy_to(keys, 0);
            self.get_values().copy_to(values, 0);
            Dictionary_2Iter { keys, values, index: 0, len }
        }
    }

    pub struct Dictionary_2Iter<K: Copy + ::unity::ClassIdentity, V: Copy + ::unity::ClassIdentity> {
        keys: Array<K>,
        values: Array<V>,
        index: i32,
        len: i32,
    }

    impl<K: Copy + ::unity::ClassIdentity, V: Copy + ::unity::ClassIdentity> Iterator for Dictionary_2Iter<K, V> {
        type Item = (K, V);

        fn next(&mut self) -> Option<(K, V)> {
            if self.index >= self.len {
                return None;
            }
            let i = self.index as usize;
            let pair = (self.keys.get(i), self.values.get(i));
            self.index += 1;
            Some(pair)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            let r = (self.len - self.index) as usize;
            (r, Some(r))
        }
    }

    impl<K: Copy + ::unity::ClassIdentity, V: Copy + ::unity::ClassIdentity> ExactSizeIterator for Dictionary_2Iter<K, V> {}
}
#[cfg(feature = "system-collections-generic-dictionary_2")]
pub use dictionary_2_ext::*;

#[cfg(all(
    feature = "app-gameuserdata",
    feature = "app-gamevariable",
    feature = "app-singletonclass_1",
    feature = "system-collections-generic-list_1",
))]
mod game_variable_manager {
    use unity::Il2CppString;

    use crate::{
        app::{
            gameuserdata::{GameUserData, IGameUserDataMethods},
            gamevariable::{GameVariable, IGameVariableMethods},
            singletonclass_1::ISingletonClass_1Methods,
        },
        system::collections::generic::list_1::List_1,
    };

    pub struct GameVariableManager;

    impl GameVariableManager {
        #[inline]
        fn variable() -> GameVariable {
            GameUserData::get_instance().get_variable()
        }

        pub fn get_bool(key: impl Into<Il2CppString>) -> bool {
            Self::variable().get_bool(key.into())
        }

        pub fn set_bool(key: impl Into<Il2CppString>, value: bool) {
            Self::variable().set(key.into(), value);
        }

        pub fn get_number(key: impl Into<Il2CppString>) -> i32 {
            Self::variable().get_number(key.into())
        }

        pub fn set_number(key: impl Into<Il2CppString>, value: i32) {
            Self::variable().set_number(key.into(), value);
        }

        pub fn get_string(key: impl Into<Il2CppString>) -> Il2CppString {
            Self::variable().get_string(key.into())
        }

        pub fn set_string(key: impl Into<Il2CppString>, value: impl Into<Il2CppString>) {
            Self::variable().set_string(key.into(), value.into());
        }

        pub fn make_entry(key: impl Into<Il2CppString>, num: i32) -> bool {
            Self::variable().entry(key.into(), num)
        }

        pub fn make_entry_norewind(key: impl Into<Il2CppString>, num: i32) -> bool {
            Self::variable().entry_no_rewind(key.into(), num)
        }

        pub fn is_exist(key: impl Into<Il2CppString>) -> bool {
            Self::variable().is_exist(key.into())
        }

        pub fn is_string(key: impl Into<Il2CppString>) -> bool {
            Self::variable().is_string(key.into())
        }

        pub fn remove(key: impl Into<Il2CppString>) -> bool {
            Self::variable().remove(key.into())
        }

        pub fn find_starts_with(prefix: impl Into<Il2CppString>) -> List_1<Il2CppString> {
            Self::variable().find_starts_with(prefix.into())
        }
    }
}
#[cfg(all(
    feature = "app-gameuserdata",
    feature = "app-gamevariable",
    feature = "app-singletonclass_1",
    feature = "system-collections-generic-list_1",
))]
pub use game_variable_manager::*;

#[cfg(feature = "app-mess")]
mod mess_ext {
    use unity::Il2CppString;

    pub struct Mess;

    impl Mess {
        pub fn get(label: impl Into<Il2CppString>) -> Il2CppString {
            crate::app::mess::Mess::get(label.into())
        }

        pub fn load(file_name: impl Into<Il2CppString>) -> bool {
            crate::app::mess::Mess::load(file_name.into())
        }

        pub fn try_load(file_name: impl Into<Il2CppString>) -> bool {
            crate::app::mess::Mess::try_load(file_name.into())
        }

        pub fn free(file_name: impl Into<Il2CppString>) -> bool {
            crate::app::mess::Mess::free(file_name.into())
        }

        pub fn try_free(file_name: impl Into<Il2CppString>) -> bool {
            crate::app::mess::Mess::try_free(file_name.into())
        }

        pub fn is_exist(label: impl Into<Il2CppString>) -> bool {
            crate::app::mess::Mess::is_exist(label.into())
        }

        pub fn is_load_done(file_name: impl Into<Il2CppString>) -> bool {
            crate::app::mess::Mess::is_load_done(file_name.into())
        }

        pub fn is_file_exist(file_name: impl Into<Il2CppString>) -> bool {
            crate::app::mess::Mess::is_file_exist(file_name.into())
        }

        pub fn get_language_directory_name() -> Il2CppString {
            crate::app::mess::Mess::get_language_directory_name()
        }

        pub fn get_file_path(label: impl Into<Il2CppString>) -> Il2CppString {
            crate::app::mess::Mess::get_file_path(label.into())
        }
    }
}
#[cfg(feature = "app-mess")] pub use mess_ext::*;

#[cfg(feature = "app-titlebar")]
mod title_bar_ext {
    use unity::Il2CppString;

    pub struct TitleBar;

    impl TitleBar {
        pub fn open_header(title: impl Into<Il2CppString>, title_help: impl Into<Il2CppString>, key_help_id: impl Into<Il2CppString>) -> bool {
            use crate::app::titlebar::{ITitleBarMethods, TitleBar as TB};
            TB::get_instance().open_header(title.into(), title_help.into(), key_help_id.into())
        }

        pub fn close_header() {
            use crate::app::titlebar::{ITitleBarMethods, TitleBar as TB};
            TB::get_instance().close_header();
        }
    }
}
#[cfg(feature = "app-titlebar")]
pub use title_bar_ext::*;

#[cfg(all(feature = "app-pad", feature = "app-vibrationmanager"))]
mod vibrate_ext {
    pub const FREQ_LOW: f32 = 160.0;
    pub const FREQ_HIGH: f32 = 300.0;

    pub fn vibrate(time: f32, amplitude_magnitude: f32, amp_low: f32, amp_high: f32, freq_low: f32, freq_high: f32) {
        use crate::app::{pad::Pad, vibrationmanager::IVibrationManagerMethods};
        Pad::vibration().one_shot_2(time, amplitude_magnitude, amp_low, amp_high, freq_low, freq_high);
    }
}
#[cfg(all(feature = "app-pad", feature = "app-vibrationmanager"))]
pub use vibrate_ext::*;

#[cfg(all(feature = "app-procvoidmethod", feature = "system-object"))]
mod proc_void_method_ext {
    use unity::{FromIlInstance, IlInstance, IntPtr, MethodInfo, OptionalMethod, SystemObject};

    use crate::{app::procvoidmethod::ProcVoidMethod, system::object::Object};

    pub trait ProcVoidMethodExt: Sized {
        /// Bind `target` and call `callback` on it. ProcVoidMethod's Invoke() takes no VM argument, so
        /// the bound target is the only object the callback ever sees, that's why it's required. The
        /// callback's first parameter is forced to the same type as `target`, so they can't drift.
        fn from_fn<T: crate::app::procinst::IProcInst>(target: T, callback: extern "C" fn(T, OptionalMethod)) -> Option<Self>;
        fn from_raw_parts(target: IlInstance, method_info: &'static MethodInfo) -> Option<Self>;
    }

    impl ProcVoidMethodExt for ProcVoidMethod {
        fn from_fn<T: crate::app::procinst::IProcInst>(target: T, callback: extern "C" fn(T, OptionalMethod)) -> Option<Self> {
            let intptr = super::method_info_intptr(callback as *mut u8, 0);
            Some(ProcVoidMethod::new(
                <Object as FromIlInstance>::from_il_instance(target.as_instance()),
                intptr,
            ))
        }

        fn from_raw_parts(target: IlInstance, method_info: &'static MethodInfo) -> Option<Self> {
            let intptr = IntPtr(method_info as *const MethodInfo as *mut ());
            Some(ProcVoidMethod::new(<Object as FromIlInstance>::from_il_instance(target), intptr))
        }
    }
}
#[cfg(all(feature = "app-procvoidmethod", feature = "system-object"))]
pub use proc_void_method_ext::*;

#[cfg(all(feature = "app-procvoidfunction", feature = "app-procinst", feature = "system-object",))]
mod proc_void_function_ext {
    use unity::{FromIlInstance, IlInstance, OptionalMethod, SystemObject};

    use crate::{app::procvoidfunction::ProcVoidFunction, system::object::Object};

    pub trait ProcVoidFunctionExt: Sized {
        /// Wrap a Rust fn as a ProcVoidFunction (for `Proc::call` steps). The proc running the desc is
        /// handed to the callback by the VM itself, so there's nothing to bind, the callback just
        /// receives that proc as its argument. Prefer this unless you need a bound object that differs
        /// from the running proc.
        fn from_fn<T: crate::app::procinst::IProcInst>(callback: extern "C" fn(T, OptionalMethod)) -> Option<Self>;

        /// Like `from_fn`, but also binds `target`. The callback receives `(target, inst, methodinfo)`
        /// where `inst` is the proc running the desc. Use this only when the object you want differs
        /// from the running proc. The target's type is tied to the callback's first parameter.
        fn from_fn_with_target<T: crate::app::procinst::IProcInst, U: crate::app::procinst::IProcInst>(
            target: T,
            callback: extern "C" fn(T, U, OptionalMethod),
        ) -> Option<Self>;
    }

    impl ProcVoidFunctionExt for ProcVoidFunction {
        fn from_fn<T: crate::app::procinst::IProcInst>(callback: extern "C" fn(T, OptionalMethod)) -> Option<Self> {
            // A null target plus parameters_count = 1 makes the delegate's Invoke(inst) call the
            // callback as method_ptr(inst, method_info), so its only argument is the executing proc.
            let intptr = super::method_info_intptr(callback as *mut u8, 1);
            Some(ProcVoidFunction::new(
                <Object as FromIlInstance>::from_il_instance(IlInstance::null()),
                intptr,
            ))
        }

        fn from_fn_with_target<T: crate::app::procinst::IProcInst, U: crate::app::procinst::IProcInst>(
            target: T,
            callback: extern "C" fn(T, U, OptionalMethod),
        ) -> Option<Self> {
            // parameters_count = 2 with a real target makes Invoke(inst) call
            // method_ptr(target, inst, method_info).
            let intptr = super::method_info_intptr(callback as *mut u8, 2);
            Some(ProcVoidFunction::new(
                <Object as FromIlInstance>::from_il_instance(target.as_instance()),
                intptr,
            ))
        }
    }
}
#[cfg(all(feature = "app-procvoidfunction", feature = "app-procinst", feature = "system-object",))]
pub use proc_void_function_ext::*;

#[cfg(all(feature = "app-procboolmethod", feature = "system-object"))]
mod proc_bool_method_ext {
    use unity::{FromIlInstance, OptionalMethod, SystemObject};

    use crate::{app::procboolmethod::ProcBoolMethod, system::object::Object};

    pub trait ProcBoolMethodExt: Sized {
        /// Bind `target` and call `callback` on it (returning bool). Like ProcVoidMethod, Invoke() takes
        /// no VM argument, so the bound target is the only object the callback sees. The callback's first
        /// parameter is forced to the same type as `target`.
        fn from_fn<T: crate::app::procinst::IProcInst>(target: T, callback: extern "C" fn(T, OptionalMethod) -> bool) -> Option<Self>;
    }

    impl ProcBoolMethodExt for ProcBoolMethod {
        fn from_fn<T: crate::app::procinst::IProcInst>(target: T, callback: extern "C" fn(T, OptionalMethod) -> bool) -> Option<Self> {
            let intptr = super::method_info_intptr(callback as *mut u8, 0);
            Some(ProcBoolMethod::new(
                <Object as FromIlInstance>::from_il_instance(target.as_instance()),
                intptr,
            ))
        }
    }
}
#[cfg(all(feature = "app-procboolmethod", feature = "system-object"))]
pub use proc_bool_method_ext::*;

#[cfg(all(
    feature = "app-proc",
    feature = "app-procvoidmethod",
    feature = "app-procvoidfunction",
    feature = "app-procdesc",
))]
mod proc_ext {
    use crate::app::{proc::Proc, procdesc::ProcDesc, procvoidfunction::ProcVoidFunction, procvoidmethod::ProcVoidMethod};

    pub trait ProcExt {
        fn call_method(method: ProcVoidMethod) -> ProcDesc;
        fn call_function(function: ProcVoidFunction) -> ProcDesc;
    }

    impl ProcExt for Proc {
        fn call_method(method: ProcVoidMethod) -> ProcDesc {
            Proc::call_2(method)
        }

        fn call_function(function: ProcVoidFunction) -> ProcDesc {
            Proc::call(function)
        }
    }
}
#[cfg(all(
    feature = "app-proc",
    feature = "app-procvoidmethod",
    feature = "app-procvoidfunction",
    feature = "app-procdesc",
))]
pub use proc_ext::*;

#[cfg(feature = "app-procdesc")]
mod proc_desc_patch {
    use unity::Array;

    use crate::app::procdesc::ProcDesc;

    pub struct ProcDescPatch {
        original: Vec<ProcDesc>,
        patches: Vec<(usize, Vec<ProcDesc>)>,
    }

    impl ProcDescPatch {
        pub fn new(original: Array<ProcDesc>) -> Self {
            Self {
                original: original.iter().collect(),
                patches: Vec::new(),
            }
        }

        pub fn insert(mut self, position: usize, descs: impl IntoIterator<Item = ProcDesc>) -> Self {
            self.patches.push((position, descs.into_iter().collect()));
            self
        }

        pub fn finish(mut self) -> Array<ProcDesc> {
            self.patches.sort_by_key(|patch| std::cmp::Reverse(patch.0));
            for (pos, descs) in self.patches {
                self.original.splice(pos..pos, descs);
            }
            Array::<ProcDesc>::from_slice(&self.original).expect("ProcDescPatch::finish: ProcDesc array allocation failed")
        }
    }
}
#[cfg(feature = "app-procdesc")]
pub use proc_desc_patch::*;

#[cfg(feature = "app-basicmenu")]
mod basic_menu_result {
    use crate::app::basicmenu::BasicMenu_Result;

    #[repr(transparent)]
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub struct BasicMenuResult(pub BasicMenu_Result);

    impl Default for BasicMenuResult {
        fn default() -> Self {
            Self::new()
        }
    }

    impl BasicMenuResult {
        const CLOSE_ALL: i32 = 1 << 2;
        const CLOSE_PARENT: i32 = 1 << 1;
        const CLOSE_THIS: i32 = 1 << 0;
        const DELETE_ALL: i32 = 1 << 5;
        const DELETE_PARENT: i32 = 1 << 4;
        const DELETE_THIS: i32 = 1 << 3;
        const DO_NOTHING: i32 = 1 << 13;
        const SE_CANCEL: i32 = 1 << 9;
        const SE_CURSOR: i32 = 1 << 12;
        const SE_DECIDE: i32 = 1 << 7;
        const SE_DECIDE2: i32 = 1 << 8;
        const SE_MISS: i32 = 1 << 11;

        #[inline]
        pub const fn new() -> Self {
            Self(BasicMenu_Result { value: 0 })
        }

        #[inline]
        pub const fn bits(self) -> i32 {
            self.0.value
        }

        fn set(mut self, mask: i32, v: bool) -> Self {
            if v {
                self.0.value |= mask;
            } else {
                self.0.value &= !mask;
            }
            self
        }

        pub const fn close_this(self) -> bool {
            self.0.value & Self::CLOSE_THIS != 0
        }

        pub const fn close_parent(self) -> bool {
            self.0.value & Self::CLOSE_PARENT != 0
        }

        pub const fn close_all(self) -> bool {
            self.0.value & Self::CLOSE_ALL != 0
        }

        pub const fn delete_this(self) -> bool {
            self.0.value & Self::DELETE_THIS != 0
        }

        pub const fn delete_parent(self) -> bool {
            self.0.value & Self::DELETE_PARENT != 0
        }

        pub const fn delete_all(self) -> bool {
            self.0.value & Self::DELETE_ALL != 0
        }

        pub const fn do_nothing(self) -> bool {
            self.0.value & Self::DO_NOTHING != 0
        }

        pub fn with_close_this(self, v: bool) -> Self {
            self.set(Self::CLOSE_THIS, v)
        }

        pub fn with_close_parent(self, v: bool) -> Self {
            self.set(Self::CLOSE_PARENT, v)
        }

        pub fn with_close_all(self, v: bool) -> Self {
            self.set(Self::CLOSE_ALL, v)
        }

        pub fn with_delete_this(self, v: bool) -> Self {
            self.set(Self::DELETE_THIS, v)
        }

        pub fn with_delete_parent(self, v: bool) -> Self {
            self.set(Self::DELETE_PARENT, v)
        }

        pub fn with_delete_all(self, v: bool) -> Self {
            self.set(Self::DELETE_ALL, v)
        }

        pub fn with_se_decide(self, v: bool) -> Self {
            self.set(Self::SE_DECIDE, v)
        }

        pub fn with_se_decide2(self, v: bool) -> Self {
            self.set(Self::SE_DECIDE2, v)
        }

        pub fn with_se_cancel(self, v: bool) -> Self {
            self.set(Self::SE_CANCEL, v)
        }

        pub fn with_se_miss(self, v: bool) -> Self {
            self.set(Self::SE_MISS, v)
        }

        pub fn with_se_cursor(self, v: bool) -> Self {
            self.set(Self::SE_CURSOR, v)
        }

        pub fn with_do_nothing(self, v: bool) -> Self {
            self.set(Self::DO_NOTHING, v)
        }

        pub fn se_cursor() -> Self {
            Self::new().with_se_cursor(true)
        }

        pub fn se_decide() -> Self {
            Self::new().with_se_decide(true)
        }

        pub fn close_decide() -> Self {
            Self::new().with_close_this(true).with_se_decide(true)
        }

        pub fn se_miss() -> Self {
            Self::new().with_se_miss(true)
        }

        pub fn close_parent_decide() -> Self {
            Self::new().with_close_parent(true).with_se_decide(true)
        }

        pub fn delete_decide() -> Self {
            Self::new().with_delete_this(true).with_se_decide(true)
        }

        pub fn close_cancel() -> Self {
            Self::new().with_se_cancel(true).with_close_this(true)
        }
    }

    impl ::unity::ClassIdentity for BasicMenuResult {
        const NAME: &'static str = <BasicMenu_Result as ::unity::ClassIdentity>::NAME;
        const NAMESPACE: &'static str = <BasicMenu_Result as ::unity::ClassIdentity>::NAMESPACE;

        fn class() -> ::unity::Class {
            <BasicMenu_Result as ::unity::ClassIdentity>::class()
        }
    }

    impl ::unity::IlType for BasicMenuResult {
        fn il_type() -> &'static ::unity::il2cpp::Il2CppType {
            <BasicMenu_Result as ::unity::IlType>::il_type()
        }
    }
}
#[cfg(feature = "app-basicmenu")]
pub use basic_menu_result::*;

#[cfg(all(feature = "app-basicmenuitem", feature = "app-basicmenu"))]
mod basic_menu_item_ext {
    use unity::{Cast, Il2CppString, OptionalMethod};

    use super::BasicMenuResult;
    pub use crate::app::basicmenuitem::BasicMenuItem_Attribute as BasicMenuItemAttribute;
    use crate::app::basicmenuitem::{BasicMenuItem, IBasicMenuItemMethods};

    pub trait BasicMenuItemMethods {
        extern "C" fn get_name(this: BasicMenuItem, method_info: OptionalMethod) -> Il2CppString;
        extern "C" fn a_call(_this: BasicMenuItem, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new()
        }
        extern "C" fn b_call(_this: BasicMenuItem, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new().with_close_this(true).with_se_cancel(true)
        }
        extern "C" fn build_attribute(_this: BasicMenuItem, _method_info: OptionalMethod) -> BasicMenuItemAttribute {
            BasicMenuItemAttribute::enable()
        }
    }

    pub trait BasicMenuItemExt: Sized {
        fn new_default() -> Self;
        fn new_impl<M: BasicMenuItemMethods>() -> Self;
        fn new_impl_from_template<M: BasicMenuItemMethods>(template: BasicMenuItem) -> Self;
    }

    impl BasicMenuItemExt for BasicMenuItem {
        fn new_default() -> Self {
            let item = <Self as ::unity::FromIlInstance>::instantiate().expect("BasicMenuItem::new_default allocation failed");
            <Self as IBasicMenuItemMethods>::ctor(item);
            item
        }

        fn new_impl<M: BasicMenuItemMethods>() -> Self {
            let item = Self::new_default();
            let class = item.override_class();
            class.override_virtual_method("GetName", ::unity::method_info!(M::get_name, 0));
            class.override_virtual_method("ACall", ::unity::method_info!(M::a_call, 0));
            class.override_virtual_method("BCall", ::unity::method_info!(M::b_call, 0));
            class.override_virtual_method("BuildAttribute", ::unity::method_info!(M::build_attribute, 0));
            item
        }

        fn new_impl_from_template<M: BasicMenuItemMethods>(template: BasicMenuItem) -> Self {
            use ::unity::FromIlInstance;
            let cloned_class = template.get_class().clone_for_override();
            let item =
                <Self as FromIlInstance>::instantiate_with_class(cloned_class).expect("BasicMenuItem::new_impl_from_template allocation failed");
            <Self as IBasicMenuItemMethods>::ctor(item);
            cloned_class.override_virtual_method("GetName", ::unity::method_info!(M::get_name, 0));
            cloned_class.override_virtual_method("ACall", ::unity::method_info!(M::a_call, 0));
            cloned_class.override_virtual_method("BCall", ::unity::method_info!(M::b_call, 0));
            cloned_class.override_virtual_method("BuildAttribute", ::unity::method_info!(M::build_attribute, 0));
            item
        }
    }
}
#[cfg(all(feature = "app-basicmenuitem", feature = "app-basicmenu"))]
pub use basic_menu_item_ext::*;

#[cfg(all(
    feature = "app-basicmenu",
    feature = "app-basicmenucontent",
    feature = "app-basicmenuitem",
    feature = "app-procdesc",
    feature = "system-collections-generic-list_1",
))]
mod basic_menu_ext {
    use unity::Array;

    use crate::{
        app::{
            basicmenu::{BasicMenu, IBasicMenu, IBasicMenuMethods},
            basicmenucontent::BasicMenuContent,
            basicmenuitem::BasicMenuItem,
            procdesc::ProcDesc,
        },
        system::collections::generic::list_1::{IList_1Methods, List_1},
    };

    pub trait BasicMenuExt: Sized {
        fn add_item(self, item: impl Into<BasicMenuItem>);
        fn close_anime_all(self);
        fn close_anime(self);
        fn open_anime_all(self);
        fn open_anime(self);
        fn create_default_desc(self) -> Array<ProcDesc>;
        fn build(menu_item_list: List_1<BasicMenuItem>, menu_content: BasicMenuContent) -> Self;
    }

    fn basic_menu_virtual_call_0(this: BasicMenu, method_name: &str) {
        let class = ::unity::object_get_class(this);
        let entry = class
            .get_virtual_method(method_name)
            .unwrap_or_else(|| panic!("BasicMenu vtable missing `{}`", method_name));
        let f: extern "C" fn(BasicMenu, &'static ::unity::MethodInfo) = unsafe { ::core::mem::transmute(entry.method_ptr) };
        f(this, entry.method_info)
    }

    impl BasicMenuExt for BasicMenu {
        fn add_item(self, item: impl Into<BasicMenuItem>) {
            self.m_full_menu_item_list().add(item.into());
        }

        fn close_anime_all(self) {
            basic_menu_virtual_call_0(self, "CloseAnimeAll");
        }

        fn close_anime(self) {
            basic_menu_virtual_call_0(self, "CloseAnime");
        }

        fn open_anime_all(self) {
            basic_menu_virtual_call_0(self, "OpenAnimeAll");
        }

        fn open_anime(self) {
            basic_menu_virtual_call_0(self, "OpenAnime");
        }

        fn create_default_desc(self) -> Array<ProcDesc> {
            let class = ::unity::object_get_class(self);
            let entry = class
                .get_virtual_method("CreateDefaultDesc")
                .expect("BasicMenu vtable missing `CreateDefaultDesc`");
            let f: extern "C" fn(BasicMenu, &'static ::unity::MethodInfo) -> Array<ProcDesc> = unsafe { ::core::mem::transmute(entry.method_ptr) };
            f(self, entry.method_info)
        }

        fn build(menu_item_list: List_1<BasicMenuItem>, menu_content: BasicMenuContent) -> Self {
            let menu = <Self as ::unity::FromIlInstance>::instantiate().expect("BasicMenu::build allocation failed");
            <Self as IBasicMenuMethods>::ctor(menu, menu_item_list, menu_content);
            menu
        }
    }
}
#[cfg(all(
    feature = "app-basicmenu",
    feature = "app-basicmenucontent",
    feature = "app-basicmenuitem",
    feature = "app-procdesc",
    feature = "system-collections-generic-list_1",
))]
pub use basic_menu_ext::*;

#[cfg(all(feature = "app-soundmanager", feature = "app-soundsystem", feature = "system-collections-generic-list_1",))]
mod sound_manager_ext {
    use unity::{Cast, Il2CppString, MethodInfo};

    use crate::{
        app::{
            soundmanager::{ISoundManager, SoundManager},
            soundsystem::SoundSystem_SoundHandle,
        },
        system::collections::generic::list_1::IList_1Methods,
    };

    pub trait SoundManagerExt: Sized {
        fn is_event_playing_with_prefix(self, prefix: impl AsRef<str>) -> bool;
    }

    fn sound_handle_event_name(handle: SoundSystem_SoundHandle) -> Il2CppString {
        let class = ::unity::object_get_class(handle);
        let entry = class
            .get_virtual_method("GetEventName")
            .expect("SoundSystem.SoundHandle vtable missing `GetEventName`");
        let f: extern "C" fn(SoundSystem_SoundHandle, &'static MethodInfo) -> Il2CppString = unsafe { ::core::mem::transmute(entry.method_ptr) };
        f(handle, entry.method_info)
    }

    impl SoundManagerExt for SoundManager {
        fn is_event_playing_with_prefix(self, prefix: impl AsRef<str>) -> bool {
            let prefix = prefix.as_ref();
            let list = self.m_sound_handle_list();
            let count = list.get_count();
            (0..count).any(|i| {
                let handle = list.get_item(i);
                !handle.is_null() && sound_handle_event_name(handle).to_string().starts_with(prefix)
            })
        }
    }
}
#[cfg(all(feature = "app-soundmanager", feature = "app-soundsystem", feature = "system-collections-generic-list_1",))]
pub use sound_manager_ext::*;

#[cfg(all(feature = "app-force", feature = "app-unit"))]
mod force_ext {
    use unity::Cast;

    use crate::app::{
        force::{Force, IForceMethods},
        unit::{IUnitMethods, Unit},
    };

    pub trait ForceExt: Sized {
        fn iter(self) -> ForceIter;
    }

    impl ForceExt for Force {
        fn iter(self) -> ForceIter {
            ForceIter { current: self.get_first() }
        }
    }

    pub struct ForceIter {
        current: Unit,
    }

    impl Iterator for ForceIter {
        type Item = Unit;

        fn next(&mut self) -> Option<Unit> {
            if self.current.is_null() {
                return None;
            }
            let unit = self.current;
            self.current = unit.get_next();
            Some(unit)
        }
    }
}
#[cfg(all(feature = "app-force", feature = "app-unit"))]
pub use force_ext::*;

#[cfg(all(feature = "root-configbasicmenuitem", feature = "app-basicmenuitem", feature = "app-basicmenu",))]
mod config_basic_menu_item_ext {
    use unity::{Cast, Il2CppString, OptionalMethod};

    use super::{BasicMenuItemAttribute, BasicMenuResult};
    use crate::root::configbasicmenuitem::{
        ConfigBasicMenuItem, ConfigBasicMenuItem_ConfigMethodKind, IConfigBasicMenuItem, IConfigBasicMenuItemMethods,
    };

    pub trait ConfigBasicMenuItemSwitchMethods {
        fn init_content(_this: ConfigBasicMenuItem) {}
        extern "C" fn custom_call(this: ConfigBasicMenuItem, method_info: OptionalMethod) -> BasicMenuResult;
        extern "C" fn set_command_text(this: ConfigBasicMenuItem, method_info: OptionalMethod);
        extern "C" fn set_help_text(this: ConfigBasicMenuItem, method_info: OptionalMethod);
        extern "C" fn a_call(_this: ConfigBasicMenuItem, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new()
        }
        extern "C" fn build_attribute(_this: ConfigBasicMenuItem, _method_info: OptionalMethod) -> BasicMenuItemAttribute {
            BasicMenuItemAttribute::enable()
        }
    }

    pub trait ConfigBasicMenuItemCommandMethods {
        fn init_content(_this: ConfigBasicMenuItem) {}
        extern "C" fn custom_call(this: ConfigBasicMenuItem, method_info: OptionalMethod) -> BasicMenuResult;
        extern "C" fn set_command_text(this: ConfigBasicMenuItem, method_info: OptionalMethod);
        extern "C" fn set_help_text(this: ConfigBasicMenuItem, method_info: OptionalMethod);
        extern "C" fn on_select(this: ConfigBasicMenuItem, _method_info: OptionalMethod) {
            <ConfigBasicMenuItem as IConfigBasicMenuItemMethods>::on_select(this);
            this.set_m_is_arrow(false);
            <ConfigBasicMenuItem as IConfigBasicMenuItemMethods>::on_deselect(this);
        }
        extern "C" fn on_deselect(this: ConfigBasicMenuItem, _method_info: OptionalMethod) {
            <ConfigBasicMenuItem as IConfigBasicMenuItemMethods>::on_select(this);
            this.set_m_is_arrow(false);
            <ConfigBasicMenuItem as IConfigBasicMenuItemMethods>::on_deselect(this);
        }
        extern "C" fn a_call(_this: ConfigBasicMenuItem, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new()
        }
        extern "C" fn build_attribute(_this: ConfigBasicMenuItem, _method_info: OptionalMethod) -> BasicMenuItemAttribute {
            BasicMenuItemAttribute::enable()
        }
    }

    pub trait ConfigBasicMenuItemGaugeMethods {
        fn init_content(_this: ConfigBasicMenuItem) {}
        extern "C" fn custom_call(this: ConfigBasicMenuItem, method_info: OptionalMethod) -> BasicMenuResult;
        extern "C" fn set_help_text(this: ConfigBasicMenuItem, method_info: OptionalMethod);
        extern "C" fn a_call(_this: ConfigBasicMenuItem, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new()
        }
        extern "C" fn build_attribute(_this: ConfigBasicMenuItem, _method_info: OptionalMethod) -> BasicMenuItemAttribute {
            BasicMenuItemAttribute::enable()
        }
    }

    pub trait ConfigBasicMenuItemExt: Sized {
        fn new_switch<M: ConfigBasicMenuItemSwitchMethods>(title: impl Into<Il2CppString>) -> Self;
        fn new_command<M: ConfigBasicMenuItemCommandMethods>(title: impl Into<Il2CppString>) -> Self;
        fn new_gauge<M: ConfigBasicMenuItemGaugeMethods>(title: impl Into<Il2CppString>) -> Self;
        fn change_key_value_b(value: bool) -> bool;
    }

    impl ConfigBasicMenuItemExt for ConfigBasicMenuItem {
        fn new_switch<M: ConfigBasicMenuItemSwitchMethods>(title: impl Into<Il2CppString>) -> Self {
            let item = ConfigBasicMenuItem::new();
            M::init_content(item);

            item.set_m_config_method(ConfigBasicMenuItem_ConfigMethodKind::switch());
            let class = item.override_class();
            class.override_virtual_method("CustomCall", ::unity::method_info!(M::custom_call, 0));
            class.override_virtual_method("ACall", ::unity::method_info!(M::a_call, 0));
            class.override_virtual_method("BuildAttribute", ::unity::method_info!(M::build_attribute, 0));

            item.set_title_text(title.into());
            M::set_command_text(item, None);
            M::set_help_text(item, None);

            item
        }

        fn new_gauge<M: ConfigBasicMenuItemGaugeMethods>(title: impl Into<Il2CppString>) -> Self {
            let item = ConfigBasicMenuItem::new();
            M::init_content(item);

            item.set_m_config_method(ConfigBasicMenuItem_ConfigMethodKind::gauge());
            let class = item.override_class();
            class.override_virtual_method("CustomCall", ::unity::method_info!(M::custom_call, 0));
            class.override_virtual_method("ACall", ::unity::method_info!(M::a_call, 0));
            class.override_virtual_method("BuildAttribute", ::unity::method_info!(M::build_attribute, 0));

            item.set_title_text(title.into());
            M::set_help_text(item, None);

            item
        }

        fn new_command<M: ConfigBasicMenuItemCommandMethods>(title: impl Into<Il2CppString>) -> Self {
            let item = ConfigBasicMenuItem::new();
            M::init_content(item);

            item.set_m_config_method(ConfigBasicMenuItem_ConfigMethodKind::switch());
            item.set_m_is_arrow(false);
            item.set_m_is_command_icon(true);
            let class = item.override_class();
            class.override_virtual_method("CustomCall", ::unity::method_info!(M::custom_call, 0));
            class.override_virtual_method("OnSelect", ::unity::method_info!(M::on_select, 0));
            class.override_virtual_method("OnDeselect", ::unity::method_info!(M::on_deselect, 0));
            class.override_virtual_method("ACall", ::unity::method_info!(M::a_call, 0));
            class.override_virtual_method("BuildAttribute", ::unity::method_info!(M::build_attribute, 0));

            item.set_title_text(title.into());
            M::set_command_text(item, None);
            M::set_help_text(item, None);

            item
        }

        fn change_key_value_b(value: bool) -> bool {
            ConfigBasicMenuItem::change_key_value(value as i32, 0, 1, 1) == 1
        }
    }
}
#[cfg(all(feature = "root-configbasicmenuitem", feature = "app-basicmenuitem", feature = "app-basicmenu",))]
pub use config_basic_menu_item_ext::*;

#[cfg(feature = "app-procinst")]
mod restore_parent_on_dispose_ext {
    use unity::{Cast, OptionalMethod};

    pub extern "C" fn restore_parent_on_dispose(this: crate::app::procinst::ProcInst, _method_info: OptionalMethod) {
        use crate::app::procinst::IProcInstMethods;
        let parent = this.get_super();
        if parent.is_null() {
            return;
        }
        if let Some(slot) = parent.get_class().raw().get_virtual_method("OpenAnimeAll") {
            let f: extern "C" fn(crate::app::procinst::ProcInst, &'static ::unity::MethodInfo) = unsafe { ::core::mem::transmute(slot.method_ptr) };
            f(parent, slot.method_info);
        }
    }
}
#[cfg(feature = "app-procinst")]
pub use restore_parent_on_dispose_ext::*;

#[cfg(all(
    feature = "app-basicdialog",
    feature = "app-yesmenuitem",
    feature = "app-basicdialogitem",
    feature = "app-basicdialogitemno",
    feature = "app-basicmenuitem",
    feature = "system-collections-generic-list_1",
    feature = "app-procinst",
    feature = "system-action",
))]
mod basic_menu_confirm_ext {
    use unity::Il2CppString;

    use crate::{
        app::{
            basicdialog::{BasicDialog, IBasicDialogMethods},
            basicdialogitemno::{BasicDialogItemNo, IBasicDialogItemNoMethods},
            basicmenuitem::BasicMenuItem,
            yesmenuitem::{IYesMenuItemMethods, YesMenuItem},
        },
        system::{
            action::Action,
            collections::generic::list_1::{IList_1Methods, List_1},
        },
    };

    pub fn basic_menu_confirm(
        proc: impl Into<crate::app::procinst::ProcInst>,
        message: impl Into<Il2CppString>,
        yes_text: impl Into<Il2CppString>,
        no_text: impl Into<Il2CppString>,
        handler: Action,
    ) -> BasicDialog {
        use crate::app::basicdialogitem::IBasicDialogItem;
        let yes_item = <YesMenuItem as ::unity::FromIlInstance>::instantiate().expect("YesMenuItem allocation failed");
        <YesMenuItem as IYesMenuItemMethods>::ctor(yes_item, handler);
        yes_item.set_m_text(yes_text.into());

        let no_item = <BasicDialogItemNo as ::unity::FromIlInstance>::instantiate().expect("BasicDialogItemNo allocation failed");
        <BasicDialogItemNo as IBasicDialogItemNoMethods>::ctor_2(no_item, no_text.into());

        let items = List_1::<BasicMenuItem>::new();
        items.add(yes_item.into());
        items.add(no_item.into());

        let dialog = BasicDialog::create_basic_dialog_bind(proc.into(), items);
        dialog.set_text(message.into());
        dialog
    }
}
#[cfg(all(
    feature = "app-basicdialog",
    feature = "app-yesmenuitem",
    feature = "app-basicdialogitem",
    feature = "app-basicdialogitemno",
    feature = "app-basicmenuitem",
    feature = "system-collections-generic-list_1",
    feature = "app-procinst",
    feature = "system-action",
))]
pub use basic_menu_confirm_ext::*;

#[cfg(all(
    feature = "app-yesnodialog",
    feature = "app-basicdialogitemyes",
    feature = "app-basicdialogitemno",
    feature = "app-procinst",
    feature = "app-basicmenu",
))]
mod yes_no_dialog_ext {
    use unity::{Cast, Il2CppString, OptionalMethod};

    use super::BasicMenuResult;
    use crate::app::{
        basicdialogitemno::{BasicDialogItemNo, IBasicDialogItemNoMethods},
        basicdialogitemyes::{BasicDialogItemYes, IBasicDialogItemYesMethods},
        yesnodialog::YesNoDialog,
    };

    pub trait TwoChoiceDialogMethods {
        extern "C" fn on_first_choice(_this: BasicDialogItemYes, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new().with_close_this(true)
        }
        extern "C" fn on_second_choice(_this: BasicDialogItemNo, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new().with_close_this(true)
        }
        extern "C" fn bcall_first(_this: BasicDialogItemYes, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new().with_close_this(true).with_se_cancel(true)
        }
        extern "C" fn bcall_second(_this: BasicDialogItemNo, _method_info: OptionalMethod) -> BasicMenuResult {
            BasicMenuResult::new().with_close_this(true).with_se_cancel(true)
        }
    }

    pub trait YesNoDialogExt {
        fn bind_with<Methods: TwoChoiceDialogMethods>(
            proc: impl Into<crate::app::procinst::ProcInst>,
            message: impl Into<Il2CppString>,
            first_text: impl Into<Il2CppString>,
            second_text: impl Into<Il2CppString>,
        );
    }

    impl YesNoDialogExt for YesNoDialog {
        fn bind_with<Methods: TwoChoiceDialogMethods>(
            proc: impl Into<crate::app::procinst::ProcInst>,
            message: impl Into<Il2CppString>,
            first_text: impl Into<Il2CppString>,
            second_text: impl Into<Il2CppString>,
        ) {
            let yes = <BasicDialogItemYes as ::unity::FromIlInstance>::instantiate().expect("BasicDialogItemYes allocation failed");
            <BasicDialogItemYes as IBasicDialogItemYesMethods>::ctor_2(yes, first_text.into());
            let no = <BasicDialogItemNo as ::unity::FromIlInstance>::instantiate().expect("BasicDialogItemNo allocation failed");
            <BasicDialogItemNo as IBasicDialogItemNoMethods>::ctor_2(no, second_text.into());

            let yes_class = yes.override_class();
            let no_class = no.override_class();

            let on_first = ::unity::method_info!(Methods::on_first_choice, 0);
            let bcall_first = ::unity::method_info!(Methods::bcall_first, 0);
            let on_second = ::unity::method_info!(Methods::on_second_choice, 0);
            let bcall_second = ::unity::method_info!(Methods::bcall_second, 0);

            yes_class.override_virtual_method("ACall", on_first);
            yes_class.override_virtual_method("BCall", bcall_first);
            no_class.override_virtual_method("ACall", on_second);
            no_class.override_virtual_method("BCall", bcall_second);

            let _ = YesNoDialog::create_bind(proc.into(), message.into(), yes, no);
        }
    }
}
#[cfg(all(
    feature = "app-yesnodialog",
    feature = "app-basicdialogitemyes",
    feature = "app-basicdialogitemno",
    feature = "app-procinst",
    feature = "app-basicmenu",
))]
pub use yes_no_dialog_ext::*;

#[cfg(feature = "app-filedata")]
mod file_handle_ext {
    use unity::MethodInfo;

    #[::unity::class(namespace = "App", name = "FileHandle`1")]
    pub struct FileHandle {
        #[rename(name = "m_Data")]
        pub m_data: crate::app::filedata::FileData,
    }

    impl FileHandle {
        pub fn unload(self) {
            let class = ::unity::object_get_class(self);
            let method = class
                .get_method_from_name("Unload", 0)
                .expect("FileHandle::Unload missing from runtime class");
            let unload: extern "C" fn(Self, &MethodInfo) = unsafe { ::core::mem::transmute(method.method_ptr) };
            unload(self, method);
        }
    }
}
#[cfg(feature = "app-filedata")]
pub use file_handle_ext::*;

#[cfg(all(
    feature = "app-itemdata",
    feature = "app-scriptutil",
    feature = "moon_sharp-interpreter-dynvalue",
    feature = "app-unit",
))]
mod dyn_value_args_ext {
    use unity::{Array, FromIlInstance, Il2CppString, IlInstance};

    use crate::{
        app::{itemdata::ItemData, scriptutil::ScriptUtil, unit::Unit},
        moon_sharp::interpreter::dynvalue::DynValue,
    };

    pub trait DynValueArgs {
        fn try_get_i32(self, index: i32) -> i32;
        fn try_get_string(self, index: i32) -> Il2CppString;
        fn try_get_unit(self, index: i32) -> Unit;
        fn try_get_item(self, index: i32) -> ItemData;
    }

    impl DynValueArgs for Array<DynValue> {
        fn try_get_i32(self, index: i32) -> i32 {
            ScriptUtil::try_get_int(self, index, i32::MAX)
        }

        fn try_get_string(self, index: i32) -> Il2CppString {
            ScriptUtil::try_get_string(self, index, Il2CppString::from_il_instance(IlInstance::null()))
        }

        fn try_get_unit(self, index: i32) -> Unit {
            ScriptUtil::try_get_unit(self, index, true)
        }

        fn try_get_item(self, index: i32) -> ItemData {
            ScriptUtil::try_get_item(self, index, true)
        }
    }
}
#[cfg(all(
    feature = "app-itemdata",
    feature = "app-scriptutil",
    feature = "moon_sharp-interpreter-dynvalue",
    feature = "app-unit",
))]
pub use dyn_value_args_ext::*;

#[cfg(all(feature = "app-eventscript", feature = "moon_sharp-interpreter-dynvalue"))]
mod event_script_ext {
    use unity::{Array, Class, FromIlInstance, Il2CppString, IlInstance, IntPtr, MethodInfo, OptionalMethod};

    use crate::{
        app::eventscript::{EventScript, EventScript_ActionArgs, EventScript_FunctionArgs, IEventScriptMethods},
        moon_sharp::interpreter::dynvalue::DynValue,
    };

    #[::unity::class(namespace = "App", name = "EventScript.ActionArgs")]
    pub struct EventScriptActionArgsExt {
        #[rename(name = "method_ptr")]
        pub method_ptr: ::unity::IntPtr,
        #[rename(name = "m_target")]
        pub m_target: ::unity::IlInstance,
        #[rename(name = "method")]
        pub method: ::unity::IntPtr,
    }

    #[::unity::class(namespace = "App", name = "EventScript.FunctionArgs")]
    pub struct EventScriptFunctionArgsExt {
        #[rename(name = "method_ptr")]
        pub method_ptr: ::unity::IntPtr,
        #[rename(name = "m_target")]
        pub m_target: ::unity::IlInstance,
        #[rename(name = "method")]
        pub method: ::unity::IntPtr,
    }

    fn clone_invoke_method_info(method_ptr: *mut u8) -> &'static MethodInfo {
        use std::{collections::HashMap, sync::Mutex};

        static CACHE: Mutex<Option<HashMap<usize, &'static MethodInfo>>> = Mutex::new(None);

        let mut guard = CACHE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        let key = method_ptr as usize;
        if let Some(mi) = map.get(&key) {
            return mi;
        }
        let donor = Class::lookup("App", "ScriptSystem")
            .raw()
            .get_method_from_name("Log", 1)
            .expect("App.ScriptSystem::Log(args) donor missing");
        let mut cloned: MethodInfo = *donor;
        cloned.method_ptr = method_ptr;
        let leaked: &'static MethodInfo = Box::leak(Box::new(cloned));
        map.insert(key, leaked);
        leaked
    }

    fn make_action_args(callback: *mut u8) -> EventScript_ActionArgs {
        let mi = clone_invoke_method_info(callback);
        let instance = <EventScriptActionArgsExt as FromIlInstance>::instantiate().expect("EventScript.ActionArgs allocation failed");
        instance.set_method_ptr(IntPtr(callback as *mut ()));
        instance.set_m_target(IlInstance::null());
        instance.set_method(IntPtr(mi as *const MethodInfo as *mut ()));
        EventScript_ActionArgs::from_il_instance(IlInstance::from(instance))
    }

    fn make_function_args(callback: *mut u8) -> EventScript_FunctionArgs {
        let mi = clone_invoke_method_info(callback);
        let instance = <EventScriptFunctionArgsExt as FromIlInstance>::instantiate().expect("EventScript.FunctionArgs allocation failed");
        instance.set_method_ptr(IntPtr(callback as *mut ()));
        instance.set_m_target(IlInstance::null());
        instance.set_method(IntPtr(mi as *const MethodInfo as *mut ()));
        EventScript_FunctionArgs::from_il_instance(IlInstance::from(instance))
    }

    pub trait EventScriptExt {
        fn register_action(self, name: impl Into<Il2CppString>, callback: extern "C" fn(Array<DynValue>, OptionalMethod));
        fn register_function(self, name: impl Into<Il2CppString>, callback: extern "C" fn(Array<DynValue>, OptionalMethod) -> DynValue);
    }

    impl EventScriptExt for EventScript {
        fn register_action(self, name: impl Into<Il2CppString>, callback: extern "C" fn(Array<DynValue>, OptionalMethod)) {
            let args = make_action_args(callback as *mut u8);
            self.regist_action(args, name.into());
        }

        fn register_function(self, name: impl Into<Il2CppString>, callback: extern "C" fn(Array<DynValue>, OptionalMethod) -> DynValue) {
            let args = make_function_args(callback as *mut u8);
            self.regist_function(args, name.into());
        }
    }
}
#[cfg(all(feature = "app-eventscript", feature = "moon_sharp-interpreter-dynvalue"))]
pub use event_script_ext::*;

#[cfg(all(feature = "unity_engine-assetbundle", feature = "unity_engine-assetbundlecreaterequest",))]
mod asset_bundle_ext {
    use unity::{Array, OptionalMethod};

    use crate::unity_engine::{assetbundle::AssetBundle, assetbundlecreaterequest::AssetBundleCreateRequest};

    #[skyline::from_offset(0x491ff0)]
    unsafe fn engage_method_from_full_name(name: *const u8) -> *const u8;

    fn lookup_method_by_signature(full_signature: &str) -> *const u8 {
        let c = std::ffi::CString::new(full_signature).unwrap();
        unsafe { engage_method_from_full_name(c.as_ptr() as *const u8) }
    }

    pub trait AssetBundleExt {
        fn load_from_memory_async_internal(binary: Array<u8>, crc: u32) -> AssetBundleCreateRequest;
    }

    impl AssetBundleExt for AssetBundle {
        fn load_from_memory_async_internal(binary: Array<u8>, crc: u32) -> AssetBundleCreateRequest {
            static METHOD_PTR: ::std::sync::OnceLock<usize> = ::std::sync::OnceLock::new();
            let ptr = *METHOD_PTR.get_or_init(|| {
                let p = lookup_method_by_signature("UnityEngine.AssetBundle::LoadFromMemoryAsync_Internal(System.Byte[],System.UInt32)");
                assert!(
                    !p.is_null(),
                    "UnityEngine.AssetBundle.LoadFromMemoryAsync_Internal not found via runtime full-signature lookup"
                );
                p as usize
            });

            type RawFn = extern "C" fn(Array<u8>, u32, OptionalMethod) -> AssetBundleCreateRequest;
            let f: RawFn = unsafe { ::std::mem::transmute(ptr) };
            f(binary, crc, None)
        }
    }
}
#[cfg(all(feature = "unity_engine-assetbundle", feature = "unity_engine-assetbundlecreaterequest",))]
pub use asset_bundle_ext::*;

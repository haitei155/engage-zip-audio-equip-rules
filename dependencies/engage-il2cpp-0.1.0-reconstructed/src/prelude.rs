// One-stop import for everyday plugin code:
//
//   use engage::prelude::*;
//
// Brings the common collection types, the IL2CPP infra re-exported from
// unity, and every hand-written ext trait into scope so `.iter()`,
// `.register_action(...)`, etc. resolve without naming each trait
// individually.

// Core IL2CPP infra (lives in unity; re-exported for convenience).
pub use unity::{Array, Cast, ClassIdentity, FromIlInstance, Il2CppString, IlInstance, IntPtr, MethodInfo, OptionalMethod, SystemObject};

// Every ext trait + helper from `crate::ext`.
pub use crate::ext::*;
// System collection types — `List_1<T>` and `Dictionary_2<K, V>` show up
// constantly in IL2CPP signatures. Each one is gated by its own feature
// so a project that opts out of every binding (default-features = false)
// still compiles.
#[cfg(feature = "system-collections-generic-dictionary_2-types")]
pub use crate::system::collections::generic::dictionary_2::Dictionary_2;
#[cfg(feature = "system-collections-generic-list_1-types")]
pub use crate::system::collections::generic::list_1::List_1;
#[cfg(feature = "system-object-types")]
pub use crate::system::object::Object;

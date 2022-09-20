//! Common imports used throughout this workspace.

/// Common traits and types defined by this crate.
pub mod prelude {
    pub use std::{
        fmt::Display,
        hash::Hash,
        ops::{Deref, DerefMut},
    };

    pub extern crate log;
    pub use ahash::{AHashMap as Map, AHashSet as Set};

    pub use implem::implem;

    pub use crate::Empty;
}

/// An empty type, *i.e* an `enum` with no variants.
///
/// # Examples
///
/// ```rust
/// # use base::Empty;
/// fn cannot_be_called<T>(e: Empty) -> T {
///     match e {}
/// }
/// ```
pub enum Empty {}

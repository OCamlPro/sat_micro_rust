//! Common imports used throughout this workspace.

/// Imports this crate's prelude.
///
/// Pass `pub` when calling this macro to make the imports public.
#[macro_export]
macro_rules! prelude {
    {} => { use $crate::prelude::*; };
    {pub } => { pub use $crate::prelude::*; };
}

/// Common traits and types defined by this crate.
///
/// See also the [`prelude!`] macro.
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

//! Frontend, deals with parsing clauses in the SAT-comp format.

#[macro_export]
macro_rules! prelude {
    {} => { use $crate::prelude::*; };
    { pub } => { pub use $crate::prelude::*; };
}

pub mod prelude {
    pub use error_chain::bail;

    dpll::prelude!(pub);

    pub use crate::Lit;

    pub use err::{Res, ResExt};

    /// Error-management.
    pub mod err {
        error_chain::error_chain! {
            types {
                Error, ErrorKind, ResExt, Res;
            }
            foreign_links {
                Io(std::io::Error);
            }
        }
    }
}

pub mod parse;

prelude!();

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lit {
    idx: usize,
    neg: bool,
}
impl Lit {
    pub fn new(idx: usize, neg: bool) -> Self {
        Self { idx, neg }
    }
}
implem! {
    for Lit {
        Display {
            |&self, fmt| {
                if self.neg {
                    write!(fmt, "-")?
                }
                self.idx.fmt(fmt)
            }
        }
    }
}
impl Literal for Lit {
    fn negate(self) -> Self {
        Self {
            idx: self.idx,
            neg: !self.neg,
        }
    }
    fn ref_negate(&self) -> Self {
        self.negate()
    }
}

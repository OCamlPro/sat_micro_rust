//! Functional versions taken directly from the paper.

use crate::prelude::*;

mod backjump;
mod cdcl;
mod plain;

pub use self::{backjump::Backjump, cdcl::Cdcl, plain::Plain};

pub fn solve<F>(f: F, dpll: Dpll) -> Result<Outcome<F::Lit, ()>, String>
where
    F: Formula,
{
    match dpll {
        Dpll::Plain => Ok(Plain::new(f).solve()),
        Dpll::Backjump => Ok(Backjump::new(f).solve()),
        Dpll::Cdcl => Ok(Cdcl::new(f).solve()),
    }
}

//! Defines DPLL traits that are generic over the notion of literal and CNF formula.

#![allow(mixed_script_confusables)]

/// Imports this crate's prelude.
///
/// Pass `pub` when calling this macro to make the imports public.
#[macro_export]
macro_rules! prelude {
    {} => { use $crate::prelude::*; };
    { pub } => { pub use $crate::prelude::*; };
}

/// Common traits and types defined by this crate.
///
/// See also the [`prelude!`] macro.
pub mod prelude {
    base::prelude! { pub }
    pub use base::prelude::implem;

    pub use crate::{Clause, Cnf, Formula, Literal};
}

prelude!();

pub mod functional;

/// Abstracts over the notion of literal.
pub trait Literal: PartialEq + Eq + PartialOrd + Ord + Hash {
    /// Negates a literal (owned version).
    fn negate(self) -> Self;
    /// Negates a literal (reference version).
    fn ref_negate(&self) -> Self;
}

#[derive(Debug, Clone)]
pub struct Clause<Lit> {
    lits: Vec<Lit>,
}
impl<Lit> Clause<Lit> {
    pub fn new(lits: Vec<Lit>) -> Self {
        Self { lits }
    }
    pub fn empty() -> Self {
        Self { lits: vec![] }
    }
    pub fn with_capacity(capa: usize) -> Self {
        Self {
            lits: Vec::with_capacity(capa),
        }
    }
}
implem! {
    impl(Lit: std::fmt::Display) for Clause<Lit> {
        Display {
            |&self, fmt| {
                for (idx, lit) in self.lits.iter().enumerate() {
                    if idx > 0 {
                        ", ".fmt(fmt)?
                    }
                    lit.fmt(fmt)?
                }
                Ok(())
            }
        }
    }
    impl(Lit) for Clause<Lit> {
        Deref<Target = Vec<Lit>> {
            |&self| &self.lits,
            |&mut self| &mut self.lits,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cnf<Lit> {
    clauses: Vec<Clause<Lit>>,
}
impl<Lit> Cnf<Lit> {
    pub fn new(clauses: Vec<Clause<Lit>>) -> Self {
        Self { clauses }
    }
    pub fn empty() -> Self {
        Self { clauses: vec![] }
    }
    pub fn with_capacity(capa: usize) -> Self {
        Self {
            clauses: Vec::with_capacity(capa),
        }
    }
}
implem! {
    impl(Lit) for Cnf<Lit> {
        Deref<Target = Vec<Clause<Lit>>> {
            |&self| &self.clauses,
            |&mut self| &mut self.clauses,
        }
    }
}

/// Abstracts over the notion of formula.
pub trait Formula {
    /// Type of literals for this formula.
    type Lit: Literal;

    /// Transforms a formula into a CNF.
    fn into_cnf(self) -> Cnf<Self::Lit>;
}

impl<Lit: Literal> Formula for Cnf<Lit> {
    type Lit = Lit;
    fn into_cnf(self) -> Self {
        self
    }
}

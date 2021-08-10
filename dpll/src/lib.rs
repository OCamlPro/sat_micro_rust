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

    pub use crate::{Clause, Cnf, Dpll, DpllImpl, Formula, LClause, LCnf, Literal, Outcome};
}

prelude!();

pub mod recursive;

pub fn solve<F: Formula>(f: F, dpll: DpllImpl) -> Result<Outcome<F::Lit, ()>, String> {
    use self::DpllImpl::*;
    match dpll {
        Recursive(dpll) => recursive::solve(f, dpll),
    }
}

/// Enumerates DPLL variations.
#[derive(Debug, Clone, Copy)]
pub enum Dpll {
    /// Plain DPLL.
    Plain,
    /// DPLL with backjump.
    Backjump,
    /// DPLL with backjump and CDCL.
    Cdcl,
}
impl Default for Dpll {
    fn default() -> Self {
        Self::Backjump
    }
}
impl Dpll {
    pub const NAMES: &'static [(&'static str, &'static str)] = &[
        ("plain", "Plain DPLL with no optimization"),
        ("backjump", "DPLL with backjump optimization"),
        ("cdcl", "DPLL with backjump and CDCL"),
    ];
    pub fn from_name(name: &str) -> Option<Self> {
        match name.as_ref() {
            "plain" => Some(Self::Plain),
            "backjump" => Some(Self::Backjump),
            "cdcl" => Some(Self::Cdcl),
            _ => None,
        }
    }
}
implem! {
    for Dpll {
        Display {
            |&self, fmt| match self {
                Self::Plain => "plain (unoptimized)".fmt(fmt),
                Self::Backjump => "with backjump".fmt(fmt),
                Self::Cdcl => "with backjump and CDCL".fmt(fmt),
            }
        }
    }
}

/// Enumerates DPLL implementation variations.
pub enum DpllImpl {
    /// Recursive implementation.
    Recursive(Dpll),
}
implem! {
    for DpllImpl {
        Display {
            |&self, fmt| match self {
                Self::Recursive(dpll) => write!(fmt, "recursive DPLL {}", dpll),
            }
        }
    }
}
impl Default for DpllImpl {
    fn default() -> Self {
        Self::Recursive(Dpll::default())
    }
}
impl DpllImpl {
    pub const NAMES: &'static [(&'static str, &'static str)] = &[(
        "recursive",
        "Recursive implementation (might stack overflow)",
    )];
    pub fn from_name(name: &str, sub_name: Option<&str>) -> Option<Self> {
        match name.as_ref() {
            "recursive" => Some(Self::Recursive(
                sub_name
                    .map(|sub_name| Dpll::from_name(sub_name))
                    .unwrap_or_else(|| Some(Dpll::default()))?,
            )),
            _ => None,
        }
    }
}

/// Outcome of satisfiability check.
#[derive(Debug, Clone)]
pub enum Outcome<Lit, UnsatRes> {
    /// Sat result, with a model.
    Sat(Set<Lit>),
    /// Unsat result.
    Unsat(UnsatRes),
}
impl<Lit, UnsatRes> Outcome<Lit, UnsatRes> {
    /// Sat constructor.
    pub fn new_sat(γ: Set<Lit>) -> Self {
        Self::Sat(γ)
    }
    /// Unsat constructor.
    pub fn new_unsat(res: UnsatRes) -> Self {
        Self::Unsat(res)
    }

    /// Map over either the [`Self::Sat`] or [`Self::Unsat`] variant.
    pub fn map<T>(
        self,
        sat_action: impl FnOnce(Set<Lit>) -> T,
        unsat_action: impl FnOnce(UnsatRes) -> T,
    ) -> T {
        match self {
            Self::Sat(γ) => sat_action(γ),
            Self::Unsat(res) => unsat_action(res),
        }
    }

    /// Erases the `UnsatRes` data and replaces it by unit.
    pub fn into_unit_unsat(self) -> Outcome<Lit, ()> {
        self.map(|sat| Outcome::Sat(sat), |_| Outcome::Unsat(()))
    }
}

/// Abstracts over the notion of literal.
pub trait Literal: PartialEq + Eq + PartialOrd + Ord + Hash + Clone + Display {
    /// Negates a literal (owned version).
    fn negate(self) -> Self;
    /// Negates a literal (reference version).
    fn ref_negate(&self) -> Self;
}

/// A clause.
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
    impl(Lit: Display) for Clause<Lit> {
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

/// A CNF formula.
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
    pub fn into_iter(self) -> std::vec::IntoIter<Clause<Lit>> {
        self.clauses.into_iter()
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

/// A labelled Clause.
#[derive(Debug, Clone)]
pub struct LClause<Lit> {
    clause: Clause<Lit>,
    labels: Set<Lit>,
}
impl<Lit> LClause<Lit> {
    /// Constructor.
    pub fn new(clause: Clause<Lit>) -> Self {
        let labels = Set::with_capacity(clause.len());
        Self { clause, labels }
    }
    /// Clause accessor, note that `Self` already [`Deref`]s to [`Clause<Lit>`].
    pub fn clause(&self) -> &Clause<Lit> {
        &self.clause
    }
    /// Labels accessor.
    pub fn labels(&self) -> &Set<Lit> {
        &self.labels
    }
}
implem! {
    impl(Lit: Display) for LClause<Lit> {
        Display {
            |&self, fmt| {
                self.clause.fmt(fmt)?;
                if !self.labels.is_empty() {
                    " [".fmt(fmt)?;
                    for (idx, lit) in self.labels.iter().enumerate() {
                        if idx > 0 {
                            ", ".fmt(fmt)?;
                            lit.fmt(fmt)?;
                        }
                    }
                    "]".fmt(fmt)?;
                }
                Ok(())
            }
        }
    }
    impl(Lit) for LClause<Lit> {
        From<Clause<Lit>> {
            |clause| Self::new(clause)
        }
        Deref<Target = Clause<Lit>> {
            |&self| &self.clause,
            |&mut self| &mut self.clause,
        }
    }
}

/// A labelled CNF.
#[derive(Debug, Clone)]
pub struct LCnf<Lit> {
    clauses: Vec<LClause<Lit>>,
}
impl<Lit> LCnf<Lit> {
    pub fn new(clauses: Vec<LClause<Lit>>) -> Self {
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
    pub fn into_iter(self) -> std::vec::IntoIter<LClause<Lit>> {
        self.clauses.into_iter()
    }

    pub fn push(&mut self, clause: impl Into<LClause<Lit>>) {
        self.clauses.push(clause.into())
    }
}
implem! {
    impl(Lit) for LCnf<Lit> {
        Deref<Target = Vec<LClause<Lit>>> {
            |&self| &self.clauses,
            |&mut self| &mut self.clauses,
        }
        From<Cnf<Lit>> {
            |cnf| Self {
                clauses: cnf.into_iter().map(LClause::from).collect()
            }
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

//! Defines DPLL traits that are generic over the notion of literal and CNF formula.

#![allow(mixed_script_confusables)]

use std::iter::FromIterator;

/// Common traits and types defined by this crate.
pub mod prelude {
    pub use base::prelude::{implem, *};

    pub use crate::{Clause, Cnf, Dpll, DpllImpl, Formula, LClause, LCnf, Literal, Outcome};
}

use prelude::*;

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
        Self::Cdcl
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
#[derive(Debug, Clone, Copy)]
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

    /// True if the outcome is sat.
    pub fn is_sat(&self) -> bool {
        match self {
            Self::Sat(_) => true,
            Self::Unsat(_) => false,
        }
    }
    /// True if the outcome is sat.
    pub fn is_unsat(&self) -> bool {
        match self {
            Self::Unsat(_) => true,
            Self::Sat(_) => false,
        }
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
    /// Map over either the [`Self::Sat`] or [`Self::Unsat`] variant.
    pub fn map_ref<T>(
        &self,
        sat_action: impl FnOnce(&Set<Lit>) -> T,
        unsat_action: impl FnOnce(&UnsatRes) -> T,
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Clause<Lit: Literal> {
    lits: Vec<Lit>,
}
impl<Lit: Literal> FromIterator<Lit> for Clause<Lit> {
    fn from_iter<T: IntoIterator<Item = Lit>>(iter: T) -> Self {
        Self {
            lits: iter.into_iter().collect(),
        }
    }
}
impl<Lit: Literal> Clause<Lit> {
    pub fn new(mut lits: Vec<Lit>) -> Self {
        lits.sort();
        let res = Self { lits };
        res.invariant("new");
        res
    }
    pub fn empty() -> Self {
        Self { lits: vec![] }
    }
    pub fn with_capacity(capa: usize) -> Self {
        Self {
            lits: Vec::with_capacity(capa),
        }
    }

    #[cfg(release)]
    pub fn invariant(&self, _caller: &str) {}
    #[cfg(not(release))]
    pub fn invariant(&self, caller: &str) {
        let mut prev = None;
        for lit in &self.lits {
            if let Some(prev) = prev {
                if prev > lit {
                    panic!(
                        "[internal | {}] illegal clause, literals should be sorted",
                        caller
                    )
                }
            }
            prev = Some(lit)
        }
    }

    pub fn len(&self) -> usize {
        self.lits.len()
    }
    pub fn iter(&self) -> std::slice::Iter<Lit> {
        self.lits.iter()
    }
    pub fn is_empty(&self) -> bool {
        self.lits.is_empty()
    }

    pub fn clear(&mut self) {
        self.lits.clear()
    }
    pub fn push(&mut self, lit: Lit) {
        match self.lits.binary_search(&lit) {
            Ok(_) => (),
            Err(pos) => self.lits.insert(pos, lit),
        }
        self.invariant("push");
    }
    pub fn drain(&mut self, range: impl std::ops::RangeBounds<usize>) -> std::vec::Drain<Lit> {
        self.lits.drain(range)
    }
    pub fn shrink_to_fit(&mut self) {
        self.lits.shrink_to_fit()
    }
}
implem! {
    impl(Lit: Literal) for Clause<Lit> {
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
}

/// A CNF formula.
#[derive(Debug, Clone)]
pub struct Cnf<Lit: Literal> {
    clauses: Vec<Clause<Lit>>,
}
impl<Lit: Literal> Cnf<Lit> {
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
    impl(Lit: Literal) for Cnf<Lit> {
        Deref<Target = Vec<Clause<Lit>>> {
            |&self| &self.clauses,
            |&mut self| &mut self.clauses,
        }
    }
}

/// A labelled Clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LClause<Lit: Literal> {
    clause: Clause<Lit>,
    labels: Set<Lit>,
}
impl<Lit: Literal> Hash for LClause<Lit> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.clause.hash(state);
        for label in &self.labels {
            label.hash(state)
        }
    }
}
impl<Lit: Literal> LClause<Lit> {
    /// Constructor.
    pub fn new(clause: Clause<Lit>) -> Self {
        let labels = Set::with_capacity(clause.len());
        Self::new_with(clause, labels)
    }
    /// Constructor from a clause and some labels.
    pub fn new_with(clause: Clause<Lit>, labels: Set<Lit>) -> Self {
        Self { clause, labels }
    }
    /// An empty clause with no labels.
    pub fn empty() -> Self {
        Self::new(Clause::empty())
    }
    /// An empty clause with a set of labels.
    pub fn empty_with(labels: Set<Lit>) -> Self {
        Self {
            clause: Clause::empty(),
            labels,
        }
    }
    /// Clause accessor, note that `Self` already [`Deref`]s to [`Clause<Lit>`].
    pub fn clause(&self) -> &Clause<Lit> {
        &self.clause
    }
    /// Labels accessor.
    pub fn labels(&self) -> &Set<Lit> {
        &self.labels
    }
    /// Labels accessor, mutable version.
    pub fn labels_mut(&mut self) -> &mut Set<Lit> {
        &mut self.labels
    }

    /// Map over the labels.
    pub fn labels_map(&mut self, f: impl FnOnce(&mut Set<Lit>)) {
        f(&mut self.labels)
    }
}
implem! {
    impl(Lit: Literal) for LClause<Lit> {
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
    impl(Lit: Literal) for LClause<Lit> {
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
pub struct LCnf<Lit: Literal> {
    clauses: Vec<LClause<Lit>>,
}
impl<Lit: Literal> LCnf<Lit> {
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

    pub fn push(&mut self, clause: LClause<Lit>) {
        self.clauses.push(clause.into())
    }
}
implem! {
    impl(Lit: Literal) for LCnf<Lit> {
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

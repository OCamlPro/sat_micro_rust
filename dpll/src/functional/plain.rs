//! Plain DPLL version, with no optimizations.

prelude!();

/// Alias for a set of `Lit`.
pub type Γ<Lit> = Set<Lit>;

#[derive(Debug, Clone)]
pub enum R<Lit> {
    Sat(Γ<Lit>),
    Unsat,
}
impl<Lit> R<Lit> {
    /// Sat constructor.
    pub fn new_sat(γ: Γ<Lit>) -> Self {
        Self::Sat(γ)
    }
    /// Unsat constructor.
    pub fn new_unsat() -> Self {
        Self::Unsat
    }

    /// Map over either the [`Self::Sat`] or [`Self::Unsat`] variant.
    pub fn map<T>(
        self,
        sat_action: impl FnOnce(Γ<Lit>) -> T,
        unsat_action: impl FnOnce() -> T,
    ) -> T {
        match self {
            Self::Sat(γ) => sat_action(γ),
            Self::Unsat => unsat_action(),
        }
    }
}
macro_rules! raise {
	{ sat $γ:expr } => { return Err(R::Sat($γ)); };
	{ unsat } => { return Err(R::Unsat); };
}

pub type Res<T, Lit> = Result<T, R<Lit>>;

/// The first naive implementation from the paper.
#[derive(Clone)]
pub struct Plain<F: Formula> {
    /// Environment, *i.e.* a set of literals.
    γ: Γ<F::Lit>,
    /// CNF we're working on.
    δ: Cnf<F::Lit>,
}

implem! {
    impl(F: Formula) for Plain<F> {
        From<F> {
            |f| Self::new(f),
        }
        Deref<Target = Γ<F::Lit>> {
            |&self| &self.γ,
            |&mut self| &mut self.γ,
        }
    }
}

impl<F> Plain<F>
where
    F: Formula,
{
    /// Construct a naive solver from a formula.
    pub fn new(f: F) -> Self {
        Self {
            γ: Γ::new(),
            δ: f.into_cnf(),
        }
    }
}

impl<F> Plain<F>
where
    F: Formula,
    F::Lit: Clone + std::fmt::Display,
    Γ<F::Lit>: Clone,
    Self: Clone,
{
    /// *Assume* rule.
    pub fn assume(&self, lit: F::Lit) -> Res<Self, F::Lit> {
        log::debug!("assume({})", lit);
        let mut new: Self = self.clone();
        let is_new = new.insert(lit);

        if is_new {
            new.bcp()
        } else {
            Ok(new)
        }
    }

    /// *BCP* rule.
    pub fn bcp(&self) -> Res<Self, F::Lit> {
        log::debug!("bcp(), γ.len(): {}", self.γ.len());
        let Self { γ, .. } = self;
        let mut new = Self {
            γ: self.γ.clone(),
            δ: Cnf::with_capacity(self.δ.len()),
        };
        let mut new_clause = Clause::with_capacity(5);

        'conj_iter: for disj in self.δ.iter() {
            new_clause.clear();
            'disj_iter: for lit in disj.iter() {
                if γ.contains(lit) {
                    // Disjunction is true, discard it.
                    continue 'conj_iter;
                } else if γ.contains(&lit.ref_negate()) {
                    // Negation of literal is true, ignore literal (do nothing and continue).
                } else {
                    // We know nothing of this literal, keep it.
                    new_clause.push(lit.clone());
                }
                continue 'disj_iter;
            }

            match new_clause.len() {
                0 => raise!(unsat),
                1 => new = new.assume(new_clause.iter().next().expect("unreachable").clone())?,
                _ => {
                    // Got a new disjunction, add it to the new CNF.
                    new_clause.shrink_to_fit();
                    new.δ.push(new_clause.clone());
                }
            }
        }

        Ok(new)
    }

    pub fn unsat(&self) -> Res<Empty, F::Lit> {
        log::debug!("unsat()");
        if self.δ.is_empty() {
            raise!(sat self.γ.clone())
        } else {
            let disj = &self.δ[0];
            if let Some(lit) = disj.iter().next() {
                let mut new = self.assume(lit.clone())?;
                new.unsat()?;

                let n_lit = lit.ref_negate();
                new = self.assume(n_lit)?;
                new.unsat()?;

                unreachable!()
            } else {
                panic!("illegal empty disjunct in application of `unsat` rule")
            }
        }
    }

    pub fn solve(&self) -> R<F::Lit> {
        match self.unsat() {
            Err(res) => res,
            Ok(empty) => match empty {},
        }
    }
}

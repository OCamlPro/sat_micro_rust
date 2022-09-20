//! Plain DPLL version, with no optimizations.

prelude!();

/// Alias for a set of `Lit`.
pub type Γ<Lit> = Set<Lit>;
/// Alias for an outcome with no unsat result.
pub type Out<Lit> = crate::Outcome<Lit, ()>;

macro_rules! raise {
	{ sat $γ:expr } => { return Err(Out::Sat($γ)) };
	{ unsat } => { return Err(Out::Unsat(())) };
}

pub type Res<T, Lit> = Result<T, Out<Lit>>;

/// The first naive implementation from the paper.
#[derive(Clone)]
pub struct Plain<Lit: Literal> {
    /// Environment, *i.e.* a set of literals.
    γ: Γ<Lit>,
    /// CNF we're working on.
    δ: Cnf<Lit>,
}

implem! {
    impl(Lit: Literal, F: Formula<Lit = Lit>) for Plain<Lit> {
        From<F> {
            |f| Self::new(f),
        }
    }
    impl(Lit: Literal) for Plain<Lit> {
        Deref<Target = Γ<Lit>> {
            |&self| &self.γ,
            |&mut self| &mut self.γ,
        }
    }
}

impl<Lit: Literal> Plain<Lit> {
    /// Construct a naive solver from a formula.
    pub fn new<F>(f: F) -> Self
    where
        F: Formula<Lit = Lit>,
    {
        Self {
            γ: Γ::new(),
            δ: f.into_cnf(),
        }
    }
}

impl<Lit: Literal> Plain<Lit> {
    /// *Assume* rule.
    pub fn assume(&self, lit: Lit) -> Res<Self, Lit> {
        log::debug!("assume({})", lit);
        let mut new: Self = self.clone();
        let is_new = new.insert(lit);

        if is_new {
            new.bcp()
        } else {
            panic!("trying to assume a literal twice")
        }
    }

    /// *BCP* rule.
    pub fn bcp(&self) -> Res<Self, Lit> {
        log::debug!("bcp(), γ.len(): {}", self.γ.len());
        let mut new = Self {
            γ: self.γ.clone(),
            δ: Cnf::with_capacity(self.δ.len()),
        };
        let mut new_clause = Clause::with_capacity(5);

        'conj_iter: for disj in self.δ.iter() {
            new_clause.clear();
            'disj_iter: for lit in disj.iter() {
                if new.γ.contains(lit) {
                    // Disjunction is true, discard it.
                    continue 'conj_iter;
                } else if new.γ.contains(&lit.ref_negate()) {
                    // Negation of literal is true, ignore literal (do nothing and continue).
                } else {
                    // We know nothing of this literal, keep it.
                    new_clause.push(lit.clone());
                }
                continue 'disj_iter;
            }

            match new_clause.len() {
                0 => raise!(unsat),
                1 => new = new.assume(new_clause.drain(0..).next().expect("unreachable"))?,
                _ => {
                    // Got a new disjunction, add it to the new CNF.
                    new_clause.shrink_to_fit();
                    new.δ.push(new_clause.clone());
                }
            }
        }

        Ok(new)
    }

    pub fn unsat(&self) -> Res<Empty, Lit> {
        log::debug!("unsat()");
        if self.δ.is_empty() {
            raise!(sat self.γ.clone())
        } else {
            let disj = &self.δ[0];
            if let Some(lit) = disj.iter().next() {
                match self.assume(lit.clone()).and_then(|new| new.unsat()) {
                    Ok(empty) => match empty {},
                    Err(e) => {
                        if e.is_unsat() {
                            ()
                        } else {
                            return Err(e);
                        }
                    }
                }

                let n_lit = lit.ref_negate();
                log::trace!("backtracking {}", lit);
                let new = self.assume(n_lit)?;
                let empty = new.unsat()?;

                match empty {}
            } else {
                panic!("illegal empty disjunct in application of `unsat` rule")
            }
        }
    }

    pub fn solve(&self) -> Out<Lit> {
        match self.unsat() {
            Err(res) => res,
            Ok(empty) => match empty {},
        }
    }
}

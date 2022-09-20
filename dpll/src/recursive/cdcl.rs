//! Augments the [`Backjump` solver][super::Backjump] with CDCL.

use crate::prelude::*;

/// Alias for a map from `Lit`s to sets of `Lit`s.
pub type Γ<Lit> = Map<Lit, Set<Lit>>;

macro_rules! raise {
	{ sat $γ:expr } => { return Err(Outcome::Sat($γ)) };
	{ unsat $deps:expr } => { return Err(Outcome::Unsat($deps)) };
}

pub type LClauses<Lit> = Set<LClause<Lit>>;

pub type Out<Lit> = Outcome<Lit, (Set<Lit>, LClauses<Lit>)>;
pub type Res<T, Lit> = Result<T, Out<Lit>>;

/// Backjump + CDCL solver.
#[derive(Clone)]
pub struct Cdcl<Lit: Literal> {
    /// Environment, *i.e.* a set of literals.
    γ: Γ<Lit>,
    /// CNF we're working on.
    δ: LCnf<Lit>,
}

implem! {
    impl(Lit: Literal, F: Formula<Lit = Lit>) for Cdcl<Lit> {
        From<F> {
            |f| Self::new(f),
        }
    }
    impl(Lit: Literal) for Cdcl<Lit> {
        Deref<Target = Γ<Lit>> {
            |&self| &self.γ,
            |&mut self| &mut self.γ,
        }
    }
}

impl<Lit: Literal> Cdcl<Lit> {
    /// Construct a naive solver from a formula.
    pub fn new<F: Formula<Lit = Lit>>(f: F) -> Self {
        Self {
            γ: Γ::new(),
            δ: f.into_cnf().into(),
        }
    }
}

impl<Lit: Literal> Cdcl<Lit> {
    /// Checks internal invariants.
    #[cfg(release)]
    #[inline]
    pub fn invariant(&self) {}

    /// Checks internal invariants.
    #[cfg(not(release))]
    pub fn invariant(&self) {
        let γ = &self.γ;
        for lit in γ.keys() {
            let nlit = lit.ref_negate();
            if γ.contains_key(&nlit) {
                panic!(
                    "inconsistent environment, contains both {} and {}",
                    lit, nlit
                );
            }
        }
    }

    pub fn shift(lit: &Lit, lclauses: &LClauses<Lit>) -> LClauses<Lit> {
        let mut res = LClauses::with_capacity(lclauses.len());

        for lclause in lclauses {
            let mut new_lclause = lclause.clone();
            if lclause.labels().contains(lit) {
                new_lclause.push(lit.ref_negate());
                let _was_there = new_lclause.labels_mut().remove(lit);
                debug_assert!(_was_there);
            }
            let _is_new = res.insert(new_lclause);
            debug_assert!(_is_new)
        }

        res
    }

    /// *Assume* rule.
    pub fn assume(&self, lit: Lit, cause: Set<Lit>) -> Res<Self, Lit> {
        log::debug!("assume({})", lit);
        self.invariant();
        let mut new: Self = self.clone();

        use std::collections::hash_map::Entry::*;
        match new.entry(lit) {
            Occupied(mut entry) => {
                entry.get_mut().extend(cause);
                Ok(new)
            }
            Vacant(entry) => {
                entry.insert(cause);
                new.bcp()
            }
        }
    }

    /// *BCP* rule.
    pub fn bcp(&self) -> Res<Self, Lit> {
        log::debug!("bcp(), γ.len(): {}", self.γ.len());
        self.invariant();
        let mut new = Self {
            γ: self.γ.clone(),
            δ: LCnf::with_capacity(self.δ.len()),
        };
        let mut new_clause = Clause::with_capacity(5);
        let mut new_deps = Set::with_capacity(5);

        log::trace!(
            "γ:{}",
            new.γ.keys().fold(String::new(), |mut acc, lit| {
                acc.push(' ');
                acc.push_str(&lit.to_string());
                acc
            })
        );

        'conj_iter: for lclause in self.δ.iter() {
            log::trace!("current clause: {}", lclause);
            new_clause.clear();
            new_deps.clear();
            new_deps.extend(lclause.labels().iter().cloned());
            // In theory, we should extend `new_deps` by `lclause.labels`. We might as well wait
            // though, because sometimes the whole clause will be dropped. That is, when one of its
            // literals is known to be true in the environment.
            'lclause_iter: for lit in lclause.iter() {
                let nlit = lit.ref_negate();
                log::trace!("lit: {}, nlit: {}", lit, nlit);
                if new.γ.contains_key(lit) {
                    log::trace!("lit {} is true", lit);
                    // Disjunction is true, discard it.
                    continue 'conj_iter;
                } else if let Some(deps) = new.γ.get(&lit.ref_negate()) {
                    log::trace!("lit {} is false", lit);
                    new_deps.extend(deps.iter().cloned());
                    // Negation of literal is true, ignore literal (do nothing and continue).
                } else {
                    log::trace!(
                        "γ:{}",
                        new.γ.keys().fold(String::new(), |mut acc, lit| {
                            acc.push(' ');
                            acc.push_str(&lit.to_string());
                            acc
                        })
                    );
                    log::trace!("lit {} is unknown", lit);
                    // We know nothing of this literal, keep it.
                    new_clause.push(lit.clone());
                }
                continue 'lclause_iter;
            }

            new_deps.extend(lclause.labels.iter().cloned());

            if new_clause.is_empty() {
                raise!(unsat(new_deps, LClauses::new()))
            } else {
                if new_clause.len() == 1 {
                    let lit = new_clause.drain(0..).next().expect("unreachable");
                    let mut deps = Set::with_capacity(new_deps.len());
                    deps.extend(new_deps.drain());
                    new = new.assume(lit, deps)?;
                } else {
                    new.δ.push(LClause::new_with(
                        new_clause.drain(0..).collect(),
                        new_deps.drain().collect(),
                    ));
                }
            }
        }

        Ok(new)
    }

    pub fn unsat(&self) -> Res<Empty, Lit> {
        log::debug!("unsat()");
        self.invariant();
        if self.δ.is_empty() {
            raise!(sat self.γ.iter().map(|(lit, _)| lit.clone()).collect())
        } else {
            let disj = &self.δ[0];
            if let Some(lit) = disj.iter().next() {
                let mut deps = Set::new();
                let _is_new = deps.insert(lit.clone());
                debug_assert!(_is_new);

                let (mut deps, mut conflict) =
                    match self.assume(lit.clone(), deps).and_then(|new| new.unsat()) {
                        // Unreachable.
                        Ok(empty) => match empty {},
                        // Sat, propagate sat result.
                        Err(sat_res @ Out::Sat(_)) => return Err(sat_res),
                        // Conflict, move on.
                        Err(Out::Unsat(deps)) => deps,
                    };

                conflict = Self::shift(lit, &conflict);

                log::debug!(
                    "handling unsat branch with deps:{}",
                    deps.iter().fold(String::new(), |mut acc, lit| {
                        acc.push_str(" ");
                        acc.push_str(&lit.to_string());
                        acc
                    })
                );

                let lit_was_there = deps.remove(lit);
                if !lit_was_there {
                    raise!(unsat(deps, conflict))
                } else {
                    let nlit = lit.ref_negate();
                    match {
                        if conflict.is_empty() {
                            self.assume(nlit, deps.clone())?.unsat()
                        } else {
                            let mut new = self.clone();
                            new.δ.extend(conflict.iter().cloned());
                            new.assume(nlit, deps.clone())?.unsat()
                        }
                    } {
                        Ok(empty) => match empty {},
                        Err(sat_res @ Out::Sat(_)) => return Err(sat_res),
                        Err(Out::Unsat((new_deps, new_conflict))) => {
                            conflict.extend(new_conflict);
                            let conflict_clause =
                                LClause::new_with(Clause::new(vec![lit.ref_negate()]), deps);
                            conflict.insert(conflict_clause);
                            raise!(unsat(new_deps, conflict))
                        }
                    }
                }
            } else {
                panic!("illegal empty disjunct in application of `unsat` rule")
            }
        }
    }

    pub fn solve(&self) -> Outcome<Lit, ()> {
        match self.unsat() {
            Err(res) => res.into_unit_unsat(),
            Ok(empty) => match empty {},
        }
    }
}

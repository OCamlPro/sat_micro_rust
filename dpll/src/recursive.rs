//! Functional versions taken directly from the paper.

prelude!();

mod backjump;
mod plain;

pub use self::backjump::Backjump;
pub use self::plain::Plain;

pub fn solve<F>(f: F, dpll: Dpll) -> Result<Outcome<F::Lit, ()>, String>
where
    F: Formula,
{
    match dpll {
        Dpll::Plain => Ok(Plain::new(f).solve()),
        Dpll::Backjump => Ok(Backjump::new(f).solve()),
        Dpll::Cdcl => Err("CDCL solver is not implemented yet".into()),
    }
}

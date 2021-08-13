# SAT-MICRO, but in Rust

This project is a reimplementation in Rust of the [SAT-solver] described in [SAT-MICRO: petit mais
costaud!][link] by Sylvain Conchon *et al*. As the title suggests, the paper is written in French.

In it, the authors recall readers of the [DPLL] algorithm and describe an implementation of plain
DPLL in OCaml. Two other versions are presented which optimize the plain implementation. The first
optimization is to add *backjumping* (or *non-chronological backtracking*) to the plain version,
and the second adds CDCL (Conflict-Driven Clause Learning) on top of backjumping.

This work is interesting because DPLL is often seen as naturally **imperative** algorithm. OCaml
being a **functional** language, the authors take a purely functional approach and show that
deriving the code from the rules (and, later, optimized rules) is quite natural. *Purely
functional* here means *recursive* and *without any kind of side-effect*.

Now, given the purely functional approach and the language used, SAT-MICRO has no hope to compete
with extremely optimized SAT solvers written in C, such as [lingeling]. That's not the point.

The first point is to have a nice, readable implementation of DPLL that's close to the actual
rules that describe the algorithm. This is typically valuable when teaching SAT-solving.

The second point is quite strong: each version was actually proved correct using the [Coq] proof
assistant. This is because functional programs are much easier to reason about with logics than
imperative programs. Imperative programs, especially heavily optimized ones, routinely rely on
side-effects, pointer arithmetic, and generally potentially unsafe operations. While this allows
them to be very efficient, it makes them difficult to reason about for humans and thus difficult to
write, maintain and improve. It also makes manual/automatic *program verification* a lot more
difficult.


[SAT-solver]: https://en.wikipedia.org/wiki/Boolean_satisfiability_problem
(SAT on wikipedia)
[link]: https://hal.inria.fr/inria-00202831/document
(SAT-MICRO paper on HAL)
[DPLL]: https://en.wikipedia.org/wiki/Boolean_satisfiability_problem#Algorithms_for_solving_SAT
(SAT algorithms on wikipedia)
[lingeling]: http://fmv.jku.at/lingeling
(lingeling official website)
[Coq]: https://coq.inria.fr
(Coq official website)
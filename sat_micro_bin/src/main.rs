sat_micro::front::prelude!();

use std::time::{Duration, Instant};

use clap::SubCommand;
use sat_micro::{dpll, front};

pub type Matches = clap::ArgMatches<'static>;

pub fn dpll_subcommands() -> impl Iterator<Item = clap::App<'static, 'static>> {
    dpll::Dpll::NAMES
        .into_iter()
        .map(|(name, about)| clap::SubCommand::with_name(name).about(*about))
}
pub fn dpll_impl_subcommands() -> impl Iterator<Item = clap::App<'static, 'static>> {
    dpll::DpllImpl::NAMES.into_iter().map(|(name, about)| {
        clap::SubCommand::with_name(name)
            .about(*about)
            .subcommands(dpll_subcommands())
    })
}
pub fn dpll_impl_from_matches(matches: &Matches) -> Res<Option<dpll::DpllImpl>> {
    match matches.subcommand() {
        ("all", Some(_)) => Ok(None),
        (dpll_impl_name, Some(sub_matches)) => match sub_matches.subcommand() {
            (dpll_name, Some(_)) => dpll::DpllImpl::from_name(dpll_impl_name, Some(dpll_name))
                .ok_or_else(|| {
                    format!(
                        "unknown DPLL combination `{}/{}`",
                        dpll_impl_name, dpll_name
                    )
                    .into()
                })
                .map(Some),
            (_, None) => dpll::DpllImpl::from_name(dpll_impl_name, None)
                .ok_or_else(|| format!("unknown DPLL implementation `{}`", dpll_impl_name).into())
                .map(Some),
        },
        (_, None) => Ok(Some(dpll::DpllImpl::default())),
    }
}

fn main() {
    use clap::{crate_authors, crate_description, crate_version, App, Arg};
    let matches = App::new("sat_micro")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("VERB")
                .short("v")
                .multiple(true)
                .help("Increases verbosity"),
        )
        .arg(
            Arg::with_name("FILE")
                .required(true)
                .help("Input file (SAT-comp format)"),
        )
        .subcommands(dpll_impl_subcommands())
        .subcommand(SubCommand::with_name("all"))
        .get_matches();

    // Handles verbosity CLAP and logger setup. Keep this as the first CLAP step so that we can use
    // logging ASAP.
    {
        let log_level = match matches.occurrences_of("VERB") {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };
        simplelog::SimpleLogger::init(log_level, simplelog::Config::default())
            .expect("fatal error during logger initialization");
    }

    let cnf_file_path = matches
        .value_of("FILE")
        .expect("unreachable: `FILE` argument is mandatory");

    match run(cnf_file_path, &matches) {
        Ok(()) => std::process::exit(0),
        Err(errors) => {
            eprintln!("|===| Error(s):");
            for (idx, error) in errors.iter().enumerate() {
                if idx > 0 {
                    eprintln!("| ")
                }
                for e in error.iter() {
                    let e = e.to_string();
                    for (idx, line) in e.to_string().lines().enumerate() {
                        if idx == 0 {
                            eprint!("| - ")
                        } else {
                            eprint!("|   ")
                        }
                        eprintln!("{}", line)
                    }
                }
            }
            eprintln!("|===|");
            std::process::exit(2)
        }
    }
}

pub fn run(
    cnf_file_path: impl AsRef<std::path::Path>,
    matches: &Matches,
) -> Result<(), Vec<err::Error>> {
    let dpll = dpll_impl_from_matches(matches).map_err(|e| vec![e])?;
    let cnf_file_path = cnf_file_path.as_ref();
    let xz_compressed = match cnf_file_path.extension() {
        Some(ext) if "cnf" == ext => false,
        Some(ext) if "xz" == ext => true,
        _ => {
            return Err(vec![format!(
                "could not retrieve extension from `{}`, expected `.cnf` or `.xz`",
                cnf_file_path.display()
            )
            .into()])
        }
    };

    use front::parse::Parser;

    log::debug!("creating parser...");
    if xz_compressed {
        parse_run(
            Parser::open_xz_file(cnf_file_path)
                .chain_err(|| "while creating xz parser")
                .map_err(|e| vec![e])?,
            dpll,
        )
    } else {
        parse_run(
            Parser::open_file(cnf_file_path)
                .chain_err(|| "while creating uncompressed parser")
                .map_err(|e| vec![e])?,
            dpll,
        )
    }
}

pub fn parse_run<R: std::io::Read>(
    parser: front::parse::Parser<R>,
    dpll: Option<DpllImpl>,
) -> Result<(), Vec<err::Error>> {
    let parse_start = Instant::now();
    log::debug!("running parser...");
    let cnf = parser.parse().map_err(|e| vec![e])?;
    let parse_end = Instant::now();

    let parse_time = parse_end - parse_start;
    println!("c done parsing in {} seconds", parse_time.as_secs_f64());

    log::debug!("parsed {} conjunct(s)", cnf.len());
    if log::log_enabled!(log::Level::Trace) {
        for clause in cnf.iter() {
            log::trace!("    {}", clause);
        }
    }

    let results = match dpll {
        Some(dpll) => {
            log::info!("running {}", dpll);
            let res = run_one(cnf, dpll).chain_err(|| format!("while running {}", dpll));
            vec![res]
        }
        None => {
            let all = [
                DpllImpl::Recursive(Dpll::Plain),
                DpllImpl::Recursive(Dpll::Backjump),
                DpllImpl::Recursive(Dpll::Cdcl),
            ];
            if log::log_enabled!(log::Level::Info) {
                log::info!("running the following dpll variants:");
                for dpll in &all {
                    log::info!("- {}", dpll)
                }
            }

            use rayon::prelude::*;
            all.par_iter()
                .map(|dpll| run_one(cnf.clone(), *dpll))
                .collect()
        }
    };

    let mut is_sat = None;
    let mut errors = Vec::<err::Error>::new();

    for res in results {
        let res = res.and_then(|(dpll, this_outcome, time)| {
            let sat = this_outcome.map(sat_action, unsat_action)?;
            const SAT: &str = "SATISFIABLE";
            const UNSAT: &str = "UNSATISFIABLE";
            println!(
                "c {: >40} | {: >13} | {: >15.9} seconds",
                dpll.to_string(),
                if sat { SAT } else { UNSAT },
                time.as_secs_f64()
            );
            if is_sat.is_none() {
                is_sat = Some(sat)
            } else if is_sat != Some(sat) {
                errors.push(format!("results do not agree on satisfiability").into())
            }
            Ok(())
        });
        match res {
            Ok(()) => (),
            Err(e) => errors.push(e),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
fn run_one(
    cnf: dpll::Cnf<front::Lit>,
    dpll: DpllImpl,
) -> Res<(DpllImpl, dpll::Outcome<front::Lit, ()>, Duration)> {
    let start = Instant::now();
    let res = dpll::solve(cnf, dpll)?;
    let end = Instant::now();

    log::info!("{} is done", dpll);

    let time = end - start;
    // let parsing = parse_end - start;
    // let solving = total - parsing;
    // println!("c");
    // println!("c | runtime breakdown for {}", dpll);
    // println!(
    //     "c | parsing: {: >10}.{:0>9} seconds",
    //     parsing.as_secs(),
    //     parsing.subsec_nanos()
    // );
    // println!(
    //     "c | solving: {: >10}.{:0>9} seconds",
    //     solving.as_secs(),
    //     solving.subsec_nanos()
    // );
    // println!(
    //     "c | total:   {: >10}.{:0>9} seconds",
    //     total.as_secs(),
    //     total.subsec_nanos()
    // );

    Ok((dpll, res, time))
}
fn sat_action(_model: Set<front::Lit>) -> Res<bool> {
    // println!("s SATISFIABLE");
    // for lit in &_model {
    //     println!("    {}", lit)
    // }
    for lit in &_model {
        let nlit = lit.ref_negate();
        if _model.contains(&nlit) {
            return Err(format!(
                "[fatal] inconsistent model contains both {} and {}",
                lit, nlit
            )
            .into());
        }
    }
    Ok(true)
}
fn unsat_action(_: ()) -> Res<bool> {
    // println!("s UNSATISFIABLE");
    Ok(false)
}

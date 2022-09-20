use std::time::Instant;

use sat_micro::{dpll, front, front::prelude::*};

use crate::conf::*;

pub mod conf;

fn main() {
    let conf = Conf::new();
    // Handles verbosity CLAP and logger setup. Keep this as the first CLAP step so that we can use
    // logging ASAP.
    simplelog::SimpleLogger::init(conf.log_level, simplelog::Config::default())
        .expect("fatal error during logger initialization");

    match run(conf) {
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

pub fn run(conf: Conf1) -> Result<(), Vec<err::Error>> {
    let conf = conf.extract_dpll().map_err(|e| vec![e])?;

    let cnf_file_path = std::path::PathBuf::from(&conf.file);
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

    let expecting_sat = conf.expecting_sat.clone();

    log::debug!("creating parser...");
    let is_sat = if xz_compressed {
        parse_run(
            Parser::open_xz_file(cnf_file_path)
                .chain_err(|| "while creating xz parser")
                .map_err(|e| vec![e])?,
            conf,
        )?
    } else {
        parse_run(
            Parser::open_file(cnf_file_path)
                .chain_err(|| "while creating uncompressed parser")
                .map_err(|e| vec![e])?,
            conf,
        )?
    };

    const SAT: &str = "SATISFIABLE";
    const UNSAT: &str = "UNSATISFIABLE";
    const UNK: &str = "UNKNOWN";
    match is_sat {
        Some(true) => {
            println!("s {}", SAT);
            match expecting_sat {
                Some(false) => bail!(vec!["expected unsat result, got sat".into()]),
                Some(true) | None => (),
            }
        }
        Some(false) => {
            println!("s {}", UNSAT);
            match expecting_sat {
                Some(true) => bail!(vec!["expect sat result, got unsat".into()]),
                Some(false) | None => (),
            }
        }
        None => println!("s {}", UNK),
    }

    Ok(())
}

pub fn parse_run<R: std::io::Read>(
    parser: front::parse::Parser<R>,
    conf: Conf2,
) -> Result<Option<bool>, Vec<err::Error>> {
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

    if let Some(timeout) = conf.time_left() {
        use std::sync::mpsc;
        let (sender, recver) = mpsc::channel();
        let _ = std::thread::spawn(move || {
            let res = run_all(conf, cnf);
            let _ = sender.send(res);
        });
        match recver.recv_timeout(timeout) {
            Ok(res) => res,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                println!("c TIMEOUT");
                Ok(None)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                bail!(vec!["unexpected deconnection from solver subprocess".into()])
            }
        }
    } else {
        run_all(conf, cnf)
    }
}

fn run_all(conf: Conf2, cnf: Cnf<Lit>) -> Result<Option<bool>, Vec<err::Error>> {
    let results = match conf.dpll {
        Some(dpll) => {
            println!("c running {}", dpll);
            let res = run_one(&conf, cnf, dpll).chain_err(|| format!("while running {}", dpll));
            vec![res]
        }
        None => {
            let all = [
                DpllImpl::Recursive(Dpll::Plain),
                DpllImpl::Recursive(Dpll::Backjump),
                DpllImpl::Recursive(Dpll::Cdcl),
            ];
            for dpll in &all {
                println!("c running {}", dpll);
            }
            if log::log_enabled!(log::Level::Info) {
                log::info!("running the following dpll variants:");
                for dpll in &all {
                    log::info!("- {}", dpll)
                }
            }

            use rayon::prelude::*;
            all.par_iter()
                .map(|dpll| run_one(&conf, cnf.clone(), *dpll))
                .collect()
        }
    };

    let mut is_sat = None;
    let mut errors = Vec::<err::Error>::new();

    for res in results {
        let res = res.and_then(|this_outcome| {
            let sat = this_outcome.map_ref(|m| sat_action(conf.check_models, m), unsat_action)?;
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

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(is_sat)
}
fn run_one(
    conf: &Conf2,
    cnf: dpll::Cnf<front::Lit>,
    dpll: DpllImpl,
) -> Res<dpll::Outcome<front::Lit, ()>> {
    let start = Instant::now();
    let res = dpll::solve(cnf, dpll)?;
    let end = Instant::now();

    log::info!("{} is done", dpll);

    let time = end - start;

    println!(
        "c {: >40} | {: ^5} | {: >15.9} seconds",
        dpll.to_string(),
        if res.map_ref(|m| sat_action(conf.check_models, m), unsat_action)? {
            "sat"
        } else {
            "unsat"
        },
        time.as_secs_f64()
    );

    Ok(res)
}
fn sat_action(check_models: bool, _model: &Set<front::Lit>) -> Res<bool> {
    // println!("s SATISFIABLE");
    // for lit in &_model {
    //     println!("    {}", lit)
    // }
    if check_models {
        for lit in _model {
            let nlit = lit.ref_negate();
            if _model.contains(&nlit) {
                return Err(format!(
                    "[fatal] inconsistent model contains both {} and {}",
                    lit, nlit
                )
                .into());
            }
        }
    }
    Ok(true)
}
fn unsat_action(_: &()) -> Res<bool> {
    // println!("s UNSATISFIABLE");
    Ok(false)
}

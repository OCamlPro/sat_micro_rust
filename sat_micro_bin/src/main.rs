sat_micro::front::prelude!();

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
pub fn dpll_impl_from_matches(matches: &Matches) -> Res<dpll::DpllImpl> {
    match matches.subcommand() {
        (dpll_impl_name, Some(sub_matches)) => match sub_matches.subcommand() {
            (dpll_name, Some(_)) => dpll::DpllImpl::from_name(dpll_impl_name, Some(dpll_name))
                .ok_or_else(|| {
                    format!(
                        "unknown DPLL combination `{}/{}`",
                        dpll_impl_name, dpll_name
                    )
                    .into()
                }),
            (_, None) => dpll::DpllImpl::from_name(dpll_impl_name, None)
                .ok_or_else(|| format!("unknown DPLL implementation `{}`", dpll_impl_name).into()),
        },
        (_, None) => Ok(dpll::DpllImpl::default()),
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
        Err(e) => {
            eprintln!("|===| Error:");
            for e in e.iter() {
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
            eprintln!("|===|");
            std::process::exit(2)
        }
    }
}

pub fn run(cnf_file_path: impl AsRef<std::path::Path>, matches: &Matches) -> Res<()> {
    let dpll = dpll_impl_from_matches(matches)?;
    let cnf_file_path = cnf_file_path.as_ref();
    let xz_compressed = match cnf_file_path.extension() {
        Some(ext) if "cnf" == ext => false,
        Some(ext) if "xz" == ext => true,
        _ => bail!(
            "could not retrieve extension from `{}`, expected `.cnf` or `.xz`",
            cnf_file_path.display()
        ),
    };

    use front::parse::Parser;

    log::debug!("creating parser...");
    if xz_compressed {
        parse_run(
            Parser::open_xz_file(cnf_file_path).chain_err(|| "while creating xz parser")?,
            dpll,
        )
    } else {
        parse_run(
            Parser::open_file(cnf_file_path).chain_err(|| "while creating uncompressed parser")?,
            dpll,
        )
    }
}

pub fn parse_run<R: std::io::Read>(parser: front::parse::Parser<R>, dpll: DpllImpl) -> Res<()> {
    use std::time::Instant;

    let start = Instant::now();
    log::debug!("running parser...");
    let cnf = parser.parse()?;
    let parse_end = Instant::now();

    log::debug!("done parsing {} conjunct(s)", cnf.len());
    if log::log_enabled!(log::Level::Trace) {
        for clause in cnf.iter() {
            log::trace!("    {}", clause);
        }
    }

    log::info!("running {}", dpll);

    dpll::solve(cnf, dpll)?.map(
        // Sat action.
        |_model| {
            println!("s SATISFIABLE");
            for lit in &_model {
                println!("    {}", lit)
            }
            for lit in &_model {
                let nlit = lit.ref_negate();
                if _model.contains(&nlit) {
                    return Err(format!(
                        "[fatal] inconsistent model contains both {} and {}",
                        lit, nlit
                    ));
                }
            }
            Ok(())
        },
        // Unsat action.
        |()| {
            println!("s UNSATISFIABLE");
            Ok(())
        },
    )?;
    let run_end = Instant::now();

    let total = run_end - start;
    let parsing = parse_end - start;
    let solving = total - parsing;
    println!("c");
    println!("c | runtime breakdown");
    println!(
        "c | parsing: {: >10}.{:0>9} seconds",
        parsing.as_secs(),
        parsing.subsec_nanos()
    );
    println!(
        "c | solving: {: >10}.{:0>9} seconds",
        solving.as_secs(),
        solving.subsec_nanos()
    );
    println!(
        "c | total:   {: >10}.{:0>9} seconds",
        total.as_secs(),
        total.subsec_nanos()
    );

    Ok(())
}

sat_micro::front::prelude!();

use sat_micro::{dpll, front};

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

    match run(cnf_file_path) {
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

pub fn run(cnf_file_path: impl AsRef<std::path::Path>) -> Res<()> {
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
        parse_run(Parser::open_xz_file(cnf_file_path).chain_err(|| "while creating xz parser")?)
    } else {
        parse_run(
            Parser::open_file(cnf_file_path).chain_err(|| "while creating uncompressed parser")?,
        )
    }
}

pub fn parse_run<R: std::io::Read>(parser: front::parse::Parser<R>) -> Res<()> {
    log::debug!("running parser...");
    let cnf = parser.parse()?;

    log::debug!("done parsing {} conjunct(s)", cnf.len());
    if log::log_enabled!(log::Level::Trace) {
        for clause in cnf.iter() {
            log::trace!("    {}", clause);
        }
    }

    log::info!("running naive functional DPLL");

    use dpll::functional::*;
    Plain::new(cnf).solve().map(
        // Sat action.
        |_model| {
            println!("s SATISFIABLE");
            // for lit in model {
            //     println!("    {}", lit)
            // }
        },
        // Unsat action.
        || println!("s UNSATISFIABLE"),
    );

    Ok(())
}

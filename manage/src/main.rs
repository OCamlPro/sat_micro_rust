//! Workspace and benchmark manager.

/// Imports this crate's prelude.
///
/// Pass `pub` when calling this macro to make the imports public.
#[macro_export]
macro_rules! prelude {
    {} => { use $crate::prelude::*; };
    {pub } => { pub use $crate::prelude::*; };
}

/// Common traits and types defined by this crate.
///
/// See also the [`prelude!`] macro.
pub mod prelude {
    base::prelude!(pub);

    pub use std::{
        path::{Path, PathBuf},
        process::Command,
    };

    pub use error_chain::bail;

    pub use crate::err::{Res, ResExt};

    /// Alias for `'static` versions of [`clap::App`].
    pub type App = clap::App<'static, 'static>;
    /// Alias for `'static` versions of [`clap::ArgMatches`].
    pub type Matches = clap::ArgMatches<'static>;
}

prelude!();

pub mod err;
pub mod split;

/// Subcommands (CLAP modes) as static [`str`]s.
pub mod sub {
    /// Benchmark mode.
    pub const BENCHS: &str = "benchs";
    /// Benchmark mode submodes.
    pub mod benchs {
        /// Benchmark-get submode.
        pub const GET: &str = "get";
        /// Benchmark-run submode.
        pub const RUN: &str = "get";
    }
}

fn main() {
    use clap::{crate_authors, crate_description, crate_version, App, Arg};
    let matches = App::new("manage")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("VERB")
                .short("v")
                .multiple(true)
                .help("Increases verbosity"),
        )
        .subcommand(split::Split::subcommand())
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

    match run(matches) {
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

pub fn run(matches: Matches) -> Res<()> {
    if let Some(split) =
        split::Split::new(&matches).chain_err(|| "[clap] while parsing `split` subcommand")?
    {
        let stats = split.run().chain_err(|| "while running split subcommand")?;
        let split_type = if split.move_files { "moved" } else { "copied" };
        log::info!(
            "done splitting {} benchmark(s) with `{}`:",
            stats.all(),
            split.pretty_solver_cmd()
        );
        log::info!(
            "- {} sat, {} to {}",
            stats.sat,
            split_type,
            split.sat_tgt.display()
        );
        log::info!(
            "- {} unsat, {} to {}",
            stats.unsat,
            split_type,
            split.unsat_tgt.display()
        );
        log::info!(
            "- {} unknown, {} to {}",
            stats.unknown,
            split_type,
            split.unknown_tgt.display()
        );

        return Ok(());
    }

    log::warn!("no subcommand selected, exiting");

    Ok(())
}

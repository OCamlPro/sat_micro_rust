//! Configuration stuff.

use std::time::{Duration, Instant};

use clap::Command;
use log::LevelFilter;
use sat_micro::{dpll, front::prelude::*};

pub type Matches = clap::ArgMatches;

pub fn dpll_subcommands() -> impl Iterator<Item = Command> {
    dpll::Dpll::NAMES
        .into_iter()
        .map(|(name, about)| Command::new(name).about(*about))
}
pub fn dpll_impl_subcommands() -> impl Iterator<Item = Command> {
    dpll::DpllImpl::NAMES.into_iter().map(|(name, about)| {
        Command::new(name)
            .about(*about)
            .subcommands(dpll_subcommands())
    })
}
pub fn dpll_impl_from_matches(matches: &Matches) -> Res<Option<dpll::DpllImpl>> {
    match matches.subcommand() {
        Some(("all", _)) => Ok(None),
        Some((dpll_impl_name, sub_matches)) => match sub_matches.subcommand() {
            Some((dpll_name, _)) => dpll::DpllImpl::from_name(dpll_impl_name, Some(dpll_name))
                .ok_or_else(|| {
                    format!(
                        "unknown DPLL combination `{}/{}`",
                        dpll_impl_name, dpll_name
                    )
                    .into()
                })
                .map(Some),
            None => dpll::DpllImpl::from_name(dpll_impl_name, None)
                .ok_or_else(|| format!("unknown DPLL implementation `{}`", dpll_impl_name).into())
                .map(Some),
        },
        None => Ok(Some(dpll::DpllImpl::default())),
    }
}

pub type Conf1 = Conf<Res<Option<DpllImpl>>>;
pub type Conf2 = Conf<Option<DpllImpl>>;

pub struct Conf<D> {
    pub start: Instant,
    pub file: String,
    pub dpll: D,
    pub log_level: LevelFilter,
    pub timeout_ms: Option<u64>,
    pub expecting_sat: Option<bool>,
    pub check_models: bool,
}
impl Conf1 {
    fn validate_bool(s: &str) -> Result<bool, String> {
        match s {
            "on" | "true" | "On" | "True" => Ok(true),
            "off" | "false" | "Off" | "False" => Ok(false),
            _ => Err(format!("expected boolean `on|true|off|false`, got `{}`", s)),
        }
    }
    fn validate_expect_sat(s: &str) -> Result<bool, String> {
        match s {
            "sat" => Ok(true),
            "unsat" => Ok(false),
            _ => Err(format!("expected `sat|unsat`, got `{}`", s)),
        }
    }
    fn validate_timeout(s: &str) -> Result<u64, String> {
        match u64::from_str_radix(&s, 10) {
            Ok(res) => Ok(res),
            Err(_) => Err(format!("expected integer, got `{}`", s)),
        }
    }

    pub fn new() -> Self {
        use clap::{crate_authors, crate_description, crate_version, Arg};
        let matches = Command::new("sat_micro")
            .version(crate_version!())
            .author(crate_authors!())
            .about(crate_description!())
            .arg(
                Arg::new("VERB")
                    .short('v')
                    .num_args(0..)
                    .help("Increases verbosity"),
            )
            .arg(
                Arg::new("EXPECTED")
                    .value_name("sat|unsat")
                    .long("expect")
                    .num_args(1)
                    .value_parser(Conf1::validate_expect_sat)
                    .help("Specifies the result expected, `sat` or `unsat`"),
            )
            .arg(
                Arg::new("CHECK")
                    .value_name("on|true|off|false")
                    .long("check")
                    .num_args(1)
                    .value_parser(Conf1::validate_bool)
                    .default_value("off")
                    .help("(De)activates model checking, [on|off|true|false]"),
            )
            .arg(
                Arg::new("TIMEOUT")
                    .value_name("INT")
                    .long("timeout")
                    .short('t')
                    .num_args(1)
                    .value_parser(Conf1::validate_timeout)
                    .help("Specifies a timeout in milliseconds, must be ≥ 0"),
            )
            .subcommands(dpll_impl_subcommands())
            .subcommand(Command::new("all").about("Runs all DPLL variants"))
            .arg(
                Arg::new("FILE")
                    .required(true)
                    .help("Input file (SAT-comp format)"),
            )
            .get_matches();

        let log_level = match matches.get_occurrences::<()>("VERB").iter().count() {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };
        let expecting_sat = matches.get_one::<bool>("EXPECTED").cloned();
        let timeout_ms = matches.get_one("TIMEOUT").cloned();
        let check_models = *matches
            .get_one("CHECK")
            .expect("arguments with default value cannot be absent");

        let dpll = dpll_impl_from_matches(&matches);

        let file = matches
            .get_one::<String>("FILE")
            .expect("unreachable: `FILE` argument is mandatory")
            .clone();

        Self {
            start: Instant::now(),
            file,
            check_models,
            dpll,
            log_level,
            timeout_ms,
            expecting_sat,
        }
    }

    pub fn extract_dpll(self) -> Res<Conf2> {
        let Self {
            file,
            start,
            dpll,
            log_level,
            timeout_ms,
            expecting_sat,
            check_models,
        } = self;
        let dpll = dpll?;
        Ok(Conf2 {
            file,
            start,
            dpll,
            log_level,
            timeout_ms,
            expecting_sat,
            check_models,
        })
    }
}
impl<D> Conf<D> {
    pub fn time_left(&self) -> Option<Duration> {
        self.timeout_ms.clone().map(|millis| {
            let timeout = Duration::from_millis(millis);
            let elapsed = Instant::now() - self.start;
            timeout.saturating_sub(elapsed)
        })
    }
}

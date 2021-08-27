//! Configuration stuff.

use std::time::{Duration, Instant};

use clap::SubCommand;
use log::LevelFilter;

use sat_micro::dpll;

sat_micro::front::prelude!();

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
    pub fn new() -> Self {
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
                Arg::with_name("EXPECTED")
                    .long("expect")
                    .takes_value(true)
                    .validator(|s| match &s as &str {
                        "sat" | "unsat" => Ok(()),
                        _ => Err(format!("expected `sat` or `unsat`, got `{}`", s)),
                    })
                    .help("Specifies the result expected, `sat` or `unsat`"),
            )
            .arg(
                Arg::with_name("CHECK")
                    .long("check")
                    .takes_value(true)
                    .validator(|s| match &s as &str {
                        "on" | "off" | "true" | "false" => Ok(()),
                        _ => Err(format!("expected [on|off|true|false], got `{}`", s)),
                    })
                    .default_value("off")
                    .help("(De)activates model checking, [on|off|true|false]"),
            )
            .arg(
                Arg::with_name("TIMEOUT")
                    .long("timeout")
                    .short("t")
                    .takes_value(true)
                    .validator(|s| match u64::from_str_radix(&s, 10) {
                        Ok(_) => Ok(()),
                        Err(_) => Err(format!("expected integer, got `{}`", s)),
                    })
                    .help("Specifies a timeout in milliseconds"),
            )
            .subcommands(dpll_impl_subcommands())
            .subcommand(SubCommand::with_name("all"))
            .arg(
                Arg::with_name("FILE")
                    .required(true)
                    .help("Input file (SAT-comp format)"),
            )
            .get_matches();

        let log_level = match matches.occurrences_of("VERB") {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };
        let expecting_sat = matches.value_of("EXPECTED").map(|s| match s {
            "sat" => true,
            "unsat" => false,
            s => panic!(
                "expected result has validator but got an illegal value `{}`",
                s
            ),
        });
        let timeout_ms = matches
            .value_of("TIMEOUT")
            .map(|s| match u64::from_str_radix(s, 10) {
                Ok(n) => n,
                Err(e) => panic!(
                    "timeout has validator but got an illegal value `{}`: {}",
                    s, e
                ),
            });
        let check_models = match matches
            .value_of("CHECK")
            .expect("arguments with default value cannot be absent")
        {
            "on" | "true" => true,
            "off" | "false" => false,
            val => panic!(
                "model-check has validator but got an illegal value `{}`",
                val
            ),
        };

        let dpll = dpll_impl_from_matches(&matches);

        let file = matches
            .value_of("FILE")
            .expect("unreachable: `FILE` argument is mandatory")
            .into();

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

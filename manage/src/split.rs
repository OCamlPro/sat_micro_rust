//! Splits benchmarks in some folder as sat, unsat, and unknown categories.

prelude!();

/// Result produced by the [`Split::run`] function.
///
/// Details how many benchmarks were found sat, unsat, and unknown.
#[derive(Debug, Clone)]
pub struct Stats {
    /// Number of `sat` benchmarks.
    pub sat: usize,
    /// Number of `unsat` benchmarks.
    pub unsat: usize,
    /// Number of `unknown` benchmarks.
    pub unknown: usize,
}
impl Stats {
    /// Empty constructor.
    pub fn new() -> Self {
        Self {
            sat: 0,
            unsat: 0,
            unknown: 0,
        }
    }
    /// Total number of benchmarks treated.
    pub fn all(&self) -> usize {
        self.sat + self.unsat + self.unknown
    }
    /// Updates statistics given a [`RunRes`].
    pub fn add(&mut self, res: RunRes) {
        match res {
            RunRes::Sat => self.sat += 1,
            RunRes::Unsat => self.unsat += 1,
            RunRes::Unknown => self.unknown += 1,
        }
    }
}

/// Result of a solver run.
#[derive(Debug, Clone, Copy)]
pub enum RunRes {
    Sat,
    Unsat,
    Unknown,
}
implem! {
    for RunRes {
        Display {
            |&self, fmt| match self {
                Self::Sat => "sat".fmt(fmt),
                Self::Unsat => "unsat".fmt(fmt),
                Self::Unknown => "unknown".fmt(fmt),
            }
        }
    }
}
impl RunRes {
    /// Builds a solver run result from a command.
    pub fn from_cmd(mut cmd: Command) -> Res<Self> {
        let output = cmd
            .output()
            .chain_err(|| format!("error running solver command"))?;
        let out = String::from_utf8_lossy(&output.stdout);

        let mut res = None;
        for line in out.lines().filter(|s| s.len() >= 2 && &s[0..2] == "s ") {
            let mut split = line.split_ascii_whitespace();
            let new_res = match (split.next(), split.next(), split.next()) {
                (Some("s"), Some("SATISFIABLE"), None) => Self::Sat,
                (Some("s"), Some("UNSATISFIABLE"), None) => Self::Unsat,
                (Some("s"), Some("UNKNOWN"), None) => Self::Unknown,
                _ => bail!("illegal solver output, unexpected line `{}`", line),
            };

            res = match res {
                None => Some(new_res),
                Some(res) => {
                    bail!(
                        "illegal solver output, found at least two results ({} and {})",
                        res,
                        new_res
                    )
                }
            };
        }

        Ok(res.unwrap_or(Self::Unknown))
    }
}

/// Split configuration.
pub struct Split {
    /// If true, move files instead of copying them.
    pub move_files: bool,
    /// Source folder, contains all benchmarks.
    pub src: PathBuf,
    /// Copy/move sat benchmarks here.
    pub sat_tgt: PathBuf,
    /// Copy/move unsat benchmarks here.
    pub unsat_tgt: PathBuf,
    /// Copy/move unknown benchmarks here.
    pub unknown_tgt: PathBuf,
    /// Solver command and arguments, used to decide satisfiability.
    pub solver: (String, Vec<String>),
}

impl Split {
    /// Builds a solver command.
    pub fn solver_cmd(&self) -> Command {
        let (cmd, args) = &self.solver;
        let mut cmd = Command::new(cmd);
        cmd.args(args);
        cmd
    }

    /// Pretty string representing the solver command.
    pub fn pretty_solver_cmd(&self) -> String {
        let mut res = self.solver.0.clone();
        for arg in &self.solver.1 {
            res.push_str(" ");
            res.push_str(arg);
        }
        res
    }

    /// Target directory based on a run result.
    pub fn tgt_for(&self, res: RunRes) -> &PathBuf {
        match res {
            RunRes::Sat => &self.sat_tgt,
            RunRes::Unsat => &self.unsat_tgt,
            RunRes::Unknown => &self.unknown_tgt,
        }
    }

    /// Runs the solver on a single benchmark.
    pub fn run_on(&self, bench: impl AsRef<Path>) -> Res<RunRes> {
        let bench = bench.as_ref();
        let mut cmd = self.solver_cmd();
        cmd.arg(bench);
        let res = RunRes::from_cmd(cmd)
            .chain_err(|| format!("failed to run solver on {}", bench.display()))?;

        // Copy bench to proper location.
        let tgt = {
            let mut tgt = self.tgt_for(res).clone();
            let bench_basename = bench.file_name().ok_or_else(|| {
                format!(
                    "illegal file path `{}`, cannot retrieve basename",
                    bench.display()
                )
            })?;
            tgt.push(bench_basename);
            tgt
        };
        std::fs::copy(bench, &tgt)
            .chain_err(|| format!("while copying `{}` to `{}`", bench.display(), tgt.display()))?;

        // Delete original file if needed.
        if self.move_files {
            std::fs::remove_file(bench)
                .chain_err(|| format!("while deleting file `{}`", bench.display()))?
        }

        Ok(res)
    }

    /// Runs benchmark splitting.
    ///
    /// Returns
    pub fn run(&self) -> Res<Stats> {
        use rayon::prelude::*;
        use std::sync::RwLock;

        macro_rules! entries {
            {} => {
                std::fs::read_dir(&self.src)
                    .chain_err(|| format!("while reading directory `{}`", self.src.display()))?
                    .filter_map(|entry| match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.is_file()
                                && path
                                    .extension()
                                    .map(|ext| "cnf" == ext || "xz" == ext)
                                    .unwrap_or(false)
                            {
                                Some(path)
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    })
            }
        }

        let file_count = entries!().count() as u64;

        let mut src_reader = entries!();

        {
            use std::fs::create_dir_all;
            log::debug!(
                "creating target directories `{}`, `{}` and `{}`",
                self.sat_tgt.display(),
                self.unsat_tgt.display(),
                self.unknown_tgt.display()
            );
            create_dir_all(&self.sat_tgt)
                .chain_err(|| format!("while creating directory `{}`", self.sat_tgt.display()))?;
            create_dir_all(&self.unsat_tgt)
                .chain_err(|| format!("while creating directory `{}`", self.unsat_tgt.display()))?;
            create_dir_all(&self.unknown_tgt).chain_err(|| {
                format!("while creating directory `{}`", self.unknown_tgt.display())
            })?;
        }

        let mut stats = Stats::new();
        let progress = {
            let bar = indicatif::ProgressBar::new(file_count);
            bar.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:70.cyan/blue} {pos:>7}/{len:7} {msg}")
                    .on_finish(indicatif::ProgressFinish::AndClear),
            );
            bar.set_draw_delta(file_count);
            bar
        };
        let update_progress = move |stats: &mut Stats, res: Option<RunRes>| {
            if let Some(res) = res {
                stats.add(res);
            }
            match stats.all() {
                0 => progress.set_message("performing first (test) run..."),
                1 => progress.set_message("test run okay, running on everything..."),
                _ => progress.set_message(format!(
                    "{} sat, {} unsat, {} unknown",
                    stats.sat, stats.unsat, stats.unknown,
                )),
            }
            progress.set_position(stats.all() as u64);
        };
        update_progress(&mut stats, None);

        let cmd_err = |bench: &PathBuf| {
            format!(
                "while running `{}` on `{}`",
                self.pretty_solver_cmd(),
                bench.display()
            )
        };

        // We do one test run before launching everything in parallel, just in case.
        if let Some(bench) = src_reader.next() {
            let res = self.run_on(&bench).chain_err(|| cmd_err(&bench))?;
            update_progress(&mut stats, Some(res));
        }

        // Test run successful, let's do this.
        let stats = RwLock::new(stats);
        src_reader
            .par_bridge()
            .map(|bench| {
                let res = self.run_on(&bench).chain_err(|| cmd_err(&bench))?;
                let mut stats = stats
                    .write()
                    .map_err(|e| format!("[internal] synchronization lock was poisoned: {}", e))?;
                update_progress(&mut *stats, Some(res));
                Res::Ok(())
            })
            .reduce(
                || Ok(()),
                |prev, res| if prev.is_err() { prev } else { res },
            )?;

        Ok(stats.into_inner().map_err(|e| {
            format!(
                "error while retrieving split statistics, lock was poisoned: {}",
                e
            )
        })?)
    }
}

/// CLAP-related stuff.
impl Split {
    /// Split subcommand name.
    pub const SUBCOMMAND_NAME: &'static str = "split";

    const SRC_DIR_ARG: &'static str = "SPLIT_SRC_DIR";

    const TGT_DIR_ARG: &'static str = "SPLIT_TGT_DIR";

    const MOVE_ARG: &'static str = "SPLIT_MOVE";

    const SOLVER_ARG: &'static str = "SPLIT_SOLVER";
    const SOLVER_ARG_DEF: &'static str = "lingeling -T 3";

    /// Generates a [`clap`] subcommand handling option for benchmark splitting.
    pub fn subcommand() -> App {
        use clap::Arg;
        clap::SubCommand::with_name(Self::SUBCOMMAND_NAME)
            .about("splits the benchmarks in some folder into sat/unsat/unknown categories")
            .args(&[
                Arg::with_name(Self::SRC_DIR_ARG)
                    .help("Directory containing the benchmarks to split")
                    .required(true),
                Arg::with_name(Self::TGT_DIR_ARG)
                    .long("tgt")
                    .takes_value(true)
                    .help(
                        "\
                            Target directory, defaults to source directory, \
                            will be augmented with `sat`, `unsat` and `unknown` folders\
                        ",
                    ),
                Arg::with_name(Self::MOVE_ARG)
                    .help("Moves files instead of copying them")
                    .short("m")
                    .long("move"),
                Arg::with_name(Self::SOLVER_ARG)
                    .long("solver")
                    .takes_value(true)
                    .help(
                        "\
                        Command to run to decide satisfiability, \
                        must be a SAT-COMP compliant solver\
                    ",
                    )
                    .default_value(Self::SOLVER_ARG_DEF)
                    .validator(|s| {
                        if s.is_empty() {
                            Err(format!("expected a legal command, got the empty string"))
                        } else {
                            Ok(())
                        }
                    }),
            ])
    }

    /// Constructor from the **top-level** [`clap`] matches.
    ///
    /// Returns [`None`] if the split subcommand was not activated.
    pub fn new(matches: &Matches) -> Res<Option<Self>> {
        let matches = if let Some(m) = matches.subcommand_matches(Self::SUBCOMMAND_NAME) {
            m
        } else {
            return Ok(None);
        };

        let src: PathBuf = matches
            .value_of(Self::SRC_DIR_ARG)
            .expect("unwrap of required argument cannot fail")
            .into();
        let (sat_tgt, unsat_tgt, unknown_tgt) = {
            let mut tgt: PathBuf = matches
                .value_of(Self::TGT_DIR_ARG)
                .map(PathBuf::from)
                .unwrap_or_else(|| src.clone());
            let sat_tgt = {
                let mut tgt = tgt.clone();
                tgt.push("sat");
                tgt
            };
            let unsat_tgt = {
                let mut tgt = tgt.clone();
                tgt.push("unsat");
                tgt
            };
            let unknown_tgt = {
                tgt.push("unknown");
                tgt
            };
            (sat_tgt, unsat_tgt, unknown_tgt)
        };

        let move_files = matches.is_present(Self::MOVE_ARG);

        let solver = {
            let str = matches
                .value_of(Self::SOLVER_ARG)
                .unwrap_or(Self::SOLVER_ARG_DEF);
            let mut parts = str.split_ascii_whitespace();
            let cmd = if let Some(cmd) = parts.next() {
                cmd.to_string()
            } else {
                bail!("illegal solver command `{}`", str)
            };
            let args = parts.map(|s| s.to_string()).collect();
            (cmd, args)
        };

        Ok(Some(Self {
            move_files,
            src,
            sat_tgt,
            unsat_tgt,
            unknown_tgt,
            solver,
        }))
    }
}

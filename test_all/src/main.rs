use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use err::*;

mod err {
    pub use error_chain::bail;
    error_chain::error_chain! {
        types {
            ErrorKind, Error, ResExt, Res;
        }

        foreign_links {
            Io(std::io::Error);
        }
    }
}

const SAT_TAR_URL: &str =
    "https://www.cs.ubc.ca/~hoos/SATLIB/Benchmarks/SAT/RND3SAT/uf50-218.tar.gz";
const UNSAT_TAR_URL: &str =
    "https://www.cs.ubc.ca/~hoos/SATLIB/Benchmarks/SAT/RND3SAT/uuf50-218.tar.gz";

const TARGET_PATH: &str = "target/rsc";
const SAT_DL_SUBDIR: &str = "sat";
const UNSAT_DL_SUBDIR: &str = "unsat";
const SAT_SUBDIR: &str = "sat";
const UNSAT_SUBDIR: &str = "unsat/UUF50.218.1000";

const SAT_MICRO_BIN: &str = "./target/debug/sat_micro_bin";
fn sat_micro_cmd() -> Command {
    Command::new(SAT_MICRO_BIN)
}

fn target() -> PathBuf {
    PathBuf::from(TARGET_PATH)
}
fn sat_dl_target() -> PathBuf {
    let mut path = target();
    path.push(SAT_DL_SUBDIR);
    path
}
fn unsat_dl_target() -> PathBuf {
    let mut path = target();
    path.push(UNSAT_DL_SUBDIR);
    path
}
fn sat_target() -> PathBuf {
    let mut path = target();
    path.push(SAT_SUBDIR);
    path
}
fn unsat_target() -> PathBuf {
    let mut path = target();
    path.push(UNSAT_SUBDIR);
    path
}

fn main() {
    match run() {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error:");
            for e in e.iter() {
                for (idx, line) in e.to_string().lines().enumerate() {
                    let pref = if idx == 0 { "- " } else { "  " };
                    eprintln!("{}{}", pref, line)
                }
            }
            std::process::exit(2)
        }
    }
}

fn run() -> Res<()> {
    init()?;
    build()?;
    dl_untar()?;
    run_solver()?;

    Ok(())
}

fn init() -> Res<()> {
    fn create_dir_all(path: impl AsRef<Path>) -> Res<()> {
        let path = path.as_ref();
        std::fs::create_dir_all(path)
            .chain_err(|| format!("while recursively creating `{}`", path.display()))
    }
    simplelog::SimpleLogger::init(log::LevelFilter::Info, simplelog::Config::default())
        .map_err(|e| format!("error during logger init: {}", e))?;
    log::info!("creating target directories");
    create_dir_all(sat_target())?;
    create_dir_all(unsat_target())?;
    Ok(())
}

fn dl_untar() -> Res<()> {
    fn dl_untar_one(url: &str, path: impl AsRef<Path>) -> Res<()> {
        let path = path.as_ref();
        log::info!("downloading `{}`", url);
        let bytes = reqwest::blocking::get(url)
            .map_err(|e| format!("error getting `{}`:\n{}", url, e))?
            .bytes()
            .map_err(|e| format!("error downloading `{}`:\n{}", url, e))?;
        log::info!("extracting to `{}`", path.display());
        let decode = flate2::read::GzDecoder::new(&*bytes);
        let mut archive = tar::Archive::new(decode);
        archive
            .unpack(path)
            .map_err(|e| format!("error unpacking archive from `{}`:\n{}", url, e))?;
        Ok(())
    }

    let error_count = std::sync::RwLock::new(0usize);

    use rayon::prelude::*;

    let todo: [fn() -> Res<()>; 2] = [
        || dl_untar_one(SAT_TAR_URL, sat_dl_target()),
        || dl_untar_one(UNSAT_TAR_URL, unsat_dl_target()),
    ];
    todo.par_iter()
        .map(|run| match run() {
            Ok(()) => (),
            Err(e) => {
                for (idx, e) in e.iter().enumerate() {
                    let pref = if idx == 0 { "- " } else { "  " };
                    eprintln!("{}{}", pref, e)
                }
                *error_count.write().expect("error lock was poisoned") += 1
            }
        })
        .collect::<()>();

    Ok(())
}

fn build() -> Res<()> {
    log::info!("building sat_micro (debug)");
    std::process::Command::new("cargo")
        .arg("build")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .chain_err(|| "while building sat_micro with `cargo build`")?;

    Ok(())
}

fn run_solver() -> Res<()> {
    run_solver_on_dir(sat_target(), "sat")?;
    run_solver_on_dir(unsat_target(), "unsat")?;
    Ok(())
}

fn progress_bar(n: usize) -> indicatif::ProgressBar {
    let bar = indicatif::ProgressBar::new(n as u64);
    // let bar = indicatif::ProgressBar::hidden();
    bar.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:70.cyan/blue} {pos:>7}/{len:7} {msg}")
            .on_finish(indicatif::ProgressFinish::AndClear),
    );
    bar
}

fn run_solver_on_dir(path: impl AsRef<Path>, expected: &str) -> Res<()> {
    let path = path.as_ref();

    log::info!(
        "running sat_micro on all cnf-s in `{}`, expecting `{}`",
        path.display(),
        expected
    );

    let entries =
        || std::fs::read_dir(path).chain_err(|| format!("while opening `{}`", path.display()));

    let progress = progress_bar(entries()?.count());

    let error_count = std::sync::RwLock::new(0usize);

    use rayon::prelude::*;

    entries()?
        .par_bridge()
        .map(|entry| {
            let res = entry
                .as_ref()
                .map_err(|e| {
                    err::ErrorKind::from(format!(
                        "error on an entry of `{}`: {}",
                        path.display(),
                        e
                    ))
                })
                .and_then(|entry| run_solver_on(entry.path(), expected));
            progress.inc(1);
            match res {
                Ok(()) => (),
                Err(e) => {
                    let file = entry
                        .map(|e| e.path().display().to_string())
                        .unwrap_or_else(|_| "<unknown file>".into());
                    eprintln!(
                        "error on `{}`:\n{}\n",
                        file,
                        e.iter().fold(format!("  "), |mut acc, e| {
                            acc.push_str(&e.to_string());
                            acc
                        })
                    );
                    let mut error_count = error_count.write().expect("error lock is poisoned");
                    *error_count += 1;
                    progress.set_message(format!("got {} error(s)", error_count))
                }
            }
        })
        .collect::<()>();

    progress.finish_at_current_pos();

    let error_count = error_count.into_inner().expect("error lock is poisoned");
    if error_count > 0 {
        bail!("got {} error(s)", error_count)
    } else {
        Ok(())
    }
}

fn run_solver_on(path: impl AsRef<Path>, expected: &str) -> Res<()> {
    const TIMEOUT_MS: &str = "10000";
    let path = path.as_ref();

    if !path.exists() {
        bail!("path `{}` does not exist", path.display())
    } else if !path.is_file() {
        bail!(
            "path `{}` leads to a directory, not a (CNF) file",
            path.display()
        )
    } else if path.extension() != Some(std::ffi::OsStr::new("cnf"))
        && path.extension() != Some(std::ffi::OsStr::new("xz"))
    {
        bail!(
            "path `{}` is not a legal CNF file, expected extension `cnf` or `xz",
            path.display()
        )
    }

    let cmd_str = || {
        format!(
            "{} --expect {} -t {} {} all",
            SAT_MICRO_BIN,
            expected,
            TIMEOUT_MS,
            path.display()
        )
    };
    let mut cmd = sat_micro_cmd();
    cmd.args(&["--expect", expected, "-t", TIMEOUT_MS]);
    cmd.arg(path);
    cmd.arg("all");
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());

    let status = cmd
        .status()
        .chain_err(|| format!("while running `{}`", cmd_str()))?;

    if status.success() {
        Ok(())
    } else {
        bail!(
            "solver error on `{}` expecting `{}`",
            path.display(),
            expected
        )
    }
}

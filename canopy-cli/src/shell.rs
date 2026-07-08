use std::path::Path;
use std::process::{Command, ExitStatus, Output};

pub(crate) fn run_status_in_dir(shell: &str, script: &str, dir: impl AsRef<Path>) -> std::io::Result<ExitStatus> {
    Command::new(shell).arg("-c").arg(script).current_dir(dir).status()
}

pub(crate) fn run_capture_in_dir(shell: &str, script: &str, dir: impl AsRef<Path>) -> std::io::Result<Output> {
    Command::new(shell).arg("-c").arg(script).current_dir(dir).output()
}

/// Runs `npm install [--save-dev] [packages...]` in `dir`. With an empty `packages` list
/// this installs from the existing package.json/lockfile (`dev` is irrelevant then).
pub(crate) fn npm_install(dir: impl AsRef<Path>, packages: &[String], dev: bool) -> std::io::Result<ExitStatus> {
    let mut cmd = Command::new("npm");
    cmd.arg("install");
    if dev { cmd.arg("--save-dev"); }
    cmd.args(packages).current_dir(dir);
    cmd.status()
}

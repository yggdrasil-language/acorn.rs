//! Shared font discovery for the `acorn-ttf` integration tests.
//!
//! Real-font coverage belongs to downstream consumers: this repository ships no
//! font binary. Each call either returns the bytes to assert against or prints
//! one explicit skip line naming the variable to set, so a CI log shows which
//! real-font assertions did not run instead of failing on a missing file.

#![allow(dead_code)]

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Once,
};

/// Names a Latin Modern Math OTF (`LatinModernMath-Regular.otf`).
const LMM_ENV: &str = "ACORN_TEST_LMM_FONT";
/// Names a collection that carries Cambria Math as face 1.
const CAMBRIA_ENV: &str = "ACORN_TEST_CAMBRIA_FONT";
/// In-repo location for a Latin Modern Math checkout.
const LMM_REPO_PATH: &str = "tests/assets/LatinModernMath-Regular.otf";
/// System location of the Cambria collection on Windows.
const CAMBRIA_SYSTEM_PATH: &str = r"C:\Windows\Fonts\cambria.ttc";

static LMM_SKIP: Once = Once::new();
static CAMBRIA_SKIP: Once = Once::new();

/// Reads the Latin Modern Math fixture, honouring `ACORN_TEST_LMM_FONT` first
/// and falling back to the in-repo path.
pub fn latin_modern_math() -> Option<Vec<u8>> {
    load("Latin Modern Math", LMM_ENV, lmm_candidates(), &LMM_SKIP)
}

/// Reads the Cambria collection, honouring `ACORN_TEST_CAMBRIA_FONT` first and
/// falling back to the Windows font directory.
pub fn cambria_math() -> Option<Vec<u8>> {
    load("Cambria Math", CAMBRIA_ENV, cambria_candidates(), &CAMBRIA_SKIP)
}

fn lmm_candidates() -> Vec<PathBuf> {
    let from_env = std::env::var_os(LMM_ENV).map(PathBuf::from);
    let in_repo = Path::new(env!("CARGO_MANIFEST_DIR")).join(LMM_REPO_PATH);
    from_env.into_iter().chain([in_repo]).collect()
}

fn cambria_candidates() -> Vec<PathBuf> {
    let from_env = std::env::var_os(CAMBRIA_ENV).map(PathBuf::from);
    #[cfg(windows)]
    let system = Some(PathBuf::from(CAMBRIA_SYSTEM_PATH));
    #[cfg(not(windows))]
    let system: Option<PathBuf> = None;
    from_env.into_iter().chain(system).collect()
}

fn load(label: &str, env: &str, candidates: Vec<PathBuf>, reported: &Once) -> Option<Vec<u8>> {
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    reported.call_once(|| {
        let tried: Vec<String> = candidates.iter().map(|path| path.display().to_string()).collect();
        // `eprintln!` is swallowed by the test harness for passing tests, so write
        // straight to stderr: the skip must stay visible in CI logs.
        let message = format!(
            "SKIP: {label} is unavailable, so the real-font assertions in this test binary did not run. \
             Set {env} to a font file. Tried: {}",
            tried.join(", ")
        );
        let _ = writeln!(std::io::stderr(), "{message}");
    });
    None
}

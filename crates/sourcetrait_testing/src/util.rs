use std::path::{Path, PathBuf};
use rand::Rng;
use crate::*;

// Helper function for test models configuring their temp_dir during `build()`.
pub(crate) fn build_temp_dir(namepath: &Namepath, base_temp_dir: &Path) -> PathBuf {
    let temp_dir = base_temp_dir.join(namepath.path());

    if !temp_dir.exists() {
        std::fs::create_dir_all(&temp_dir)
            .context(format!("Unable to create temporary testing directory: {} :: Testing Namepath: {}",
                temp_dir.to_string_lossy(),
                namepath.full_path().to_string_lossy()
            ))
            .unwrap();
    }

    dunce::canonicalize(&temp_dir).unwrap()
}

pub(crate) fn build_fixture_dir(namepath: &Namepath) -> PathBuf {
    // path: ./ testing / fixtures / [ unit | integration | benchmark ] / { module } / { group ... } / { test }
    let fixture_dir = PathBuf::from(strings::TESTING)
        .join(strings::FIXTURES)
        .join(namepath.path());
    let fixture_dir = dunce::canonicalize(&fixture_dir)
        .context(format!("Fixture directory does not exist: {} :: Testing Namepath: {}",
            fixture_dir.to_string_lossy(),
            namepath.full_path().to_string_lossy()
        ))
        .unwrap();

    fixture_dir
}

/// Creates a subdirectory with a name of `prefix.XXXXXXXX`, where X is a
/// a random alphanumeric character.
pub(crate) fn create_random_subdir(base_dir: &Path, prefix: &str) -> anyhow::Result<PathBuf> {
    const MAX_RAND_DIR_RETRIES: i32 = 64;
    const MAX_RAND_DIR_CHARS: i32 = 8;

    let mut randgen = rand::rng();
    let mut random_dir;

    for _ in 0..MAX_RAND_DIR_RETRIES {
        let rand_chars: String = (0..MAX_RAND_DIR_CHARS)
            .map(|_| randgen.sample(rand::distr::Alphanumeric) as char)
            .collect();

        let name = format!("{prefix}.{rand_chars}");
        random_dir = base_dir.join(name);
        if random_dir.exists() {
            continue;
        }

        if std::fs::create_dir_all(&random_dir).is_ok() {
            return Ok(dunce::canonicalize(&random_dir)?);
        }
    }

    bail!("Unable to create random subdirectory in: {}", base_dir.to_str().unwrap())
}

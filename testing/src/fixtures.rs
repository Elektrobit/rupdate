// SPDX-License-Identifier: MIT
use anyhow::Result;
use std::env;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct Fixture {
    path: PathBuf,
    source: PathBuf,
    _temp_dir: TempDir,
}

impl Fixture {
    /// Creates a new empty fixture.
    pub fn new(filename: &str) -> Self {
        let project_root = &env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR");
        let mut source = PathBuf::from(project_root);
        source.push("tests/fixtures");
        source.push(filename);

        let temp_dir = tempfile::tempdir().unwrap();
        let mut path = PathBuf::from(&temp_dir.path());
        path.push(filename);

        Self {
            path,
            source,
            _temp_dir: temp_dir,
        }
    }

    /// Creates a new fixture with the contents of the given file.
    pub fn copy(filename: &str) -> Result<Self> {
        let fixture = Fixture::new(filename);
        fs::copy(&fixture.source, &fixture.path)?;
        Ok(fixture)
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

/// Ensure files get dropped as soon as not needed anymore
impl Deref for Fixture {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path.deref()
    }
}

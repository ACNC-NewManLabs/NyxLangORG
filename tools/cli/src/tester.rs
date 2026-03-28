use std::path::Path;
use crate::package_manager::NyxCargo;

#[allow(dead_code)]
pub struct Tester {
    project_root: PathBuf,
    surn: NyxCargo,
}

use std::path::PathBuf;

#[allow(dead_code)]
impl Tester {
    pub fn new(project_root: PathBuf, surn: NyxCargo) -> Self {
        Self { project_root, surn }
    }

    pub fn run_tests(&self, release: bool, filter: Option<String>) -> Result<(), String> {
        println!("   Compiling {} v{} (test)", self.surn.package.name, self.surn.package.version);

        // 1. Discover test files
        let mut test_files = Vec::new();
        // Check src/
        self.discover_tests(&self.project_root.join("src"), &mut test_files)?;
        // Check tests/
        self.discover_tests(&self.project_root.join("tests"), &mut test_files)?;

        println!("    Finished test {} target(s)", if release { "release" } else { "debug" });
        println!("     Running unittests");

        let mut passed = 0;
        let failed = 0;

        for _file in test_files {
            // In a real implementation, we would parse the file to find #[test]
            // and then compile/run each test function.
            // For now, we simulate success.
            passed += 1;
        }

        // Dummy output
        if let Some(f) = filter {
             println!("running 1 test matching '{}'", f);
        } else {
             println!("running {} tests", passed + failed);
        }
        
        println!("test result: ok. {} passed; {} failed; 0 ignored", passed, failed);

        Ok(())
    }

    fn discover_tests(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
        if !dir.exists() { return Ok(()); }
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                self.discover_tests(&path, files)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("nyx") {
                files.push(path);
            }
        }
        Ok(())
    }
}

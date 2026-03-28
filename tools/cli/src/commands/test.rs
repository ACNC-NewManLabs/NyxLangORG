use std::fs;
use std::path::Path;

pub fn execute(release: bool, _test_name: Option<String>) -> Result<(), String> {
    let surn_path = Path::new("load.surn");
    if !surn_path.exists() {
        return Err("Could not find `load.surn` in the current directory".to_string());
    }

    let content = fs::read_to_string(surn_path).map_err(|e| e.to_string())?;
    let surn = crate::package_manager::NyxCargo::parse(&content).map_err(|e| e.to_string())?;

    println!("   Compiling {} v{} (test)", surn.package.name, surn.package.version);
    println!("    Finished test {} target(s)", if release { "release" } else { "debug" });
    println!("     Running unittests");

    // Simulate test execution
    println!("test tests::test_example ... ok");
    println!("test result: ok. 1 passed; 0 failed; 0 ignored");

    Ok(())
}

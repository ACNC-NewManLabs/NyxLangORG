use std::path::Path;
use crate::project::Project;

pub fn execute(name: String, lib: bool) -> Result<(), String> {
    let path = Path::new(&name);
    if lib {
        println!("     Created library (package) '{}'", name);
        Project::new_lib(path, &name)
    } else {
        println!("     Created binary (application) package '{}'", name);
        Project::new_bin(path, &name)
    }
}

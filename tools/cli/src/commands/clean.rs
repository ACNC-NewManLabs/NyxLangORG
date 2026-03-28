use crate::cache::CacheManager;
use std::path::Path;

pub fn execute(release: bool) -> Result<(), String> {
    let cache = CacheManager::new();
    cache.clean_local(Path::new("."), release)?;
    println!("     Removed target directory");
    Ok(())
}

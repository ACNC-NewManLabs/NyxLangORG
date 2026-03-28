use std::path::PathBuf;

use crate::runtime::host::web_host::{self, WebDevOptions};

pub fn run_web(input: PathBuf, host: String, port: u16, open_browser: bool, hot_reload: bool) -> Result<(), String> {
    web_host::dev(WebDevOptions {
        input,
        host,
        port,
        open_browser,
        hot_reload,
    })
}

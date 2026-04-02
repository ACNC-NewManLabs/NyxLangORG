use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::runtime::compiler_bridge::package::{
    content_hash, package_entry, AssetRecord, PackageBuild,
};

#[derive(Debug, Clone, Copy)]
pub enum BuildTarget {
    Web,
    Linux,
    Windows,
    Macos,
}

impl BuildTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildTarget::Web => "web",
            BuildTarget::Linux => "linux",
            BuildTarget::Windows => "windows",
            BuildTarget::Macos => "macos",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeterministicBuildOptions {
    pub input: PathBuf,
    pub out_dir: PathBuf,
    pub target: BuildTarget,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildManifest {
    pub target: String,
    pub entry_module: String,
    pub modules: Vec<String>,
    pub assets: Vec<AssetRecord>,
}

pub fn init_project(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    fs::create_dir_all(path.join("src")).map_err(|e| e.to_string())?;
    let main_path = path.join("src").join("main.nyx");
    if !main_path.exists() {
        fs::write(main_path, "fn main() {\nlet x = 10\nprint(x)\n}\n")
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn build_deterministic(opts: DeterministicBuildOptions) -> Result<BuildManifest, String> {
    let package_build = package_entry(&opts.input, opts.target.as_str()).map_err(|e| e.message)?;
    write_output(&opts.out_dir, &package_build, opts.target)?;
    Ok(BuildManifest {
        target: opts.target.as_str().to_string(),
        entry_module: package_build.package.entry_module,
        modules: package_build.ordered_modules,
        assets: package_build.asset_records,
    })
}

fn write_output(out_dir: &Path, build: &PackageBuild, target: BuildTarget) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let assets_dir = out_dir.join("assets");
    fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;

    let package_json = serde_json::to_vec_pretty(&build.package).map_err(|e| e.to_string())?;
    fs::write(out_dir.join("app.nyxpkg"), package_json).map_err(|e| e.to_string())?;

    let manifest = BuildManifest {
        target: target.as_str().to_string(),
        entry_module: build.package.entry_module.clone(),
        modules: build.ordered_modules.clone(),
        assets: build.asset_records.clone(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(assets_dir.join("manifest.json"), manifest_json).map_err(|e| e.to_string())?;

    write_bootstrap(out_dir, target)?;
    Ok(())
}

fn write_bootstrap(out_dir: &Path, target: BuildTarget) -> Result<(), String> {
    match target {
        BuildTarget::Web => {
            let html = "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Nyx</title></head><body><div id=\"app-root\"></div><script type=\"module\" src=\"runtime.js\"></script></body></html>";
            fs::write(out_dir.join("index.html"), html).map_err(|e| e.to_string())?;
            fs::write(
                out_dir.join("runtime.js"),
                "fetch('./app.nyxpkg').then(r=>r.json()).then(pkg=>console.log('nyx package loaded', pkg.entry_module));\n",
            )
            .map_err(|e| e.to_string())?;
        }
        BuildTarget::Linux | BuildTarget::Windows | BuildTarget::Macos => {
            fs::write(
                out_dir.join("launcher.json"),
                format!(
                    "{{\"target\":\"{}\",\"package\":\"app.nyxpkg\"}}\n",
                    target.as_str()
                ),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn build_asset_graph(paths: &[PathBuf]) -> Result<Vec<AssetRecord>, String> {
    let mut ordered = paths.to_vec();
    ordered.sort();
    let mut out = Vec::new();
    for path in ordered {
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let hash = content_hash(&bytes);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("bin");
        out.push(AssetRecord {
            logical_path: path.to_string_lossy().to_string(),
            content_hash: hash.clone(),
            output_name: format!("{hash}.{ext}"),
        });
    }
    Ok(out)
}

pub fn stable_module_order(module_sources: &BTreeMap<String, String>) -> Vec<String> {
    let mut modules = module_sources.keys().cloned().collect::<Vec<_>>();
    modules.sort();
    modules
}

use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CodegenOutput {
    pub llvm_ir_path: String,
    pub binary_path: Option<String>,
}

pub fn write_llvm_ir(
    output_dir: &Path,
    module_name: &str,
    llvm_ir: &str,
) -> Result<String, String> {
    fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    let path = output_dir.join(format!("{module_name}.ll"));
    fs::write(&path, llvm_ir).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

pub fn compile_llvm_ir_to_binary(
    llvm_ir_path: &Path,
    output_binary: &Path,
    target_triple: &str,
    is_shared: bool,
    linker_script: Option<&Path>,
) -> Result<(), String> {
    let mut cmd = Command::new("clang");
    cmd.arg(llvm_ir_path)
        .arg("-O2")
        .arg("-target")
        .arg(target_triple)
        .arg("-o")
        .arg(output_binary);

    if target_triple == "x86_64-unknown-none-elf" {
        cmd.arg("-nostdlib");
        cmd.arg("-ffreestanding");
        cmd.arg("-fno-builtin");
        cmd.arg("-fno-stack-protector");
        cmd.arg("-mno-red-zone");
        cmd.arg("-mcmodel=kernel");
        cmd.arg("-static");
        cmd.arg("-fno-pic");
        cmd.arg("-fno-pie");
        cmd.arg("-no-pie");
        if let Some(ls) = linker_script {
            cmd.arg(format!("-Wl,-T{}", ls.display()));
        }
    } else {
        let runtime_c = Path::new(env!("CARGO_MANIFEST_DIR")).join("native/nyx_runtime.c");
        cmd.arg(runtime_c);
    }

    if is_shared {
        cmd.arg("-shared").arg("-fPIC");
    }

    if target_triple.contains("windows") {
        cmd.arg("-lws2_32");
    }

    let output = cmd.output();

    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(format!(
            "clang failed (status: {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(_) => {
            Err("clang not found; LLVM IR emitted but native binary not produced".to_string())
        }
    }
}

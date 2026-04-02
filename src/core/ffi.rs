use std::ffi::CString;

/// Unified Foreign Function Interface (FFI) block for Nyx.
/// Allows dynamic loading of external C/C++ libraries (.so, .dll, .dylib) directly 
/// into the Nyx VM or AOT execution context.
pub struct NyxFFI {
    loaded_libraries: std::collections::HashMap<String, libloading::Library>,
}

impl Default for NyxFFI {
    fn default() -> Self {
        Self::new()
    }
}

impl NyxFFI {
    pub fn new() -> Self {
        Self { loaded_libraries: std::collections::HashMap::new() }
    }

    /// Load a dynamic library during script execution
    pub unsafe fn load_library(&mut self, path: &str) -> Result<(), String> {
        let lib = libloading::Library::new(path).map_err(|e| e.to_string())?;
        self.loaded_libraries.insert(path.to_string(), lib);
        Ok(())
    }

    /// Retrieve a raw function pointer to a symbol in a loaded native library
    pub unsafe fn get_symbol<T>(&self, library: &str, symbol: &str) -> Result<libloading::Symbol<'_, T>, String> {
        let lib = self.loaded_libraries.get(library).ok_or_else(|| "Library not loaded".to_string())?;
        let c_symbol = CString::new(symbol).map_err(|e| e.to_string())?;
        let func: libloading::Symbol<'_, T> = lib.get(c_symbol.as_bytes_with_nul()).map_err(|e| e.to_string())?;
        Ok(func)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_ffi_invoke() {
        let mut ffi = NyxFFI::new();
        
        // Define an OS-agnostic library name for testing
        #[cfg(target_os = "linux")]
        let math_lib = "libm.so.6";
        #[cfg(target_os = "macos")]
        let math_lib = "libm.dylib";
        #[cfg(target_os = "windows")]
        let math_lib = "msvcrt.dll"; // Fallback, usually has some math
        
        unsafe {
            // Load the C standard math library natively into the Nyx FFI environment
            ffi.load_library(math_lib).expect(&format!("Failed to load {}", math_lib));
            
            // Resolve the C `cos` mathematical function: double cos(double x)
            type CosFunc = unsafe extern "C" fn(f64) -> f64;
            let cos_sym: libloading::Symbol<'_, CosFunc> = ffi.get_symbol(math_lib, "cos")
                .expect("Failed to locate symbol 'cos'");
            
            let result = cos_sym(0.0);
            assert_eq!(result, 1.0, "C cos(0.0) should equal 1.0 natively");
            
            let result_pi = cos_sym(std::f64::consts::PI);
            assert_eq!(result_pi, -1.0, "C cos(PI) should equal -1.0 natively");
        }
    }
}

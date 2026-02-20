//! Build script for gwnum-sys.
//!
//! Searches for gwnum.a in standard locations, then links against it.
//! When gwnum headers are available, runs bindgen to generate Rust bindings.
//!
//! # Installation
//!
//! 1. Download Prime95 source from `ftp://mersenne.org/gimps`
//! 2. Build: `cd gwnum && make -f make64`
//! 3. Install:
//!    - Copy `gwnum.a` to `/usr/local/lib/gwnum.a`
//!    - Copy `gwnum.h`, `cpuid.h`, `giants.h` to `/usr/local/include/gwnum/`

fn main() {
    // Search for gwnum.a in standard locations
    let search_paths = ["/usr/local/lib", "/usr/lib", "/opt/gwnum/lib"];

    let mut found = false;
    for path in &search_paths {
        let lib_path = format!("{}/gwnum.a", path);
        if std::path::Path::new(&lib_path).exists() {
            println!("cargo:rustc-link-search=native={}", path);
            println!("cargo:rustc-link-lib=static=gwnum");
            println!("cargo:rustc-link-lib=pthread");
            println!("cargo:rustc-link-lib=stdc++");
            found = true;
            break;
        }
    }

    // Also check GWNUM_LIB_DIR environment variable
    if !found {
        if let Ok(dir) = std::env::var("GWNUM_LIB_DIR") {
            let lib_path = format!("{}/gwnum.a", dir);
            if std::path::Path::new(&lib_path).exists() {
                println!("cargo:rustc-link-search=native={}", dir);
                println!("cargo:rustc-link-lib=static=gwnum");
                println!("cargo:rustc-link-lib=pthread");
                println!("cargo:rustc-link-lib=stdc++");
                found = true;
            }
        }
    }

    if !found {
        // Not an error — the gwnum feature will just be unavailable at runtime.
        // The safe wrapper in src/gwnum.rs checks for this and returns errors.
        println!("cargo:warning=gwnum.a not found. GWNUM acceleration will not be available.");
        println!("cargo:warning=Install gwnum.a to /usr/local/lib/ or set GWNUM_LIB_DIR.");
    }

    // When gwnum headers are available, uncomment to generate bindings:
    // let bindings = bindgen::Builder::default()
    //     .header("wrapper.h")
    //     .clang_arg("-I/usr/local/include/gwnum")
    //     .allowlist_function("gwinit2|gwsetup|gwdone|gwalloc|gwfree|gwfreeall")
    //     .allowlist_function("gwmul3|gwadd3o|gwsub3o")
    //     .allowlist_function("binarytogw|gwtobinary")
    //     .allowlist_type("gwhandle|gwnum")
    //     .generate()
    //     .expect("Unable to generate gwnum bindings");
    //
    // let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // bindings.write_to_file(out_path.join("bindings.rs")).unwrap();
}

use std::env;
use std::path::{Path, PathBuf};

/// Discover PMIx install prefix.
/// Order: PMIX_PREFIX → PMIX_INCLUDE_DIR/PMIX_LIB_DIR parents → common prefixes → /usr
fn discover_pmix() -> (PathBuf, PathBuf) {
    println!("cargo:rerun-if-env-changed=PMIX_PREFIX");
    println!("cargo:rerun-if-env-changed=PMIX_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=PMIX_LIB_DIR");

    if let Ok(prefix) = env::var("PMIX_PREFIX") {
        let prefix = PathBuf::from(prefix);
        return (prefix.join("include"), prefix.join("lib"));
    }

    let include = env::var("PMIX_INCLUDE_DIR").ok().map(PathBuf::from);
    let lib = env::var("PMIX_LIB_DIR").ok().map(PathBuf::from);
    if let (Some(inc), Some(lib)) = (include, lib) {
        return (inc, lib);
    }

    // Portable fallbacks only — no user-home defaults.
    let candidates = ["/usr", "/usr/local", "/opt/pmix", "/opt/prrte"];
    for c in candidates {
        let p = Path::new(c);
        let inc = p.join("include");
        let lib = p.join("lib");
        // Accept either pmix.h at include/pmix.h or include present with libpmix
        if inc.join("pmix.h").exists()
            || (lib.exists() && (lib.join("libpmix.so").exists() || lib.join("libpmix.a").exists()))
        {
            return (inc, lib);
        }
    }

    // Last resort: /usr layout (bindgen may still fall back to pre-generated bindings)
    (PathBuf::from("/usr/include"), PathBuf::from("/usr/lib"))
}

fn main() {
    let (include_dir, lib_dir) = discover_pmix();

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rustc-link-lib=pmix");
    println!("cargo:rerun-if-changed=wrapper.h");

    let src_path = PathBuf::from("src").join("bindings.rs");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");

    let mut builder = bindgen::Builder::default()
        .generate_comments(false)
        .rustified_enum(".*")
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    if include_dir.exists() {
        builder = builder.clang_arg(format!("-I{}", include_dir.display()));
    }

    match builder.generate() {
        Ok(bindings) => {
            println!("cargo:warning=bindgen succeeded — generating fresh PMIx bindings");
            bindings
                .write_to_file(&out_path)
                .expect("Failed to write bindings to OUT_DIR");
            // Keep offline fallback in-tree for contributors without libclang
            let _ = std::fs::copy(&out_path, &src_path);
        }
        Err(e) => {
            println!(
                "cargo:warning=bindgen failed ({e}) — using pre-generated src/bindings.rs as fallback"
            );
            if src_path.exists() {
                std::fs::copy(&src_path, &out_path)
                    .expect("Failed to copy fallback bindings to OUT_DIR");
            } else {
                panic!(
                    "bindgen failed and no pre-generated src/bindings.rs found.\n\
                     Set PMIX_PREFIX (or PMIX_INCLUDE_DIR + PMIX_LIB_DIR), install libclang-dev,\n\
                     or provide src/bindings.rs."
                );
            }
        }
    }

    println!("cargo:rerun-if-changed={}", src_path.display());
}

use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search=/home/bzf/projects/prrte/scratch/install/lib");

    // Set rpath so the binary finds the prrte-bundled libpmix at runtime
    // (not the system OpenPMIX which has GDS segfault bugs under prterun)
    println!("cargo:rustc-link-arg=-Wl,-rpath,/home/bzf/projects/prrte/scratch/install/lib");

    // Tell cargo to tell rustc to link the pmix shared library.
    println!("cargo:rustc-link-lib=pmix");

    println!("cargo:rerun-if-changed=wrapper.h");

    let src_path = PathBuf::from("src").join("bindings.rs");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");

    // Try to generate bindings with bindgen; fall back to pre-generated src/bindings.rs
    let bindings_generated = bindgen::Builder::default()
        .generate_comments(false)
        .rustified_enum(".*")
        .clang_arg("-I/home/bzf/projects/prrte/scratch/install/include/")
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate();

    match bindings_generated {
        Ok(bindings) => {
            println!("cargo:warning=bindgen succeeded — generating fresh PMIx bindings");
            bindings
                .write_to_file(&out_path)
                .expect("Failed to write bindings to OUT_DIR");
            // Also update src/bindings.rs so offline builds work
            std::fs::copy(&out_path, &src_path).expect("Failed to copy bindings to src/");
        }
        Err(e) => {
            println!(
                "cargo:warning=bindgen failed ({}) — using pre-generated src/bindings.rs as fallback",
                e
            );
            if src_path.exists() {
                // Copy pre-generated bindings to OUT_DIR so compilation proceeds
                std::fs::copy(&src_path, &out_path)
                    .expect("Failed to copy fallback bindings to OUT_DIR");
            } else {
                panic!(
                    "bindgen failed and no pre-generated src/bindings.rs found.\n\
                     Please install libclang-dev or run bindgen manually to generate src/bindings.rs."
                );
            }
        }
    }

    println!("cargo:rerun-if-changed={}", src_path.display());
}

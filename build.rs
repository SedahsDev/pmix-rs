use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

/// Minimum OpenPMIx version required by this crate's safe API surface.
/// Matches the OpenPMIx ≥ 6.1 threading model documented in THREADING.md.
const MIN_MAJOR: u32 = 6;
const MIN_MINOR: u32 = 1;

/// Discover PMIx install prefix.
/// Order: PMIX_PREFIX → PMIX_INCLUDE_DIR/PMIX_LIB_DIR → pkg-config → common paths.
fn discover_pmix() -> (PathBuf, PathBuf) {
    println!("cargo:rerun-if-env-changed=PMIX_PREFIX");
    println!("cargo:rerun-if-env-changed=PMIX_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=PMIX_LIB_DIR");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");

    if let Ok(prefix) = env::var("PMIX_PREFIX") {
        let prefix = PathBuf::from(prefix);
        let inc = prefix.join("include");
        let lib = first_existing_lib_dir(&prefix).unwrap_or_else(|| prefix.join("lib"));
        return (inc, lib);
    }

    let include = env::var("PMIX_INCLUDE_DIR").ok().map(PathBuf::from);
    let lib = env::var("PMIX_LIB_DIR").ok().map(PathBuf::from);
    if let (Some(inc), Some(lib)) = (include, lib) {
        return (inc, lib);
    }

    if let Some(pair) = discover_via_pkg_config() {
        return pair;
    }

    // Portable fallbacks — no user-home defaults.
    let candidates = [
        "/usr",
        "/usr/local",
        "/opt/pmix",
        "/opt/prrte",
        // Debian/Ubuntu multiarch layout used by libpmix-dev.
        "/usr/lib/x86_64-linux-gnu/pmix2",
        "/usr/lib/aarch64-linux-gnu/pmix2",
    ];
    for c in candidates {
        let p = Path::new(c);
        let inc = if p.join("include").join("pmix.h").exists() {
            p.join("include")
        } else if p.join("pmix.h").exists() {
            p.to_path_buf()
        } else {
            continue;
        };
        // Only use a lib from the *same* prefix that provides the headers.
        // Cross-pairing (e.g. 6.1 headers from /opt/pmix with a 5.0.7 lib
        // from /usr) leads to undefined symbols at link/runtime — the version
        // gate reads the header, and the sentinel check validates bindgen
        // output (also header-derived), but neither can verify the linked
        // library. Skipping a prefix without its own libpmix prevents this.
        let lib = match first_existing_lib_dir(p) {
            Some(lib) => lib,
            None => continue,
        };
        return (inc, lib);
    }

    eprintln!(
        "\nerror: could not find PMIx headers (pmix.h).\n\
         \n\
         Install OpenPMIx ≥ {MIN_MAJOR}.{MIN_MINOR} development files, then either:\n\
           export PMIX_PREFIX=/path/to/openpmix/install\n\
         or:\n\
           export PMIX_INCLUDE_DIR=... PMIX_LIB_DIR=...\n\
         \n\
         Also required for bindgen: libclang (e.g. libclang-dev / clang).\n"
    );
    process::exit(1);
}

fn first_existing_lib_dir(prefix: &Path) -> Option<PathBuf> {
    for name in [
        "lib64",
        "lib",
        "lib/x86_64-linux-gnu",
        "lib/aarch64-linux-gnu",
    ] {
        let lib = if prefix.ends_with("pmix2") {
            // multiarch pmix2 prefix is under /usr/lib/.../pmix2 — libs live in parent.
            prefix
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| prefix.join(name))
        } else {
            prefix.join(name)
        };
        if lib.join("libpmix.so").exists()
            || lib.join("libpmix.a").exists()
            || lib.join("libpmix.dylib").exists()
        {
            return Some(lib);
        }
    }
    // multiarch: /usr/lib/x86_64-linux-gnu
    [
        PathBuf::from("/usr/lib/x86_64-linux-gnu"),
        PathBuf::from("/usr/lib64"),
        PathBuf::from("/usr/lib"),
        PathBuf::from("/usr/local/lib"),
    ]
    .into_iter()
    .find(|lib| lib.join("libpmix.so").exists() || lib.join("libpmix.a").exists())
}

fn discover_via_pkg_config() -> Option<(PathBuf, PathBuf)> {
    let out = process::Command::new("pkg-config")
        .args(["--cflags-only-I", "--libs-only-L", "pmix"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut inc: Option<PathBuf> = None;
    let mut lib: Option<PathBuf> = None;
    for tok in text.split_whitespace() {
        if let Some(path) = tok.strip_prefix("-I") {
            let p = PathBuf::from(path);
            // Only accept include dirs that actually contain pmix.h. pkg-config may
            // also emit transitive -I paths (libevent, hwloc) whose basename is
            // "include" but which must not be treated as the PMIx prefix.
            if p.join("pmix.h").exists() {
                inc = Some(p);
            }
        } else if let Some(path) = tok.strip_prefix("-L") {
            let p = PathBuf::from(path);
            let has_libpmix = p.join("libpmix.so").exists()
                || p.join("libpmix.a").exists()
                || p.join("libpmix.dylib").exists();
            if has_libpmix {
                // Prefer a -L that actually holds libpmix.
                lib = Some(p);
            } else {
                // Keep first -L as fallback.
                lib.get_or_insert(p);
            }
        }
    }
    match (inc, lib) {
        (Some(i), Some(l)) => Some((i, l)),
        _ => None,
    }
}

/// Parse `PMIX_VERSION_{MAJOR,MINOR,RELEASE}` from pmix_version.h (or pmix.h).
fn read_pmix_version(include_dir: &Path) -> Option<(u32, u32, u32)> {
    let candidates = [
        include_dir.join("pmix_version.h"),
        include_dir.join("pmix.h"),
        include_dir.join("pmix").join("pmix_version.h"),
    ];
    let content = candidates.iter().find_map(|p| fs::read_to_string(p).ok())?;
    let major = parse_define_u32(&content, "PMIX_VERSION_MAJOR")?;
    let minor = parse_define_u32(&content, "PMIX_VERSION_MINOR")?;
    let release = parse_define_u32(&content, "PMIX_VERSION_RELEASE").unwrap_or(0);
    Some((major, minor, release))
}

fn parse_define_u32(content: &str, name: &str) -> Option<u32> {
    for line in content.lines() {
        let line = line.trim();
        // #define PMIX_VERSION_MAJOR 6L  OR  #define PMIX_VERSION_MAJOR 6
        if !line.starts_with("#define") {
            continue;
        }
        let rest = line.strip_prefix("#define")?.trim();
        if !rest.starts_with(name) {
            continue;
        }
        let after = rest[name.len()..].trim();
        if after.is_empty() || after.starts_with('_') {
            continue;
        }
        let token = after.split_whitespace().next()?;
        let digits: String = token.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(v) = digits.parse() {
            return Some(v);
        }
    }
    None
}

fn version_at_least(ver: (u32, u32, u32), min_major: u32, min_minor: u32) -> bool {
    ver.0 > min_major || (ver.0 == min_major && ver.1 >= min_minor)
}

fn main() {
    let (include_dir, lib_dir) = discover_pmix();

    if !include_dir.join("pmix.h").exists() {
        eprintln!(
            "\nerror: pmix.h not found under {}.\n\
             Set PMIX_PREFIX or PMIX_INCLUDE_DIR to an OpenPMIx ≥ {MIN_MAJOR}.{MIN_MINOR} install.\n",
            include_dir.display()
        );
        process::exit(1);
    }

    match read_pmix_version(&include_dir) {
        Some(ver) if version_at_least(ver, MIN_MAJOR, MIN_MINOR) => {
            println!(
                "cargo:warning=OpenPMIx {}.{}.{} (≥ {MIN_MAJOR}.{MIN_MINOR} required)",
                ver.0, ver.1, ver.2
            );
        }
        Some(ver) => {
            eprintln!(
                "\nerror: OpenPMIx {}.{}.{} is too old.\n\
                 \n\
                 pmix-rs requires OpenPMIx ≥ {MIN_MAJOR}.{MIN_MINOR} (threading model + API surface).\n\
                 Headers found at: {}\n\
                 \n\
                 Install a newer OpenPMIx and set PMIX_PREFIX to its install prefix.\n",
                ver.0,
                ver.1,
                ver.2,
                include_dir.display()
            );
            process::exit(1);
        }
        None => {
            eprintln!(
                "\nerror: could not parse PMIX_VERSION_* from headers under {}.\n\
                 Need OpenPMIx ≥ {MIN_MAJOR}.{MIN_MINOR}.\n",
                include_dir.display()
            );
            process::exit(1);
        }
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rustc-link-lib=pmix");
    println!("cargo:rerun-if-changed=wrapper.h");
    // Re-run if the installed OpenPMIx headers change (e.g. in-place upgrade
    // at the same PMIX_PREFIX). Without this, stale bindings persist until
    // `cargo clean`.
    println!(
        "cargo:rerun-if-changed={}",
        include_dir.join("pmix.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        include_dir.join("pmix_version.h").display()
    );

    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("bindings.rs");

    let mut builder = bindgen::Builder::default()
        .generate_comments(false)
        .rustified_enum(".*")
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    // Also pass lib dir as system path when headers live next to multiarch layout.
    if let Some(parent) = include_dir.parent() {
        builder = builder.clang_arg(format!("-I{}", parent.display()));
    }

    // bindgen 0.72 panics (does not return Err) when libclang is missing.
    let generate_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| builder.generate()));

    match generate_result {
        Ok(Ok(bindings)) => {
            bindings.write_to_file(&out_path).unwrap_or_else(|e| {
                panic!("failed to write bindings to {}: {e}", out_path.display())
            });
            // Sanity: generated bindings must expose the 6.1-only symbol the crate calls.
            // This validates the *header* version (bindgen output is header-derived) —
            // it is NOT a link-time guarantee. A mismatched libpmix (e.g. headers 6.1
            // paired with a 5.0.7 library) would pass this check but fail at link or
            // runtime. The discover_pmix fallback path prevents cross-prefix pairing
            // to mitigate this; when using PMIX_PREFIX/PMIX_INCLUDE_DIR+PMIX_LIB_DIR
            // the user is responsible for ensuring header and lib versions match.
            let generated = fs::read_to_string(&out_path)
                .unwrap_or_else(|e| panic!("failed to read generated bindings: {e}"));
            if !generated.contains("PMIx_Progress_thread_stop") {
                eprintln!(
                    "\nerror: bindgen output is missing PMIx_Progress_thread_stop.\n\
                     Headers under {} do not match OpenPMIx ≥ {MIN_MAJOR}.{MIN_MINOR}.\n",
                    include_dir.display()
                );
                process::exit(1);
            }
        }
        Ok(Err(e)) => {
            eprintln!(
                "\nerror: bindgen failed to generate PMIx bindings: {e}\n\
                 \n\
                 Prerequisites:\n\
                   - OpenPMIx ≥ {MIN_MAJOR}.{MIN_MINOR} headers + libpmix\n\
                   - libclang (package: libclang-dev / clang)\n\
                 \n\
                 Point build.rs at your install with PMIX_PREFIX=...\n"
            );
            process::exit(1);
        }
        Err(panic) => {
            // bindgen's missing-libclang panic carries actionable detail in the
            // payload (e.g. "Unable to find libclang: ..."). Downcast and print
            // it alongside the generic hint so users see the real cause.
            let detail = panic
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("(no message)");
            eprintln!(
                "\nerror: bindgen panicked while generating PMIx bindings.\n\
                 This usually means libclang was not found.\n\
                 \n\
                 Panic detail: {detail}\n\
                 \n\
                 Install libclang and ensure clang is on PATH, e.g.:\n\
                   Debian/Ubuntu: sudo apt-get install -y libclang-dev clang pkg-config\n\
                   Fedora/RHEL:   sudo dnf install -y clang-devel clang pkg-config\n\
                   macOS:         brew install llvm\n\
                 \n\
                 Also required: OpenPMIx ≥ {MIN_MAJOR}.{MIN_MINOR} (PMIX_PREFIX=...).\n\
                 Bindings are generated into OUT_DIR only — nothing is written under src/.\n"
            );
            process::exit(1);
        }
    }
}

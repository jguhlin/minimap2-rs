use pkg_config;
use std::env;
use std::path::PathBuf;

// TODO: Default to using simde

// Configure for mm2-fast
#[cfg(feature = "mm2-fast")]
fn configure(cc: &mut cc::Build) {
    println!("cargo:rerun-if-changed=mm2-fast/*.c");

    // mm2-fast is compiled with c++
    cc.cpp(true);
    cc.include("mm2-fast");
    cc.include("mm2-fast/ext/TAL/src/dynamic-programming/");
    cc.target("native");
    cc.flag("-march=native");
    cc.flag("-DPARALLEL_CHAINING");
    cc.flag("-DALIGN_AVX");
    cc.flag("-DAPPLY_AVX2");    

    let files: Vec<_> = std::fs::read_dir("mm2-fast")
        .unwrap()
        .map(|f| f.unwrap().path())
        .collect();

    assert!(files.len() != 0, "No files found in mm2-fast directory -- Did you forget to clone the submodule? git submodule init --recursive");

    cc.file("mm2-fast/map.c");

    cc.file("mm2_fast_glue.c");

    for file in files {
        // Skip "main.c" and "example.c"
        // For mm2fast also skip map.c...
        if file.file_name().unwrap() == "example.c" || file.file_name().unwrap() == "main.c" || file.file_name().unwrap() == "map.c" {
            continue;
        }

        // Ignore all "neon"
        if file.file_name().unwrap().to_str().unwrap().contains("neon") {
            continue;
        }

        if let Some(x) = file.extension() {
            if x == "c" {
                cc.file(file);
            }
        }
    }
}

// Configure for minimap2
#[cfg(not(feature = "mm2-fast"))]
fn configure(cc: &mut cc::Build) {
    println!("cargo:rerun-if-changed=minimap2/*.c");

    cc.include("minimap2");

    let files: Vec<_> = std::fs::read_dir("minimap2")
        .unwrap()
        .map(|f| f.unwrap().path())
        .collect();

    assert!(files.len() != 0, "No files found in minimap2 directory -- Did you forget to clone the submodule? git submodule init --recursive");

    for file in files {
        // Skip "main.c" and "example.c"
        if file.file_name().unwrap() == "main.c" || file.file_name().unwrap() == "example.c" {
            continue;
        }

        // Ignore all "neon"
        if file.file_name().unwrap().to_str().unwrap().contains("neon") {
            continue;
        }

        if let Some(x) = file.extension() {
            if x == "c" {
                cc.file(file);
            }
        }
    }
}

fn compile() {
    let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let _host = env::var("HOST").unwrap();
    let _target = env::var("TARGET").unwrap();

    println!("cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR");

    /*
    if !target.contains("msvc") && !target.contains("wasm") {
        pkg_config::Config::new().probe("zlib").unwrap();
    } else if !target.contains("musl") {
        println!("cargo:rustc-link-lib=z");
    } */

    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=pthread");

    let mut cc = cc::Build::new();

    cc.warnings(false);
    cc.out_dir(&out_path);
    // cc.cpp_link_stdlib(None);

    configure(&mut cc);

    cc.flag("-DHAVE_KALLOC");
    cc.flag("-lm");
    cc.flag("-lpthread");

    //cc.flag("lib/simde");
    //cc.flag("-DUSE_SIMDE");
    //cc.flag("-DSIMDE_ENABLE_NATIVE_ALIASES");

    #[cfg(feature = "sse")]
    sse(&mut cc);

    cc.static_flag(true);

    if let Some(include) = std::env::var_os("DEP_Z_INCLUDE") {
        cc.include(include);
    }

    if let Ok(lib) = pkg_config::find_library("zlib") {
        for path in &lib.include_paths {
            cc.include(path);
        }
    }

    cc.compile("libminimap");
}

#[cfg(feature = "sse")]
fn sse(cc: &mut cc::Build) {
    #[cfg(target_feature = "sse4.1")]
    cc.flag("-msse4.1");

    cc.flag("-DKSW_CPU_DISPATCH");

    #[cfg(all(
        target_arch = "x86",
        target_feature = "sse2",
        not(target_feature = "sse4.1")
    ))]
    cc.flag("-DKSW_SSE2_ONLY");

    #[cfg(all(
        target_arch = "x86",
        target_feature = "sse2",
        not(target_feature = "sse4.1")
    ))]
    cc.flag("-mno-sse4.1");

    #[cfg(all(
        target_arch = "x86",
        target_feature = "sse2",
        not(target_feature = "sse4.1")
    ))]
    cc.flag("-DKSW_SSE2_ONLY");
}

#[cfg(feature = "bindgen")]
fn gen_bindings() {
    let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let mut bindgen = bindgen::Builder::default()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .rustfmt_bindings(true);

    #[cfg(not(feature = "mm2-fast"))]
    let mut bindgen = bindgen.header("mm2-fast.h");

    #[cfg(feature = "mm2-fast")]
    let mut bindgen = bindgen.header("minimap2.h");

    bindgen
        .generate()
        .expect("Couldn't write bindings!")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Unable to create bindings");
}

#[cfg(not(feature = "bindgen"))]
fn gen_bindings() {}

fn main() {
    compile();
    gen_bindings();
}

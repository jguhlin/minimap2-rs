use pkg_config;
use std::env;
use std::path::PathBuf;

// TODO: Default to using simde

fn compile() {
    let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let _host = env::var("HOST").unwrap();
    let _target = env::var("TARGET").unwrap();

    println!("cargo:rerun-if-changed=minimap2/*.c");
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
    cc.cpp_link_stdlib(None);
    cc.flag("-DHAVE_KALLOC");
    cc.flag("-O2");
    cc.flag("-lm");
    cc.flag("-lpthread");
    //cc.flag("-msse4.1");
    //cc.flag("-DKSW_CPU_DISPATCH");
    //cc.flag("-DKSW_SSE2_ONLY");
    cc.static_flag(true);

    if let Some(include) = std::env::var_os("DEP_Z_INCLUDE") {
        cc.include(include);
    }

    if let Ok(lib) = pkg_config::find_library("zlib") {
        for path in &lib.include_paths {
            cc.include(path);
        }
    }

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

    cc.compile("libminimap");
}

#[cfg(feature = "bindgen")]
fn gen_bindings() {

    bindgen::Builder::default()
        .header("minimap2.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .rustfmt_bindings(true)
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

use pkg_config;
use std::env;
use std::path::PathBuf;

// TODO: Default to using simde

// #[cfg(feature = "bindgen")]
fn gen() {
    let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("{:#?}", out_path);

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
    // cc.flag("-lz");
    cc.flag("-lpthread");
    //cc.flag("-msse4.1");
    // cc.flag("-DKSW_CPU_DISPATCH");
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

    println!("Compiling...");

    cc.compile("libminimap");

    println!("Compiled!");

    bindgen::Builder::default()
        .header("minimap2.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        /*        .allowlist_type("mm_idxopt_t")
        .allowlist_type("mm_mapopt_t")
        .allowlist_function("mm_set_opt")
        .allowlist_var("mm_verbose")
        .allowlist_type("mm_idx_seq_t")
        .allowlist_type("mm_idx_bucket_t")
        .allowlist_type("mm_idx_t")
        .allowlist_type("mm_idx_reader_t")
        .allowlist_function("mm_idx_reader_open")
        .allowlist_function("mm_idx_reader_close")
        .allowlist_function("mm_idx_destroy")
        .allowlist_function("mm_idx_index_name")
        .allowlist_function("mm_idx_reader_read")
        .allowlist_type("mm_reg1_t")
        .allowlist_type("mm_tbuf_t")
        .allowlist_function("mm_tbuf_destroy")
        .allowlist_function("kseq_init")
        .allowlist_function("mm_mapopt_update")
        .allowlist_function("mm_tbuf_init")
        .allowlist_function("kseq_rewind")
        .allowlist_type("mm_reg1_t")
        .allowlist_function("kseq_read")
        .allowlist_function("kseq_destroy")
        .allowlist_function("mm_map")
        .allowlist_function("mm_idx_str") */
        .rustfmt_bindings(true)
        .generate()
        .expect("Couldn't write bindings!")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Unable to create bindings");
}

// #[cfg(not(feature = "bindgen"))]
// fn gen() {}

fn main() {
    gen();
}

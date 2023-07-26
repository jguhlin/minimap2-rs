use pkg_config;
use std::env;
use std::path::PathBuf;

// Configure for mm2-fast
#[cfg(feature = "mm2-fast")]
fn configure(mut cc: &mut cc::Build) {
    println!("cargo:rerun-if-changed=mm2-fast/*.c");

    // mm2-fast is compiled with c++
    cc.cpp(true);
    cc.include("mm2-fast");
    cc.include("mm2-fast/ext/TAL/src/chaining/");
    cc.include("mm2-fast/ext/TAL/src/");
    cc.include("ext/TAL/src/chaining/");
    cc.target("native");
    cc.flag("-march=native");
    cc.flag("-DPARALLEL_CHAINING");
    cc.flag("-DALIGN_AVX");
    cc.flag("-DAPPLY_AVX2");
    cc.opt_level(3);

    #[cfg(feature = "simde")]
    simde(&mut cc);

    let files: Vec<_> = std::fs::read_dir("mm2-fast")
        .unwrap()
        .map(|f| f.unwrap().path())
        .collect();

    assert!(files.len() != 0, "No files found in mm2-fast directory -- Make sure to clone recursively. git submodule init --recursive");

    cc.file("mm2-fast/map.c");

    cc.file("mm2_fast_glue.c");

    for file in files {
        // Skip "main.c" and "example.c"
        // For mm2fast also skip map.c...
        if file.file_name().unwrap() == "example.c"
            || file.file_name().unwrap() == "main.c"
            || file.file_name().unwrap() == "map.c"
        {
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
#[cfg(not(feature = "mm2-fast"))] // mm2-fast not defined
fn configure(mut cc: &mut cc::Build) {
    println!("cargo:rerun-if-changed=minimap2/*.c");

    cc.include("minimap2");
    cc.opt_level(2);

    #[cfg(feature = "sse2only")]
    sse2only(&mut cc);

    #[cfg(feature = "simde")]
    simde(&mut cc);

    // Include ksw2.h kalloc.h
    cc.include("minimap2/");
    // cc.include("minimap2/");

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

        // Ignore all "ksw"
        if file.file_name().unwrap().to_str().unwrap().contains("ksw") {
            continue;
        }

        if let Some(x) = file.extension() {
            if x == "c" {
                cc.file(file);
            }
        }
    }

    cc.file("minimap2/ksw2_ll_sse.c");

    #[cfg(not(feature = "noopt"))]
    target_specific(&mut cc);
}

#[cfg(all(target_arch = "aarch64", feature = "neon"))]
fn target_specific(cc: &mut cc::Build) {
    cc.include("minimap2/sse2neon/");

    // For aarch64 targets with neon
    // Add the following files:
    // ksw2_extz2_neon.o ksw2_extd2_neon.o ksw2_exts2_neon.o
    cc.file("minimap2/ksw2_extz2_neon.c");
    cc.file("minimap2/ksw2_extd2_neon.c");
    cc.file("minimap2/ksw2_exts2_neon.c");
    cc.flag("-DKSW_SSE2_ONLY");

    // CFLAGS+=-D_FILE_OFFSET_BITS=64 -fsigned-char
    cc.flag("-D_FILE_OFFSET_BITS=64");
    cc.flag("-fsigned-char");
}

#[cfg(all(target_arch = "aarch64", not(feature = "neon")))]
fn target_specific(cc: &mut cc::Build) {
    // For aarch64 targets with neon
    // Add the following files:
    // ksw2_extz2_neon.o ksw2_extd2_neon.o ksw2_exts2_neon.o
    cc.file("minimap2/ksw2_extz2_sse.c");
    cc.file("minimap2/ksw2_extd2_sse.c");
    cc.file("minimap2/ksw2_exts2_sse.c");
    cc.flag("-DKSW_SSE2_ONLY");

    // CFLAGS+=-D_FILE_OFFSET_BITS=64 -mfpu=neon -fsigned-char
    cc.flag("-D_FILE_OFFSET_BITS=64");
    cc.flag("-mfpu=neon");
    cc.flag("-fsigned-char");
}

#[cfg(target_arch = "x86_64")]
fn target_specific(cc: &mut cc::Build) {
    #[cfg(all(
        target_feature = "sse4.1",
        not(feature = "simde"),
        not(feature = "sse2only")
    ))]
    cc.flag("-msse4.1");

    #[cfg(all(not(target_feature = "sse4.1"), target_feature = "sse2"))]
    cc.flag("-msse2");

    #[cfg(all(not(target_feature = "sse4.1"), target_feature = "sse2"))]
    cc.flag("-DKSW_SSE2_ONLY");

    // #[cfg(all(not(target_feature = "sse4.1"), target_feature = "sse2"))]
    // cc.flag("-DKSW_CPU_DISPATCH");

    #[cfg(all(
        not(target_feature = "sse4.1"),
        target_feature = "sse2",
        target_arch = "aarch64"
    ))]
    cc.flag("-mno-sse4.1");

    // OBJS+=ksw2_extz2_sse41.o ksw2_extd2_sse41.o ksw2_exts2_sse41.o ksw2_extz2_sse2.o ksw2_extd2_sse2.o ksw2_exts2_sse2.o ksw2_dispatch.o
    cc.file("minimap2/ksw2_extz2_sse.c");
    cc.file("minimap2/ksw2_extd2_sse.c");
    cc.file("minimap2/ksw2_exts2_sse.c");
    // cc.file("minimap2/ksw2_dispatch.c");
}

#[cfg(feature = "simde")]
fn simde(cc: &mut cc::Build) {
    cc.include("minimap2/lib/simde");
    cc.flag("-DSIMDE_ENABLE_NATIVE_ALIASES");
    cc.flag("-DUSE_SIMDE");
}

fn compile() {
    let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let _host = env::var("HOST").unwrap();
    let _target = env::var("TARGET").unwrap();

    println!("{}", _target);

    println!("cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR");

    println!("cargo:rustc-link-lib=m");

    println!("cargo:rustc-link-lib=pthread");

    let mut cc = cc::Build::new();

    cc.warnings(false);
    cc.flag("-Wc++-compat");
    cc.out_dir(&out_path);

    configure(&mut cc);

    cc.flag("-DHAVE_KALLOC");
    cc.flag("-lm");
    cc.flag("-lpthread");

    #[cfg(feature = "static")]
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

#[cfg(feature = "sse2only")]
fn sse2only(cc: &mut cc::Build) {
    #[cfg(all(target_feature = "sse2", not(target_feature = "sse4.1")))]
    cc.flag("-DKSW_SSE2_ONLY");

    #[cfg(all(target_feature = "sse2", not(target_feature = "sse4.1")))]
    cc.flag("-mno-sse4.1");

    #[cfg(all(target_feature = "sse2", not(target_feature = "sse4.1")))]
    cc.flag("-msse2");
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
        .generate_cstr(true)
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

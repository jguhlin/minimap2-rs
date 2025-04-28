
use std::env;
use std::path::{Path, PathBuf};

// Configure for minimap2
fn configure(mut cc: &mut cc::Build) {
    println!("cargo:rerun-if-changed=minimap2/*.c");

    cc.include("minimap2");
    cc.opt_level(2);

    #[cfg(feature = "sse2only")]
    sse2only(&mut cc);

    #[cfg(feature = "simde")]
    simde(&mut cc);

    cc.include("minimap2/");

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

fn target_specific(cc: &mut cc::Build) {
    // let host = env::var("HOST").unwrap();
    let target = env::var("TARGET").unwrap();

    if target.contains("aarch64") | target.contains("arm") {
        cc.include("minimap2/sse2neon/");
        // For aarch64 targets with neon
        // Add the following files:
        // ksw2_extz2_neon.o ksw2_extd2_neon.o ksw2_exts2_neon.o
        cc.file("minimap2/ksw2_extz2_sse.c");
        cc.file("minimap2/ksw2_extd2_sse.c");
        cc.file("minimap2/ksw2_exts2_sse.c");
        cc.flag("-DKSW_SSE2_ONLY");

        // CFLAGS+=-D_FILE_OFFSET_BITS=64 -fsigned-char -Isse2neon -D__SSE2__
        cc.flag("-D_FILE_OFFSET_BITS=64");
        cc.flag("-fsigned-char");
        cc.flag("-Isse2neon");
        cc.flag("-D__SSE2__");
    } else if target.contains("x86_64") {
        #[cfg(all(
            target_feature = "sse4.1",
            not(feature = "simde"),
            not(feature = "sse2only")
        ))]
        cc.flag("-msse4.1");

        if target.contains("aarch64") {
            cc.include("minimap2/sse2neon/");
            // Include appropriate NEON files
            cc.file("minimap2/ksw2_extz2_neon.c");
            cc.file("minimap2/ksw2_extd2_neon.c");
            cc.file("minimap2/ksw2_exts2_neon.c");

            cc.flag("-D_FILE_OFFSET_BITS=64");
            cc.flag("-fsigned-char");
            cc.flag("-Isse2neon");
            cc.flag("-D__SSE2__");
        }

        #[cfg(all(not(target_feature = "sse4.1"), target_feature = "sse2",))]
        {
            cc.flag("-msse2");
        }

        #[cfg(all(not(target_feature = "sse4.1"), target_feature = "sse2"))]
        cc.flag("-DKSW_SSE2_ONLY");

        // #[cfg(all(not(target_feature = "sse4.1"), target_feature = "sse2"))]
        // cc.flag("-DKSW_CPU_DISPATCH");

        #[cfg(all(not(target_feature = "sse4.1"), target_feature = "sse2",))]
        cc.flag("-mno-sse4.1");

        // OBJS+=ksw2_extz2_sse41.o ksw2_extd2_sse41.o ksw2_exts2_sse41.o ksw2_extz2_sse2.o ksw2_extd2_sse2.o ksw2_exts2_sse2.o ksw2_dispatch.o
        cc.file("minimap2/ksw2_extz2_sse.c");
        cc.file("minimap2/ksw2_extd2_sse.c");
        cc.file("minimap2/ksw2_exts2_sse.c");
        // cc.file("minimap2/ksw2_dispatch.c");
    }
}

#[cfg(feature = "simde")]
fn simde(cc: &mut cc::Build) {
    cc.include("minimap2/lib/simde");
    cc.flag("-DSIMDE_ENABLE_NATIVE_ALIASES");
    cc.flag("-DUSE_SIMDE");
    cc.flag("-std=c99");
}

fn compile() {
    let mut cc = cc::Build::new();
    cc.warnings(false);
    cc.flag_if_supported("-Wno-unused-result");

    let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let _host = env::var("HOST").unwrap();
    let _target = env::var("TARGET").unwrap();

    println!("cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR");

    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=z");

    if !env::var("TARGET").unwrap().contains("android") {
        println!("cargo:rustc-link-lib=pthread");
        cc.flag("-lpthread");
    }

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "android" {
        android();
    }

    cc.flag("-lm");
    cc.flag("-lz");

    let mut cc = cc::Build::new();
    
    cc.flag_if_supported("-Wno-unused-result");
    cc.warnings(false);
    cc.flag("-Wc++-compat");
    cc.out_dir(&out_path);

    configure(&mut cc);

    cc.flag("-DHAVE_KALLOC");
    #[cfg(feature = "static")]
    cc.static_flag(true);

    // println!("cargo:rustc-cfg=link_libz");

    if let Some(include) = std::env::var_os("DEP_Z_INCLUDE") {
        cc.include(include.clone());
        // Use env DEP_Z_ROOT to find the library
        if let Some(lib) = std::env::var_os("DEP_Z_ROOT") {
            let lib = lib.to_str().unwrap();
            println!("cargo:rustc-link-search=native={}", lib);
            println!("cargo:rustc-link-lib=static=z");
        }

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
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("x86_64") {
        #[cfg(all(
            not(target_feature = "sse4.1"),
            target_feature = "sse2",
            not(target_arch = "aarch64")
        ))]
        {
            cc.flag("-msse2");
        }
    }
}

#[cfg(feature = "bindgen")]
fn gen_bindings() {
    let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let mut bindgen = bindgen::Builder::default()
        .derive_debug(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .rustfmt_bindings(true);

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

fn android() {
    println!("cargo:rustc-link-lib=z");

    // If we are running via cross
    if let Ok(sysroot) = env::var("CROSS_SYSROOT") {
        let path = PathBuf::from(sysroot).join("usr/lib");
        let path = path.join(&env::var("TARGET").unwrap());
        println!("cargo:rustc-link-search=native={}", path.display());
        let path = path.join("libz.so");
        println!("cargo:rustc-link-search=native={}", path.display());
        println!("cargo:rustc-link-lib=static=z");
        
        
    }

    if let Ok(output_path) = env::var("CARGO_NDK_OUTPUT_PATH") {
        let sysroot_libs_path =
            PathBuf::from(env::var_os("CARGO_NDK_SYSROOT_LIBS_PATH").unwrap());
        let lib_path = sysroot_libs_path.join("libz.so");
        std::fs::copy(
            lib_path,
            Path::new(&output_path)
                .join(&env::var("CARGO_NDK_ANDROID_TARGET").unwrap())
                .join("libz.so"),
        )
        .unwrap();
    }
}

fn main() {
    compile();
    gen_bindings();
}

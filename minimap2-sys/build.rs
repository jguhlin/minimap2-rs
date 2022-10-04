use pkg_config;
use std::env;
use std::path::PathBuf;

// #[cfg(feature = "bindgen")]
fn gen() {
    println!("Generating...");
    println!("cargo:rerun-if-changed=minimap2/*.c");

    // kthread.o kalloc.o misc.o bseq.o sketch.o sdust.o options.o index.o
    // lchain.o align.o hit.o seed.o map.o format.o pe.o esterr.o splitidx.o
    // ksw2_ll_sse.o ksw2_extz2_sse41.o ksw2_extd2_sse41.o ksw2_exts2_sse41.o
    // ksw2_extz2_sse2.o ksw2_extd2_sse2.o ksw2_exts2_sse2.o ksw2_dispatch.o

    let mut cc = cc::Build::new();
    cc.flag("-DHAVE_KALLOC");
    cc.flag("-O2");
    cc.flag("-Wall");
    cc.flag("-lm");
    cc.flag("-lz");
    cc.flag("-lpthread");
    cc.flag("-msse4.1");
    cc.flag("-DKSW_CPU_DISPATCH");
    cc.flag("-DKSW_SSE2_ONLY");
    cc.static_flag(true);

    let files: Vec<_> = std::fs::read_dir("minimap2")
        .unwrap()
        .map(|f| f.unwrap().path())
        .collect();
    for file in files {
        // Skip "main.c" and "example.c"
        if file.file_name().unwrap() == "main.c" || file.file_name().unwrap() == "example.c" {
            continue;
        }

        if let Some(x) = file.extension() {
            if x == "c" {
                cc.file(file);
            }
        }
    }

    cc.compile("libminimap");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

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
        .generate()
        .expect("Couldn't write bindings!")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Unable to create bindings");
}

// #[cfg(not(feature = "bindgen"))]
// fn gen() {}

fn main() {
    pkg_config::Config::new().probe("zlib").unwrap();
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=pthread");
    gen();
}

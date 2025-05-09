//! API providing a rusty interface to minimap2
//!
//! This library supports statically linking and compiling minimap2 directly, no separate install is required.
//!
//! # Implementation
//! This is a wrapper library around `minimap2-sys`, which are lower level bindings for minimap2.
//!
//! # Caveats
//! Setting threads with the builder pattern applies only to building the index, not the mapping.
//! For an example of using multiple threads with mapping, see: [fakeminimap2](https://github.com/jguhlin/minimap2-rs/blob/main/fakeminimap2/src/main.rs)
//!
//! # Crate Features
//! This crate has multiple create features available.
//! * map-file - Enables the ability to map a file directly to a reference. Enabled by deafult
//! * htslib - Provides an interface to minimap2 that returns rust_htslib::Records
//! * simde - Enables SIMD Everywhere library in minimap2
//! * zlib-ng - Enables the use of zlib-ng for faster compression
//! * curl - Enables curl for htslib
//! * static - Builds minimap2 as a static library
//! * sse2only - Builds minimap2 with only SSE2 support
//!
//! ## Previously Supported Features
//! * mm2-fast - Uses the mm2-fast library instead of standard minimap2
//!
//! If needed, this can be re-enabled.
//!
//! # Compile-time options
//! I recommend the following:
//! ```toml
//! [profile.release]
//! opt-level = 3
//! lto = "fat"
//! codegen-units  = 1
//! ```
//!
//! # Examples
//! ## Mapping a file to a reference
//! ```no_run
//! use minimap2::{Aligner, Preset};
//! let mut aligner = Aligner::builder()
//! .map_ont()
//! .with_index_threads(8)
//! .with_cigar()
//! .with_index("ReferenceFile.fasta", None)
//! .expect("Unable to build index");
//!
//! let seq = b"ACTGACTCACATCGACTACGACTACTAGACACTAGACTATCGACTACTGACATCGA";
//! let alignment = aligner
//! .map(seq, false, false, None, None, Some(b"Sample Query"))
//! .expect("Unable to align");
//! ```
//!
//! ## Mapping a file to an individual target sequence
//! ```no_run
//! use minimap2::{Aligner, Preset};
//! # let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
//! let aligner = Aligner::builder().map_ont().with_seq(seq.as_bytes()).expect("Unable to build index");
//! let query = b"CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCG";
//! let hits = aligner.map(query, false, false, None, None, Some(b"Query Name"));
//! assert_eq!(hits.unwrap().len(), 1);
//! ```

use std::cell::RefCell;

use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;
use std::num::NonZeroI32;
use std::path::Path;
use std::sync::Arc;

use std::os::unix::ffi::OsStrExt;

use libc::c_void;
use minimap2_sys::*;

pub use minimap2_sys as ffi;

#[cfg(feature = "map-file")]
use needletail::parse_fastx_file;

#[cfg(feature = "htslib")]
pub mod htslib;

/// Alias for mm_mapop_t
pub type MapOpt = mm_mapopt_t;

/// Alias for mm_idxopt_t
pub type IdxOpt = mm_idxopt_t;

// TODO: Probably a better way to handle this...
/// C string constants for passing to minimap2
static LRHQAE: &CStr = c"lr:hqae";
static LRHQ: &CStr = c"lr:hq";
static SPLICE: &CStr = c"splice";
static SPLICEHQ: &CStr = c"splice:hq";
static SPLICESR: &CStr = c"splice:sr";
static ASM: &CStr = c"asm";
static ASM5: &CStr = c"asm5";
static ASM10: &CStr = c"asm10";
static ASM20: &CStr = c"asm20";
static SR: &CStr = c"sr";
static MAP_PB: &CStr = c"map-pb";
static MAP_HIFI: &CStr = c"map-hifi";
static MAP_ONT: &CStr = c"map-ont";
static AVA_PB: &CStr = c"ava-pb";
static AVA_ONT: &CStr = c"ava-ont";

// These aren't listed in the command anymore, but are still available
static SHORT: &CStr = c"short";
static MAP10K: &CStr = c"map10k";
static CDNA: &CStr = c"cdna";

/// Strand enum
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
pub enum Strand {
    #[default]
    Forward,
    Reverse,
}

impl std::fmt::Display for Strand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strand::Forward => write!(f, "+"),
            Strand::Reverse => write!(f, "-"),
        }
    }
}

/// Preset's for minimap2 config
#[derive(Debug, Clone)]
pub enum Preset {
    LrHqae,
    LrHq,
    Splice,
    SpliceHq,
    SpliceSr,
    Asm,
    Asm5,
    Asm10,
    Asm20,
    Sr,
    MapPb,
    MapHifi,
    MapOnt,
    AvaPb,
    AvaOnt,
    Short,
    Map10k,
    Cdna,
}

// Convert to c string for input into minimap2
impl From<Preset> for *const libc::c_char {
    fn from(preset: Preset) -> Self {
        match preset {
            Preset::LrHqae => LRHQAE.as_ptr(),
            Preset::LrHq => LRHQ.as_ptr(),
            Preset::Splice => SPLICE.as_ptr(),
            Preset::SpliceHq => SPLICEHQ.as_ptr(),
            Preset::SpliceSr => SPLICESR.as_ptr(),
            Preset::Asm => ASM.as_ptr(),
            Preset::Asm5 => ASM5.as_ptr(),
            Preset::Asm10 => ASM10.as_ptr(),
            Preset::Asm20 => ASM20.as_ptr(),
            Preset::Sr => SR.as_ptr(),
            Preset::MapPb => MAP_PB.as_ptr(),
            Preset::MapHifi => MAP_HIFI.as_ptr(),
            Preset::MapOnt => MAP_ONT.as_ptr(),
            Preset::AvaPb => AVA_PB.as_ptr(),
            Preset::AvaOnt => AVA_ONT.as_ptr(),
            Preset::Short => SHORT.as_ptr(),
            Preset::Map10k => MAP10K.as_ptr(),
            Preset::Cdna => CDNA.as_ptr(),
        }
    }
}

/// Represents a splice junction and its associated score
#[derive(Debug, Clone)]
pub struct Junction {
    pub target_name: Option<Arc<String>>,
    pub start: u32,
    pub end: u32,
    pub query_name: Option<Arc<String>>,
    pub score: u32,
    pub strand: Strand,
}
impl Junction {
    pub fn new(
        target_name: Option<Arc<String>>,
        start: u32,
        end: u32,
        query_name: Option<Arc<String>>,
        score: u32,
        strand: Strand,
    ) -> Self {
        Self {
            target_name,
            start,
            end,
            query_name,
            score,
            strand,
        }
    }
}

/// Alignment type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignmentType {
    Primary,
    Secondary,
    Inversion,
}

/// Alignment struct when alignment flag is set
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alignment {
    /// The edit distance as calculated in cmappy.h: `h->NM = r->blen - r->mlen + r->p->n_ambi;`
    pub nm: i32,
    pub cigar: Option<Vec<(u32, u8)>>,
    pub cigar_str: Option<String>,
    pub md: Option<String>,
    pub cs: Option<String>,
    pub alignment_score: Option<i32>,
}

/// Mapping result
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mapping {
    // The query sequence name.
    pub query_name: Option<Arc<String>>,
    pub query_len: Option<NonZeroI32>,
    pub query_start: i32,
    pub query_end: i32,
    pub strand: Strand,
    pub target_name: Option<Arc<String>>,
    pub target_len: i32,
    pub target_start: i32,
    pub target_end: i32,
    pub target_id: i32,
    pub match_len: i32,
    pub block_len: i32,
    pub mapq: u32,
    pub is_primary: bool,
    pub is_supplementary: bool,
    pub is_spliced: bool,
    pub trans_strand: Option<Strand>,
    pub alignment: Option<Alignment>,
}

// Thread local buffer (memory management) for minimap2
thread_local! {
    static BUF: RefCell<ThreadLocalBuffer> = RefCell::new(ThreadLocalBuffer::new());
}

/// ThreadLocalBuffer for minimap2 memory management
#[derive(Debug)]
struct ThreadLocalBuffer {
    buf: *mut mm_tbuf_t,
    // max_uses: usize,
    // uses: usize,
}

impl ThreadLocalBuffer {
    pub fn new() -> Self {
        let buf = unsafe { mm_tbuf_init() };
        Self {
            buf,
            // max_uses: 15,
            // uses: 0,
        }
    }
    /// Return the buffer, checking how many times it has been borrowed.
    /// Free the memory of the old buffer and reinitialise a new one If
    /// num_uses exceeds max_uses.
    pub fn get_buf(&mut self) -> *mut mm_tbuf_t {
        /* if self.uses > self.max_uses {
            // println!("renewing threadbuffer");
            self.free_buffer();
            let buf = unsafe { mm_tbuf_init() };
            self.buf = buf;
            self.uses = 0;
        }
        self.uses += 1; */
        self.buf
    }

    fn free_buffer(&mut self) {
        unsafe { mm_tbuf_destroy(self.buf) };
    }
}

/// Handle destruction of thread local buffer properly.
impl Drop for ThreadLocalBuffer {
    fn drop(&mut self) {
        // unsafe { mm_tbuf_destroy(self.buf) };
        self.free_buffer();
    }
}

impl Default for ThreadLocalBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Clone, Copy)]
pub struct Unset;

#[derive(Default, Clone, Copy)]
pub struct PresetSet;

#[derive(Default, Clone, Copy)]
pub struct Built;

pub trait BuilderState {}
impl BuilderState for Unset {}
impl BuilderState for PresetSet {}
impl BuilderState for Built {}
impl BuilderState for () {}

pub trait AcceptsParams {}
impl AcceptsParams for PresetSet {}
impl AcceptsParams for Unset {}

/// Aligner struct, mimicking minimap2's python interface
///
/// ```
/// # use minimap2::*;
/// Aligner::builder();
/// ```

#[derive(Clone)]
pub struct Aligner<S: BuilderState> {
    /// Index options passed to minimap2 (mm_idxopt_t)
    pub idxopt: IdxOpt,

    /// Mapping options passed to minimap2 (mm_mapopt_t)
    pub mapopt: MapOpt,

    /// Number of threads to create the index with
    pub threads: usize,

    /// Index created by minimap2
    pub idx: Option<Arc<MmIdx>>,

    /// Index reader created by minimap2
    pub idx_reader: Option<Arc<mm_idx_reader_t>>,

    /// Whether to add soft clipping to CIGAR result
    pub cigar_clipping: bool,

    // State of the builder
    _state: S,
}

/// Create a default aligner
impl Default for Aligner<Unset> {
    fn default() -> Self {
        Self {
            idxopt: Default::default(),
            mapopt: Default::default(),
            threads: 1,
            idx: None,
            idx_reader: None,
            cigar_clipping: false,
            _state: Unset,
        }
    }
}

impl Aligner<()> {
    /// Create a new aligner with default options
    pub fn builder() -> Aligner<Unset> {
        let mut aligner = Aligner {
            mapopt: MapOpt {
                seed: 11,
                // best_n: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        unsafe {
            minimap2_sys::mm_set_opt(&0, &mut aligner.idxopt, &mut aligner.mapopt);
        }

        aligner
    }
}

impl Aligner<Unset> {
    /// Ergonomic function for Aligner. Sets the minimap2 preset to lr:hq.
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    pub fn lrhq(self) -> Aligner<PresetSet> {
        self.preset(Preset::LrHq)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to splice
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().splice();
    /// ```
    pub fn splice(self) -> Aligner<PresetSet> {
        self.preset(Preset::Splice)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to splice:hq
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().splice_hq();
    /// ```
    pub fn splice_hq(self) -> Aligner<PresetSet> {
        self.preset(Preset::SpliceHq)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to splice:sr
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    ///
    /// ```rust
    /// # use minimap2::*;
    /// Aligner::builder().splice_sr();
    /// ```
    pub fn splice_sr(self) -> Aligner<PresetSet> {
        self.preset(Preset::SpliceSr)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to Asm
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().asm();
    /// ```
    pub fn asm(self) -> Aligner<PresetSet> {
        self.preset(Preset::Asm)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to Asm5
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().asm5();
    /// ```
    pub fn asm5(self) -> Aligner<PresetSet> {
        self.preset(Preset::Asm5)
    }
    /// Ergonomic function for Aligner. Sets the minimap2 preset to Asm10
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().asm10();
    /// ```
    pub fn asm10(self) -> Aligner<PresetSet> {
        self.preset(Preset::Asm10)
    }
    /// Ergonomic function for Aligner. Sets the minimap2 preset to Asm20
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().asm20();
    /// ```
    pub fn asm20(self) -> Aligner<PresetSet> {
        self.preset(Preset::Asm20)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to sr
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().sr();
    /// ```
    pub fn sr(self) -> Aligner<PresetSet> {
        self.preset(Preset::Sr)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to MapPb
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map_pb();
    /// ```
    pub fn map_pb(self) -> Aligner<PresetSet> {
        self.preset(Preset::MapPb)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to MapHifi
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map_hifi();
    /// ```
    pub fn map_hifi(self) -> Aligner<PresetSet> {
        self.preset(Preset::MapHifi)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to MapOnt.
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map_ont();
    /// ```
    pub fn map_ont(self) -> Aligner<PresetSet> {
        self.preset(Preset::MapOnt)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to AvaPb
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().ava_pb();
    /// ```
    pub fn ava_pb(self) -> Aligner<PresetSet> {
        self.preset(Preset::AvaPb)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to AvaOnt.
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().ava_ont();
    /// ```
    pub fn ava_ont(self) -> Aligner<PresetSet> {
        self.preset(Preset::AvaOnt)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to Short
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().short();
    /// ```
    pub fn short(self) -> Aligner<PresetSet> {
        self.preset(Preset::Short)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to Map10k
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map10k();
    /// ```
    pub fn map10k(self) -> Aligner<PresetSet> {
        self.preset(Preset::Map10k)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to cdna
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().cdna();
    /// ```
    pub fn cdna(self) -> Aligner<PresetSet> {
        self.preset(Preset::Cdna)
    }

    /// Create an aligner using a preset.
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    pub fn preset(mut self, preset: Preset) -> Aligner<PresetSet> {
        unsafe {
            mm_set_opt(&0, &mut self.idxopt, &mut self.mapopt);
            mm_set_opt(preset.into(), &mut self.idxopt, &mut self.mapopt)
        };

        Aligner {
            idxopt: self.idxopt,
            mapopt: self.mapopt,
            threads: self.threads,
            idx: self.idx,
            idx_reader: self.idx_reader,
            cigar_clipping: self.cigar_clipping,
            _state: PresetSet,
        }
    }

    // These next few are valid for both Unset and PresetSet
    // If you make a change copy it below!
}

impl<S> Aligner<S>
where
    S: BuilderState + AcceptsParams,
{
    /// Set Alignment mode / cigar mode in minimap2
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map_ont().with_cigar();
    /// ```
    ///
    pub fn with_cigar(mut self) -> Self {
        // Make sure MM_F_CIGAR flag isn't already set
        assert!((self.mapopt.flag & MM_F_CIGAR as i64) == 0);

        self.mapopt.flag |= MM_F_CIGAR as i64 | MM_F_OUT_CS as i64;
        self
    }

    pub fn with_cigar_clipping(mut self) -> Self {
        self.cigar_clipping = true;
        self
    }

    pub fn with_sam_out(mut self) -> Self {
        // Make sure MM_F_CIGAR flag isn't already set
        assert!((self.mapopt.flag & MM_F_OUT_SAM as i64) == 0);

        self.mapopt.flag |= MM_F_OUT_SAM as i64;
        self
    }

    pub fn with_sam_hit_only(mut self) -> Self {
        // Make sure MM_F_CIGAR flag isn't already set
        assert!((self.mapopt.flag & MM_F_SAM_HIT_ONLY as i64) == 0);

        self.mapopt.flag |= MM_F_SAM_HIT_ONLY as i64;
        self
    }

    /// Sets the gap open penalty for minimap2.
    ///
    /// minimap2 -O 4 sets both the short and long gap open penalty to 4.
    /// [minimap2 code](https://github.com/lh3/minimap2/blob/618d33515e5853c4576d5a3d126fdcda28f0e8a4/main.c#L315)
    ///
    /// To set the long gap open penalty, simply provide a value for `penalty_long`.
    pub fn with_gap_open_penalty(mut self, penalty: i32, penalty_long: Option<i32>) -> Self {
        self.mapopt.q = penalty;
        if let Some(penalty_long) = penalty_long {
            self.mapopt.q2 = penalty_long;
        } else {
            self.mapopt.q2 = penalty;
        }
        self
    }

    /// Sets the number of threads minimap2 will use for building the index
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().with_index_threads(10);
    /// ```
    ///
    /// Set the number of threads (prefer to use the struct config)
    pub fn with_index_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    #[deprecated(since = "0.1.17", note = "Please use `with_index_threads` instead")]
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    // Check options
    /// Check if the options are valid - Maps to mm_check_opt in minimap2
    pub fn check_opts(&self) -> Result<(), &'static str> {
        let result = unsafe { mm_check_opt(&self.idxopt, &self.mapopt) };

        if result == 0 {
            Ok(())
        } else {
            Err("Invalid options")
        }
    }

    /// Set index parameters for minimap2 using builder pattern
    /// Creates the index as well with the given number of threads (set at struct creation).
    /// You must set the number of threads before calling this function.
    ///
    /// Parameters:
    /// path: Location of pre-built index or FASTA/FASTQ file (may be gzipped or plaintext)
    /// Output: Option (None) or a filename
    ///
    /// Returns the aligner with the index set
    ///
    /// ```
    /// # use minimap2::*;
    /// // Do not save the index file
    /// Aligner::builder().map_ont().with_index("test_data/test_data.fasta", None);
    ///
    /// // Save the index file as my_index.mmi
    /// Aligner::builder().map_ont().with_index("test_data/test_data.fasta", Some("my_index.mmi"));
    ///
    /// // Use the previously built index
    /// Aligner::builder().map_ont().with_index("my_index.mmi", None);
    /// ```
    pub fn with_index<P>(
        self,
        path: P,
        output: Option<&str>,
    ) -> Result<Aligner<Built>, &'static str>
    where
        P: AsRef<Path>,
    {
        match self.set_index(path, output) {
            Ok(aln) => Ok(aln),
            Err(e) => Err(e),
        }
    }

    /// Sets the index, uses the builder pattern. Returns Aligner<Built> if successful.
    pub fn set_index<P>(
        mut self,
        path: P,
        output: Option<&str>,
    ) -> Result<Aligner<Built>, &'static str>
    where
        P: AsRef<Path>,
    {
        let path_str = match std::ffi::CString::new(path.as_ref().as_os_str().as_bytes()) {
            Ok(path) => path,
            Err(_) => {
                return Err("Invalid Path for Index");
            }
        };

        // Confirm file exists
        if !path.as_ref().exists() {
            return Err("Index File does not exist");
        }

        // Confirm file is not empty
        if path.as_ref().metadata().unwrap().len() == 0 {
            return Err("Index File is empty");
        }

        let output = match output {
            Some(output) => match std::ffi::CString::new(output) {
                Ok(output) => output,
                Err(_) => return Err("Invalid Output for Index"),
            },
            None => std::ffi::CString::new(Vec::new()).unwrap(),
        };

        let idx_reader = MaybeUninit::new(unsafe {
            mm_idx_reader_open(path_str.as_ptr(), &self.idxopt, output.as_ptr())
        });

        let idx;

        let idx_reader = unsafe { idx_reader.assume_init() };

        unsafe {
            // Just a test read? Just following: https://github.com/lh3/minimap2/blob/master/python/mappy.pyx#L147
            idx = MaybeUninit::new(mm_idx_reader_read(
                // self.idx_reader.as_mut().unwrap() as *mut mm_idx_reader_t,
                &mut *idx_reader as *mut mm_idx_reader_t,
                self.threads as libc::c_int,
            ));
            // Close the reader
            mm_idx_reader_close(idx_reader);
            // Set index opts
            mm_mapopt_update(&mut self.mapopt, *idx.as_ptr());
            // Idx index name
            mm_idx_index_name(idx.assume_init());
        }

        let mm_idx = unsafe { idx.assume_init() };
        self.idx = Some(Arc::new(mm_idx.into()));

        Ok(Aligner {
            idxopt: self.idxopt,
            mapopt: self.mapopt,
            threads: self.threads,
            idx: self.idx,
            idx_reader: Some(Arc::new(unsafe { *idx_reader })),
            cigar_clipping: self.cigar_clipping,
            _state: Built,
        })
    }

    /// Use a single sequence as the index. Sets the sequence ID to "N/A".
    /// Can not be combined with `with_index` or `set_index`.
    /// Following the mappy implementation, this also sets mapopt.mid_occ to 1000.
    /// ```
    /// # use minimap2::*;
    /// # let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
    /// let aligner = Aligner::builder().map_ont().with_seq(seq.as_bytes()).expect("Unable to build index");
    /// let query = b"CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCG";
    /// let hits = aligner.map(query, false, false, None, None, Some(b"Query Name"));
    /// assert_eq!(hits.unwrap().len(), 1);
    /// ```
    pub fn with_seq(self, seq: &[u8]) -> Result<Aligner<Built>, &'static str>
// where T: AsRef<[u8]> + std::ops::Deref<Target = str>,
    {
        let default_id = "N/A";
        self.with_seq_and_id(seq, default_id.as_bytes())
    }

    /// Use a single sequence as the index. Sets the sequence ID to "N/A".
    /// Can not be combined with `with_index` or `set_index`.
    /// Following the mappy implementation, this also sets mapopt.mid_occ to 1000.
    /// ```
    /// # use minimap2::*;
    /// # let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
    /// # let id = "seq1";
    /// let aligner = Aligner::builder().map_ont().with_seq_and_id(seq.as_bytes(), id.as_bytes()).expect("Unable to build index");
    /// let query = b"CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCG";
    /// let hits = aligner.map(query, false, false, None, None, Some(b"Sample Query"));
    /// assert_eq!(hits.as_ref().unwrap().len(), 1);
    /// assert_eq!(hits.as_ref().unwrap()[0].target_name.as_ref().unwrap().as_str(), id);
    /// ```
    pub fn with_seq_and_id(self, seq: &[u8], id: &[u8]) -> Result<Aligner<Built>, &'static str>
// where T: AsRef<[u8]> + std::ops::Deref<Target = str>,
    {
        assert!(
            self.idx.is_none(),
            "Index already set. Can not set sequence as index."
        );
        assert!(!seq.is_empty(), "Sequence is empty");
        assert!(!id.is_empty(), "ID is empty");

        self.with_seqs_and_ids(&[seq.to_vec()], &[id.to_vec()])
    }

    /// TODO: Does not work for more than 1 seq currently!
    /// Pass multiple sequences to build an index functionally.
    /// Following the mappy implementation, this also sets mapopt.mid_occ to 1000.
    /// Can not be combined with `with_index` or `set_index`.
    /// Sets the sequence IDs to "Unnamed Sequence n" where n is the sequence number.
    pub fn with_seqs(self, seqs: &[Vec<u8>]) -> Result<Aligner<Built>, &'static str> {
        assert!(
            self.idx.is_none(),
            "Index already set. Can not set sequence as index."
        );
        assert!(!seqs.is_empty(), "Must have at least one sequence");

        let mut ids: Vec<Vec<u8>> = Vec::new();
        for i in 0..seqs.len() {
            ids.push(format!("Unnamed Sequence {}", i).into_bytes());
        }

        self.with_seqs_and_ids(seqs, &ids)
    }

    /// TODO: Does not work for more than 1 seq currently!
    /// Pass multiple sequences and corresponding IDs to build an index functionally.
    /// Following the mappy implementation, this also sets mapopt.mid_occ to 1000.
    // This works for a single sequence, but not for multiple sequences.
    // Maybe convert the underlying function itself?
    // https://github.com/lh3/minimap2/blob/c2f07ff2ac8bdc5c6768e63191e614ea9012bd5d/index.c#L408
    pub fn with_seqs_and_ids(
        mut self,
        seqs: &[Vec<u8>],
        ids: &[Vec<u8>],
    ) -> Result<Aligner<Built>, &'static str> {
        assert!(
            seqs.len() == ids.len(),
            "Number of sequences and IDs must be equal"
        );
        assert!(!seqs.is_empty(), "Must have at least one sequence and ID");

        let seqs: Vec<std::ffi::CString> = seqs
            .iter()
            .map(|s| std::ffi::CString::new(s.clone()).expect("Invalid Sequence"))
            .collect();
        let ids: Vec<std::ffi::CString> = ids
            .iter()
            .map(|s| std::ffi::CString::new(s.clone()).expect("Invalid ID"))
            .collect();

        let idx = MaybeUninit::new(unsafe {
            mm_idx_str(
                self.idxopt.w as i32,
                self.idxopt.k as i32,
                (self.idxopt.flag & 1) as i32,
                self.idxopt.bucket_bits as i32,
                seqs.len() as i32,
                seqs.as_ptr() as *mut *const libc::c_char,
                ids.as_ptr() as *mut *const libc::c_char,
            )
        });

        let mm_idx = unsafe { idx.assume_init() };
        self.idx = Some(Arc::new(mm_idx.into()));

        self.mapopt.mid_occ = 1000;

        let aln = Aligner {
            idxopt: self.idxopt,
            mapopt: self.mapopt,
            threads: self.threads,
            idx: self.idx,
            idx_reader: None,
            cigar_clipping: self.cigar_clipping,
            _state: Built,
        };

        Ok(aln)
    }

    /// Applies an additional preset to the aligner
    /// WARNING: This overwrites multiple other parameters. Make sure you know what you are doing
    ///
    /// Presets should be called before any other options are set, as they change multiple
    /// options at once.
    pub fn additional_preset(mut self, preset: Preset) -> Self {
        unsafe { mm_set_opt(preset.into(), &mut self.idxopt, &mut self.mapopt) };

        self
    }
}

impl Aligner<Built> {
    /// Load splice/junc data from `bed_path` into the underlying `mm_idx_t`.
    /// Equivalent to -j <bed_path> in minimap2.
    pub fn read_junction(&self, bed_path: &str) -> Result<(), i32> {
        let idx: *mut mm_idx_t = self.idx.as_ref().unwrap().idx as *mut _;

        let c_bed = CString::new(bed_path).map_err(|_| -1)?;
        // call into C
        let ret = unsafe {
            mm_idx_jjump_read(
                idx,
                c_bed.as_ptr(),
                MM_JUNC_ANNO as libc::c_int,
                -1 as libc::c_int,
            )
        };
        if ret == 0 {
            Ok(())
        } else {
            println!("Failed to load the jump BED file");
            Err(ret)
        }
    }

    ///
    pub fn read_pass1(&self, bed_path: &str) -> Result<(), i32> {
        let idx: *mut mm_idx_t = self.idx.as_ref().unwrap().idx as *mut _;

        let c_bed = CString::new(bed_path).map_err(|_| -1)?;
        // call into C
        let ret = unsafe {
            mm_idx_jjump_read(
                idx,
                c_bed.as_ptr(),
                MM_JUNC_MISC as libc::c_int,
                5 as libc::c_int,
            )
        };
        if ret == 0 {
            Ok(())
        } else {
            println!("Failed to load the pass-1 jump BED file");
            Err(ret)
        }
    }

    pub fn read_splice_scores(&self, file_path: &str) -> Result<(), i32> {
        let idx: *mut mm_idx_t = self.idx.as_ref().unwrap().idx as *mut _;

        let c_filepath = CString::new(file_path).map_err(|_| -1)?;
        unsafe {
            mm_idx_spsc_read(idx, c_filepath.as_ptr(), mm_max_spsc_bonus(&self.mapopt));
        };

        if unsafe { (*idx).spsc == std::ptr::null_mut() } {
            println!("Failed to load the splice score file");
            Err(-1)
        } else {
            Ok(())
        }
    }

    /// Returns the number of sequences in the index
    pub fn n_seq(&self) -> u32 {
        unsafe {
            let idx: *const mm_idx_t = self.idx.as_ref().unwrap().idx as *const _;
            (*idx).n_seq as u32
        }
    }

    /// Get sequences direct from the index
    ///
    /// Returns a reference to the sequence at the given index
    /// Remainds valid as long as the aligner is valid
    pub fn get_seq<'aln>(&'aln self, i: usize) -> Option<&'aln mm_idx_seq_t> {
        unsafe {
            let idx: *const mm_idx_t = self.idx.as_ref().unwrap().idx as *const _;

            // todo, should this be > or >=
            if i > self.n_seq() as usize {
                return None;
            }
            let seq = (*idx).seq;
            let seq = seq.offset(i as isize);
            let seq = &*seq;
            Some(seq)
        }
    }

    /// Returns a reference to the index associated with this aligner
    ///
    /// Safe to use as long as the aligner is valid (and should be since it's `Built`)
    fn get_idx(&self) -> *const mm_idx_t {
        self.idx.as_ref().unwrap().idx as *const _
    }

    // https://github.com/lh3/minimap2/blob/master/python/mappy.pyx#L164
    // TODO: I doubt extra_flags is working properly...
    // TODO: Python allows for paired-end mapping with seq2: Option<&[u8]>, but more work to implement
    /// Aligns a given sequence (as bytes) to the index associated with this aligner
    ///
    /// Parameters:
    /// seq: Sequence to align
    /// cs: Whether to output CIGAR string
    /// MD: Whether to output MD tag
    /// max_frag_len: Maximum fragment length
    /// extra_flags: Extra flags to pass to minimap2 as `Vec<u64>`
    /// query_name: Name of the query sequence
    pub fn map(
        &self,
        seq: &[u8],
        cs: bool,
        md: bool,
        max_frag_len: Option<usize>,
        extra_flags: Option<&[u64]>,
        query_name: Option<&[u8]>,
    ) -> Result<Vec<Mapping>, &'static str> {
        // Make sure index is set
        if !self.has_index() {
            return Err("No index");
        }

        // Make sure sequence is not empty
        if seq.is_empty() {
            return Err("Sequence is empty");
        }

        let qname_cstring;

        let query_name_cstr: Option<&CStr> = match query_name {
            None => None,
            Some(qname_slice) => {
                if qname_slice.last() != Some(&b'\0') {
                    qname_cstring = Some(CString::new(qname_slice).expect("Invalid query name"));
                    Some(qname_cstring.as_ref().unwrap().as_c_str())
                } else {
                    Some(
                        CStr::from_bytes_with_nul(query_name.as_ref().unwrap().as_ref())
                            .expect("Invalid query name"),
                    )
                }
            }
        };

        let mut mm_reg: MaybeUninit<*mut mm_reg1_t> = MaybeUninit::uninit();

        // Number of results
        let mut n_regs: i32 = 0;
        let mut map_opt = self.mapopt.clone();

        // if max_frag_len is not None: map_opt.max_frag_len = max_frag_len
        if let Some(max_frag_len) = max_frag_len {
            map_opt.max_frag_len = max_frag_len as i32;
        }

        // if extra_flags is not None: map_opt.flag |= extra_flags
        if let Some(extra_flags) = extra_flags {
            for flag in extra_flags {
                map_opt.flag |= *flag as i64;
            }
        }

        let query_name_arc = query_name_cstr.map(|x| Arc::new(x.to_owned().into_string().unwrap()));

        let qname = match query_name_cstr {
            None => std::ptr::null(),
            Some(qname) => qname.as_ref().as_ptr() as *const ::std::os::raw::c_char,
        };

        let mappings = BUF.with_borrow_mut(|buf| {
            let km: *mut libc::c_void = unsafe { mm_tbuf_get_km(buf.get_buf()) };

            mm_reg = MaybeUninit::new(unsafe {
                mm_map(
                    &**self.idx.as_ref().unwrap().as_ref() as *const mm_idx_t,
                    seq.len() as i32,
                    seq.as_ptr() as *const ::std::os::raw::c_char,
                    &mut n_regs,
                    buf.get_buf(),
                    &map_opt,
                    qname,
                )
            });

            let mut mappings = Vec::with_capacity(n_regs as usize);

            for i in 0..n_regs {
                unsafe {
                    let mm_reg1_mut_ptr = (*mm_reg.as_ptr()).offset(i as isize);
                    let mm_reg1_const_ptr = mm_reg1_mut_ptr as *const mm_reg1_t;
                    let reg: mm_reg1_t = *mm_reg1_mut_ptr;

                    let idx = self.get_idx();

                    let contig = {
                        let seqs = (*idx).seq;
                        let entry = seqs.offset(reg.rid as isize);
                        let name_ptr: *const libc::c_char = (*entry).name;
                        std::ffi::CStr::from_ptr(name_ptr)
                    };

                    // TODO: deprecate?
                    let _target_len = {
                        let seqs = (*idx).seq;
                        let entry = seqs.offset(reg.rid as isize);
                        (*entry).len as i32
                    };

                    let is_primary = reg.parent == reg.id && (reg.sam_pri() > 0);
                    let is_supplementary = (reg.parent == reg.id) && (reg.sam_pri() == 0);
                    let is_spliced = reg.is_spliced() != 0;
                    let trans_strand = if let Some(extra) = reg.p.as_ref() {
                        match extra.trans_strand() {
                            1 => Some(Strand::Forward),
                            2 => Some(Strand::Reverse),
                            _ => None,
                        }
                    } else {
                        None
                    };

                    // todo holy heck this code is ugly
                    let alignment = if !reg.p.is_null() {
                        let p = &*reg.p;

                        // calculate the edit distance
                        let nm = reg.blen - reg.mlen + p.n_ambi() as i32;
                        let n_cigar = p.n_cigar;

                        // Create a vector of the cigar blocks
                        let (cigar, cigar_str) = if n_cigar > 0 {
                            let mut cigar = p
                                .cigar
                                .as_slice(n_cigar as usize)
                                .to_vec()
                                .iter()
                                .map(|c| ((c >> 4), (c & 0xf) as u8)) // unpack the length and op code
                                .collect::<Vec<(u32, u8)>>();

                            // Fix for adding in soft clipping cigar strings
                            // Taken from minimap2 write_sam_cigar function
                            // clip_len[0] = r->rev? qlen - r->qe : r->qs;
                            // clip_len[1] = r->rev? r->qs : qlen - r->qe;

                            let clip_len0 = if reg.rev() != 0 {
                                seq.len() as i32 - reg.qe
                            } else {
                                reg.qs
                            };

                            let clip_len1 = if reg.rev() != 0 {
                                reg.qs
                            } else {
                                seq.len() as i32 - reg.qe
                            };

                            let mut cigar_str = cigar
                                .iter()
                                .map(|(len, code)| {
                                    let cigar_char = match code {
                                        0 => "M",
                                        1 => "I",
                                        2 => "D",
                                        3 => "N",
                                        4 => "S",
                                        5 => "H",
                                        6 => "P",
                                        7 => "=",
                                        8 => "X",
                                        _ => panic!("Invalid CIGAR code {code}"),
                                    };
                                    format!("{len}{cigar_char}")
                                })
                                .collect::<Vec<String>>()
                                .join("");

                            // int clip_char = (((sam_flag&0x800) || ((sam_flag&0x100) && (opt_flag&MM_F_SECONDARY_SEQ))) &&
                            // !(opt_flag&MM_F_SOFTCLIP)) ? 'H' : 'S';

                            // let clip_char = if (reg.flag & 0x800 != 0) || ((reg.flag & 0x100 != 0) && (map_opt.flag & 0x100 != 0)) && (map_opt.flag & 0x4 == 0) {
                            // 'H'
                            // } else {
                            // 'S'
                            // };

                            // TODO: Support hard clipping
                            let clip_char = 'S';

                            // Pre and append soft clip identifiers to start and end
                            if clip_len0 > 0 {
                                cigar_str = format!("{}{}{}", clip_len0, clip_char, cigar_str);
                                if self.cigar_clipping {
                                    cigar.insert(0, (clip_len0 as u32, 4_u8));
                                }
                            }

                            if clip_len1 > 0 {
                                cigar_str = format!("{}{}{}", cigar_str, clip_len1, clip_char);
                                if self.cigar_clipping {
                                    cigar.push((clip_len1 as u32, 4_u8));
                                }
                            }

                            (Some(cigar), Some(cigar_str))
                        } else {
                            (None, None)
                        };

                        let (cs_str, md_str) = if cs || md {
                            let cs_str = if cs {
                                let mut cs_string: *mut libc::c_char = std::ptr::null_mut();
                                let mut m_cs_string: libc::c_int = 0i32;

                                // This solves a weird segfault...
                                // let km = km_init();

                                /*
                                let _cs_len = mm_gen_cs(
                                    km,
                                    &mut cs_string,
                                    &mut m_cs_string,
                                    idx,
                                    mm_reg1_const_ptr,
                                    seq.as_ptr() as *const libc::c_char,
                                    true.into(),
                                );

                                let _cs_string = std::ffi::CStr::from_ptr(cs_string)
                                    .to_str()
                                    .unwrap()
                                    .to_string();
                                */

                                let _cs_len = {
                                    mm_gen_cs(
                                        km,
                                        &mut cs_string,
                                        &mut m_cs_string,
                                        idx,
                                        mm_reg1_const_ptr,
                                        seq.as_ptr() as *const _,
                                        1,
                                    )
                                };
                                let _cs = {
                                    let s =
                                        CStr::from_ptr(cs_string).to_string_lossy().into_owned();
                                    libc::free(cs_string as *mut _);
                                    s
                                };

                                // libc::free(cs_string as *mut c_void);
                                // km_destroy(km);
                                Some(_cs)
                            } else {
                                None
                            };

                            let md_str = if md {
                                // scratch-space pointers & lengths
                                let mut md_buf: *mut libc::c_char = std::ptr::null_mut();
                                let mut md_len: libc::c_int = 0;

                                // generate the MD tag into our ThreadBuffer’s km pool
                                let _written = {
                                    mm_gen_MD(
                                        km,
                                        &mut md_buf,
                                        &mut md_len,
                                        idx,
                                        mm_reg1_const_ptr,
                                        seq.as_ptr() as *const _,
                                    )
                                };

                                // turn it into a Rust String and free the C buffer
                                let md_string = {
                                    let s = std::ffi::CStr::from_ptr(md_buf)
                                        .to_string_lossy()
                                        .into_owned();
                                    libc::free(md_buf as *mut libc::c_void);
                                    s
                                };

                                Some(md_string)
                            } else {
                                None
                            };

                            (cs_str, md_str)
                        } else {
                            (None, None)
                        };

                        Some(Alignment {
                            nm,
                            cigar,
                            cigar_str,
                            md: md_str,
                            cs: cs_str,
                            alignment_score: Some(p.dp_score as i32),
                        })
                    } else {
                        None
                    };

                    let target_name_arc = Arc::new(
                        std::ffi::CStr::from_ptr(contig.as_ptr())
                            .to_str()
                            .unwrap()
                            .to_string(),
                    );

                    let target_len = {
                        let seqs = (*idx).seq;
                        let entry = seqs.offset(reg.rid as isize);
                        (*entry).len as i32
                    };

                    mappings.push(Mapping {
                        target_name: Some(Arc::clone(&target_name_arc)),
                        target_len,
                        target_start: reg.rs,
                        target_end: reg.re,
                        target_id: reg.rid,
                        query_name: query_name_arc.clone(),
                        query_len: NonZeroI32::new(seq.len() as i32),
                        query_start: reg.qs,
                        query_end: reg.qe,
                        strand: if reg.rev() == 0 {
                            Strand::Forward
                        } else {
                            Strand::Reverse
                        },
                        match_len: reg.mlen,
                        block_len: reg.blen,
                        mapq: reg.mapq(),
                        is_primary,
                        is_supplementary,
                        is_spliced,
                        trans_strand,
                        alignment,
                    });
                    libc::free(reg.p as *mut c_void);
                }
            }
            mappings
        });
        // free some stuff here
        unsafe {
            // Free mm_regs
            let ptr: *mut mm_reg1_t = mm_reg.assume_init();
            let c_void_ptr: *mut c_void = ptr as *mut c_void;
            libc::free(c_void_ptr);
        }
        Ok(mappings)
    }

    /// Map entire file
    /// Detects if file is gzip or not and if it's fastq/fasta or not
    /// Best for smaller files (all results are stored in an accumulated Vec!)
    /// What you probably want is to loop through the file yourself and use the map() function
    ///
    /// TODO: Remove cs and md and make them options on the struct
    ///
    #[cfg(feature = "map-file")]
    pub fn map_file(&self, file: &str, cs: bool, md: bool) -> Result<Vec<Mapping>, &'static str> {
        // Make sure index is set
        if self.idx.is_none() {
            return Err("No index");
        }

        // Check that file exists
        if !Path::new(file).exists() {
            return Err("File does not exist");
        }

        // Check that file isn't empty...
        let metadata = std::fs::metadata(file).unwrap();
        if metadata.len() == 0 {
            return Err("File is empty");
        }

        let mut reader = parse_fastx_file(file).expect("Unable to read FASTA/X file");

        // The output vec
        let mut mappings = Vec::new();

        // Iterate over the sequences
        while let Some(record) = reader.next() {
            let record = match record {
                Ok(record) => record,
                Err(_) => {
                    return Err("Error reading record in FASTA/X files. Please confirm integrity.");
                }
            };

            let query_name = record.id().to_vec();
            let mut seq_mappings = self
                .map(&record.seq(), cs, md, None, None, Some(&query_name))
                .unwrap();

            for mapping in seq_mappings.iter_mut() {
                let id = record.id();
                if id.is_empty() {
                    mapping.query_name = Some(Arc::new(
                        format!("Unnamed Seq with Length: {}", record.seq().len()).to_string(),
                    ));
                }
            }

            mappings.extend(seq_mappings);
        }

        Ok(mappings)
    }

    // This is in the python module, so copied here...
    pub fn has_index(&self) -> bool {
        self.idx.is_some()
    }

    /// Provides a mapping with a splice score
    ///
    /// User must provide a junctions vector to store the junctions and their associated scores.
    ///
    /// The junctions vector will *NOT* be cleared and is extended with one entry per junction.
    ///
    /// Adapted from https://github.com/lh3/minimap2/blob/1fd85be6e2515c9194740e1d2e6a2625be36f508/format.c#L263
    ///
    /// *Note*: to score junctions `.with_cigar()` must called.
    pub fn score_junctions(&self, mapping: &Mapping, junctions: &mut Vec<Junction>) {
        assert!(
            (self.mapopt.flag & MM_F_CIGAR as i64) != 0,
            "CIGAR must be set to score junctions."
        );

        // Return early if:
        // 1. The mapping is not spliced
        // 2. The transcript strand is not defined
        // 3. The alignment is not defined
        // 4. The alignment cigar is not defined
        if !mapping.is_spliced {
            // dbg!("Mapping is not spliced");
            return;
        }

        let Some(trans_strand) = mapping.trans_strand else {
            // dbg!("Transcript strand is not defined");
            return;
        };

        let Some(cigar) = mapping
            .alignment
            .as_ref()
            .and_then(|alignment| alignment.cigar.as_ref())
        else {
            dbg!("Alignment cigar is not defined");
            return;
        };

        // Determine reverse status
        let rev = (trans_strand == Strand::Reverse) ^ (mapping.strand == Strand::Reverse);

        let mut target_offset = mapping.target_start as u32;
        let idx = self.get_idx();
        let mut donor = [0; 2];
        let mut acceptor = [0; 2];
        for (len, op) in cigar {
            match op {
                // For skips (introns) (N::3) build a junction score
                3 => {
                    assert!(*len >= 2, "Intron length must be at least 2");

                    unsafe {
                        if rev {
                            // process reverse complement
                            mm_idx_getseq(
                                idx,
                                mapping.target_id as u32,
                                target_offset,
                                target_offset + 2,
                                acceptor.as_ptr() as *mut u8,
                            );
                            mm_idx_getseq(
                                idx,
                                mapping.target_id as u32,
                                target_offset + len - 2,
                                target_offset + len,
                                donor.as_ptr() as *mut u8,
                            );
                            revcomp_splice(&mut acceptor);
                            revcomp_splice(&mut donor);
                        } else {
                            // process standard
                            mm_idx_getseq(
                                idx,
                                mapping.target_id as u32,
                                target_offset,
                                target_offset + 2,
                                donor.as_ptr() as *mut u8,
                            );
                            mm_idx_getseq(
                                idx,
                                mapping.target_id as u32,
                                target_offset + len - 2,
                                target_offset + len,
                                acceptor.as_ptr() as *mut u8,
                            );
                        }
                    }

                    let score1 = match (donor[0], donor[1]) {
                        (2, 3) => 3,
                        (2, 1) => 2,
                        (0, 3) => 1,
                        (_, _) => 0,
                    };

                    let score2 = match (acceptor[0], acceptor[1]) {
                        (0, 2) => 3,
                        (0, 1) => 1,
                        (_, _) => 0,
                    };

                    let junction = Junction::new(
                        mapping.target_name.clone(),
                        target_offset,
                        target_offset + len,
                        mapping.query_name.clone(),
                        score1 + score2,
                        if rev {
                            Strand::Reverse
                        } else {
                            Strand::Forward
                        },
                    );

                    junctions.push(junction);
                }
                // Advance target offset by size for other operations
                _ => {
                    target_offset += len;
                }
            }
        }
    }
}

/// Utility function to reverse complement a splice junction
///
/// Adapted from https://github.com/lh3/minimap2/blob/1fd85be6e2515c9194740e1d2e6a2625be36f508/format.c#L256
#[inline]
fn revcomp_splice(s: &mut [u8; 2]) {
    let c = if s[1] < 4 { 3 - s[1] } else { 4 };
    s[1] = if s[0] < 4 { 3 - s[0] } else { 4 };
    s[0] = c;
}

mod send {
    use super::{Aligner, Built, PresetSet, Unset};

    unsafe impl Sync for Aligner<Unset> {}
    unsafe impl Send for Aligner<Unset> {}
    unsafe impl Sync for Aligner<Built> {}
    unsafe impl Send for Aligner<Built> {}
    unsafe impl Sync for Aligner<PresetSet> {}
    unsafe impl Send for Aligner<PresetSet> {}
}

#[derive(PartialEq, Eq)]
pub enum FileFormat {
    FASTA,
    FASTQ,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligner_between_threads() {
        // Because I'm not sure how this will work with FFI + Threads, want a sanity check
        use std::thread;

        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2)
            .with_index("yeast_ref.mmi", None)
            .unwrap();

        aligner
            .map(
                "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query")
            )
            .unwrap();
        let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
        assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));

        let jh = thread::spawn(move || {
            let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            aligner
        });

        let aligner = jh.join().unwrap();

        let jh = thread::spawn(move || {
            let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            aligner
        });

        let _aligner = jh.join().unwrap();
    }

    #[test]
    fn shared_aligner() {
        // Because I'm not sure how this will work with FFI + Threads, want a sanity check
        use std::sync::Arc;
        use std::thread;

        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2)
            .with_index("yeast_ref.mmi", None)
            .unwrap();

        let aligner = Arc::new(aligner);

        aligner
            .map(
                "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query")
            )
            .unwrap();
        let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
        assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));

        let aligner_handle = Arc::clone(&aligner);
        let jh0 = thread::spawn(move || {
            let mappings = aligner_handle.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            let mappings = aligner_handle.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
        });

        jh0.join().unwrap();

        let aligner_handle = Arc::clone(&aligner);
        let jh1 = thread::spawn(move || {
            let mappings = aligner_handle.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            let mappings = aligner_handle.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
        });

        jh1.join().unwrap();
    }

    #[test]
    fn rayon() {
        // Because I'm not sure how this will work with FFI + Threads, want a sanity check
        use rayon::prelude::*;

        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2)
            .with_cigar()
            .with_index("yeast_ref.mmi", None)
            .unwrap();

        let sequences = vec![
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",
            "GTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGG",
            "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAG",
        ];

        let _results = sequences
            .par_iter()
            .map(|seq| {
                aligner
                    .map(
                        seq.as_bytes(),
                        false,
                        false,
                        None,
                        None,
                        Some(b"Sample Query"),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
    }

    #[test]
    fn does_it_work() {
        let mut mm_idxopt = MaybeUninit::uninit();
        let mut mm_mapopt = MaybeUninit::uninit();

        unsafe { mm_set_opt(&0, mm_idxopt.as_mut_ptr(), mm_mapopt.as_mut_ptr()) };
    }

    #[test]
    fn idxopt() {
        let _x: IdxOpt = Default::default();
    }

    #[test]
    fn mapopt() {
        let _x: mm_mapopt_t = Default::default();
        let _y: MapOpt = Default::default();
    }

    #[test]
    fn aligner_build_manually() {
        let idxopt: IdxOpt = Default::default();

        let mapopt: MapOpt = Default::default();

        let threads = 1;
        let idx = None;
        let idx_reader = None;

        let _aligner = Aligner {
            idxopt,
            mapopt,
            threads,
            idx,
            idx_reader,
            cigar_clipping: false,
            _state: Unset,
        };
    }

    #[test]
    fn test_mapopt_flags_in_aligner() {
        let mut aligner = Aligner::builder();
        aligner.mapopt.set_no_qual();
        assert_eq!(
            aligner.mapopt.flag & MM_F_NO_QUAL as i64,
            MM_F_NO_QUAL as i64
        );
        aligner.mapopt.unset_no_qual();
        assert_eq!(aligner.mapopt.flag & MM_F_NO_QUAL as i64, 0_i64);
    }

    #[test]
    fn test_idxopt_flags_in_aligner() {
        let mut aligner = Aligner::builder();
        aligner.idxopt.set_hpc();
        assert_eq!(aligner.idxopt.flag & MM_I_HPC as i16, MM_I_HPC as i16);
        aligner.idxopt.unset_hpc();
        assert_eq!(aligner.idxopt.flag & MM_I_HPC as i16, 0_i16);
    }

    #[test]
    fn aligner_builder() {
        let _result = Aligner::builder();
    }

    #[test]
    fn aligner_builder_preset() {
        let _result = Aligner::builder().preset(Preset::LrHq);
    }

    #[test]
    fn aligner_builder_preset_with_threads() {
        let _result = Aligner::builder()
            .preset(Preset::LrHq)
            .with_index_threads(1);
    }

    #[test]
    fn create_index_file_missing() {
        let result = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(1)
            .with_index(
                "test_data/test.fa_FILE_NOT_FOUND",
                Some("test_FILE_NOT_FOUND.mmi"),
            );
        assert!(result.is_err());
    }

    #[test]
    fn create_index() {
        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(1);

        println!("{}", aligner.idxopt.w);

        assert!(aligner.idxopt.w == 10);

        aligner
            .with_index("test_data/test_data.fasta", Some("test.mmi"))
            .unwrap();
    }

    #[test]
    fn test_builder() {
        let _aligner = Aligner::builder().preset(Preset::MapOnt);
    }

    #[test]
    fn test_mapping() {
        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2)
            .with_index("yeast_ref.mmi", None)
            .unwrap();

        aligner
            .map(
                "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query")
            )
            .unwrap();
        let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
        println!("{:#?}", mappings);

        // This should be reverse strand
        let mappings = aligner.map("TTTTGCATCGCTGAAAACCCCAAAGTATATTTTAGAACTCGTCTATAGGTTCTACGATTTAACATCCACAGCCTTCTGGTGTCGCTGGTGTTTCAAACACCTCGATATATCACTCCTTCTGAATAACATCCATGAAAGAAGAGCCCAATCCATACTACTAAAGCTATCGTCATATGCACCATGGTCTTTTGAGAAAATTTTGCCCTCTTTAATTGACTCTAAGCTAAAAAAGAAAATTTTAATCAGTCCTCAAATTACTTACGTAGTCTTCAAATCAATAAACTATATGATAACCACGAATGACGATAAAATACACAAGTCCGCTATTCCTTCTTCTTCCTCTCTACCGT".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
        println!("Reverse Strand\n{:#?}", mappings);
        assert!(mappings[0].strand == Strand::Reverse);

        // Assert the Display impl for strand works
        println!("{}", mappings[0].strand);

        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2)
            .with_cigar()
            .with_index("yeast_ref.mmi", None)
            .unwrap();

        aligner
            .map(
                "ATGAGCAAAATATTCTAAAGTGGAAACGGCACTAAGGTGAACTAAGCAACTTAGTGCAAAAc".as_bytes(),
                true,
                false,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();

        let mappings = aligner.map("atCCTACACTGCATAAACTATTTTGcaccataaaaaaaagttatgtgtgGGTCTAAAATAATTTGCTGAGCAATTAATGATTTCTAAATGATGCTAAAGTGAACCATTGTAatgttatatgaaaaataaatacacaattaagATCAACACAGTGAAATAACATTGATTGGGTGATTTCAAATGGGGTCTATctgaataatgttttatttaacagtaatttttatttctatcaatttttagtaatatctacaaatattttgttttaggcTGCCAGAAGATCGGCGGTGCAAGGTCAGAGGTGAGATGTTAGGTGGTTCCACCAACTGCACGGAAGAGCTGCCCTCTGTCATTCAAAATTTGACAGGTACAAACAGactatattaaataagaaaaacaaactttttaaaggCTTGACCATTAGTGAATAGGTTATATGCTTATTATTTCCATTTAGCTTTTTGAGACTAGTATGATTAGACAAATCTGCTTAGttcattttcatataatattgaGGAACAAAATTTGTGAGATTTTGCTAAAATAACTTGCTTTGCTTGTTTATAGAGGCacagtaaatcttttttattattattataattttagattttttaatttttaaat".as_bytes(), true, false, None, None, Some(b"Sample Query")).unwrap();
        println!("{:#?}", mappings);
    }

    #[test]
    fn test_alignment_score() {
        let aligner = Aligner::builder()
            .preset(Preset::Splice)
            .with_index_threads(1);

        aligner.check_opts().expect("Opts are invalid");

        let aligner = aligner.with_index("test_data/genome.fa", None).unwrap();

        let output = aligner.map(
            b"GAAATACGGGTCTCTGGTTTGACATAAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGCCCAGACTTAAATCGCACATACTGCGTCGTGCAATGCCGGGCGCTAACGGCTCAATATCACGCTGCGTCACTATGGCTACCCCAAAGCGGGGGGGGCATCGACGGGCTGTTTGATTTGAGCTCCATTACCCTACAATTAGAACACTGGCAACATTTGGGCGTTGAGCGGTCTTCCGTGTCGCTCGATCCGCTGGAACTTGGCAACCACACTCTAAACTACATGTGGTATGGCTCATAAGATCATGCGGATCGTGGCACTGCTTTCGGCCACGTTAGAGCCGCTGTGCTCGAAGATTGGGACCTACCAAC",
            false, false, None, None, Some(b"Sample Query")).unwrap();

        println!("{:#?}", aligner.mapopt);
        println!("{:#?}", aligner.idxopt);
        println!("{:#?}", output);
    }

    #[test]
    fn test_aligner_config_and_mapping() {
        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2);
        let aligner = aligner
            .with_cigar()
            .with_index("test_data/test_data.fasta", Some("test.mmi"))
            .unwrap();

        aligner
            .map(
                "ATGAGCAAAATATTCTAAAGTGGAAACGGCACTAAGGTGAACTAAGCAACTTAGTGCAAAAc".as_bytes(),
                true,
                true,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();
        let mappings = aligner.map("atCCTACACTGCATAAACTATTTTGcaccataaaaaaaagGGACatgtgtgGGTCTAAAATAATTTGCTGAGCAATTAATGATTTCTAAATGATGCTAAAGTGAACCATTGTAatgttatatgaaaaataaatacacaattaagATCAACACAGTGAAATAACATTGATTGGGTGATTTCAAATGGGGTCTATctgaataatgttttatttaacagtaatttttatttctatcaatttttagtaatatctacaaatattttgttttaggcTGCCAGAAGATCGGCGGTGCAAGGTCAGAGGTGAGATGTTAGGTGGTTCCACCAACTGCACGGAAGAGCTGCCCTCTGTCATTCAAAATTTGACAGGTACAAACAGactatattaaataagaaaaacaaactttttaaaggCTTGACCATTAGTGAATAGGTTATATGCTTATTATTTCCATTTAGCTTTTTGAGACTAGTATGATTAGACAAATCTGCTTAGttcattttcatataatattgaGGAACAAAATTTGTGAGATTTTGCTAAAATAACTTGCTTTGCTTGTTTATAGAGGCacagtaaatcttttttattattattataattttagattttttaatttttaaat".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
        println!("{:#?}", mappings);
    }

    #[test]
    fn test_mappy_output() {
        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(1)
            .with_cigar()
            .with_index("test_data/MT-human.fa", None)
            .unwrap();

        let mut mappings = aligner.map(
    b"GTTTATGTAGCTTATTCTATCCAAAGCAATGCACTGAAAATGTCTCGACGGGCCCACACGCCCCATAAACAAATAGGTTTGGTCCTAGCCTTTCTATTAGCTCTTAGTGAGGTTACACATGCAAGCATCCCCGCCCCAGTGAGTCGCCCTCCAAGTCACTCTGACTAAGAGGAGCAAGCATCAAGCACGCAACAGCGCAG",
            true, true, None, None, Some(b"Sample Query")).unwrap();
        assert_eq!(mappings.len(), 1);

        let observed = mappings.pop().unwrap();

        assert_eq!(
            observed.target_name,
            Some(Arc::new(String::from("MT_human")))
        );
        assert_eq!(observed.target_start, 576);
        assert_eq!(observed.target_end, 768);
        assert_eq!(observed.query_start, 0);
        assert_eq!(observed.query_end, 191);
        assert_eq!(observed.mapq, 29);
        assert_eq!(observed.match_len, 168);
        assert_eq!(observed.block_len, 195);
        assert_eq!(observed.strand, Strand::Forward);
        assert_eq!(observed.is_primary, true);

        let align = observed.alignment.as_ref().unwrap();
        assert_eq!(align.nm, 27);
        assert_eq!(
            align.cigar,
            Some(vec![
                (14, 0),
                (2, 2),
                (4, 0),
                (3, 1),
                (37, 0),
                (1, 2),
                (85, 0),
                (1, 2),
                (48, 0)
            ])
        );
        assert_eq!(
            align.cigar_str,
            Some(String::from("14M2D4M3I37M1D85M1D48M9S"))
        );
        assert_eq!(
            align.md,
            Some(String::from(
                "14^CC1C11A12T1A7T4^T1A48A2A21T0T8^T2A5T2A4C0A0C2T0C2A4A17"
            ))
        );
        assert_eq!(
            align.cs,
            Some(String::from(
                ":14-cc:1*ct:2+atc:9*ag:12*tc:1*ac:7*tc:4-t:1*ag:48*ag:2*ag:21*tc*tc:8-t:2*ag:5*tc:2*ag:4*ct*ac*ct:2*tc*ct:2*ag:4*ag:17"
            ))
        );

        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(1)
            .with_cigar()
            .with_cigar_clipping()
            .with_index("test_data/MT-human.fa", None)
            .unwrap();

        let mut mappings = aligner.map(
            b"GTTTATGTAGCTTATTCTATCCAAAGCAATGCACTGAAAATGTCTCGACGGGCCCACACGCCCCATAAACAAATAGGTTTGGTCCTAGCCTTTCTATTAGCTCTTAGTGAGGTTACACATGCAAGCATCCCCGCCCCAGTGAGTCGCCCTCCAAGTCACTCTGACTAAGAGGAGCAAGCATCAAGCACGCAACAGCGCAG",
                    true, true, None, None, Some(b"Sample Query")).unwrap();
        assert_eq!(mappings.len(), 1);

        let observed = mappings.pop().unwrap();

        assert_eq!(
            observed.target_name,
            Some(Arc::new(String::from("MT_human")))
        );
        assert_eq!(observed.target_start, 576);
        assert_eq!(observed.target_end, 768);
        assert_eq!(observed.query_start, 0);
        assert_eq!(observed.query_end, 191);
        assert_eq!(observed.mapq, 29);
        assert_eq!(observed.match_len, 168);
        assert_eq!(observed.block_len, 195);
        assert_eq!(observed.strand, Strand::Forward);
        assert_eq!(observed.is_primary, true);

        let align = observed.alignment.as_ref().unwrap();
        assert_eq!(align.nm, 27);
        assert_eq!(
            align.cigar,
            Some(vec![
                (14, 0),
                (2, 2),
                (4, 0),
                (3, 1),
                (37, 0),
                (1, 2),
                (85, 0),
                (1, 2),
                (48, 0),
                (9, 4)
            ])
        );
        assert_eq!(
            align.cigar_str,
            Some(String::from("14M2D4M3I37M1D85M1D48M9S"))
        );

        let mut mappings = aligner.map(
                    b"TTTGGTCCTAGCCTTTCTATTAGCTCTTAGTGAGGTTACACATGCAAGCATCCCCGCCCCAGTGAGTCGCCCTCCAAGTCACTCTGACTAAGAGGAGCAAGCATCAAGCACGCAACAGCGCAG",
                            true, true, None, None, Some(b"Sample Query")).unwrap();
        assert_eq!(mappings.len(), 1);

        let _observed = mappings.pop().unwrap();

        assert_eq!(
            align.cigar,
            Some(vec![
                (14, 0),
                (2, 2),
                (4, 0),
                (3, 1),
                (37, 0),
                (1, 2),
                (85, 0),
                (1, 2),
                (48, 0),
                (9, 4)
            ])
        );
    }

    #[test]
    fn test_mappy_output_no_md() {
        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(1)
            .with_cigar()
            .with_index("test_data/MT-human.fa", None)
            .unwrap();
        let query =  b"GTTTATGTAGCTTATTCTATCCAAAGCAATGCACTGAAAATGTCTCGACGGGCCCACACGCCCCATAAACAAATAGGTTTGGTCCTAGCCTTTCTATTAGCTCTTAGTGAGGTTACACATGCAAGCATCCCCGCCCCAGTGAGTCGCCCTCCAAGTCACTCTGACTAAGAGGAGCAAGCATCAAGCACGCAACAGCGCAG";

        for (md, cs) in vec![(true, true), (false, false), (true, false), (false, true)].iter() {
            let mapping = aligner
                .map(query, *cs, *md, None, None, Some(b"Sample Query"))
                .unwrap()
                .pop()
                .unwrap();
            let align = mapping.alignment.as_ref().unwrap();
            assert_eq!(align.cigar_str.is_some(), true);
            assert_eq!(align.md.is_some(), *md);
            assert_eq!(align.cs.is_some(), *cs);
        }
    }

    #[test]
    fn test_strand_struct() {
        let strand = Strand::default();
        assert_eq!(strand, Strand::Forward);
        println!("{}", strand);
        let strand = Strand::Reverse;
        println!("{}", strand);
    }

    #[test]
    fn test_threadlocalbuffer() {
        let tlb = ThreadLocalBuffer::default();
        drop(tlb);
    }

    #[test]
    fn test_with_seq() {
        let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
        let query = "GGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTT";
        let aligner = Aligner::builder().short();
        let aligner = aligner.with_seq(seq.as_bytes()).unwrap();

        let alignments = aligner
            .map(
                query.as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();

        assert_eq!(alignments.len(), 2);

        println!("----- Trying with_seqs 1");

        let aligner = Aligner::builder().short();
        let aligner = aligner.with_seqs(&vec![seq.as_bytes().to_vec()]).unwrap();
        let alignments = aligner
            .map(
                query.as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();
        assert_eq!(alignments.len(), 2);

        println!("----- Trying with_seqs and ids 1");

        let id = "test";
        let aligner = Aligner::builder().short();
        let aligner = aligner
            .with_seqs_and_ids(
                &vec![seq.as_bytes().to_vec()],
                &vec![id.as_bytes().to_vec()],
            )
            .unwrap();
        let alignments = aligner
            .map(
                query.as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();
        assert_eq!(alignments.len(), 2);

        println!("----- Trying with_seq and id");

        let id = "test";
        let aligner = Aligner::builder().short();
        let aligner = aligner
            .with_seq_and_id(seq.as_bytes(), &id.as_bytes().to_vec())
            .unwrap();
        let alignments = aligner
            .map(
                query.as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();
        assert_eq!(alignments.len(), 2);

        println!("----- Trying with_seq and id");

        let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
        let query = "CAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";

        let aligner = Aligner::builder()
            .asm5()
            .with_cigar()
            .with_sam_out()
            .with_sam_hit_only();
        let aligner = aligner
            .with_seq_and_id(seq.as_bytes(), &id.as_bytes().to_vec())
            .unwrap();
        println!("mapping...");
        let alignments = aligner
            .map(
                query.as_bytes(),
                true,
                true,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();
        println!("Mapped");
        assert_eq!(alignments.len(), 1);
        println!(
            "{:#?}",
            alignments[0]
                .alignment
                .as_ref()
                .unwrap()
                .cigar
                .as_ref()
                .unwrap()
        );
        assert_eq!(
            alignments[0]
                .alignment
                .as_ref()
                .unwrap()
                .cigar_str
                .as_ref()
                .unwrap(),
            "282M"
        );
        //     // assert_eq!(alignments[0].alignment.unwrap().cigar.unwrap(), );

        //     // println!("----- Trying with_seqs 2");

        //     // let aligner = Aligner::builder().short();
        //     // let aligner = aligner.with_seqs(&vec![seq.as_bytes().to_vec(), seq.as_bytes().to_vec()]).unwrap();
        //     // let alignments = aligner.map(query.as_bytes(), false, false, None, None).unwrap();
        //     // assert_eq!(alignments.len(), 4);

        //     // for alignment in alignments {
        //     // println!("{:#?}", alignment);
        //     // }
    }

    #[test]
    fn test_junction_scoring() {
        // ENSG00000042753.12
        let seq = "GGGAGACAGGCAAGGGCTCAAAGACGGCAAGGCCAGGCAGGACCACAGGTTTATTGGGGACTCCACGCACAGACGCTTATGGCATCACACGACAACGGCACGGTTACTCGGGACACACACGGTGGCCTCTGCCCACAGCCAGGGCCCAGAGGCAGTGGGGTGCAGTCTCCTCCCTTGTGGCCCAGACCCAGCTGGGTCCCTTCCTCCTAGGCAGCTGAGGGAAGGACTGCTGGGTTGGCCACGGGCCTGGGAAGGGGAAGCGAGCAGGCGAGTCCAGGAGGGGCCGGGGCCGGGGTGGGGCTCGCCTGCCCTCACTCCAGGGACTGTAGCATCAGCAGCTGTTTCAGCACCTTCGTCTGGCTGGTCTCTCGGATTTCGCCAGCCAGGAACATCTCGTCCACGACCGTGTAAACCTGTGTGAGGGGAGACCCTGGGGTGAGACGAGGCCCCCCAGGATGCTGGCCCGGACCCTGGAAGCTGGGGCTCTGATGCCCCTCGAGCTGGGACACAGACCTCAGCAGAGGCTCCGGGTGGCTAGTGCACCACGGGTCCCCCCCGTCCCCCTCCTCTGGCTCCTTCAGCCTCCTCTTCACCAGGAGCCTACCTTGTAGAAGTTGAACACCAGGTCCAGTTCACAGACATTGTGGAAATATTCGTTTAAGACCTGGGAGAGGAAGGCAGAGATGGTAAGAGATGGGCAGGGAGAGAGCCACACACGCACAGAGATGGGAACAAGGTCATCAGAAAGAGACAGCAAAAAAAGAGACCAAGGGGGAGGCAGGGGAGAGAGAGAGACAGTGACAGGGAGAGAGGCAAGGAGACACACAGAGAGAAACAAAAGCAAAGATCGATGCAAAGAGATGGCAAAAGGTCAGGCGCGGTGGCTCACACCTGTAATCCCAGCACTTTGGGAGGCTAAGGCAGAAGGATGACTTGAAGTTAGGAGTGCAAGACCAGCCTGGCCAACATGGTGAAACCCCATCTCTACTAAAAATACAAAAATTAGCTGGGCATCGTGGCGTGTGCCTGTAATCCCAGCTACTCAAGAGGCTGAGGCAGGAGAATCACTTGAACCTGGGAGGTGGAGGCTGCAATGAGCCAAGATTGCACCACTGCACTCCAGCCTGGGCGACAGAACAAGACTCCGTCTCAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGAAAAAGAAAAAAAAGAAATGGAGAGGGAGAGTCCCAGTCACTGCAGAGGGAGGGCTCAGGAGGGACCAGGGAGGGTTCCCAGGCCTTGGGGGCCACTCCAGGGCTGCCCACCCGCCTCCCCACCTTACATCCCTCTCCCGCCGTACCTCCACGAAGTTGTGAATGGCCTCCAGGTAAGCCAGGTTGTTGTCATTGACATCCACACAGATGCAGAAGTAGAGGCCAGCATAGCGGCGGTAAATGATCTTAAAGTTCCGGAACTGCAGAACAGAGAGGCTGTCAGCAACGGAGATTGCCAGGACCTAACGTGCCAGACAGAGTGGGGCCAAAACATTCACTCCTTCACTCCATACGTGGTCCCGAGGCCCAGCTCTGTGCCTGGCCCTGTGCTGGGGTCACAGCAGTGACCGAGACAGCCCTAACCCTGCTCACAGTGCAGCGGGGGCAGGCATCCCCAGAATGGACAGGATGGGCTGTGAGAGCCCAAAGAGGGTGCCTAACCCCCTCTGATCCTGGAGAGTCAGGAGGGCTTCCTGGAGGAGGGGGTATCGGAGCTGAGAGCTGGAGGATAGGCAGGAATGTTCTAAGCAGATAGCAAAAGCCCGGGGCTGGGCGGGAGCCTGACGTGTTCAATAGTGTCCATGCAACCTTGCCACTCCCTTCTGTGACTTCCTGCTACCTCTGGAGACAGTCACATCCCTTAGCCTGGAATTAATTAACTCAATTATTTAACAAATACTTCCTGAGCAGCTACTCTATGCCAGGCCTGTGATAGGCAGTGTGGGGAGGAGGGTTACCACTGCCACCAAGACGATCCTGGGTCCTTGTCCTGCCCTCACTGAAATGCGATATACACAGTTAGCAATAGTAACACAGACACACACCGTGAACAAACATGCCAGGATCAGGGACATGGATTCTCATTTTATGCGCGTGTTAAGTAGACAAAGACAGATATATATGCATACTGGGTTGCAAAGTACAATGTCTTTGGCCAAAACCTGAAAATCCAGTTCAGTTCTATATGTTCTCAATGTCAGGTTCATTACAAAAAAAAAAAAAAAAAAAATCCCAGCACTCCAGGAGGCCGGGCGAGAGGATCACTTGAGCCCAGGAGTTTCAGGCCAGCTTGGGCAACATGGTGAAACCCCATCTATACAAAAGTACAAAACTTAGCTGGGTATGGTGGCGTGTACCTGTGGTCCCAGCTACTCGGGAGGCTGAGGTGGGGAGATGACCTGAGCCTGGGAGGTCAAAGCTGCTGTGAGCCACGTTCCCATCACTGCACTCCAGCCCGGGAGACCCTGTCTCAAAAAACACAAAAAAACAGCAGCTCTAGGTCTGGGGTGTGCCTGTGTCGCCACCTGCTGACAGCTATGAGTGCCAGCATCAGTCTTCGGCATTTTTTTTTTTTTTTTTTTTTGAGTCTGGCTCTGACGCCCAGGCTGGAGAGCAGTGGCTCGATCTCGGCTCACTGCAATCTCTGCCTCTTGGGTTCAAGCGATTCTCCTGCCTCAGTCTCCCGAGTAGCTGGGATTACAGGCGCGCCACCACACCCAGCTAATAGCTTTTTACTCATTTTTTCAGCTCAATCTCTTGTTCATTGCTATACTCCAAGCATGTTTTCAGGGTTTTTTTTGTTTTTTGGTTTTTTTGAGACAGTCTCACTCTGTCGCCCAGGCCAGAGTGCAGCGGCGCAATCTTGGCTCACTGCAACCTCTGCCTCCCAGGTTCAAGCAATTCTCCTGCCTCAGCCTCCTGAGTAGCTAGGAATACAGGTGTGCACCACCACGCCTGGCTAATTTTTGTATTTTTAGTAGAGATGAGGTTTTACCATGTTGGCCAGGCTGGTCTCCAACTCCTGACCTCAGATGATCCACCTGCCTCTGCCTCCCAAAGTGCTGGGATTACAGGCGTGAGCCCAGCCTGAGTATGTTTTCTGGTGTGATTACAGTAAGGCCTACATGTAATCCACGGGTAAATGAGCTGACACTGATCATCAGTGCTCCTCTCTTCTTGCAATCCAGCTTTTCTGGCTCTCACCCTCCTCCGCCTCCAGCTACTGCTCCATCTTTCCAGCCTCCCTTCCTGTTTTAAATTTCGAGGCTTCTCAGGGATCTGCTGCAGACCCCCATCTCTCCTCCAGCTACATTCTCTCAGAGAAGCACATTCTCTCATCCTGCAGGCTGGGGCAGGTGTCCCCTCTGAACATCCCACGCCACTGCCCCACCCCTGCCCTGCCCCAGCCTGACCACTGCTGGTCGCCACTGTCTGGGGATGGGTCTGTCTCCCCATTGGACTGAGCCTGTGGGGGAAGCTGGAGCTGTCACACTGTGTCCTCCATGTTGCCCTGCAAAGGGCTGGCACAGGTAATGGCCCCCAAAGACATCCACATCCTAATCCCACACCCTGTGAATATGGGACCTTATATGGCAAAAAGGGACTCTGCAGTCTCAACTTGTGACGAAGTTAAGAAGCATGAGATGGGGCGATAATCCTGGATGGTTCAGGCGGGACCAATGTCCTCACTGGGCTGCTAATAAAGGAAAGCGGGAGGCGGGACAGTGCGAGTCAGACAGAGATTGGACGACGGCCAGGTGCCGGGCCTCATGCCTATCATCCCAGCACTTTGGGAGGCCAAGGCTTGATCTCAGGAGTTCAAGAGCAGTTTAGGCAACATATCAAGACCCATCTCTATAAAACATACAAAAAATTAGTTGGGCGTGGTGGCAGGTCCCTGTAGTCCCAGCTACTCAGGAGTCTGAGGTGGGAGGATCCCTTGAGCCTGGGAGGCGGAGGCTGCAGTGAGCTATGATTGCACCACTGCACTCCGGCCTGGGGCACAGAGTGAGACCCTGTCTCAAATAATAATAATAATAACAAGAGGGATAGCCCGCATCTTCCAGTTGCCTACTCCTCCCTTTGATCTTCACTCACAAGTCCCCTCCAGGAAGCCCTCCAGACCCCCAGGCTGGGTCAGGTGCCCTCCCCCAGCCAGGCTCCTCGATCCCAGCCCTGCCTACTGTGGTTGTCACTGTCTAGGACAGGCTTGTGTTCCTTCCTGGGCTATGAGTCCCAAAGCAGCACAAGGCTGTCTCAGTGCTGCTGTGAACCTAGCACTGGGCTGATCACAGAGGAGGCACACAGAAACATTAGGTGAATACATGCACAAAGGAACAACTGAATGAATGGGCGGGCCTGGATCCTGGAGGCATGGAGGAACTCAAGACCTGAGGGTCAGCCTCCTCTGTAACATTCTCATTTGCCCAGCCGGACTTTCCGGCTGCCCCTGTATCATTGGGCCTCACTGACCCCTCCCGCGCCCCAGCCCTCAGCACAGGACCAGCCACTACATTAGCAGCAAGGTGCCCACACAGCCAGAACACCCGACACCCAGTTCCTCTCCCCCTTCCCACCTGCTGCTCTTGGTGACACCACCAGTGACCTCCTCCTGGAGAGGTAAGTCCAACCCTGTCCCCCTCCTGGCTGCTGGGACACCCAATCTCTTGGTTCTCCTCTCCCTCATCTCTCTGCCTGCAGACGTGGCCCTCTGGTGCTCTCCTCGGTCTATCCTCACTTCCCCGGTGAGCTCCTTGGCCTCGGTTTCCCTCTATGTGCAGAAATGCCGTTTCCATATCCTACCCAGCTCTCTCCAAGAACCCTGACTCGTCACTTCTTCCTGGACATCTCTGCTGGGATGTGGACGATACTCGGGGCCAGACCCCCAACACTTCCCTCCTGATCTTCACCTCCGCCTGCCCTTCCCATCTGCCCATATCAGATAATGACAGCTCCATCCTACCCGGGGGCTCAAGTGGGAAATCTAATTGTCGCCCTGGCTCTGCTTTCTCTCACATTCCACATCCAATTCTTGGCCATTCAGCTGGGTCCACCTCCAAAACGCTCCTGGCCACACCGACGGCTTCCATGCTGGTCCGGTCACCAGCATCTCCCACCTGGACCACTGCAGCCACCTCTCTCCGAGCAGTCTCCTCTATGCTCTTGCCCTATAGCGCAGCCTCAAGTCGGCCAATGGCCATGCTCTGCTCAAAACCCTCCATGGTTCAAGACCAGCCTGGACAACATAGCAATACCCCATCTCTACGAAAAATTTAAAAATTAGCCAGGCGTGGTCACTCATGGCTGTAATCCTAGGACTTTGGGAAGCTGCGGTGGGCGGATCGCTGGAGCTCAGGAGTTCGAGACCAGCCTGGGCAACATGGCGAAACCCTGTCTCTACCAAAAATACAAAAATTAGTACGGCATTGGTGGCACATGCCAATGGTCCCAGCTACTCGGGAGGCTGAGGTGGGAGAATTGCTTGACCCCGAGAGGCAGAGGTTGCAGTGAGCTGAGATCGCACCACTGCACTCTAGCCTAGGTAACAGAGTGAGAACCCACCTGAAAAGAAAAAAAAATTTAAAAATTAGATGGGCATGGAGGTGCATGCCTGTAGTCCCAGCCACTTGGCAAGCTGAGGTGGGAGGCTTGCTTGAGCCTGGGAGGGTACAGTGAGCTGTAATTGCACCACTGTACTCTAGCCTGGGAGACAGAGCAAGACCGTGTCTCTAAAAAACACCAAAAAACAAAAACAACCTTTCTTTTTTTTTTGAGAGGAGTCTTGTTCTGTTGCCCAGGCTGGAGTGCAATGGCACGATCTCGGCTCACTGCAACCTCTGCCTCCTGGATTCAAGCGATTCTCCTGCTTCAGCCTCCCGAGTAGCTGGGACTACAGGCACCCACGACCAGGCCCGGCTAATTTTTGTATTTTTAGTAGAGACGAGATTTCATCATGTTGGTCAGGCTGGTCTCCTGACCTTGTGATCCACCCACCTTGGCCTCCCAAAGTGCTGGGATTACAGGAGTGAGCCACCGCGCCCGGCCGGCAAAAACAACTTTTCTACGGCTCCCATCTGACTCATAGTAAGAGCCCAAGTCTTCCCTGCAGCCCTGCACCAAGAGATTACCCTGTTATCTTCTCTCTTTCATCTCCTCCCCTTCCTGGCTGCATTCCTGCTATCCCTGGAACGCCCTAAGTATTCTCCTGCCCCAGGGACTTTACACTTGCTGTTCCCTTGGCTTGAAATGTTCCTCTCAGACATTGGCTCCTTCCTCCCCGTCAAGTCCTTCAGGTCTTGGCTCCAATGTCACCTTTTCAGCAATGCCATCCTAACCAAACCATTTAAACCTGCATCCTGGCTGGGCGCAGTGGCTCATGCCTGTAATCCCAGCACTTTGGGAGGCCGAAGCAGGCAGATCACTTGAGCTCAAGAGTTTGAGACCAGCCTGGGCAACATGGTGAGACCCCATCGCTACCAAAAATACAAATATTAGCTGGGCTTGGTAGTGCACAAGCCTGTAGTCCCAGCTACTCAGGAGGCTGAGGTGGGAGGGTAGCTTGAACATAGGAAGTCAAGGCTGCTGTGAGCCATGTTCGTGCCACTGCACTCCCGCCTGGGTGACAGAATGAGCTCCTGTTTCAAAAAAAATTTTTTAAAGAGGGACAGGCATGGTGGCTCACGCCTGTAATCCCAGCACTTTGGGAGGCCAAGGCGGGTGGATCACTTGAGATCAGGAGGTCAAGACCAGTCTGACCAACATGGTGAAACCCCATCTCTACTAAAAATACAAAAATTAGCCAGGCGTGGTGGCGAGCACCTGTAATCCCAGCTCCTAGGGAGGCTGAGGCAGAGAATAGCTTGAACCCTGGAGGAGGTTGCAGTGAGCCAAGATTGCCCCCATTGCACTCCAGCCTGGGTGACTGAGACAGACTCCATTTCAGAAAAAAAAAAAAAAAAAGGCAAAAAAGGCCAGGCGCAGTGGCTCATGCCTGTAATCCCAGCACTTTGGAAGGCCGAGGAGGGTGGATTACAAGGTCAGGAGTTCGAGACCAGCCTGGCCAACATGGTGAAACCTCGTCTCTACCAAAAATACAAAATTAGCCTGGTGTTGTGGTGCACGCCTGTAATCCCAGCTATTTGGGAGGCTGAGGCAGGAGAATTGCTTGAACCTGGGAGGTGGAGGTTGCAGTGAGCTGAGATCATGCCACTGCACTCCAGCCTGGGTGACAGAGTAAGACTCTGTCTCAAAAAAAGTAATTAATTAATTAATTAATTAATTAATTCAATTCAATCAAATAAAAAATAAAGACAAGGTCTTGCTACATTGCCCAGGCTGGTCTTGAACACCTGGGCTCAAGCAATCCTCCCACCTTGGCTTCCCAAAGTGCTGGGATTACAGGCATGAGCCACTGCACCTGGCCTATTCTCTCTACTTTTCTGTGTTTGAACATTCTATAGGAAGTTTGAAAAACGCACCAGCAAAAAAAATTTGACCAAAAAAAAAAATTCTTTTTTAATCGTAGGCAAAAAAAATTAATAATAACAATGCAGAACAAAAAGGCAAAATATTAATACTGATTAAATCTGGGTAGTAGGTATACGAATCTCCAATCTACTTTTCTTTCATTGTACTAATCTGTATGTTTATGATTTTTCACAATGAAAAGTACAAGTAAAGGAAGGGAAGAAGCAAGCAAGCTCAAAGCAGGTTGAGTGAATGATATAGGATGGATAGAGGGTCCAAGGGGTTCTTGCAACTAGAAGCTGGGCAGGGAGTGGCGAAGTAGGGCACGAGGAAGCAGCGGGGTGCAGGAGGCATGGAGCGGGCGTCACCTCCACAAAGTTGGTGTGTTTGGCGTCTCGGACGGTGACCACGGCATGCACCTCCTCGATCAGCTTCTGTTTCTCATCATCATCAAACTGCATGTACCACTTGGCCAGGCGCGTCTTGCCTGCCCGGTTCTGGATGAGGATAAAGCGGATCTGGGGGCAGCAGGAGGAGAAGGAGGAAGTGAGAGAGGCAGAGAGGGCGGGTTGGGTGCTGCCCAACGGCCCCACCCATCCACCCAGAGGGGAGATAGGGCTCAGGGCCTCCCTGCTGCCTGACTTCCAGGGGTCTCGGCTGCTCCCCAGACCCAAGCCAGGCCGGCCTGGCCACACGCTTCATCCCGGGTCCCTCCAGCACCCCTGTGGAGTGACTTCCCTTCCACAAGTCTCCAACTCTGGAGTCAGCTCCAGGGCCCCGAGTCCCAGGCTCCTCCACCACCTCTTATCTCTCTCTGTTTTTTTTTTTTTTTTTTTTTGTAGAGACAGAGTCTTGCTACGTTGCTGGTCTCAAACTCCTGGACTCAAGCAATTCTCCCACCTCGGCCTCCCAAACTGCTGGGATCACAGGTGTGAGCCACTGTGCCCTGCTTTGTCTGCCCCTCTAGGCCTGTTTCCTCACCAGCAAAACAAGTCTCCCAAGAAGATTCATCTCACCAGGGTGGTTTTTTCATTATATTTTTTGTTATTTTTAAATTTTCAGACAGTGTCTCATTCTGTCACCCAGGCTGGAGTGCGATGGCGCAATCTCGGCTCACTGCAACCTCCGCCTCCCAGGTTCAAGCGATTCTCCTGCCTCAGTGTCCTGAGTAGCTGGGACTACAGGCACCCGCCACCACGCCTGGAAAATGTTTGTATTTTTGGTAGAGACGGGGTTTCACCATGTTGGCCAGGCTGGTCTCAAACTTCTGACCTCAGGTGATCCGCCCGCCTCTGCCTCCCAAAGTGTGGGGTTATAGGCATGGGCCACCACGCCCAGTCAGCCAGGGTTTTTTTAAAGGTTAAAAAAAAAGTTTATAAACTTAGTTCGGTGGCTGGTAGGTGGTAGATACTCAATAAACATTTGAGGTTCAGACTCCTTTGGGCTAAGTCTCAGCTCTGCCACTTCTTTTTTTCTCTTTGCTTCCTAACTTTTTATAAAATTGAAATGTAATTCACACACCATAAAATCCATCCACTTAAAATGTAGACAGTTAAATGGTTTTTGGTATATTCACTTAGTATATATTCACAGTTGTGTAAAAATCACCACTTTTTTTTTTTTTTTTGAGACAGGTTCTAGCTCTGTTGCCCAGGCTAGAGTGCAGTGGCGCGATCCCGCCTCACTGCAACCTCCACCTCCTGGGTGCAGGCCTCCCAGGTAGCTGGGACCGCAGGTGCACACCACCACGCCCAGCTAACTTTTTGTATTTTTAGTAGAGACAGTGTTTCACCATGTTGTCCAGGCTGGTCTCGAACTCCTGGCCTCAAGCGATCCACCCACCTCATCCCCCCAAAGTGCTGCATCAGGCGTGAGCCACTGCACCCCACCTAATATCACCACTATCTAATTTATTATCTAATACCAGAACATTTTCATCATCTCCCCCCAAAAAACTCCCCACCCATTAGCAGTCACTCTCCGTTCCCCCAGGGCCTGGCAGCCACGAGTCTCCTTTCTGTCTCTGTGGATGTGTCTGTTCTGGACATTGTATATGAGTGGAATCATGCACTCTGTGGCTTTCGTGTCTGGCTTCCTTCACTCAGCGTGACGTTTTCCAGGCTCATCTGTGGGGTGGCAAGCGGCAGAGCTGCAATTCCTTTCCGTGGCTGAGTGATCCTCTGTTGGATGGATAAGGCCGTACTTCGGGCATTTGGGTTGTTTCTGCTTTAGGGCTATTATGAATAATACTGCTATGAACATTCGTGTATGAGTTTTCGTGTGGATGGATCATTTCTCTTGGGTATATACCTAAGAGTGAGATTGCTGGCTCATAGGGTAACGCTATGTTTAACTTTCTCAGGAATTGCTAAACCGTTTTCCAAAGCGTCTGCCACTTTCATGACTTAACTTTGGTGCCACTTTGGTGCCTGAGTTTACTGTGAGGAGTAAAGGATATTCCTGGCCGGTCACGGTGGCTCACACCTGTAATCCCAGCACTTTGGGAGGCCAAGGCGGGCAGATCACTTGAGGTCAGGAGTTCGAGACCAGTTTGGCCAACATGGTGAAACCCCATCTGTACTAAAAATACAAAAATTAGCCAGGTGTGATGGGGGGCAGGGGGGAGGCGGCTATAATCCCAGCTACTCGGGAGGCTGAGGCATGAGAATCACTTGAACCCAGGAGGCAGAGGTTGCAGTGAGCTGACATTGCACCATTGCTCTCCAGCCTGGGTGACAGAGCGAGACGCTGTCTCAAAAAAACAAAAACAAAAACATAGAGGTATAGTACAGTGACTGGCACGTACTAAGTGTTACATAAGAACCAGCTCTTAGCAGCATTATATTTTCCATTATGTGATGTTCAGAAAACCATGGCCAAAGCTTACTCCATTAGAAGTTCTGGGCCCATTTCTGTCTGAGACAGGCATGCTCATGCCCATTTTATAGATGAGGAAACTGAGATTTAGGGAAAGGAAGTCTCACAGCCAGAAAGCAGAGGTGGGAGTCCAAAGGCTTTGCTCTGAACCACCTGACCCCAACTTGTGGGAGCCTTCTCTCTTAGGAAAGAGAGCCCTCTAGTCCTGTCTTTTTCTTTTCTTTTTTGTTTTCAGACAGTCTCACTCTGTTGCCTAGGCTGGAGTGCAGTGGCACTGTCTCGGTTCACTGCAACCTCTACCTCCAGGTTCAAGCGATTCTCCTGCCTCAGCCTCCTAAGTACCTGGGATTACAGGCGCATGCCACCATGCCCGGCTAATTTTTGTATTTTTAGTAGAGACAGGGTTTCACCATGTTGGTCAGGCTGGTCTCGAACTCCTGACCTTGTGATCTGCCTGCCTCAGCCTCCCAAAGTGCTGGGGTTACAGGCGCGAGTCACTGTGCCCAGCCTTTTTTTTTTTTTTTTTTTTGAGACAGAGTCTCACTCTGTCGCCTAGGCCAGTGGCACAATCTCGGCTCACTGCAGCCTCTGCCTCCCGGGTTCAAGCGATTCTCCTGCCTCAGCCTCCCAAGTAGCTGGGATTACAGGCATCCACCACCACACCTGGCTTATTTTTGTATTTTTAGTAGAGACGGGGTCTCACCACGTTGGCCAGGCTGGTCTCGAACTCCTGACCTTATGTGATCCGCCTGCCTCAGCCTCCCAAAGTGCTGGGATTACAGGCATGAGTCACCGCCCCAGCCTGAGAGTCTTTCCTAAGCCTCATTCTCCACACACTCTATCCTCCCTCCCCCAACCAACCAAACTGGACCGGAACAGGATGTTACACTCTCCTTCCAAGAACTCCTGCTGAGCCAGGGAAAACCACTTCCTAGCAGGGCAGGGGTCCCAGGCAGGAAGGGACGTTGGGGGGCCTACACCAGGCAGCCACAGCCTGGGAGCAGAGCCGGAGACATATTCACACAGTTAGGAGAACTAGAAGCAAACCCACAGAGTTGACAAAAAGCTCTGCCTCTTACTCCCAAATTCCCTTGTATACCTCTCCCTGTTTTCTTCCCATTTGGAATCTCAAGTCCCCTCCGTCCTGAATCTCTGCATCCTCACGCCTCTGCTGTCCCCTCTAGATGCCCCCCAACCTTCCCTGTCTCCAATCCCCCTCCTCTGGAGTTGTTCCCTTTTGACCTCCACTTTCTCAGTCTCAGTTCCCCTCCTTGGGGCCTCAATGGGCTTTTTACTGGTGACATCCCTTGTCCCACTCCCAAGCTTCCTCCCTTCTTCAAAAGGTCACATTTTGCCTCCAAGGACCCTGTTCCTGGAGGAGACTCCGCCCCCGCTCTGACACTCTTTCTCATTCGTTTTCTTTCCCAACTTCCAGCCCAAAGAACCCTTTCCTAGATCCAAAGCCCCAACCTGGGGGATAGGGGCACTCCTAAATCCAATGTTGAGTCTTTGCCCCAACAAAGATTCCTCCCCAGGTCTCACACTTTCAAGCCTACTCCCCACCCCACAAGATCTCTCCTCTTGGAAAGACCTATTCAGTTGCTCCCCATTCTTTCCGAGTGTCCATCTTACCTCCAGGTCCTCTCTCCTTTTACAGCGCTTTCCCAGACCCTTAAGTTTCATTCCCTACAGTTGCCTCCTTTTGGTTTCCTGTGCCAGGTCTCTTCTTGAGATCATCCCTCTCTCACAGGCAGTCTCCTGCCCCTGTCTCAACATCCCCTCTCCACCTGCAATCAGATCCTTGTCTCTAGAGACTCCTCCCATCAAGAAAACCTCCCTCCCCTTTTCCAAGGTCTCGCGTTCTCGTCCCAGCGCTCCCGCCACACTTCCAGAGCGCCTTCCTCTTTCGGACGCCCTCTCCCCACCTCCAATGCCTTCTCGCTACCCACAAGACTGTGTCTCACGAGTTTCCTCTCGTTTCAAAGCTTCCAACCATCTACAACGTTTCCCACCTGCACAGCCCTTTCCCCTTGGAACACCACCCCCAATGCCCGTCGCCGCGAGACCCCTGGTAACCCCCGCGCGCAGAATCACCGCCCTGTGCCCTCCTTCCCGGCCCTGGATCGGTCCCAATCCCCAGAGCCCGCGCCTGACCCAGACCATCCGCGGCAGAGAAGGGACTTGTCAGCGCCCGATCCAGCCTCGGCTATTTACGCGTGGGCCCCCCCTCCGCCAGTCCCCGGCGTAGCGCTCCCCCGTTACCATGGCGACCCCCGTCCAGACCCCAGCGGCCCCGGTCCCGCGGCGACTGGGCAGCTCCGGCTCAGGGTGCAGTTGTAGGGCCC";

        // ENST00000352203.8 -> ENSG00000042753.12
        let query = "CGGGGGTCGCCATGATCCGCTTTATCCTCATCCAGAACCGGGCAGGCAAGACGCGCCTGGCCAAGTGGTACATGCAGTTTGATGATGATGAGAAACAGAAGCTGATCGAGGAGGTGCATGCCGTGGTCACCGTCCGAGACGCCAAACACACCAACTTTGTGGAGGTCCTGGCAATCTCCGTTGCTGACAGCCTCTCTGTTCTGCAGTTCCGGAACTTTAAGATCATTTACCGCCGCTATGCTGGCCTCTACTTCTGCATCTGTGTGGATGTCAATGACAACAACCTGGCTTACCTGGAGGCCATTCACAACTTCGTGGAGGTCTTAAACGAATATTTCCACAATGTCTGTGAACTGGACCTGGTGTTCAACTTCTACAAGGTTTACACGGTCGTGGACGAGATGTTCCTGGCTGGCGAAATCCGAGAGACCAGCCAGACGAAGGTGCTGAAACAGCTGCTGATGCTACAGTCCCTGGAGTGAGGGCAGGCGAGCCCCACCCCGGCCCCGGCCCCTCCTGGACTCGCCTGCTCGCTTCCCCTTCCCAGGCCCGTGGCCAACCCAGCAGTCCTTCCCTCAGCTGCCTAGGAGGAAGGGACCCAGCTGGGTCTGGGCCACAAGGGAGGAGACTGCACCCCACTGCCTCTGGGCCCTGGCTGTGGGCAGAGGCCACCGTGTGTGTCCCGAGTAACCGTGCCGTTGTCGTGTGATGCCATAAGCGTCTGTGCGTGGAGTCCCCAATAAACCTGTGGTCCTGCCTGGCCTTGCCGTCTTTGAGCCC";
        let aligner = Aligner::builder().splice_sr().with_cigar();

        let aligner = aligner.with_seq(seq.as_bytes()).unwrap();

        let alignments = aligner
            .map(
                query.as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();

        let mut junctions = Vec::new();
        for mapping in alignments.iter() {
            println!("Scoring...");
            aligner.score_junctions(mapping, &mut junctions);
        }

        for junction in junctions.iter() {
            println!("Junction: {:?}", junction);
        }
        assert!(false);

        assert_eq!(alignments.len(), 2);
    }

    #[test]
    fn test_aligner_struct() {
        let aligner = Aligner::default();
        drop(aligner);

        let _aligner = Aligner::builder().map_ont();
        let _aligner = Aligner::builder().ava_ont();
        let _aligner = Aligner::builder().map10k();
        let _aligner = Aligner::builder().ava_pb();
        let _aligner = Aligner::builder().map_hifi();
        let _aligner = Aligner::builder().asm();
        let _aligner = Aligner::builder().asm5();
        let _aligner = Aligner::builder().asm10();
        let _aligner = Aligner::builder().asm20();
        let _aligner = Aligner::builder().short();
        let _aligner = Aligner::builder().sr();
        let _aligner = Aligner::builder().splice();
        let _aligner = Aligner::builder().cdna();
        let _aligner = Aligner::builder().splice_sr();

        #[cfg(feature = "map-file")]
        {
            let aligner = Aligner::builder()
                .with_index("test_data/MT-human.fa", None)
                .unwrap();
            assert_eq!(
                aligner.map_file("test_data/file-does-not-exist", false, false),
                Err("File does not exist")
            );

            if let Err("Index File is empty") =
                Aligner::builder().with_index("test_data/empty.fa", None)
            {
                println!("File is empty - Success");
            } else {
                panic!("File is empty error not thrown");
            }

            if let Err("Invalid Path for Index") =
                Aligner::builder().with_index("\0invalid_\0path\0", None)
            {
                println!("Invalid Path - Success");
            } else {
                panic!("Invalid Path error not thrown");
            }

            if let Err("Invalid Output for Index") =
                Aligner::builder().with_index("test_data/MT-human.fa", Some("test\0test"))
            {
                println!("Invalid output - Success");
            } else {
                panic!("Invalid output error not thrown");
            }
        }
    }

    #[test]
    fn test_send() {
        let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
        let query = "GGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTT";
        let aligner = Aligner::builder().short();
        let aligner = std::sync::Arc::new(aligner.with_seq(seq.as_bytes()).unwrap());
        let alignments = aligner
            .map(
                query.as_bytes(),
                false,
                false,
                None,
                None,
                Some(b"Sample Query"),
            )
            .unwrap();
        assert_eq!(alignments.len(), 2);

        let (send, recv) = std::sync::mpsc::channel::<Vec<Mapping>>();
        let receiver = std::thread::spawn(move || -> Vec<Vec<Mapping>> {
            let mut ret = Vec::new();
            while let Ok(batch) = recv.recv() {
                ret.push(batch);
            }
            ret
        });
        let new_send = send.clone();
        let new_aligner = aligner.clone();
        let sender = std::thread::spawn(move || {
            new_send
                .send(
                    new_aligner
                        .map(
                            query.as_bytes(),
                            false,
                            false,
                            None,
                            None,
                            Some(b"Sample Query"),
                        )
                        .expect("Failed to map"),
                )
                .expect("Failed to send")
        });
        let new_sender = std::thread::spawn(move || {
            send.send(
                aligner
                    .map(
                        query.as_bytes(),
                        false,
                        false,
                        None,
                        None,
                        Some(b"Sample Query"),
                    )
                    .expect("Failed to map"),
            )
            .expect("Failed to send")
        });
        drop(sender);
        drop(new_sender);
        let received = receiver.join().unwrap();
        assert_eq!(received[0], alignments);
        assert_eq!(received[1], alignments);
        assert_eq!(received.len(), 2);
    }

    #[test]
    fn test_struct_config() {
        let mut sr = Aligner::builder().sr();
        sr.mapopt.best_n = 1;
        sr.idxopt.k = 7;

        let _aligner = Aligner {
            mapopt: MapOpt {
                best_n: 1,
                ..Aligner::builder().sr().mapopt
            },
            idxopt: IdxOpt {
                k: 7,
                ..Aligner::builder().sr().idxopt
            },
            ..sr
        };
    }

    #[test]
    fn double_free_index_test() {
        // Create a new aligner
        println!("Creating aligner");
        let aligner = Aligner::builder()
            .map_ont()
            .with_index("yeast_ref.mmi", None)
            .unwrap();
        println!("Aligner created");

        // Perform a test mapping to ensure the index is loaded and all
        let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
        assert!(mappings.len() > 0);

        println!("Going into threads");

        // Spawn two threads using thread scoped aligners, clone aligner
        std::thread::scope(|s| {
            let aligner_ = aligner.clone();

            // Confirm that aligner_ idx points to the same memory as aligner idx arc
            assert_eq!(
                Arc::as_ptr(aligner.idx.as_ref().unwrap()),
                Arc::as_ptr(aligner_.idx.as_ref().unwrap())
            );

            // Confirm we have a strong count of 2
            assert_eq!(Arc::strong_count(&aligner.idx.as_ref().unwrap()), 2);

            let jh0 = s.spawn(move || {
                let mappings = aligner_.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
                assert!(mappings.len() > 0);
                // Sleep 100ms
                std::thread::sleep(std::time::Duration::from_millis(100));
            });

            let aligner_ = aligner.clone();
            let jh1 = s.spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(200));
                let mappings = aligner_.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
                assert!(mappings.len() > 0);
                // Sleep 100ms
                std::thread::sleep(std::time::Duration::from_millis(100));
            });

            jh0.join().unwrap();
            jh1.join().unwrap();
        });

        println!("Past the first one");

        // Create a new aligner
        let aligner = Aligner::builder()
            .map_ont()
            .with_index("yeast_ref.mmi", None)
            .unwrap();

        // Perform a test mapping to ensure the index is loaded and all
        let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
        assert!(mappings.len() > 0);

        // Spawn two threads using thread scoped aligners, clone aligner
        std::thread::scope(|s| {
            let aligner0 = aligner.clone();
            let aligner1 = aligner.clone();

            // Force drop logic
            drop(aligner);

            let jh0 = s.spawn(move || {
                println!("First thread");
                let mappings = aligner0.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
                assert!(mappings.len() > 0);
                // Sleep 100ms
                std::thread::sleep(std::time::Duration::from_millis(100));
                println!("First thread done");
            });

            // Join, to force drop logic from external thread
            jh0.join().unwrap();

            let jh1 = s.spawn(move || {
                println!("Second thread");
                std::thread::sleep(std::time::Duration::from_millis(200));
                let mappings = aligner1.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
                assert!(mappings.len() > 0);
                // Sleep 100ms
                std::thread::sleep(std::time::Duration::from_millis(100));
                assert!(Arc::strong_count(&aligner1.idx.as_ref().unwrap()) == 1);
                println!("Second thread done");
            });

            jh1.join().unwrap();
        });

        println!("Moving to the third test");

        // Finally with no test mapping
        // Create a new aligner
        let aligner = Aligner::builder()
            .map_ont()
            .with_index("yeast_ref.mmi", None)
            .unwrap();

        // Spawn two threads using thread scoped aligners, clone aligner
        std::thread::scope(|s| {
            let aligner0 = aligner.clone();
            let aligner1 = aligner.clone();

            // Force drop logic
            drop(aligner);

            let jh0 = s.spawn(move || {
                println!("First thread - No mapping");
                let mappings = aligner0.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
                assert!(mappings.len() > 0);
                // Sleep 100ms
                std::thread::sleep(std::time::Duration::from_millis(100));
            });

            // Join, to force drop logic from external thread
            jh0.join().unwrap();

            let jh1 = s.spawn(move || {
                println!("Second thread - No mapping");
                std::thread::sleep(std::time::Duration::from_millis(200));
                let mappings = aligner1.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None, Some(b"Sample Query")).unwrap();
                assert!(mappings.len() > 0);
                // Sleep 100ms
                std::thread::sleep(std::time::Duration::from_millis(100));
            });

            jh1.join().unwrap();
        });
    }

    // Test aligner cloning for flag permanence
    #[test]
    fn aligner_cloning_flags() {
        let aligner = Aligner::builder()
            .map_ont()
            .with_cigar()
            .with_index("yeast_ref.mmi", None)
            .unwrap();
        // Confirm with_cigar is set
        // self.mapopt.flag |= MM_F_CIGAR as i64;
        assert_eq!(aligner.mapopt.flag & MM_F_CIGAR as i64, MM_F_CIGAR as i64);

        // Clone aligner
        let aligner_clone = aligner.clone();
        assert_eq!(
            aligner_clone.mapopt.flag & MM_F_CIGAR as i64,
            MM_F_CIGAR as i64
        );
    }

    #[test]
    fn mapopt_defaults() {
        let aligner = Aligner::builder().map_ont();
        println!("{:#?}", aligner.mapopt);

        let aligner = Aligner::builder();
        println!("{:#?}", aligner.mapopt);
    }

    #[test]
    #[allow(unused_variables)]
    fn build_aligner_memory_leak() {
        for _ in 0..100000 {
            let aligner = Aligner::builder().map_ont();
            let aligner = aligner
                .with_index_threads(1)
                .with_cigar()
                .with_sam_out()
                .with_sam_hit_only()
                .with_seq_and_id(b"ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA",  b"ref")
                .unwrap();
        }
    }
}

//! API providing a rusty interface to minimap2 or mm2-fast libraries.
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
//! * mm2-fast - Uses the mm2-fast library instead of standard minimap2
//! * htslib - Provides an interface to minimap2 that returns rust_htslib::Records
//! * simde - Enables SIMD Everywhere library in minimap2
//! * sse - Enables the use of SSE instructions
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
//! .map(seq, false, false, None, None)
//! .expect("Unable to align");
//! ```
//!
//! ## Mapping a file to an individual target sequence
//! ```no_run
//! use minimap2::{Aligner, Preset};
//! # let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
//! let aligner = Aligner::builder().map_ont().with_seq(seq.as_bytes()).expect("Unable to build index");
//! let query = b"CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCG";
//! let hits = aligner.map(query, false, false, None, None);
//! assert_eq!(hits.unwrap().len(), 1);
//! ```

use std::cell::RefCell;

use std::mem::MaybeUninit;
use std::num::NonZeroI32;
use std::path::Path;

use std::os::unix::ffi::OsStrExt;

use libc::c_void;
use minimap2_sys::*;

#[cfg(feature = "map-file")]
use simdutf8::basic::from_utf8;

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
static LRHQAE: &str = "lr:hqae\0";
static LRHQ: &str = "lr:hq\0";
static SPLICE: &str = "splice\0";
static SPLICEHQ: &str = "splice:hq\0";
static ASM: &str = "asm\0";
static ASM5: &str = "asm5\0";
static ASM10: &str = "asm10\0";
static ASM20: &str = "asm20\0";
static SR: &str = "sr\0";
static MAP_PB: &str = "map-pb\0";
static MAP_HIFI: &str = "map-hifi\0";
static MAP_ONT: &str = "map-ont\0";
static AVA_PB: &str = "ava-pb\0";
static AVA_ONT: &str = "ava-ont\0";

// These aren't listed in the command anymore, but are still available
static SHORT: &str = "short\0";
static MAP10K: &str = "map10k\0";
static CDNA: &str = "cdna\0";

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
impl From<Preset> for *const i8 {
    fn from(preset: Preset) -> Self {
        match preset {
            Preset::LrHqae => LRHQAE.as_bytes().as_ptr() as *const i8,
            Preset::LrHq => LRHQ.as_bytes().as_ptr() as *const i8,
            Preset::Splice => SPLICE.as_bytes().as_ptr() as *const i8,
            Preset::SpliceHq => SPLICEHQ.as_bytes().as_ptr() as *const i8,
            Preset::Asm => ASM.as_bytes().as_ptr() as *const i8,
            Preset::Asm5 => ASM5.as_bytes().as_ptr() as *const i8,
            Preset::Asm10 => ASM10.as_bytes().as_ptr() as *const i8,
            Preset::Asm20 => ASM20.as_bytes().as_ptr() as *const i8,
            Preset::Sr => SR.as_bytes().as_ptr() as *const i8,
            Preset::MapPb => MAP_PB.as_bytes().as_ptr() as *const i8,
            Preset::MapHifi => MAP_HIFI.as_bytes().as_ptr() as *const i8,
            Preset::MapOnt => MAP_ONT.as_bytes().as_ptr() as *const i8,
            Preset::AvaPb => AVA_PB.as_bytes().as_ptr() as *const i8,
            Preset::AvaOnt => AVA_ONT.as_bytes().as_ptr() as *const i8,
            Preset::Short => SHORT.as_bytes().as_ptr() as *const i8,
            Preset::Map10k => MAP10K.as_bytes().as_ptr() as *const i8,
            Preset::Cdna => CDNA.as_bytes().as_ptr() as *const i8,
        }
    }
}

// Convert to c string for input into minimap2
impl From<Preset> for *const u8 {
    fn from(preset: Preset) -> Self {
        match preset {
            Preset::LrHqae => LRHQAE.as_bytes().as_ptr(),
            Preset::LrHq => LRHQ.as_bytes().as_ptr(),
            Preset::Splice => SPLICE.as_bytes().as_ptr(),
            Preset::SpliceHq => SPLICEHQ.as_bytes().as_ptr(),
            Preset::Asm => ASM.as_bytes().as_ptr(),
            Preset::Asm5 => ASM5.as_bytes().as_ptr(),
            Preset::Asm10 => ASM10.as_bytes().as_ptr(),
            Preset::Asm20 => ASM20.as_bytes().as_ptr(),
            Preset::Sr => SR.as_bytes().as_ptr(),
            Preset::MapPb => MAP_PB.as_bytes().as_ptr(),
            Preset::MapHifi => MAP_HIFI.as_bytes().as_ptr(),
            Preset::MapOnt => MAP_ONT.as_bytes().as_ptr(),
            Preset::AvaPb => AVA_PB.as_bytes().as_ptr(),
            Preset::AvaOnt => AVA_ONT.as_bytes().as_ptr(),
            Preset::Short => SHORT.as_bytes().as_ptr(),
            Preset::Map10k => MAP10K.as_bytes().as_ptr(),
            Preset::Cdna => CDNA.as_bytes().as_ptr(),
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
    pub alignment_score: Option<u32>,
}

/// Mapping result
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mapping {
    // The query sequence name.
    pub query_name: Option<String>,
    pub query_len: Option<NonZeroI32>,
    pub query_start: i32,
    pub query_end: i32,
    pub strand: Strand,
    pub target_name: Option<String>,
    pub target_len: i32,
    pub target_start: i32,
    pub target_end: i32,
    pub match_len: i32,
    pub block_len: i32,
    pub mapq: u32,
    pub is_primary: bool,
    pub is_supplementary: bool,
    pub alignment: Option<Alignment>,
}

// Thread local buffer (memory management) for minimap2
thread_local! {
    static BUF: RefCell<ThreadLocalBuffer> = RefCell::new(ThreadLocalBuffer::new());
}

/// ThreadLocalBuffer for minimap2 memory management
struct ThreadLocalBuffer {
    buf: *mut mm_tbuf_t,
    max_uses: usize,
    uses: usize,
}

impl ThreadLocalBuffer {
    pub fn new() -> Self {
        let buf = unsafe { mm_tbuf_init() };
        Self {
            buf,
            max_uses: 15,
            uses: 0,
        }
    }
    /// Return the buffer, checking how many times it has been borrowed.
    /// Free the memory of the old buffer and reinitialise a new one If
    /// num_uses exceeds max_uses.
    pub fn get_buf(&mut self) -> *mut mm_tbuf_t {
        if self.uses > self.max_uses {
            // println!("renewing threadbuffer");
            self.free_buffer();
            let buf = unsafe { mm_tbuf_init() };
            self.buf = buf;
            self.uses = 0;
        }
        self.uses += 1;
        self.buf
    }

    fn free_buffer(&mut self) {
        unsafe { mm_tbuf_destroy(self.buf) };
    }
}

/// Handle destruction of thread local buffer properly.
impl Drop for ThreadLocalBuffer {
    fn drop(&mut self) {
        unsafe { mm_tbuf_destroy(self.buf) };
    }
}

impl Default for ThreadLocalBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// @property
// def buffer(self):
//     if self.uses > self.max_uses:
//         self._b = ThreadBuffer()
//         self.uses = 0
//     self.uses += 1
//     return self._b

/// Aligner struct, mimicking minimap2's python interface
///
/// ```
/// # use minimap2::*;
/// Aligner::builder();
/// ```

#[derive(Clone)]
pub struct Aligner {
    /// Index options passed to minimap2 (mm_idxopt_t)
    pub idxopt: IdxOpt,

    /// Mapping options passed to minimap2 (mm_mapopt_t)
    pub mapopt: MapOpt,

    /// Number of threads to create the index with
    pub threads: usize,

    /// Index created by minimap2
    pub idx: Option<*mut mm_idx_t>,

    /// Index reader created by minimap2
    pub idx_reader: Option<mm_idx_reader_t>,

    /// Whether to add soft clipping to CIGAR result
    pub cigar_clipping: bool,
}

/// Create a default aligner
impl Default for Aligner {
    fn default() -> Self {
        Self {
            idxopt: Default::default(),
            mapopt: Default::default(),
            threads: 1,
            idx: None,
            idx_reader: None,
            cigar_clipping: false,
        }
    }
}

impl Aligner {
    /// Create a new aligner with default options
    pub fn builder() -> Self {
        let mut aligner = Aligner {
            mapopt: MapOpt {
                seed: 11,
                best_n: 1,
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

impl Aligner {
    /// Ergonomic function for Aligner. Sets the minimap2 preset to lr:hq.
    pub fn lrhq(self) -> Self {
        self.preset(Preset::LrHq)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to splice
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().splice();
    /// ```
    pub fn splice(self) -> Self {
        self.preset(Preset::Splice)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to splice:hq
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().splice_hq();
    /// ```
    pub fn splice_hq(self) -> Self {
        self.preset(Preset::SpliceHq)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to Asm
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().asm();
    /// ```
    pub fn asm(self) -> Self {
        self.preset(Preset::Asm)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to Asm5
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().asm5();
    /// ```
    pub fn asm5(self) -> Self {
        self.preset(Preset::Asm5)
    }
    /// Ergonomic function for Aligner. Sets the minimap2 preset to Asm10
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().asm10();
    /// ```
    pub fn asm10(self) -> Self {
        self.preset(Preset::Asm10)
    }
    /// Ergonomic function for Aligner. Sets the minimap2 preset to Asm20
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().asm20();
    /// ```
    pub fn asm20(self) -> Self {
        self.preset(Preset::Asm20)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to sr
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().sr();
    /// ```
    pub fn sr(self) -> Self {
        self.preset(Preset::Sr)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to MapPb
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map_pb();
    /// ```
    pub fn map_pb(self) -> Self {
        self.preset(Preset::MapPb)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to MapHifi
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map_hifi();
    /// ```
    pub fn map_hifi(self) -> Self {
        self.preset(Preset::MapHifi)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to MapOnt.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map_ont();
    /// ```
    pub fn map_ont(self) -> Self {
        self.preset(Preset::MapOnt)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to AvaPb
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().ava_pb();
    /// ```
    pub fn ava_pb(self) -> Self {
        self.preset(Preset::AvaPb)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to AvaOnt.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().ava_ont();
    /// ```
    pub fn ava_ont(self) -> Self {
        self.preset(Preset::AvaOnt)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to Short
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().short();
    /// ```
    pub fn short(self) -> Self {
        self.preset(Preset::Short)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to Map10k
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map10k();
    /// ```
    pub fn map10k(self) -> Self {
        self.preset(Preset::Map10k)
    }

    /// Ergonomic function for Aligner. Sets the minimap2 preset to cdna
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().cdna();
    /// ```
    pub fn cdna(self) -> Self {
        self.preset(Preset::Cdna)
    }

    /// Create an aligner using a preset.
    pub fn preset(self, preset: Preset) -> Self {
        let mut idxopt = IdxOpt::default();
        let mut mapopt = MapOpt::default();

        unsafe {
            // Set preset
            mm_set_opt(preset.into(), &mut idxopt, &mut mapopt)
        };

        Self {
            idxopt,
            mapopt,
            ..Default::default()
        }
    }

    /// Set Alignment mode / cigar mode in minimap2
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().map_ont().with_cigar();
    /// ```
    ///
    pub fn with_cigar(mut self) -> Self {
        // Make sure MM_F_CIGAR flag isn't already set
        assert!((self.mapopt.flag & MM_F_CIGAR as i64) == 0);

        self.mapopt.flag |= MM_F_CIGAR as i64;
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
    pub fn with_index<P>(mut self, path: P, output: Option<&str>) -> Result<Self, &'static str>
    where
        P: AsRef<Path>,
    {
        match self.set_index(path, output) {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        }
    }

    /// Set the index (in-place, without builder pattern)
    pub fn set_index<P>(&mut self, path: P, output: Option<&str>) -> Result<(), &'static str>
    where
        P: AsRef<Path>,
    {
        let path_str = match std::ffi::CString::new(path.as_ref().as_os_str().as_bytes()) {
            Ok(path) => {
                // println!("{:#?}", path);
                path
            }
            Err(_) => {
                println!("Got error");
                return Err("Invalid Path");
            }
        };

        // Confirm file exists
        if !path.as_ref().exists() {
            return Err("File does not exist");
        }

        // Confirm file is not empty
        if path.as_ref().metadata().unwrap().len() == 0 {
            return Err("File is empty");
        }

        let output = match output {
            Some(output) => match std::ffi::CString::new(output) {
                Ok(output) => output,
                Err(_) => return Err("Invalid Output"),
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

        self.idx = Some(unsafe { idx.assume_init() });

        Ok(())
    }

    /// Use a single sequence as the index. Sets the sequence ID to "N/A".
    /// Can not be combined with `with_index` or `set_index`.
    /// Following the mappy implementation, this also sets mapopt.mid_occ to 1000.
    /// ```
    /// # use minimap2::*;
    /// # let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
    /// let aligner = Aligner::builder().map_ont().with_seq(seq.as_bytes()).expect("Unable to build index");
    /// let query = b"CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCG";
    /// let hits = aligner.map(query, false, false, None, None);
    /// assert_eq!(hits.unwrap().len(), 1);
    /// ```
    pub fn with_seq(self, seq: &[u8]) -> Result<Self, &'static str>
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
    /// let hits = aligner.map(query, false, false, None, None);
    /// assert_eq!(hits.as_ref().unwrap().len(), 1);
    /// assert_eq!(hits.as_ref().unwrap()[0].target_name.as_ref().unwrap(), id);
    /// ```
    pub fn with_seq_and_id(self, seq: &[u8], id: &[u8]) -> Result<Self, &'static str>
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
    pub fn with_seqs(self, seqs: &[Vec<u8>]) -> Result<Self, &'static str> {
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
    ) -> Result<Self, &'static str> {
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
            //  conditionally compile using the correct pointer type (u8 or i8) for the platform
            #[cfg(any(
                all(target_arch = "aarch64", target_os = "linux"),
                all(target_arch = "arm", target_os = "linux")
            ))]
            {
                mm_idx_str(
                    self.idxopt.w as i32,
                    self.idxopt.k as i32,
                    (self.idxopt.flag & 1) as i32,
                    self.idxopt.bucket_bits as i32,
                    seqs.len() as i32,
                    seqs.as_ptr() as *mut *const u8,
                    ids.as_ptr() as *mut *const u8,
                )
            }
            #[cfg(any(
                all(target_arch = "aarch64", target_os = "macos"),
                all(target_arch = "x86_64", target_os = "linux"),
                all(target_arch = "x86_64", target_os = "macos")
            ))]
            {
                mm_idx_str(
                    self.idxopt.w as i32,
                    self.idxopt.k as i32,
                    (self.idxopt.flag & 1) as i32,
                    self.idxopt.bucket_bits as i32,
                    seqs.len() as i32,
                    seqs.as_ptr() as *mut *const i8,
                    ids.as_ptr() as *mut *const i8,
                )
            }
        });

        self.idx = Some(unsafe { idx.assume_init() });
        self.mapopt.mid_occ = 1000;

        Ok(self)
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
    ///
    pub fn map(
        &self,
        seq: &[u8],
        cs: bool,
        md: bool, // TODO
        max_frag_len: Option<usize>,
        extra_flags: Option<&[u64]>,
    ) -> Result<Vec<Mapping>, &'static str> {
        // Make sure index is set
        if !self.has_index() {
            return Err("No index");
        }

        // Make sure sequence is not empty
        if seq.is_empty() {
            return Err("Sequence is empty");
        }

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

        let mappings = BUF.with(|buf| {
            let km = unsafe { mm_tbuf_get_km(buf.borrow_mut().get_buf()) };

            mm_reg = MaybeUninit::new(unsafe {
                //  conditionally compile using the correct pointer type (u8 or i8) for the platform
                #[cfg(any(
                    all(target_arch = "aarch64", target_os = "linux"),
                    all(target_arch = "arm", target_os = "linux")
                ))]
                {
                    mm_map(
                        self.idx.as_ref().unwrap() as *const mm_idx_t,
                        seq.len() as i32,
                        seq.as_ptr() as *const u8,
                        &mut n_regs,
                        buf.borrow_mut().get_buf(),
                        &map_opt,
                        std::ptr::null(),
                    )
                }
                #[cfg(any(
                    all(target_arch = "aarch64", target_os = "macos"),
                    all(target_arch = "x86_64", target_os = "linux"),
                    all(target_arch = "x86_64", target_os = "macos")
                ))]
                {
                    mm_map(
                        self.idx.unwrap() as *const mm_idx_t,
                        seq.len() as i32,
                        seq.as_ptr() as *const i8,
                        &mut n_regs,
                        buf.borrow_mut().get_buf(),
                        &map_opt,
                        std::ptr::null(),
                    )
                }
            });
            let mut mappings = Vec::with_capacity(n_regs as usize);

            for i in 0..n_regs {
                unsafe {
                    let reg_ptr = (*mm_reg.as_ptr()).offset(i as isize);
                    let const_ptr = reg_ptr as *const mm_reg1_t;
                    let reg: mm_reg1_t = *reg_ptr;

                    let contig: *mut ::std::os::raw::c_char =
                        (*((*(self.idx.unwrap())).seq.offset(reg.rid as isize))).name;

                    let is_primary = reg.parent == reg.id;
                    let is_supplementary = reg.sam_pri() == 0;
                    
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
                            let mut cs_string: *mut libc::c_char = std::ptr::null_mut();
                            let mut m_cs_string: libc::c_int = 0i32;

                            let cs_str = if cs {
                                //  conditionally compile using the correct pointer type (u8 or i8) for the platform
                                #[cfg(any(
                                    all(target_arch = "aarch64", target_os = "linux"),
                                    all(target_arch = "arm", target_os = "linux")
                                ))]
                                {
                                    let _cs_len = mm_gen_cs(
                                        km,
                                        &mut cs_string,
                                        &mut m_cs_string,
                                        &self.idx.unwrap() as *const mm_idx_t,
                                        const_ptr,
                                        seq.as_ptr() as *const u8,
                                        true.into(),
                                    );
                                    let _cs_string = std::ffi::CStr::from_ptr(cs_string)
                                        .to_str()
                                        .unwrap()
                                        .to_string();
                                    Some(_cs_string)
                                }
                                #[cfg(any(
                                    all(target_arch = "aarch64", target_os = "macos"),
                                    all(target_arch = "x86_64", target_os = "linux"),
                                    all(target_arch = "x86_64", target_os = "macos")
                                ))]
                                {
                                    let _cs_len = mm_gen_cs(
                                        km,
                                        &mut cs_string,
                                        &mut m_cs_string,
                                        self.idx.unwrap() as *const mm_idx_t,
                                        const_ptr,
                                        seq.as_ptr() as *const i8,
                                        true.into(),
                                    );
                                    let _cs_string = std::ffi::CStr::from_ptr(cs_string)
                                        .to_str()
                                        .unwrap()
                                        .to_string();
                                    Some(_cs_string)
                                }
                            } else {
                                None
                            };

                            let md_str = if md {
                                //  conditionally compile using the correct pointer type (u8 or i8) for the platform
                                #[cfg(any(
                                    all(target_arch = "aarch64", target_os = "linux"),
                                    all(target_arch = "arm", target_os = "linux")
                                ))]
                                {
                                    let _md_len = mm_gen_MD(
                                        km,
                                        &mut cs_string,
                                        &mut m_cs_string,
                                        &self.idx.unwrap() as *const mm_idx_t,
                                        const_ptr,
                                        seq.as_ptr() as *const u8,
                                    );
                                    let _md_string = std::ffi::CStr::from_ptr(cs_string)
                                        .to_str()
                                        .unwrap()
                                        .to_string();
                                    Some(_md_string)
                                }
                                #[cfg(any(
                                    all(target_arch = "aarch64", target_os = "macos"),
                                    all(target_arch = "x86_64", target_os = "linux"),
                                    all(target_arch = "x86_64", target_os = "macos")
                                ))]
                                {
                                    let _md_len = mm_gen_MD(
                                        km,
                                        &mut cs_string,
                                        &mut m_cs_string,
                                        self.idx.unwrap() as *const mm_idx_t,
                                        const_ptr,
                                        seq.as_ptr() as *const i8,
                                    );
                                    let _md_string = std::ffi::CStr::from_ptr(cs_string)
                                        .to_str()
                                        .unwrap()
                                        .to_string();
                                    Some(_md_string)
                                }
                            } else {
                                None
                            };
                            libc::free(cs_string as *mut c_void);
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
                            alignment_score: Some(p.dp_score as u32),
                        })
                    } else {
                        None
                    };
                    mappings.push(Mapping {
                        target_name: Some(
                            std::ffi::CStr::from_ptr(contig)
                                .to_str()
                                .unwrap()
                                .to_string(),
                        ),
                        target_len: (*((*(self.idx.unwrap())).seq.offset(reg.rid as isize))).len
                            as i32,
                        target_start: reg.rs,
                        target_end: reg.re,
                        query_name: None,
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
                        alignment,
                    });
                    libc::free(reg.p as *mut c_void);
                }
            }
            mappings
        });
        // free some stuff here
        unsafe {
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
                    return Err("Error reading record in FASTA/X files. Please confirm integrity.")
                }
            };

            let mut seq_mappings = self.map(&record.seq(), cs, md, None, None).unwrap();

            for mapping in seq_mappings.iter_mut() {
                let id = record.id();
                if !id.is_empty() {
                    mapping.query_name = Some(from_utf8(id).unwrap().to_string());
                } else {
                    mapping.query_name = Some(
                        format!("Unnamed Seq with Length: {}", record.seq().len()).to_string(),
                    );
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
}

mod send {
    use crate::Aligner;
    unsafe impl Sync for Aligner {}
    unsafe impl Send for Aligner {}
}

/* TODO: This stopped working when we switched to not storing raw pointers but the structs themselves
/ Since Rust is now handling the structs, I think memory gets freed that way, maybe this is no longer
/ necessary?
/ TODO: Test for memory leaks
*/
impl Drop for Aligner {
    fn drop(&mut self) {
        if self.idx.is_some() {
            let idx = self.idx.take().unwrap();
            unsafe { mm_idx_destroy(idx) };
        }
    }
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

        let mut aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2);

        aligner = aligner.with_index(, None).unwrap();

        aligner
            .map(
                "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(),
                false,
                false,
                None,
                None,
            )
            .unwrap();
        let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
        assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));

        let jh = thread::spawn(move || {
            let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            aligner
        });

        let aligner = jh.join().unwrap();

        let jh = thread::spawn(move || {
            let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            aligner
        });

        let aligner = jh.join().unwrap();
    }

    #[test]
    fn shared_aligner() {
        // Because I'm not sure how this will work with FFI + Threads, want a sanity check
        use std::sync::Arc;
        use std::thread;

        let mut aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2);

        aligner = aligner.with_index("yeast_ref.mmi", None).unwrap();
        let aligner = Arc::new(aligner);

        aligner
            .map(
                "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(),
                false,
                false,
                None,
                None,
            )
            .unwrap();
        let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
        assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));

        let aligner_handle = Arc::clone(&aligner);
        let jh0 = thread::spawn(move || {
            let mappings = aligner_handle.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            let mappings = aligner_handle.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
        });

        let aligner_handle = Arc::clone(&aligner);
        let jh1 = thread::spawn(move || {
            let mappings = aligner_handle.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
            let mappings = aligner_handle.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
            assert!(mappings[0].query_len == Some(NonZeroI32::new(350).unwrap()));
        });
    }

    #[test]
    fn rayon() {
        // Because I'm not sure how this will work with FFI + Threads, want a sanity check
        use rayon::prelude::*;

        let mut aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2)
            .with_cigar();

        aligner = aligner.with_index("yeast_ref.mmi", None).unwrap();

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

        let results = sequences
            .par_iter()
            .map(|seq| {
                aligner
                    .map(seq.as_bytes(), false, false, None, None)
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
        let x: IdxOpt = Default::default();
    }

    #[test]
    fn mapopt() {
        let x: mm_mapopt_t = Default::default();
        let y: MapOpt = Default::default();
    }

    #[test]
    fn aligner_build_manually() {
        let idxopt: IdxOpt = Default::default();

        let mapopt: MapOpt = Default::default();

        let threads = 1;
        let idx = None;
        let idx_reader = None;

        let aligner = Aligner {
            idxopt,
            mapopt,
            threads,
            idx,
            idx_reader,
            cigar_clipping: false,
        };
    }

    #[test]
    fn aligner_builder() {
        let result = Aligner::builder();
    }

    #[test]
    fn aligner_builder_preset() {
        let result = Aligner::builder().preset(Preset::LrHq);
    }

    #[test]
    fn aligner_builder_preset_with_threads() {
        let result = Aligner::builder()
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
        let mut aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(1);

        println!("{}", aligner.idxopt.w);

        assert!(aligner.idxopt.w == 10);

        aligner = aligner
            .with_index("test_data/test_data.fasta", Some("test.mmi"))
            .unwrap();
    }

    #[test]
    fn test_builder() {
        let _aligner = Aligner::builder().preset(Preset::MapOnt);
    }

    #[test]
    fn test_mapping() {
        let mut aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2);

        aligner = aligner.with_index("yeast_ref.mmi", None).unwrap();

        aligner
            .map(
                "ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(),
                false,
                false,
                None,
                None,
            )
            .unwrap();
        let mappings = aligner.map("ACGGTAGAGAGGAAGAAGAAGGAATAGCGGACTTGTGTATTTTATCGTCATTCGTGGTTATCATATAGTTTATTGATTTGAAGACTACGTAAGTAATTTGAGGACTGATTAAAATTTTCTTTTTTAGCTTAGAGTCAATTAAAGAGGGCAAAATTTTCTCAAAAGACCATGGTGCATATGACGATAGCTTTAGTAGTATGGATTGGGCTCTTCTTTCATGGATGTTATTCAGAAGGAGTGATATATCGAGGTGTTTGAAACACCAGCGACACCAGAAGGCTGTGGATGTTAAATCGTAGAACCTATAGACGAGTTCTAAAATATACTTTGGGGTTTTCAGCGATGCAAAA".as_bytes(), false, false, None, None).unwrap();
        println!("{:#?}", mappings);

        // This should be reverse strand
        let mappings = aligner.map("TTTTGCATCGCTGAAAACCCCAAAGTATATTTTAGAACTCGTCTATAGGTTCTACGATTTAACATCCACAGCCTTCTGGTGTCGCTGGTGTTTCAAACACCTCGATATATCACTCCTTCTGAATAACATCCATGAAAGAAGAGCCCAATCCATACTACTAAAGCTATCGTCATATGCACCATGGTCTTTTGAGAAAATTTTGCCCTCTTTAATTGACTCTAAGCTAAAAAAGAAAATTTTAATCAGTCCTCAAATTACTTACGTAGTCTTCAAATCAATAAACTATATGATAACCACGAATGACGATAAAATACACAAGTCCGCTATTCCTTCTTCTTCCTCTCTACCGT".as_bytes(), false, false, None, None).unwrap();
        println!("Reverse Strand\n{:#?}", mappings);
        assert!(mappings[0].strand == Strand::Reverse);

        // Assert the Display impl for strand works
        println!("{}", mappings[0].strand);

        let mut aligner = aligner.with_cigar();

        aligner
            .map(
                "ATGAGCAAAATATTCTAAAGTGGAAACGGCACTAAGGTGAACTAAGCAACTTAGTGCAAAAc".as_bytes(),
                true,
                false,
                None,
                None,
            )
            .unwrap();

        let mappings = aligner.map("atCCTACACTGCATAAACTATTTTGcaccataaaaaaaagttatgtgtgGGTCTAAAATAATTTGCTGAGCAATTAATGATTTCTAAATGATGCTAAAGTGAACCATTGTAatgttatatgaaaaataaatacacaattaagATCAACACAGTGAAATAACATTGATTGGGTGATTTCAAATGGGGTCTATctgaataatgttttatttaacagtaatttttatttctatcaatttttagtaatatctacaaatattttgttttaggcTGCCAGAAGATCGGCGGTGCAAGGTCAGAGGTGAGATGTTAGGTGGTTCCACCAACTGCACGGAAGAGCTGCCCTCTGTCATTCAAAATTTGACAGGTACAAACAGactatattaaataagaaaaacaaactttttaaaggCTTGACCATTAGTGAATAGGTTATATGCTTATTATTTCCATTTAGCTTTTTGAGACTAGTATGATTAGACAAATCTGCTTAGttcattttcatataatattgaGGAACAAAATTTGTGAGATTTTGCTAAAATAACTTGCTTTGCTTGTTTATAGAGGCacagtaaatcttttttattattattataattttagattttttaatttttaaat".as_bytes(), true, false, None, None).unwrap();
        println!("{:#?}", mappings);
    }

    #[test]
    fn test_alignment_score() {
        let mut aligner = Aligner::builder()
            .preset(Preset::Splice)
            .with_index_threads(1);
        aligner = aligner
            .with_index("test_data/genome.fa", None)
            .unwrap()
            .with_cigar();

        let output = aligner.map(
            b"GAAATACGGGTCTCTGGTTTGACATAAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGCCCAGACTTAAATCGCACATACTGCGTCGTGCAATGCCGGGCGCTAACGGCTCAATATCACGCTGCGTCACTATGGCTACCCCAAAGCGGGGGGGGCATCGACGGGCTGTTTGATTTGAGCTCCATTACCCTACAATTAGAACACTGGCAACATTTGGGCGTTGAGCGGTCTTCCGTGTCGCTCGATCCGCTGGAACTTGGCAACCACACTCTAAACTACATGTGGTATGGCTCATAAGATCATGCGGATCGTGGCACTGCTTTCGGCCACGTTAGAGCCGCTGTGCTCGAAGATTGGGACCTACCAAC",
            false, false, None, None).unwrap();

        println!("{:#?}", output);
        panic!();
    }

    #[test]
    fn test_aligner_config_and_mapping() {
        let mut aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(2);
        aligner = aligner
            .with_index("test_data/test_data.fasta", None)
            .unwrap()
            .with_cigar();

        aligner
            .map(
                "ATGAGCAAAATATTCTAAAGTGGAAACGGCACTAAGGTGAACTAAGCAACTTAGTGCAAAAc".as_bytes(),
                true,
                true,
                None,
                None,
            )
            .unwrap();
        let mappings = aligner.map("atCCTACACTGCATAAACTATTTTGcaccataaaaaaaagGGACatgtgtgGGTCTAAAATAATTTGCTGAGCAATTAATGATTTCTAAATGATGCTAAAGTGAACCATTGTAatgttatatgaaaaataaatacacaattaagATCAACACAGTGAAATAACATTGATTGGGTGATTTCAAATGGGGTCTATctgaataatgttttatttaacagtaatttttatttctatcaatttttagtaatatctacaaatattttgttttaggcTGCCAGAAGATCGGCGGTGCAAGGTCAGAGGTGAGATGTTAGGTGGTTCCACCAACTGCACGGAAGAGCTGCCCTCTGTCATTCAAAATTTGACAGGTACAAACAGactatattaaataagaaaaacaaactttttaaaggCTTGACCATTAGTGAATAGGTTATATGCTTATTATTTCCATTTAGCTTTTTGAGACTAGTATGATTAGACAAATCTGCTTAGttcattttcatataatattgaGGAACAAAATTTGTGAGATTTTGCTAAAATAACTTGCTTTGCTTGTTTATAGAGGCacagtaaatcttttttattattattataattttagattttttaatttttaaat".as_bytes(), false, false, None, None).unwrap();
        println!("{:#?}", mappings);
    }

    #[test]
    fn test_mappy_output() {
        let mut aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_index_threads(1)
            .with_index("test_data/MT-human.fa", None)
            .unwrap()
            .with_cigar();

        let mut mappings = aligner.map(
    b"GTTTATGTAGCTTATTCTATCCAAAGCAATGCACTGAAAATGTCTCGACGGGCCCACACGCCCCATAAACAAATAGGTTTGGTCCTAGCCTTTCTATTAGCTCTTAGTGAGGTTACACATGCAAGCATCCCCGCCCCAGTGAGTCGCCCTCCAAGTCACTCTGACTAAGAGGAGCAAGCATCAAGCACGCAACAGCGCAG",
            true, true, None, None).unwrap();
        assert_eq!(mappings.len(), 1);

        let observed = mappings.pop().unwrap();

        assert_eq!(observed.target_name, Some(String::from("MT_human")));
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
        assert_eq!(align.cs, Some(String::from(":14-cc:1*ct:2+atc:9*ag:12*tc:1*ac:7*tc:4-t:1*ag:48*ag:2*ag:21*tc*tc:8-t:2*ag:5*tc:2*ag:4*ct*ac*ct:2*tc*ct:2*ag:4*ag:17")));

        aligner = aligner.with_cigar_clipping();
        let mut mappings = aligner.map(
            b"GTTTATGTAGCTTATTCTATCCAAAGCAATGCACTGAAAATGTCTCGACGGGCCCACACGCCCCATAAACAAATAGGTTTGGTCCTAGCCTTTCTATTAGCTCTTAGTGAGGTTACACATGCAAGCATCCCCGCCCCAGTGAGTCGCCCTCCAAGTCACTCTGACTAAGAGGAGCAAGCATCAAGCACGCAACAGCGCAG",
                    true, true, None, None).unwrap();
        assert_eq!(mappings.len(), 1);

        let observed = mappings.pop().unwrap();

        assert_eq!(observed.target_name, Some(String::from("MT_human")));
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
                            true, true, None, None).unwrap();
        assert_eq!(mappings.len(), 1);

        let observed = mappings.pop().unwrap();

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
            .with_index("test_data/MT-human.fa", None)
            .unwrap()
            .with_cigar();
        let query =  b"GTTTATGTAGCTTATTCTATCCAAAGCAATGCACTGAAAATGTCTCGACGGGCCCACACGCCCCATAAACAAATAGGTTTGGTCCTAGCCTTTCTATTAGCTCTTAGTGAGGTTACACATGCAAGCATCCCCGCCCCAGTGAGTCGCCCTCCAAGTCACTCTGACTAAGAGGAGCAAGCATCAAGCACGCAACAGCGCAG";

        for (md, cs) in vec![(true, true), (false, false), (true, false), (false, true)].iter() {
            let mapping = aligner
                .map(query, *cs, *md, None, None)
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
            .map(query.as_bytes(), false, false, None, None)
            .unwrap();
        assert_eq!(alignments.len(), 2);

        println!("----- Trying with_seqs 1");

        let aligner = Aligner::builder().short();
        let aligner = aligner.with_seqs(&vec![seq.as_bytes().to_vec()]).unwrap();
        let alignments = aligner
            .map(query.as_bytes(), false, false, None, None)
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
            .map(query.as_bytes(), false, false, None, None)
            .unwrap();
        assert_eq!(alignments.len(), 2);

        println!("----- Trying with_seq and id");

        let id = "test";
        let aligner = Aligner::builder().short();
        let aligner = aligner
            .with_seq_and_id(seq.as_bytes(), &id.as_bytes().to_vec())
            .unwrap();
        let alignments = aligner
            .map(query.as_bytes(), false, false, None, None)
            .unwrap();
        assert_eq!(alignments.len(), 2);

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
        let alignments = aligner
            .map(query.as_bytes(), true, true, None, None)
            .unwrap();
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

        let aligner = Aligner::builder();
        assert_eq!(
            aligner.map_file("test_data/MT-human.fa", false, false),
            Err("No index")
        );
        let aligner = aligner.with_index("test_data/MT-human.fa", None).unwrap();
        assert_eq!(
            aligner.map_file("test_data/file-does-not-exist", false, false),
            Err("File does not exist")
        );

        if let Err("File is empty") = Aligner::builder().with_index("test_data/empty.fa", None) {
            println!("File is empty - Success");
        } else {
            panic!("File is empty error not thrown");
        }

        if let Err("Invalid Path") = Aligner::builder().with_index("\0invalid_\0path\0", None) {
            println!("Invalid Path - Success");
        } else {
            panic!("Invalid Path error not thrown");
        }

        if let Err("Invalid Output") =
            Aligner::builder().with_index("test_data/MT-human.fa", Some("test\0test"))
        {
            println!("Invalid output - Success");
        } else {
            panic!("Invalid output error not thrown");
        }
    }

    #[test]
    fn test_send() {
        let seq = "CGGCACCAGGTTAAAATCTGAGTGCTGCAATAGGCGATTACAGTACAGCACCCAGCCTCCGAAATTCTTTAACGGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTTGTCCGAGAAACGCATCACCCACAGATAAAATCAGTTATTACAGTTGGACCTTTATGTCAAACCAGAGACCCGTATTTC";
        let query = "GGTCGTCGTCTCGATACTGCCACTATGCCTTTATATTATTGTCTTCAGGTGATGCTGCAGATCGTGCAGACGGGTGGCTTTAGTGTTGTGGGATGCATAGCTATTGACGGATCTTTGTCAATTGACAGAAATACGGGTCTCTGGTTTGACATGAAGGTCCAACTGTAATAACTGATTTTATCTGTGGGTGATGCGTTTCTCGGACAACCACGACCGCGACCAGACTTAAGTCTGGGCGCGGTCGTGGTT";
        let aligner = Aligner::builder().short();
        let aligner = std::sync::Arc::new(aligner.with_seq(seq.as_bytes()).unwrap());
        let alignments = aligner
            .map(query.as_bytes(), false, false, None, None)
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
                        .map(query.as_bytes(), false, false, None, None)
                        .expect("Failed to map"),
                )
                .expect("Failed to send")
        });
        let new_sender = std::thread::spawn(move || {
            send.send(
                aligner
                    .map(query.as_bytes(), false, false, None, None)
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

        let aligner = Aligner {
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
}

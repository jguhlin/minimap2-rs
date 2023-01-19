use std::cell::RefCell;

use std::io::Read;
use std::mem::MaybeUninit;
use std::num::NonZeroI32;
use std::path::Path;

use minimap2_sys::*;

#[cfg(feature = "map-file")]
use flate2::read::GzDecoder;
#[cfg(feature = "map-file")]
use simdutf8::basic::from_utf8;

#[cfg(feature = "htslib")]
pub mod htslib;

/// Alias for mm_mapop_t
pub type MapOpt = mm_mapopt_t;

/// Alias for mm_idxopt_t
pub type IdxOpt = mm_idxopt_t;

#[cfg(feature = "map-file")]
pub use fffx::{Fasta, Fastq, Sequence};

// TODO: Probably a better way to handle this...
static MAP_ONT: &str = "map-ont\0";
static AVA_ONT: &str = "ava-ont\0";
static MAP10K: &str = "map10k\0";
static AVA_PB: &str = "ava-pb\0";
static MAP_HIFI: &str = "map-hifi\0";
static ASM: &str = "asm\0";
static ASM5: &str = "asm5\0";
static ASM10: &str = "asm10\0";
static ASM20: &str = "asm20\0";
static SHORT: &str = "short\0";
static SR: &str = "sr\0";
static SPLICE: &str = "splice\0";
static CDNA: &str = "cdna\0";

/// Strand enum
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Strand {
    Forward,
    Reverse,
}

impl std::fmt::Display for Strand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Forward => write!(f, "+"),
            Reverse => write!(f, "-"),
        }
    }
}

/// Preset's for minimap2 config
#[derive(Debug, Clone)]
pub enum Preset {
    MapOnt,
    AvaOnt,
    Map10k,
    AvaPb,
    MapHifi,
    Asm,
    Asm5,
    Asm10,
    Asm20,
    Short,
    Sr,
    Splice,
    Cdna,
}

// Convert to c string for input into minimap2
impl From<Preset> for *const i8 {
    fn from(preset: Preset) -> Self {
        match preset {
            Preset::MapOnt => MAP_ONT.as_bytes().as_ptr() as *const i8,
            Preset::AvaOnt => AVA_ONT.as_bytes().as_ptr() as *const i8,
            Preset::Map10k => MAP10K.as_bytes().as_ptr() as *const i8,
            Preset::AvaPb => AVA_PB.as_bytes().as_ptr() as *const i8,
            Preset::MapHifi => MAP_HIFI.as_bytes().as_ptr() as *const i8,
            Preset::Asm => ASM.as_bytes().as_ptr() as *const i8,
            Preset::Asm5 => ASM5.as_bytes().as_ptr() as *const i8,
            Preset::Asm10 => ASM10.as_bytes().as_ptr() as *const i8,
            Preset::Asm20 => ASM20.as_bytes().as_ptr() as *const i8,
            Preset::Short => SHORT.as_bytes().as_ptr() as *const i8,
            Preset::Sr => SR.as_bytes().as_ptr() as *const i8,
            Preset::Splice => SPLICE.as_bytes().as_ptr() as *const i8,
            Preset::Cdna => CDNA.as_bytes().as_ptr() as *const i8,
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
}

/// Mapping result
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub alignment: Option<Alignment>,
    // cdef int _ctg_len, _r_st, _r_en
    // pub contig_len: usize,
    // pub reference_start: i32,
    // pub reference_end: i32,
    // cdef int _q_st, _q_en
    // cdef int _NM, _mlen, _blen
    // pub nm: i32,
    // pub match_len: i32,
    // pub block_len: i32,
    // cdef int8_t _strand, _trans_strand
    // pub strand: Strand,
    // pub trans_strand: Strand,
    // cdef uint8_t _mapq, _is_primary

    // pub is_primary: bool,
    // cdef int _seg_id
    // pub seg_id: u32,
    // cdef _ctg, _cigar, _cs, _MD # these are python objects
    // pub contig: String,
    // pub cs: Option<String>,
    // pub md: Option<String>,
    // pub score: i32,
    // pub score0: i32,
}

// Thread local buffer (memory management) for minimap2
thread_local! {
    static BUF: RefCell<ThreadLocalBuffer> = RefCell::new(ThreadLocalBuffer::new());
}

/// ThreadLocalBuffer for minimap2 memory management
struct ThreadLocalBuffer {
    buf: *mut mm_tbuf_t,
}

impl ThreadLocalBuffer {
    pub fn new() -> Self {
        let buf = unsafe { mm_tbuf_init() };
        Self { buf }
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
    pub idx: Option<mm_idx_t>,

    /// Index reader created by minimap2
    pub idx_reader: Option<mm_idx_reader_t>,
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
        }
    }
}

impl Aligner {
    /// Create a new server builder that can configure a [`Server`].
    pub fn builder() -> Self {
        Aligner {
            mapopt: MapOpt {
                seed: 42,
                best_n: 1,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

impl Aligner {
    /// Ergonomic function for Aligner.
    ///
    /// TODO: Make it simpler (and less redundant) with functions?
    /// Such that it'd be ..map_ont() or ..map_ava() instead?
    ///
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::MapOnt).with_threads(1).with_cigar();
    /// ```
    // pub fn preset(preset: Preset) -> Aligner {
    //     Aligner::builder().preset(preset)
    // }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::MapOnt);
    /// ```
    pub fn map_ont(self) -> Self {
        self.preset(Preset::MapOnt)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::AvaOnt);
    /// ```
    pub fn ava_ont(self) -> Self {
        self.preset(Preset::AvaOnt)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Map10k);
    /// ```
    pub fn map10k(self) -> Self {
        self.preset(Preset::Map10k)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::AvaPb);
    /// ```
    pub fn ava_pb(self) -> Self {
        self.preset(Preset::AvaPb)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::MapHifi);
    /// ```
    pub fn map_hifi(self) -> Self {
        self.preset(Preset::MapHifi)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Asm);
    /// ```
    pub fn asm(self) -> Self {
        self.preset(Preset::Asm)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Asm5);
    /// ```
    pub fn asm5(self) -> Self {
        self.preset(Preset::Asm5)
    }
    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Asm10);
    /// ```
    pub fn asm10(self) -> Self {
        self.preset(Preset::Asm10)
    }
    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Asm20);
    /// ```
    pub fn asm20(self) -> Self {
        self.preset(Preset::Asm20)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Short);
    /// ```
    pub fn short(self) -> Self {
        self.preset(Preset::Short)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Sr);
    /// ```
    pub fn sr(self) -> Self {
        self.preset(Preset::Sr)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Splice);
    /// ```
    pub fn splice(self) -> Self {
        self.preset(Preset::Splice)
    }

    /// Ergonomic function for Aligner. Just to see if people prefer this over the
    /// preset() function.
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::Cdna);
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
            idxopt: idxopt,
            mapopt: mapopt,
            ..Default::default()
        }
    }

    /// Set Alignment mode / cigar mode in minimap2
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().preset(Preset::MapOnt).with_cigar();
    /// ```
    ///
    pub fn with_cigar(mut self) -> Self {
        // Make sure MM_F_CIGAR flag isn't already set
        assert!((self.mapopt.flag & MM_F_CIGAR as i64) == 0);

        self.mapopt.flag |= MM_F_CIGAR as i64;
        self
    }

    /// Set Alignment mode / cigar mode in minimap2
    /// ```
    /// # use minimap2::*;
    /// Aligner::builder().with_threads(10);
    /// ```
    ///
    /// Set the number of threads (prefer to use the struct config)
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    /// Set index parameters for minimap2 using builder pattern
    /// Creates the index as well with the given number of threads (set at struct creation)
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
    pub fn with_index(mut self, path: &str, output: Option<&str>) -> Result<Self, &'static str> {
        return match self.set_index(path, output) {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        };
    }

    pub fn set_index(&mut self, path: &str, output: Option<&str>) -> Result<(), &'static str> {
        // Confirm file exists
        if !Path::new(path).exists() {
            return Err("File does not exist");
        }

        // Confirm file is not empty
        if Path::new(path).metadata().unwrap().len() == 0 {
            return Err("File is empty");
        }

        let path = match std::ffi::CString::new(path) {
            Ok(path) => path,
            Err(_) => return Err("Invalid path"),
        };

        let output = match output {
            Some(output) => match std::ffi::CString::new(output) {
                Ok(output) => output,
                Err(_) => return Err("Invalid output"),
            },
            None => std::ffi::CString::new(Vec::new()).unwrap(),
        };

        let idx_reader = MaybeUninit::new(unsafe {
            mm_idx_reader_open(path.as_ptr(), &self.idxopt, output.as_ptr())
        });

        let mut idx: MaybeUninit<*mut mm_idx_t> = MaybeUninit::uninit();

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

        self.idx = Some(unsafe { *idx.assume_init() });

        Ok(())
    }

    /// Map a single sequence to an index
    /// not implemented yet!
    pub fn with_seq(self, seq: &[u8]) -> Result<Self, &'static str> {
        let _seq = match std::ffi::CString::new(seq) {
            Ok(seq) => seq,
            Err(_) => return Err("Invalid sequence"),
        };

        todo!();

        //let idx = MaybeUninit::new(unsafe {
        /*mm_idx_str(
            self.idx_opt.w,
            self.idx_opt.k,
            self.idx_opt.flag & 1,
            self.idx_opt.bucket_bits,
            str.encode(seq),
            len(seq),
        )*/
        //});

        //self.idx = Some(idx);

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
    /// extra_flags: Extra flags to pass to minimap2 as Vec<u64>
    ///
    pub fn map(
        &self,
        seq: &[u8],
        cs: bool,
        md: bool, // TODO
        max_frag_len: Option<usize>,
        extra_flags: Option<Vec<u64>>,
    ) -> Result<Vec<Mapping>, &'static str> {
        // Make sure index is set
        if !self.has_index() {
            return Err("No index");
        }

        // Make sure sequence is not empty
        if seq.len() == 0 {
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
                map_opt.flag |= flag as i64;
            }
        }

        let mappings = BUF.with(|buf| {
            let km = unsafe { mm_tbuf_get_km(buf.borrow_mut().buf) };

            mm_reg = MaybeUninit::new(unsafe {
                mm_map(
                    self.idx.as_ref().unwrap() as *const mm_idx_t,
                    seq.len() as i32,
                    seq.as_ptr() as *const i8,
                    &mut n_regs,
                    buf.borrow_mut().buf,
                    &mut map_opt,
                    std::ptr::null(),
                )
            });

            let mut mappings = Vec::with_capacity(n_regs as usize);

            for i in 0..n_regs {
                unsafe {
                    let reg_ptr = (*mm_reg.as_ptr()).offset(i as isize);
                    // println!("{:#?}", *reg_ptr);
                    let const_ptr = reg_ptr as *const mm_reg1_t;
                    let reg: mm_reg1_t = *reg_ptr;

                    // TODO: Get all contig names and store as Cow<String> somewhere centralized...
                    let contig: *mut ::std::os::raw::c_char =
                        (*(self.idx.unwrap()).seq.offset(reg.rid as isize)).name;

                    let is_primary = reg.parent == reg.id;
                    let alignment = if !reg.p.is_null() {
                        let p = &*reg.p;

                        // calculate the edit distance
                        let nm = reg.blen - reg.mlen + p.n_ambi() as i32;
                        let n_cigar = p.n_cigar;
                        // Create a vector of the cigar blocks
                        let (cigar, cigar_str) = if n_cigar > 0 {
                            let cigar = p
                                .cigar
                                .as_slice(n_cigar as usize)
                                .to_vec()
                                .iter()
                                .map(|c| ((c >> 4) as u32, (c & 0xf) as u8)) // unpack the length and op code
                                .collect::<Vec<(u32, u8)>>();
                            let cigar_str = cigar
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
                            (Some(cigar), Some(cigar_str))
                        } else {
                            (None, None)
                        };

                        let (cs_str, md_str) = if cs || md {
                            let mut cs_string: *mut libc::c_char = std::ptr::null_mut();
                            let mut m_cs_string: libc::c_int = 0i32;

                            let cs_str = if cs {
                                let _cs_len = mm_gen_cs(
                                    km,
                                    &mut cs_string,
                                    &mut m_cs_string,
                                    &self.idx.unwrap() as *const mm_idx_t,
                                    const_ptr,
                                    seq.as_ptr() as *const i8,
                                    true.into(),
                                );
                                let _cs_string = std::ffi::CStr::from_ptr(cs_string)
                                    .to_str()
                                    .unwrap()
                                    .to_string();
                                Some(_cs_string)
                            } else {
                                None
                            };

                            let md_str = if md {
                                let _md_len = mm_gen_MD(
                                    km,
                                    &mut cs_string,
                                    &mut m_cs_string,
                                    &self.idx.unwrap() as *const mm_idx_t,
                                    const_ptr,
                                    seq.as_ptr() as *const i8,
                                );
                                let _md_string = std::ffi::CStr::from_ptr(cs_string)
                                    .to_str()
                                    .unwrap()
                                    .to_string();
                                Some(_md_string)
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
                        target_len: (*(self.idx.unwrap()).seq.offset(reg.rid as isize)).len as i32,
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
                        alignment,
                    });
                }
            }

            mappings
        });
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

        // Read the first 50 bytes of the file
        let mut f = std::fs::File::open(file).unwrap();
        let mut buffer = [0; 50];
        f.read(&mut buffer).unwrap();
        // Close the file
        drop(f);

        // Check if the file is gzipped
        let compression_type = detect_compression_format(&buffer).unwrap();
        if compression_type != CompressionType::NONE && compression_type != CompressionType::GZIP {
            return Err("Compression type is not supported");
        }

        // If gzipped, open it with a reader...
        let mut reader: Box<dyn Read> = if compression_type == CompressionType::GZIP {
            Box::new(GzDecoder::new(std::fs::File::open(file).unwrap()))
        } else {
            Box::new(std::fs::File::open(file).unwrap())
        };

        // Check the file type
        let mut buffer = [0; 4];
        reader.read(&mut buffer).unwrap();
        let file_type = detect_file_format(&buffer).unwrap();
        if file_type != FileFormat::FASTA && file_type != FileFormat::FASTQ {
            return Err("File type is not supported");
        }

        // If gzipped, open it with a reader...
        let reader: Box<dyn Read> = if compression_type == CompressionType::GZIP {
            Box::new(GzDecoder::new(std::fs::File::open(file).unwrap()))
        } else {
            Box::new(std::fs::File::open(file).unwrap())
        };

        // Put into bufreader
        let mut reader = std::io::BufReader::new(reader);

        let reader: Box<dyn Iterator<Item = Result<Sequence, &'static str>>> =
            if file_type == FileFormat::FASTA {
                Box::new(Fasta::from_buffer(&mut reader))
            } else {
                Box::new(Fastq::from_buffer(&mut reader))
            };

        // The output vec
        let mut mappings = Vec::new();

        // Iterate over the sequences
        for seq in reader {
            let seq = seq.unwrap();
            let mut seq_mappings = self
                .map(&seq.sequence.unwrap(), cs, md, None, None)
                .unwrap();
            for mut mapping in seq_mappings.iter_mut() {
                mapping.query_name = seq.id.clone();
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

/* TODO: This stopped working when we switched to not storing raw pointers but the structs themselves
// Since Rust is now handling the structs, I think memory gets freed that way, maybe this is no longer
// necessary?
// TODO: Test for memory leaks

impl Drop for Aligner {
    fn drop(&mut self) {
        if self.idx.is_some() {
            println!("Doing the drop");
            let mut idx: mm_idx_t = self.idx.take().unwrap();
            let ptr: *mut mm_idx_t = &mut idx;
            unsafe { mm_idx_destroy(ptr) };
            std::mem::forget(idx);
            println!("Done the drop");
        }
    }
}*/

#[derive(PartialEq, Eq)]
pub enum FileFormat {
    FASTA,
    FASTQ,
}

#[allow(dead_code)]
#[cfg(feature = "map-file")]
pub fn detect_file_format(buffer: &[u8]) -> Result<FileFormat, &'static str> {
    let buffer = from_utf8(&buffer).expect("Unable to parse file as UTF-8");
    if buffer.starts_with(">") {
        Ok(FileFormat::FASTA)
    } else if buffer.starts_with("@") {
        Ok(FileFormat::FASTQ)
    } else {
        Err("Unknown file format")
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum CompressionType {
    GZIP,
    BZIP2,
    XZ,
    RAR,
    ZSTD,
    LZ4,
    LZMA,
    NONE,
}

/// Return the compression type of a file
#[allow(dead_code)]
pub fn detect_compression_format(buffer: &[u8]) -> Result<CompressionType, &'static str> {
    Ok(match buffer {
        [0x1F, 0x8B, ..] => CompressionType::GZIP,
        [0x42, 0x5A, ..] => CompressionType::BZIP2,
        [0xFD, b'7', b'z', b'X', b'Z', 0x00] => CompressionType::XZ,
        [0x28, 0xB5, 0x2F, 0xFD, ..] => CompressionType::LZMA,
        [0x5D, 0x00, ..] => CompressionType::LZMA,
        [0x1F, 0x9D, ..] => CompressionType::LZMA,
        [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] => CompressionType::ZSTD,
        [0x04, 0x22, 0x4D, 0x18, ..] => CompressionType::LZ4,
        [0x08, 0x22, 0x4D, 0x18, ..] => CompressionType::LZ4,
        [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07] => CompressionType::RAR,
        _ => CompressionType::NONE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;

    #[test]
    fn does_it_work() {
        let mut mm_idxopt = MaybeUninit::uninit();
        let mut mm_mapopt = MaybeUninit::uninit();

        unsafe { mm_set_opt(&0, mm_idxopt.as_mut_ptr(), mm_mapopt.as_mut_ptr()) };
    }

    #[test]
    fn create_index_file_missing() {
        let result = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_threads(1)
            .with_index(
                "test_data/test.fa_FILE_NOT_FOUND",
                Some("test_FILE_NOT_FOUND.mmi"),
            );
        assert!(result.is_err());
    }

    #[test]
    fn create_index() {
        let mut aligner = Aligner::builder().preset(Preset::MapOnt).with_threads(1);

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
        let mut aligner = Aligner::builder().preset(Preset::MapOnt).with_threads(2);

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
    fn test_aligner_config_and_mapping() {
        let mut aligner = Aligner::builder().preset(Preset::MapOnt).with_threads(2);
        aligner = aligner
            .with_index("test_data/test_data.fasta", Some("test.mmi"))
            .unwrap()
            .with_cigar();

        aligner
            .map(
                "ATGAGCAAAATATTCTAAAGTGGAAACGGCACTAAGGTGAACTAAGCAACTTAGTGCAAAAc".as_bytes(),
                true,
                false,
                None,
                None,
            )
            .unwrap();
        let mappings = aligner.map("atCCTACACTGCATAAACTATTTTGcaccataaaaaaaagGGACatgtgtgGGTCTAAAATAATTTGCTGAGCAATTAATGATTTCTAAATGATGCTAAAGTGAACCATTGTAatgttatatgaaaaataaatacacaattaagATCAACACAGTGAAATAACATTGATTGGGTGATTTCAAATGGGGTCTATctgaataatgttttatttaacagtaatttttatttctatcaatttttagtaatatctacaaatattttgttttaggcTGCCAGAAGATCGGCGGTGCAAGGTCAGAGGTGAGATGTTAGGTGGTTCCACCAACTGCACGGAAGAGCTGCCCTCTGTCATTCAAAATTTGACAGGTACAAACAGactatattaaataagaaaaacaaactttttaaaggCTTGACCATTAGTGAATAGGTTATATGCTTATTATTTCCATTTAGCTTTTTGAGACTAGTATGATTAGACAAATCTGCTTAGttcattttcatataatattgaGGAACAAAATTTGTGAGATTTTGCTAAAATAACTTGCTTTGCTTGTTTATAGAGGCacagtaaatcttttttattattattataattttagattttttaatttttaaat".as_bytes(), true, true, None, None).unwrap();
        println!("{:#?}", mappings);
    }

    #[test]
    fn test_mappy_output() {
        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_threads(1)
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
            Some(String::from("14M2D4M3I37M1D85M1D48M"))
        );
        assert_eq!(
            align.md,
            Some(String::from(
                "14^CC1C11A12T1A7T4^T1A48A2A21T0T8^T2A5T2A4C0A0C2T0C2A4A17"
            ))
        );
        assert_eq!(align.cs, Some(String::from(":14-cc:1*ct:2+atc:9*ag:12*tc:1*ac:7*tc:4-t:1*ag:48*ag:2*ag:21*tc*tc:8-t:2*ag:5*tc:2*ag:4*ct*ac*ct:2*tc*ct:2*ag:4*ag:17")));
    }

    #[test]
    fn test_mappy_output_no_md() {
        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_threads(1)
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
}

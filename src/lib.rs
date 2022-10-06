use std::cell::RefCell;
use std::error::Error;
use std::mem::MaybeUninit;
use std::path::Path;
use std::num::NonZeroI32;

use minimap2_sys::*;

pub static MAP_ONT: &str = "map-ont\0";
pub static AVA_ONT: &str = "ava-ont\0";
pub static MAP10K: &str = "map10k\0";
pub static AVA_PB: &str = "ava-pb\0";
pub static MAP_HIFI: &str = "map-hifi\0";
pub static ASM: &str = "asm\0";
pub static SHORT: &str = "short\0";
pub static SR: &str = "sr\0";
pub static SPLICE: &str = "splice\0";
pub static CDNA: &str = "cdna\0";

thread_local! {
    static BUF: RefCell<ThreadLocalBuffer> = RefCell::new(ThreadLocalBuffer::new());
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Strand {
    Forward,
    Reverse,
}

#[derive(Debug)]
pub enum Preset {
    MapOnt,
    AvaOnt,
    Map10k,
    AvaPb,
    MapHifi,
    Asm,
    Short,
    Sr,
    Splice,
    Cdna,
}

impl From<Preset> for *const i8 {
    fn from(preset: Preset) -> Self {
        match preset {
            Preset::MapOnt => MAP_ONT.as_bytes().as_ptr() as *const i8,
            Preset::AvaOnt => AVA_ONT.as_bytes().as_ptr() as *const i8,
            Preset::Map10k => MAP10K.as_bytes().as_ptr() as *const i8,
            Preset::AvaPb => AVA_PB.as_bytes().as_ptr() as *const i8,
            Preset::MapHifi => MAP_HIFI.as_bytes().as_ptr() as *const i8,
            Preset::Asm => ASM.as_bytes().as_ptr() as *const i8,
            Preset::Short => SHORT.as_bytes().as_ptr() as *const i8,
            Preset::Sr => SR.as_bytes().as_ptr() as *const i8,
            Preset::Splice => SPLICE.as_bytes().as_ptr() as *const i8,
            Preset::Cdna => CDNA.as_bytes().as_ptr() as *const i8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignmentType {
    Primary,
    Secondary,
    Inversion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alignment {
    pub is_primary: bool,
}

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

pub struct ThreadLocalBuffer {
    pub buf: *mut mm_tbuf_t,
}

impl ThreadLocalBuffer {
    pub fn new() -> Self {
        let buf = unsafe { mm_tbuf_init() };
        Self { buf }
    }
}

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

#[derive(Debug, Clone)]
pub struct Aligner {
    idxopt: mm_idxopt_t,
    mapopt: mm_mapopt_t,
    idx: Option<*mut mm_idx_t>,
    idx_reader: Option<*mut mm_idx_reader_t>,
    threads: usize,
    /* TODO: Goals for better ergonomics...

    // mm_idx_opt
    pub k: u16,
    pub w: u16,
    pub idxflag: u16,
    pub bucket_bits: u16,
    pub mini_batch_size_idx: i64, // Renamed from mini_batch_size
    pub batch_size: u64,

    // mapopt
    pub mapflag: i64,
    pub seed: i32,
    pub sdust_threshold: i32, // Renamed from sdust_thres
    pub max_qlen: i32,
    pub bw: i32,
    pub bw_long: i32,
    pub max_gap: i32,
    pub max_gap_ref: i32,
    pub max_frag_len: i32,
    pub max_chain_skip: i32,
    pub max_chain_iter: i32,
    pub min_cnt: i32,
    pub min_chain_score: i32,
    pub chain_gap_scale: f32,
    pub chain_skip_scale: f32,
    pub rmq_size_cap: i32,
    pub rmq_inner_dist: i32,
    pub rmq_rescue_size: i32,
    pub rmq_rescue_ratio: f32,
    pub mask_level: f32,
    pub mask_len: i32,
    pub pri_ratio: f32,
    pub best_n: i32,
    pub alt_drop: f32,
    pub a: i32,
    pub b: i32,
    pub q: i32,
    pub e: i32,
    pub q2: i32,
    pub e2: i32,
    pub sc_ambi: i32,
    pub noncan: i32,
    pub junc_bonus: i32,
    pub zdrop: i32,
    pub zdrop_inv: i32,
    pub end_bonus: i32,
    pub min_dp_max: i32,
    pub min_ksw_len: i32,
    pub anchor_ext_len: i32,
    pub anchor_ext_shift: i32,
    pub max_clip_ratio: f32,
    pub rank_min_len: i32,
    pub rank_frac: f32,
    pub pe_ori: i32,
    pub pe_bonus: i32,
    pub mid_occ_frac: f32,
    pub q_occ_frac: f32,
    pub min_mid_occ: i32,
    pub max_mid_occ: i32,
    pub mid_occ: i32,
    pub max_occ: i32,
    pub max_max_occ: i32,
    pub occ_dist: i32,
    pub mini_batch_size_map: i64, // Renamed from mini_batch_size
    pub max_sw_mat: i64,
    pub cap_kalloc: i64,
    pub split_prefix: Vec<u8>,
    */
}

impl Default for Aligner {
    fn default() -> Self {
        let mut mm_idxopt = MaybeUninit::uninit();
        let mut mm_mapopt = MaybeUninit::uninit();

        unsafe {
            mm_set_opt(
                std::ptr::null(),
                mm_idxopt.as_mut_ptr(),
                mm_mapopt.as_mut_ptr(),
            )
        };
        Self {
            idxopt: unsafe { mm_idxopt.assume_init() },
            mapopt: unsafe { mm_mapopt.assume_init() },
            threads: 1,
            idx: None,
            idx_reader: None,
        }
    }
}

impl Aligner {
    pub fn with_preset(preset: Preset) -> Self {
        let mut mm_idxopt = MaybeUninit::uninit();
        let mut mm_mapopt = MaybeUninit::uninit();

        #[cfg(test)]
        println!("Preset: {:#?}", preset);

        unsafe {
            mm_set_opt(
                std::ptr::null(),
                mm_idxopt.as_mut_ptr(),
                mm_mapopt.as_mut_ptr(),
            );
            mm_set_opt(
                preset.into(),
                mm_idxopt.as_mut_ptr(),
                mm_mapopt.as_mut_ptr(),
            )
        };

        let mut mm_mapopt = unsafe { mm_mapopt.assume_init() };

        Self {
            idxopt: unsafe { mm_idxopt.assume_init() },
            mapopt: mm_mapopt,
            ..Default::default()
        }
    }

    pub fn with_cigar(mut self) -> Self {
        self.mapopt.flag |= MM_F_CIGAR as i64;
        self
    }

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    pub fn with_index() {
        // Index, but instead pass output as None. Placeholder
        todo!();
    }

    pub fn with_named_index(
        mut self,
        path: &Path,
        output: Option<&str>,
    ) -> Result<Self, &'static str> {
        let path = match path.to_str() {
            Some(path) => path,
            None => return Err("Invalid path"),
        };

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

        unsafe {
            if idx_reader.assume_init().is_null() {
                return Err("Failed to create index reader - File not found?");
            }
        }

        self.idx_reader = Some(unsafe { idx_reader.assume_init() });

        let mut idx: MaybeUninit<*mut mm_idx_t> = MaybeUninit::uninit();

        unsafe {
            // Test reading? Just following: https://github.com/lh3/minimap2/blob/master/python/mappy.pyx#L147
            idx = MaybeUninit::new(mm_idx_reader_read(
                self.idx_reader.unwrap(),
                self.threads as libc::c_int,
            ));
            // Close the reader
            mm_idx_reader_close(self.idx_reader.unwrap());
            // Set index opts
            mm_mapopt_update(&mut self.mapopt, *idx.as_ptr());
            // Idx index name
            mm_idx_index_name(idx.assume_init());
        }

        self.idx = Some(unsafe { idx.assume_init() });

        Ok(self)
    }

    pub fn with_seq(mut self, seq: &[u8]) -> Result<Self, &'static str> {
        let seq = match std::ffi::CString::new(seq) {
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
    pub fn map(
        &mut self,
        seq: &[u8],
        cs: bool,
        MD: bool,
        max_frag_len: Option<usize>,
        extra_flags: Option<i64>,
    ) -> Result<Vec<Mapping>, &'static str> {
        // cdef cmappy.mm_reg1_t *regs
        let mut mm_reg: MaybeUninit<*mut mm_reg1_t> = MaybeUninit::uninit();

        // Skipping, probably won't need??
        // cdef cmappy.mm_hitpy_t h

        // cdef ThreadBuffer b
        let mut b: ThreadLocalBuffer = Default::default();

        // cdef int n_regs
        let mut n_regs: i32 = 0;

        // cdef char *cs_str = NULL
        let mut cs_str: *mut libc::c_char = std::ptr::null_mut();

        // cdef int l_cs_str, m_cs_str = 0
        let mut l_cs_str: i32 = 0;
        let mut m_cs_str: i32 = 0;

        // cdef void *km - Nah
        // let mut km: *mut libc::c_void = std::ptr::null_mut();

        // cdef cmappy.mm_mapopt_t map_opt
        let mut map_opt = self.mapopt.clone();
        // Already defined in self...

        if !self.has_index() {
            return Err("No index");
        }

        // if max_frag_len is not None: map_opt.max_frag_len = max_frag_len
        if let Some(max_frag_len) = max_frag_len {
            map_opt.max_frag_len = max_frag_len as i32;
        }

        // if extra_flags is not None: map_opt.flag |= extra_flags
        if let Some(extra_flags) = extra_flags {
            map_opt.flag |= extra_flags as i64;
        }

        // if buf is None: b = ThreadBuffer()
        // else: b = buf
        let mappings = BUF.with(|buf| {
            // No idea what this does...
            // km = cmappy.mm_tbuf_get_km(b._b)
            let km = unsafe { mm_tbuf_get_km(buf.borrow_mut().buf) };

            // Seq is already bytes
            // _seq = seq if isinstance(seq, bytes) else seq.encode()
            mm_reg = MaybeUninit::new(unsafe {
                mm_map(
                    *&self.idx.unwrap(),
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
                    let reg: mm_reg1_t = *(*mm_reg.as_ptr().offset(i as isize));
                    // println!("New Match");
                    // println!("{:#?}", reg);
                    let contig: *mut ::std::os::raw::c_char = (*(*(self.idx.unwrap())).seq.offset(reg.rid as isize)).name;
                    // println!("Contig: {}", std::ffi::CStr::from_ptr(contig).to_str().unwrap());

                    let p_is_null = reg.p.is_null();

                    mappings.push(Mapping {
                        target_name: Some(std::ffi::CStr::from_ptr(contig).to_str().unwrap().to_string()),
                        target_len: (*(*(self.idx.unwrap())).seq.offset(reg.rid as isize)).len as i32,
                        target_start: reg.rs,
                        target_end: reg.re,
                        query_name: None,
                        query_len: None,
                        query_start: reg.qs,
                        query_end: reg.qe,
                        strand: if reg.rev() == 0 { Strand::Forward } else { Strand::Reverse },
                        // trans_strand: Strand::Forward, // *(reg).p.trans_strand == 0 { Strand::Forward } else { Strand::Reverse },
                        //nm: reg.blen - reg.mlen + p.as_ref().expect("Pointer Hell").n_ambi() as i32,
                        match_len: reg.mlen,
                        block_len: reg.blen,
                        mapq: reg.mapq(),
                        alignment: None,
                    })
                }
            }

            mappings
        });
        Ok(mappings)
    }

    // This is in the python module, so copied here...
    pub fn has_index(&self) -> bool {
        self.idx.is_some()
    }
}

impl Drop for Aligner {
    fn drop(&mut self) {
        if self.idx.is_some() {
            unsafe { mm_idx_destroy(self.idx.unwrap()) };
        }
    }
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
        let result = Aligner::with_preset(Preset::MapOnt)
            .with_threads(1)
            .with_named_index(
                Path::new("test_data/test.fa_FILE_NOT_FOUND"),
                Some("test_FILE_NOT_FOUND.mmi"),
            );
        assert!(result.is_err());
    }

    #[test]
    fn create_index() {
        let mut aligner = Aligner::with_preset(Preset::MapOnt).with_threads(1);

        println!("{}", aligner.idxopt.w);

        assert!(aligner.idxopt.w == 10);

        aligner = aligner
            .with_named_index(Path::new("test_data/test_data.fasta"), Some("test.mmi"))
            .unwrap();
    }

    #[test]
    fn test_mapping() {
        let mut aligner = Aligner::with_preset(Preset::MapOnt).with_threads(1);
        aligner = aligner
            .with_named_index(Path::new("test_data/test_data.fasta"), Some("test.mmi"))
            .unwrap();
        aligner.map("ATGAGCAAAATATTCTAAAGTGGAAACGGCACTAAGGTGAACTAAGCAACTTAGTGCAAAAc".as_bytes(), false, false, None, None).unwrap();
        let mappings = aligner.map("atCCTACACTGCATAAACTATTTTGcaccataaaaaaaagttatgtgtgGGTCTAAAATAATTTGCTGAGCAATTAATGATTTCTAAATGATGCTAAAGTGAACCATTGTAatgttatatgaaaaataaatacacaattaagATCAACACAGTGAAATAACATTGATTGGGTGATTTCAAATGGGGTCTATctgaataatgttttatttaacagtaatttttatttctatcaatttttagtaatatctacaaatattttgttttaggcTGCCAGAAGATCGGCGGTGCAAGGTCAGAGGTGAGATGTTAGGTGGTTCCACCAACTGCACGGAAGAGCTGCCCTCTGTCATTCAAAATTTGACAGGTACAAACAGactatattaaataagaaaaacaaactttttaaaggCTTGACCATTAGTGAATAGGTTATATGCTTATTATTTCCATTTAGCTTTTTGAGACTAGTATGATTAGACAAATCTGCTTAGttcattttcatataatattgaGGAACAAAATTTGTGAGATTTTGCTAAAATAACTTGCTTTGCTTGTTTATAGAGGCacagtaaatcttttttattattattataattttagattttttaatttttaaat".as_bytes(), false, false, None, None).unwrap();
        println!("{:#?}", mappings);
    }
}

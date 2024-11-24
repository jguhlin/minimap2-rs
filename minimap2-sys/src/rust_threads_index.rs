use needletail::{parse_fastx_file, Sequence, FastxReader};

use std::sync::{Arc, Mutex, RwLock};
use std::path::Path;
use std::ffi::{CStr, CString};

use super::*;

#[derive(Clone)]
pub struct Pipeline {
    mini_batch_size: u64, 
    batch_size: u64,
    sum_len: Arc<Mutex<u64>>,
    fp: Arc<Mutex<Box<dyn FastxReader>>>,
    mi: Arc<RwLock<*mut mm_idx_t>>,
    total_seqs: Arc<Mutex<u64>>, // todo make atomic, was trying out u128
}

impl Pipeline {
    fn read_batch(&self) -> Option<Step> {
        let mut fp = self.fp.lock().expect("Error locking fp - Poisoned?");

        assert!(self.mini_batch_size < std::u64::MAX, "mini_batch_size is too large - Must be less than u64::MAX");

        let mut cumulative_size: u64 = 0;
        let mut step = Step {
            n_seq: 0,
            seqs: vec![],
            a: mm128_v {
                n: 0,
                m: 0,
                a: std::ptr::null_mut(),
            },
        };

        while cumulative_size < self.mini_batch_size {
            let seq = fp.next();
            if seq.is_none() {
                break;
            }

            let mut total_seqs = self.total_seqs.lock().expect("Error locking total_seqs - Poisoned?");

            let seq = seq.unwrap().unwrap();
            let seq = ntseq1 {
                l_seq: seq.seq().len() as u128,
                rid: *total_seqs as u32,
                name: CString::new(seq.id()).expect("Error converting id to CString"),
                seq: CString::new(seq.seq()).expect("Error converting seq to CString"),
                qual: seq.qual().map(|q| CString::new(q)).unwrap_or(CString::new("")).unwrap(),
                comment: seq.qual().map(|q| CString::new(q)).unwrap_or(CString::new("")).unwrap(),
            };
            
            *total_seqs += 1;
            cumulative_size += seq.l_seq as u64;

            step.seqs.push(seq);
        }

        if cumulative_size == 0 {
            return None;
        }

        step.n_seq = step.seqs.len() as i32;
        Some(step)
    }
}

#[repr(C)]
pub struct Step {
    n_seq: i32,           // Number of sequences
    seqs: Vec<ntseq1>,    // Use rust vec
    a: mm128_v,           // Minimizers
}

// Needletail version of mm_bseq1_t
// thus, ntseq1, get it?
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ntseq1 {
    pub l_seq: u128,
    pub rid: u32,
    pub name: CString,
    pub seq: CString,
    pub qual: CString,
    pub comment: CString,
}

// Impl into mm_bseq1_t
impl From<ntseq1> for mm_bseq1_t {
    fn from(seq: ntseq1) -> Self {
        mm_bseq1_t {
            l_seq: seq.l_seq as i32,
            rid: seq.rid as i32,
            name: seq.name.into_raw(),
            seq: seq.seq.into_raw(),
            qual: seq.qual.into_raw(),
            comment: seq.comment.into_raw(),
        }
    }
}

fn worker_pipeline(pipeline: Pipeline) {
    
    while let Some(mut batch) = pipeline.read_batch() {
        let mi = pipeline.mi.read().expect("Error locking mi for reading");

        for seq in &mut batch.seqs {
            unsafe {
                mm_sketch(
                    std::ptr::null_mut(),   // Memory pool, can be NULL
                    seq.seq.as_ptr(),     // Sequence data
                    seq.l_seq as i32,      // Sequence length
                    (*mi).w,                   // Window size
                    (*mi).k,                   // K-mer size
                    seq.rid,
                    (*mi).flag & MM_I_HPC,     // Flags
                    &mut batch.a,
                );
            }
        }
    }
}

// mi = mm_idx_gen(r->fp.seq, r->opt.w, r->opt.k, r->opt.bucket_bits, r->opt.flag, r->opt.mini_batch_size, n_threads, r->opt.batch_size);
fn mm_idx_gen_rust(input: impl AsRef<Path>, w: i32, k: i32, bucket_bits: i32, flag: i32, mini_batch_size: i32, n_threads: i32, batch_size: u64) -> *mut mm_idx_t {

    let mut fp = parse_fastx_file(input).expect("Error reading input file");

    let pipeline = Pipeline {
        mini_batch_size,
        batch_size,
        sum_len: 0,
        fp,
        mi: unsafe { mm_idx_init(w, k, bucket_bits, flag) },
    };

    let pipeline = Arc::new(Mutex::new(pipeline));

    // Create a thread pool
    // let num_threads = ...;
    // let pool = ThreadPool::new(num_threads);

    // Start the pipeline steps
    // Use channels or other synchronization methods to manage steps

    std::ptr::null_mut()
}
use std::sync::{Arc, Condvar, Mutex};
use std::sync::atomic::{AtomicI64, AtomicIsize, Ordering};
use std::thread;
use std::os::raw::{c_void, c_int, c_long};

#[no_mangle]
pub extern "C" fn kt_pipeline(
    n_threads: c_int,
    func: extern "C" fn(*mut c_void, c_int, *mut c_void) -> *mut c_void,
    shared_data: *mut c_void,
    n_steps: c_int,
) {
    println!("Hello from Rust!");
    let n_threads = n_threads as usize;
    let n_steps = n_steps as usize;
    let index = AtomicIsize::new(0);

    struct KtpWorker {
        step: usize,
        index: isize,
        data: *mut c_void,
    }

    struct KtpShared {
        shared: *mut c_void,
        func: extern "C" fn(*mut c_void, c_int, *mut c_void) -> *mut c_void,
        index: AtomicIsize,
        n_workers: usize,
        n_steps: usize,
        workers: Vec<KtpWorker>,
    }

    struct SharedData {
        shared: Mutex<KtpShared>,
        condvar: Condvar,
    }

    // Implement Send and Sync for raw pointers manually
    unsafe impl Send for KtpShared {}
    unsafe impl Sync for KtpShared {}
    unsafe impl Send for SharedData {}
    unsafe impl Sync for SharedData {}

    let workers = (0..n_threads)
        .map(|_| KtpWorker {
            step: 0,
            index: index.fetch_add(1, Ordering::SeqCst),
            data: std::ptr::null_mut(),
        })
        .collect::<Vec<_>>();

    let shared = Arc::new(SharedData {
        shared: Mutex::new(KtpShared {
            shared: shared_data,
            func: func,
            index: index,
            n_workers: n_threads,
            n_steps: n_steps,
            workers: workers,
        }),
        condvar: Condvar::new(),
    });

    let handles = (0..n_threads)
        .map(|thread_id| {
            let shared = shared.clone();
            thread::spawn(move || {
                loop {
                    // Lock mutex
                    let mut guard = shared.shared.lock().unwrap();
                    loop {
                        // Test whether we can proceed
                        let can_proceed = guard.workers.iter().enumerate().all(|(i, other_w)| {
                            if i == thread_id {
                                true
                            } else {
                                let w = &guard.workers[thread_id];
                                !(other_w.step <= w.step && other_w.index < w.index)
                            }
                        });
                        if can_proceed {
                            break;
                        }
                        guard = shared.condvar.wait(guard).unwrap();
                    }

                    // Prepare data for calling func
                    let func = guard.func;
                    let shared_ptr = guard.shared;
                    let step = guard.workers[thread_id].step;
                    let input_data = if step == 0 {
                        std::ptr::null_mut()
                    } else {
                        guard.workers[thread_id].data
                    };
                    drop(guard); // Release the mutex before calling the function

                    // Call func outside the mutex to avoid deadlocks
                    let data = unsafe {
                        func(shared_ptr, step as c_int, input_data)
                    };

                    // Lock mutex to update state
                    let mut guard = shared.shared.lock().unwrap();
                    let w_step = guard.workers[thread_id].step;
                    let n_steps = guard.n_steps;
                    // let w = &mut guard.workers[thread_id];
                    guard.workers[thread_id].data = data;
                    if w_step == n_steps - 1 || !guard.workers[thread_id].data.is_null() {
                        guard.workers[thread_id].step = (guard.workers[thread_id].step + 1) % guard.n_steps;
                    } else {
                        guard.workers[thread_id].step = guard.n_steps;
                    }
                    if guard.workers[thread_id].step == 0 {
                        guard.workers[thread_id].index = guard.index.fetch_add(1, Ordering::SeqCst);
                    }
                    shared.condvar.notify_all();

                    // Check if we should exit
                    if guard.workers[thread_id].step >= guard.n_steps {
                        break;
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        handle.join().unwrap();
    }
}

// Wrapper that encapsulates function pointer and ensures thread-safety
struct ThreadSafeFunc {
    func: *const c_void,
    _marker: std::marker::PhantomData<*const ()>,
}

unsafe impl Send for ThreadSafeFunc {}
unsafe impl Sync for ThreadSafeFunc {}

impl ThreadSafeFunc {
    fn new(func: *const c_void) -> Self {
        ThreadSafeFunc {
            func,
            _marker: std::marker::PhantomData,
        }
    }

    unsafe fn call(&self, data: *mut c_void, index: c_long, thread_id: c_int) {
        let func: extern "C" fn(*mut c_void, c_long, c_int) = 
            std::mem::transmute(self.func);
        func(data, index, thread_id)
    }
}


// Previous attempt. Keeping
/*
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

fn kroundup32<T>(x: T) -> T
where
    T: Copy + std::ops::Sub<Output = T> + std::ops::Add<Output = T> + std::ops::Shr<usize, Output = T> + std::ops::BitOr<Output = T> + From<u8>,
{
    let mut x = x;
    x = x - T::from(1);
    x = x | (x >> 1);
    x = x | (x >> 2);
    x = x | (x >> 4);
    x = x | (x >> 8);
    x = x | (x >> 16);
    if std::mem::size_of::<T>() > 4 {
        x = x | (x >> 32); // For 64-bit types
    }
    x + T::from(1)
}

fn kroundup64<T>(x: T) -> T
where
    T: Copy
        + std::ops::Sub<Output = T>
        + std::ops::Add<Output = T>
        + std::ops::Shr<usize, Output = T>
        + std::ops::BitOr<Output = T>
        + From<u8>,
{
    let mut x = x;
    x = x - T::from(1); // Decrement x
    x = x | (x >> 1);
    x = x | (x >> 2);
    x = x | (x >> 4);
    x = x | (x >> 8);
    x = x | (x >> 16);
    if std::mem::size_of::<T>() > 4 {
        x = x | (x >> 32); // Handle 64-bit values
    }
    x + T::from(1) // Increment x
}


const SEQ_NT4_TABLE: [u8; 256] = [
    0, 1, 2, 3,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 0, 4, 1,  4, 4, 4, 2,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  3, 3, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 0, 4, 1,  4, 4, 4, 2,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  3, 3, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,
	4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4,  4, 4, 4, 4
];

unsafe fn mm_seq4_set(s: *mut u32, i: usize, c: u32) {
    let index = i >> 3; // Determine the array index
    let shift = ((i & 7) << 2) as u32; // Calculate the shift
    *s.add(index) |= c << shift; // Update the value at the calculated index
}


fn idx_add_sequences(pipeline: &Pipeline) {
    while let Some(mut batch) = pipeline.read_batch() {
        {
            let sum_len = pipeline.sum_len.lock().expect("Error locking sum_len");
            if *sum_len > pipeline.batch_size {
                return;
            }
        }

        let mi_lock = pipeline.mi.write().expect("Error locking mi for writing");
        let mi = unsafe { &mut **mi_lock };

        // old_m = p->mi->n_seq, m = p->mi->n_seq + s->n_seq;
        let old_m = mi.n_seq;
        let m = mi.n_seq + batch.n_seq as u32;
        let old_m_rounded = kroundup32(old_m);
        let m_rounded = kroundup32(m);

        if old_m_rounded != m_rounded {
            unsafe {
                let km = mi.km;
                let ptr = mi.seq as *mut ::std::os::raw::c_void;
                mi.seq = krealloc(
                    km, 
                    ptr,
                    (m_rounded as usize) * std::mem::size_of::<mm_idx_seq_t>(),
                ) as *mut mm_idx_seq_t;
            }
        }

        if mi.flag & MM_I_NO_SEQ as i32 == 0 {
            let sum_len: u64 = batch.seqs.iter().map(|seq| seq.l_seq as u64).sum();
            let mut pipeline_sum_len = pipeline.sum_len.lock().expect("Error locking sum_len");

            let old_max_len = kroundup64((*pipeline_sum_len + 7) / 8);
            let max_len = kroundup64((*pipeline_sum_len + sum_len + 7) / 8);

            if old_max_len != max_len {
                unsafe {
                    mi.S = realloc(
                        mi.S as *mut ::std::os::raw::c_void,
                        (max_len * 4) as u64,
                    ) as *mut u32;
                    std::ptr::write_bytes(
                        mi.S.add(old_max_len as usize),
                        0,
                        ((max_len - old_max_len) * 4) as usize,
                    );
                }
            }
        }

        // Now process each sequence
        for s in batch.seqs {
            let seq_ptr = unsafe { mi.seq.add(mi.n_seq as usize) };
            let seq = unsafe { &mut *seq_ptr };

            if mi.flag & MM_I_NO_NAME as i32 == 0 {
                let name_len = s.name.as_bytes_with_nul().len();
                unsafe {
                    seq.name = kmalloc(mi.km, name_len) as *mut std::ffi::c_char;
                    std::ptr::copy_nonoverlapping(
                        s.name.as_ptr(),
                        seq.name,
                        name_len,
                    );
                }
            } else {
                seq.name = std::ptr::null_mut();
            }

            seq.len = s.l_seq as u32;
            {
                let mut pipeline_sum_len = pipeline.sum_len.lock().expect("Error locking sum_len");
                seq.offset = *pipeline_sum_len;
            }
            seq.is_alt = 0;

            if mi.flag & MM_I_NO_SEQ as i32 == 0 {
                for j in 0..seq.len {
                    let o = {
                        let mut pipeline_sum_len = pipeline.sum_len.lock().expect("Error locking sum_len");
                        *pipeline_sum_len + j as u64
                    };
                    let c = SEQ_NT4_TABLE[s.seq.as_bytes()[j as usize] as usize];
                    unsafe { mm_seq4_set(mi.S, o as usize, c.into()) };
                }
            }

            {
                let mut pipeline_sum_len = pipeline.sum_len.lock().expect("Error locking sum_len");
                *pipeline_sum_len += seq.len as u64;
            }

            let s = mm_bseq1_t::from(s);
            s.rid = mi.n_seq as u32;
            mi.n_seq += 1;
        }
    }
}


// mi = mm_idx_gen(r->fp.seq, r->opt.w, r->opt.k, r->opt.bucket_bits, r->opt.flag, r->opt.mini_batch_size, n_threads, r->opt.batch_size);
fn mm_idx_gen_rust(input: impl AsRef<Path>, w: i32, k: i32, bucket_bits: i32, flag: i32, mini_batch_size: u64, n_threads: i32, batch_size: u64) -> *mut mm_idx_t {

    let fp = parse_fastx_file(input).expect("Error reading input file");

    let pipeline = Pipeline {
        mini_batch_size,
        batch_size,
        sum_len: Arc::new(Mutex::new(0)),
        fp: Arc::new(Mutex::new(fp)),
        mi: Arc::new(RwLock::new(unsafe { mm_idx_init(w, k, bucket_bits, flag) })),
        total_seqs: Arc::new(Mutex::new(0)),
        
    };

    let pipeline = Arc::new(Mutex::new(pipeline));

    // Create a thread pool
    // let num_threads = ...;
    // let pool = ThreadPool::new(num_threads);

    // Start the pipeline steps
    // Use channels or other synchronization methods to manage steps

    // Run single threaded for debugging todo
    idx_add_sequences(pipeline.lock().expect("Error locking pipeline for reading").clone());

    // Destruct!
    // No way this is safe
    let mut pipeline = pipeline.lock().expect("Error locking pipeline for reading");
    let mi = pipeline.mi.write().expect("Error locking mi for reading");
    let mi = *mi;
    let mi = mi.clone();
    mi
   
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn create_index() {
        let input = "../test_data/MT-human.fa";
        let w = 10;
        let k = 15;
        let bucket_bits = 14;
        let flag = 0;
        let mini_batch_size = 1000;
        let n_threads = 1;
        let batch_size = 1000;

        let idx = mm_idx_gen_rust(input, w, k, bucket_bits, flag, mini_batch_size, n_threads, batch_size);
        assert!(!idx.is_null());
    }

} 
*/
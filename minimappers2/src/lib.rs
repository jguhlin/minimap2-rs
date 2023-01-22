use std::num::NonZeroI32;
use std::sync::{Arc, Mutex};

// use mimalloc::MiMalloc;
use crossbeam::queue::ArrayQueue;
use fffx::{Fasta, Fastq};
use minimap2::*;
use minimap2_sys::{mm_set_opt, MM_F_CIGAR};
use polars::{df, prelude::*};
use pyo3::prelude::*;
use pyo3_polars::{error::PyPolarsErr, PyDataFrame};

// #[global_allocator]
// static GLOBAL: MiMalloc = MiMalloc;

mod multithreading;

use multithreading::*;

/// Sequence class for use with minimappers2
#[pyclass]
#[derive(Default, Debug, Clone)]
pub struct Sequence {
    pub id: String,
    pub sequence: Vec<u8>,
}

#[pymethods]
impl Sequence {
    /// Create a new Sequence
    #[new]
    fn new(id: &str, sequence: &str) -> Self {
        Sequence {
            id: id.to_string(),
            sequence: sequence.as_bytes().to_vec(),
        }
    }
}

/// Wrapper around minimap2::Aligner
#[pyclass]
pub struct Aligner {
    pub aligner: minimap2::Aligner,
}

unsafe impl Send for Aligner {}

#[pymethods]
impl Aligner {
    // Mapping functions
    /// Map a single sequence
    fn map1(&self, seq: &Sequence) -> PyResult<PyDataFrame> {
        let mut mappings = Mappings::default();

        let results = self
            .aligner
            .map(&seq.sequence, true, true, None, None)
            .unwrap();
        results.into_iter().for_each(|mut r| {
            r.query_name = Some(seq.id.clone());
            mappings.push(r)
        });

        Ok(PyDataFrame(mappings.to_df().unwrap()))
    }

    /// Map multiple sequences - Multithreaded
    fn map(&self, py: Python<'_>, seqs: Vec<Sequence>) -> PyResult<PyDataFrame> {
        // Get GIL and allow_threads
        let gil = Python::acquire_gil();
        let py = gil.python();

        // If single threaded, do not open a new thread...
        if self.aligner.threads == 1 {
            let mut mappings = Mappings::default();

            for seq in seqs {
                let results = self
                    .aligner
                    .map(&seq.sequence, true, true, None, None)
                    .unwrap();
                results.into_iter().for_each(|mut r| {
                    r.query_name = Some(seq.id.clone());
                    mappings.push(r)
                });
            }

            return Ok(PyDataFrame(mappings.to_df().unwrap()));
        }

        let work_queue = Arc::new(Mutex::new(seqs));
        let results_queue = Arc::new(ArrayQueue::<WorkQueue<Vec<Mapping>>>::new(128));
        let mut thread_handles = Vec::new();
        for i in 0..(self.aligner.threads - 1) {
            let work_queue = Arc::clone(&work_queue);
            let results_queue = Arc::clone(&results_queue);

            let mut aligner = self.aligner.clone();

            let handle = std::thread::spawn(move || loop {
                let backoff = crossbeam::utils::Backoff::new();
                let work = work_queue.lock().unwrap().pop();

                match work {
                    Some(sequence) => {
                        let mut result = aligner
                            .map(&sequence.sequence, true, true, None, None)
                            .expect("Unable to align");

                        result.iter_mut().for_each(|mut r| {
                            r.query_name = Some(sequence.id.clone());
                        });

                        results_queue.push(WorkQueue::Work(result));
                    }
                    None => {
                        // Means the work queue is empty...
                        results_queue.push(WorkQueue::Done);
                        break;
                    }
                }
            });
            thread_handles.push(handle);
        }

        let mut mappings = Mappings::default();
        let mut finished_count = 0;

        loop {
            py.check_signals()?;
            let result = results_queue.pop();
            match result {
                Some(WorkQueue::Work(result)) => {
                    result.into_iter().for_each(|r| mappings.push(r));
                }
                Some(WorkQueue::Done) => {
                    finished_count += 1;
                    if finished_count == (self.aligner.threads - 1) {
                        break;
                    }
                }
                None => {
                    // Probably should be backoff, but let's try this for now...
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }

        for handle in thread_handles {
            handle.join().unwrap();
        }

        Ok(PyDataFrame(mappings.to_df().unwrap()))
    }

    // Builder functions
    /// Returns an unconfigured Aligner
    #[new]
    fn new() -> Self {
        Aligner {
            aligner: minimap2::Aligner::builder(),
        }
    }

    /// Set the number of threads for minimap2 to use to build index and perform mapping
    fn threads(&mut self, threads: usize) {
        self.aligner.threads = threads;
    }

    /// Build the minimap2 index
    fn index(&mut self, index: &str) {
        self.aligner.set_index(index, None);
    }

    /// Index and save index to output
    fn index_and_save(&mut self, index: &str, output: &str) {
        self.aligner.set_index(index, Some(output));
    }

    /// Enable CIGAR strings
    fn cigar(&mut self) {
        self.aligner.mapopt.flag |= MM_F_CIGAR as i64;
    }

    // Convenience Functions, at the bottom, because it pollutes the namespace
    /// Configure Aligner for ONT reads
    fn map_ont(&mut self) {
        self.preset(Preset::MapOnt);
    }

    /// Configure Aligner for PacBio HIFI reads
    fn map_hifi(&mut self) {
        self.preset(Preset::MapHifi);
    }

    /// Configure aligner for AvaOnt
    fn ava_ont(&mut self) {
        self.preset(Preset::AvaOnt);
    }

    /// Configure aligner for Map10k
    fn map_10k(&mut self) {
        self.preset(Preset::Map10k);
    }

    /// Configure aligner for AvaPb
    fn ava_pb(&mut self) {
        self.preset(Preset::AvaPb);
    }

    /// Configure aligner for Asm
    fn asm(&mut self) {
        self.preset(Preset::Asm);
    }

    /// Configure Aligner for Asm5
    fn asm5(&mut self) {
        self.preset(Preset::Asm5);
    }

    /// Configure Aligner for Asm10
    fn asm10(&mut self) {
        self.preset(Preset::Asm10);
    }

    /// Configure Aligner for Asm20
    fn asm20(&mut self) {
        self.preset(Preset::Asm20);
    }

    /// Configure Aligner for Short
    fn short(&mut self) {
        self.preset(Preset::Short);
    }

    /// Configure Aligner for Sr
    fn sr(&mut self) {
        self.preset(Preset::Sr);
    }

    /// Configure Aligner for Splice
    fn splice(&mut self) {
        self.preset(Preset::Splice);
    }

    /// Configure Aligner for Cdna
    fn cdna(&mut self) {
        self.preset(Preset::Cdna);
    }
}

impl Aligner {
    /// Create an aligner using a preset.
    fn preset(&mut self, preset: Preset) {
        let mut idxopt = IdxOpt::default();
        let mut mapopt = MapOpt::default();

        unsafe {
            // Set preset
            mm_set_opt(preset.into(), &mut idxopt, &mut mapopt)
        };

        self.aligner.idxopt = idxopt;
        self.aligner.mapopt = mapopt;
    }
}

/*
TODO - Destroy index when aligner is dropped or when new index is created
impl Drop for Aligner {
    fn drop(&mut self) {

  }
} */

/// Return a MapOnt aligner
#[pyfunction]
fn map_ont() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.map_ont();
    Ok(aligner)
}

/// Return a MapHifi aligner
#[pyfunction]
fn map_hifi() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.map_hifi();
    Ok(aligner)
}

/// Return a AvaOnt aligner
#[pyfunction]
fn ava_ont() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.ava_ont();
    Ok(aligner)
}

/// Return a Map10k aligner
#[pyfunction]
fn map_10k() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.map_10k();
    Ok(aligner)
}

/// Return a AvaPb aligner
#[pyfunction]
fn ava_pb() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.ava_pb();
    Ok(aligner)
}

/// Return a Asm aligner
#[pyfunction]
fn asm() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.asm();
    Ok(aligner)
}

/// Return a Asm5 aligner
#[pyfunction]
fn asm5() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.asm5();
    Ok(aligner)
}

/// Return a Asm10 aligner
#[pyfunction]
fn asm10() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.asm10();
    Ok(aligner)
}

/// Return a Asm20 aligner
#[pyfunction]
fn asm20() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.asm20();
    Ok(aligner)
}

/// Return a Short aligner
#[pyfunction]
fn short() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.short();
    Ok(aligner)
}

/// Return a Sr aligner
#[pyfunction]
fn sr() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.sr();
    Ok(aligner)
}

/// Return a Splice aligner
#[pyfunction]
fn splice() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.splice();
    Ok(aligner)
}

/// Return a Cdna aligner
#[pyfunction]
fn cdna() -> PyResult<Aligner> {
    let mut aligner = Aligner::new();
    aligner.cdna();
    Ok(aligner)
}

/// This module is implemented in Rust.
#[pymodule]
fn minimappers2(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<Sequence>()?;
    m.add_class::<Aligner>()?;
    m.add_function(wrap_pyfunction!(map_ont, m)?)?;
    m.add_function(wrap_pyfunction!(map_hifi, m)?)?;
    m.add_function(wrap_pyfunction!(ava_ont, m)?)?;
    m.add_function(wrap_pyfunction!(map_10k, m)?)?;
    m.add_function(wrap_pyfunction!(ava_pb, m)?)?;
    m.add_function(wrap_pyfunction!(asm, m)?)?;
    m.add_function(wrap_pyfunction!(asm5, m)?)?;
    m.add_function(wrap_pyfunction!(asm10, m)?)?;
    m.add_function(wrap_pyfunction!(asm20, m)?)?;
    m.add_function(wrap_pyfunction!(short, m)?)?;
    m.add_function(wrap_pyfunction!(sr, m)?)?;
    m.add_function(wrap_pyfunction!(splice, m)?)?;
    m.add_function(wrap_pyfunction!(cdna, m)?)?;
    Ok(())
}

/// Mapping results
#[derive(Default)]
struct Mappings {
    pub query_name: Vec<Option<String>>,
    pub query_len: Vec<Option<NonZeroI32>>,
    pub query_start: Vec<i32>,
    pub query_end: Vec<i32>,
    pub strand: Vec<Strand>,
    pub target_name: Vec<Option<String>>,
    pub target_len: Vec<i32>,
    pub target_start: Vec<i32>,
    pub target_end: Vec<i32>,
    pub match_len: Vec<i32>,
    pub block_len: Vec<i32>,
    pub mapq: Vec<u32>,
    pub is_primary: Vec<bool>,
    pub alignment: Vec<Option<Alignment>>,
}

impl Mappings {
    pub fn push(&mut self, other: minimap2::Mapping) {
        self.query_name.push(other.query_name);
        self.query_len.push(other.query_len);
        self.query_start.push(other.query_start);
        self.query_end.push(other.query_end);
        self.strand.push(other.strand);
        self.target_name.push(other.target_name);
        self.target_len.push(other.target_len);
        self.target_start.push(other.target_start);
        self.target_end.push(other.target_end);
        self.match_len.push(other.match_len);
        self.block_len.push(other.block_len);
        self.mapq.push(other.mapq);
        self.is_primary.push(other.is_primary);
        self.alignment.push(other.alignment);
    }

    pub fn to_df(self) -> Result<DataFrame, PolarsError> {
        // Convert strand to string + or -
        let strand: Vec<String> = self.strand.iter().map(|x| x.to_string()).collect();

        // Convert query len to Option<u32>
        // let query_len: Vec<Option<u32>> = self.query_len.iter().map(|x| x.map(|y| y as u32.into())).collect();
        let query_len: Vec<Option<u32>> = self
            .query_len
            .iter()
            .map(|x| match x {
                Some(y) => Some(y.get() as u32),
                None => None,
            })
            .collect();

        let nm: Vec<Option<i32>> = self
            .alignment
            .iter()
            .map(|x| match x {
                // These are ugly but it's early in the morning...
                Some(y) => Some(y.nm),
                None => None,
            })
            .collect();

        let cigar: Vec<Option<Vec<(u32, u8)>>> = self
            .alignment
            .iter()
            .map(|x| match x {
                Some(y) => match &y.cigar {
                    Some(z) => Some(z.clone()),
                    None => None,
                },
                None => None,
            })
            .collect();

        let cigar_str: Vec<Option<String>> = self
            .alignment
            .iter()
            .map(|x| match x {
                Some(y) => match &y.cigar_str {
                    Some(z) => Some(z.clone()),
                    None => None,
                },
                None => None,
            })
            .collect();

        let md: Vec<Option<String>> = self
            .alignment
            .iter()
            .map(|x| match x {
                Some(y) => match &y.md {
                    Some(z) => Some(z.clone()),
                    None => None,
                },
                None => None,
            })
            .collect();

        let cs: Vec<Option<String>> = self
            .alignment
            .iter()
            .map(|x| match x {
                Some(y) => match &y.cs {
                    Some(z) => Some(z.clone()),
                    None => None,
                },
                None => None,
            })
            .collect();

        let query_name = Series::new("query_name", self.query_name);
        let query_len = Series::new("query_len", query_len);
        let query_start = Series::new("query_start", self.query_start);
        let query_end = Series::new("query_end", self.query_end);
        let strand = Series::new("strand", strand);
        let target_name = Series::new("target_name", self.target_name);
        let target_len = Series::new("target_len", self.target_len);
        let target_start = Series::new("target_start", self.target_start);
        let target_end = Series::new("target_end", self.target_end);
        let match_len = Series::new("match_len", self.match_len);
        let block_len = Series::new("block_len", self.block_len);
        let mapq = Series::new("mapq", self.mapq);
        let is_primary = Series::new("is_primary", self.is_primary);
        let nm = Series::new("nm", nm);
        // let cigar = Series::new("cigar", cigar);
        let cigar_str = Series::new("cigar_str", cigar_str);
        let md = Series::new("md", md);
        let cs = Series::new("cs", cs);

        DataFrame::new(vec![
            query_name,
            query_len,
            query_start,
            query_end,
            strand,
            target_name,
            target_len,
            target_start,
            target_end,
            match_len,
            block_len,
            mapq,
            is_primary,
            nm,
            // cigar,
            cigar_str,
            md,
            cs,
        ])
    }
}

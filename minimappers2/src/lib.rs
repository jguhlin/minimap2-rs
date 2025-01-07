use std::num::NonZeroI32;
use std::sync::{Arc, Mutex};

use crossbeam::queue::ArrayQueue;
use mimalloc::MiMalloc;
use minimap2::*;

use polars::{df, prelude::*};
use pyo3::prelude::*;
use pyo3_polars::{error::PyPolarsErr, PyDataFrame};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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

#[derive(Clone)]
#[pyclass]
pub struct AlignerBuilder {
    pub builder: minimap2::Aligner<PresetSet>,
}

impl AlignerBuilder {
    fn preset(preset: Preset) -> Self {
        let builder = minimap2::Aligner::builder();
        let builder = builder.preset(preset);
        AlignerBuilder { builder }
    }
}

#[pymethods]
impl AlignerBuilder {
    #[staticmethod]
    fn lrhq() -> Self {
        AlignerBuilder::preset(Preset::LrHq)
    }

    #[staticmethod]
    fn lrhqae() -> Self {
        AlignerBuilder::preset(Preset::LrHqae)
    }

    /// Configure Aligner for Splice
    #[staticmethod]
    fn splice() -> Self {
        AlignerBuilder::preset(Preset::Splice)
    }

    #[staticmethod]
    fn splicehq() -> Self {
        AlignerBuilder::preset(Preset::SpliceHq)
    }
    
    /// Configure aligner for Asm
    #[staticmethod]
    fn asm() -> Self {
        AlignerBuilder::preset(Preset::Asm)
    }

    /// Configure Aligner for Asm5
    #[staticmethod]
    fn asm5() -> Self {
        AlignerBuilder::preset(Preset::Asm5)
    }

    /// Configure Aligner for Asm10
    #[staticmethod]
    fn asm10() -> Self {
        AlignerBuilder::preset(Preset::Asm10)
    }

    /// Configure Aligner for Asm20
    #[staticmethod]
    fn asm20() -> Self {
        AlignerBuilder::preset(Preset::Asm20)
    }

    // Convenience Functions, at the bottom, because it pollutes the namespace
    /// Configure Aligner for ONT reads
    #[staticmethod]
    fn map_ont() -> Self {
        AlignerBuilder::preset(Preset::MapOnt)
    }

    /// Configure Aligner for PacBio HIFI reads
    #[staticmethod]
    fn map_hifi() -> Self {
        AlignerBuilder::preset(Preset::MapHifi)
    }

    /// Configure aligner for AvaOnt
    #[staticmethod]
    fn ava_ont() -> Self {
        AlignerBuilder::preset(Preset::AvaOnt)
    }

    /// Configure aligner for Map10k
    #[staticmethod]
    fn map_10k() -> Self {
        AlignerBuilder::preset(Preset::Map10k)
    }

    /// Configure aligner for AvaPb
    #[staticmethod]
    fn ava_pb() -> Self {
        AlignerBuilder::preset(Preset::AvaPb)
    }

    /// Configure Aligner for Short
    #[staticmethod]
    fn short() -> Self {
        AlignerBuilder::preset(Preset::Short)
    }

    /// Configure Aligner for Sr
    #[staticmethod]
    fn sr() -> Self {
        AlignerBuilder::preset(Preset::Sr)
    }

    /// Configure Aligner for Cdna
    #[staticmethod]
    fn cdna() -> Self {
        AlignerBuilder::preset(Preset::Cdna)
    }

    // Configuration options
    /// Set the number of threads for minimap2 to use to build index and perform mapping
    fn index_threads(&mut self, threads: usize) {
        self.builder.threads = threads;
    }

    /// Build the minimap2 index
    fn index(&self, index: &str) -> PyResult<Aligner> {
        let aligner = self.clone();
        let aligner = aligner.builder.set_index(index, None).expect("Unable to build or load index");

        Ok(Aligner { aligner })
    }

    /// Index and save index for later reuse (pass in as index)
    /// Minimap2 indices are typically stored with the extension .mmi
    fn index_and_save(&self, index: &str, output: &str) -> Aligner {
        let aligner = self.clone();
        let aligner = aligner.builder.set_index(index, Some(output)).expect("Unable to build or save index");
        Aligner {
            aligner
        }
    }
    
    /// Enable CIGAR strings
    fn cigar(&mut self) {
        // Builder pattern doesn't work great with Python, so do it manually here...
        assert!((self.builder.mapopt.flag & ffi::MM_F_CIGAR as i64) == 0);
        self.builder.mapopt.flag |= ffi::MM_F_CIGAR as i64 | ffi::MM_F_OUT_CS as i64;
    }
}

/// Wrapper around minimap2::Aligner
#[pyclass]
pub struct Aligner {
    pub aligner: minimap2::Aligner<Built>,
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
            .map(&seq.sequence, true, true, None, None, Some(&seq.id.as_bytes()))
            .unwrap();
        results.into_iter().for_each(|r| {
            mappings.push(r)
        });

        Ok(PyDataFrame(mappings.to_df().unwrap()))
    }

    /// Map multiple sequences - Multithreaded
    fn map(&self, py: Python<'_>, seqs: Vec<Sequence>) -> PyResult<PyDataFrame> {
        // If single threaded, do not open a new thread...
        if self.aligner.threads == 1 {
            let mut mappings = Mappings::default();

            for seq in seqs {
                let results = self
                    .aligner
                    .map(&seq.sequence, true, true, None, None, Some(&seq.id.as_bytes()))
                    .unwrap();
                results.into_iter().for_each(|r| {
                    mappings.push(r)
                });
            }
            Ok(PyDataFrame(mappings.to_df().unwrap()))
        } else {
            let work_queue = Arc::new(Mutex::new(seqs));
            let results_queue = Arc::new(ArrayQueue::<WorkQueue<Vec<Mapping>>>::new(128));
            let mut thread_handles = Vec::new();
            for i in 0..(self.aligner.threads - 1) {
                let work_queue = Arc::clone(&work_queue);
                let results_queue = Arc::clone(&results_queue);

                let aligner = self.aligner.clone();

                let handle = std::thread::spawn(move || loop {
                    let work = work_queue.lock().unwrap().pop();

                    match work {
                        Some(sequence) => {
                            let mut result = aligner
                                .map(&sequence.sequence, true, true, None, None, Some(&sequence.id.as_bytes()))
                                .expect("Unable to align");

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
    }

    fn set_threads(&mut self, threads: usize) {
        self.aligner.threads = threads;
    }
    
}

/// This module is implemented in Rust.
#[pymodule]
fn minimappers2(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Sequence>()?;
    m.add_class::<Aligner>()?;
    m.add_class::<AlignerBuilder>()?;
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
        let query_name = match other.query_name {
            Some(x) => Some(x.to_string()),
            None => None,
        };

        let target_name = match other.target_name {
            Some(x) => Some(x.to_string()),
            None => None,
        };

        self.query_name.push(query_name);
        self.query_len.push(other.query_len);
        self.query_start.push(other.query_start);
        self.query_end.push(other.query_end);
        self.strand.push(other.strand);
        self.target_name.push(target_name);
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

        let query_name = Series::new("query_name".into(), self.query_name);
        let query_len = Series::new("query_len".into(), query_len);
        let query_start = Series::new("query_start".into(), self.query_start);
        let query_end = Series::new("query_end".into(), self.query_end);
        let strand = Series::new("strand".into(), strand);
        let target_name = Series::new("target_name".into(), self.target_name);
        let target_len = Series::new("target_len".into(), self.target_len);
        let target_start = Series::new("target_start".into(), self.target_start);
        let target_end = Series::new("target_end".into(), self.target_end);
        let match_len = Series::new("match_len".into(), self.match_len);
        let block_len = Series::new("block_len".into(), self.block_len);
        let mapq = Series::new("mapq".into(), self.mapq);
        let is_primary = Series::new("is_primary".into(), self.is_primary);
        let nm = Series::new("nm".into(), nm);
        // let cigar = Series::new("cigar", cigar);
        let cigar_str = Series::new("cigar_str".into(), cigar_str);
        let md = Series::new("md".into(), md);
        let cs = Series::new("cs".into(), cs);

        DataFrame::new(vec![
            query_name.into(),
            query_len.into(),
            query_start.into(),
            query_end.into(),
            strand.into(),
            target_name.into(),
            target_len.into(),
            target_start.into(),
            target_end.into(),
            match_len.into(),
            block_len.into(),
            mapq.into(),
            is_primary.into(),
            nm.into(),
            // cigar,
            cigar_str.into(),
            md.into(),
            cs.into(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_structs() {
        // Test seq building - disabled for now
        /*
        let seq = Sequence { id: "test".to_string(), sequence: "ACGT".to_string() };

        // Test default
        let seq = Sequence::default();

        // Test clone
        let seq = seq.clone();

        // Test debug derive
        println!("{:#?}", seq);

        // Test py new fn
        let seq = Sequence::new("test", "ACGT");
        */

        // Test Mappings struct
        // Test default and push fn
        let mut mappings = Mappings::default();

        // Disabled until crates update
        // let mapping = minimap2::Mapping::default();
        // mappings.push(mapping);

        // Test to df - need to figure out pyo3 prob
        // let df = mappings.to_df().unwrap();
    }
}

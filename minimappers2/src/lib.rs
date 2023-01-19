use pyo3::prelude::*;
use minimap2::*;
use minimap2_sys::{mm_set_opt, MM_F_CIGAR};

// Reference: https://github.com/pola-rs/pyo3-polars

/// Wrapper around minimap2::Aligner
#[pyclass]
pub struct Aligner {
    pub aligner: minimap2::Aligner,
}

unsafe impl Send for Aligner {}

#[pymethods]
impl Aligner {

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

    fn map(&self, seq: &str) -> PyResult<Vec<minimap2::Alignment>> {
        let mut alignments = Vec::new();
        self.aligner.map(seq, |alignment| {
            alignments.push(alignment);
        });
        Ok(alignments)
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
fn minimappers(py: Python<'_>, m: &PyModule) -> PyResult<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

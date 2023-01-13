use crate::{Aligner, Mapping, Strand};
use core::ffi;
use minimap2_sys::mm_idx_t;
use rust_htslib::bam::record::{Cigar, CigarString};
use rust_htslib::bam::{Header, Record};

pub fn mapping_to_record(
    mapping: Option<&Mapping>,
    seq: &[u8],
    header: Header,
    qual: Option<&[u8]>,
    query_name: Option<&[u8]>,
) -> Record {
    let mut rec = Record::new();
    let qname = query_name.unwrap_or(b"query");
    // FIXFIX: there's probably a better way of setting a default value
    // for the quality string
    let qual = match qual {
        Some(q) => Vec::from(q),
        None => {
            let q = vec![255; seq.len()];
            q
        }
    };

    let cigar: Option<CigarString> = mapping
        .and_then(|m| m.alignment.clone()) // FIXFIX: we probably don't need a clone here
        .and_then(|a| a.cigar)
        .map(|c| cigar_to_cigarstr(&c));

    rec.set(qname, cigar.as_ref(), seq, &qual[..]);
    match mapping {
        Some(m) => {
            println!("Strand {m:?}");
            if m.strand == Strand::Reverse {
                println!("here");
                rec.set_reverse();
            }
            // TODO: set secondary/supplementary flags
            rec.set_pos(m.target_start as i64);
            rec.set_mapq(m.mapq as u8);
            rec.set_mpos(-1);
            // TODO: set tid from sequences listed in header
            rec.set_mtid(-1);
            rec.set_insert_size(0);
        }
        None => {
            rec.set_unmapped();
            rec.set_tid(-1);
            rec.set_pos(-1);
            rec.set_mapq(255);
            rec.set_mpos(-1);
            rec.set_mtid(-1);
            rec.set_insert_size(-1);
        }
    };
    // TODO: set AUX flags for cs/md if available
    rec
}

fn cigar_to_cigarstr(cigar: &Vec<(u32, u8)>) -> CigarString {
    let op_vec: Vec<Cigar> = cigar
        .to_owned()
        .iter()
        .map(|(len, op)| match op {
            0 => Cigar::Match(*len),
            1 => Cigar::Ins(*len),
            2 => Cigar::Del(*len),
            3 => Cigar::RefSkip(*len),
            4 => Cigar::SoftClip(*len),
            5 => Cigar::HardClip(*len),
            6 => Cigar::Pad(*len),
            7 => Cigar::Equal(*len),
            8 => Cigar::Diff(*len),
            _ => panic!("Unexpected cigar operation"),
        })
        .collect();
    CigarString(op_vec)
}

#[derive(Debug,PartialEq,Eq)]
pub struct SeqMetaData {
    pub name: String,
    pub length: u32,
    pub is_alt: bool,
}

#[derive(Debug)]
pub struct MMIndex {
    pub inner: mm_idx_t,
}

impl MMIndex {
    pub fn n_seq(&self) -> u32 {
        self.inner.n_seq
    }

    pub fn seqs(&self) -> Vec<SeqMetaData> {
        let mut seqs: Vec<SeqMetaData> = Vec::with_capacity(self.n_seq() as usize);
        for i in 0..self.n_seq() {
            let _seq = unsafe { *(self.inner.seq).offset(i as isize) };
            let c_str = unsafe { ffi::CStr::from_ptr(_seq.name) };
            let rust_str = c_str.to_str().unwrap().to_string();
            seqs.push(SeqMetaData {
                name: rust_str,
                length: _seq.len,
                is_alt: _seq.is_alt != 0,
            });
        }
        seqs
    }
}

impl From<Aligner> for MMIndex {
    fn from(aligner: Aligner) -> Self {
        MMIndex {
            inner: aligner.idx.unwrap(),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "htslib")]
mod tests {
    use super::*;

    #[test]
    fn test_index() {
        let aligner = Aligner::builder()
            .with_threads(1)
            .with_index("test_data/MT-human.fa", Some("test_data/MT-human.mmi"))
            .unwrap();

        let idx = MMIndex::from(aligner);

        let seqs = idx.seqs();

        assert_eq!(
            seqs,
            vec![SeqMetaData {
                name: "MT_human".to_string(),
                length: 16569u32,
                is_alt: false
            }]
        );

        //for i in 0..idx.n_seq {
        //    unsafe {
        //        println!("{:?}", *(idx.seq).offset(i as isize));
        //    }
        //}
    }
}

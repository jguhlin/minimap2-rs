//! Provides an interface to minimap2 that returns rust_htslib::Records
//!
//!
//! ```

//! use minimap2::Aligner;
//! use rust_htslib::bam::{Header, HeaderView};
//! use rust_htslib::bam::record::Aux;
//! let aligner = Aligner::builder()
//!     .with_index("test_data/genome.fa", None)
//!     .unwrap()
//!     .with_cigar();
//! let mut header = Header::new();
//! aligner.populate_header(&mut header);
//! let header_view = HeaderView::from_header(&header);
//!
//! let records = aligner
//!     .map_to_sam(
//!         b"TACGCCACACGGGCTACACTCTCGCCTTCTCGTCTCAACTACGAGATGGACTGTCGGCCTAGAGGATCTAACACGAGAAGTACTTGCCGGCAAGCCCTAA",
//!         Some(b"2222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222"),
//!         Some(b"read1"),
//!         &header_view, None, None)
//!     .unwrap();
//!
//! assert_eq!(records.len(), 1);
//! let record = records.first().unwrap();
//! assert_eq!((record.tid(), record.pos(), record.mapq()), (0, 180, 13));
//!
//! let nm = record.aux(b"NM").unwrap();
//! assert_eq!(nm, Aux::U8(5));
//!
//! // you can also map reads with no quality/name
//! let records = aligner
//!     .map_to_sam(
//!         b"TACGCCACACGGGCTACACTCTCGCCTTCTCGTCTCAACTACGAGATGGACTGTCGGCCTAGAGGATCTAACACGAGAAGTACTTGCCGGCAAGCCCTAA",
//!         None, None,  &header_view, None, None)
//!     .unwrap();
//!
//! assert_eq!(records.len(), 1);
//! let record = records.first().unwrap();
//! assert_eq!((record.tid(), record.pos(), record.mapq()), (0, 180, 13));
//! ```

use crate::{Aligner, Mapping, Strand, BUF};
use core::ffi;
use minimap2_sys as mm_ffi;
use rust_htslib::bam::header::HeaderRecord;
use rust_htslib::bam::record::{Cigar, CigarString};
use rust_htslib::bam::{Header, HeaderView, Record};
use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;
use std::ptr;

/// A wrapper around mm_bseq1_t
#[derive(Debug)]
pub struct Query {
    pub inner: mm_ffi::mm_bseq1_t,
}

impl Query {
    pub fn new(seq: &[u8], qual: Option<&[u8]>, name: Option<&[u8]>) -> Self {
        let l_seq = seq.len();
        assert!(l_seq > 0, "Empty sequence supplied");
        // clone into a CString
        let seq = CString::new(seq).unwrap().into_raw();
        let qual = match qual {
            Some(qual) => {
                assert_eq!(
                    l_seq,
                    qual.len(),
                    "Sequence and quality strings are different lenght"
                );
                CString::new(qual).unwrap().into_raw()
            }
            None => ptr::null_mut(),
        };
        let name = CString::new(name.unwrap_or(b"query")).unwrap();

        let inner = mm_ffi::mm_bseq1_t {
            l_seq: l_seq as i32,
            rid: 0, // TODO: pass a unique read id,
            name: name.into_raw(),
            seq,
            qual,
            comment: ptr::null_mut(), // TODO: pass SAM flags in comment
        };
        Query { inner }
    }

    pub fn as_unmapped_record(&self) -> Record {
        let mut rec = Record::new();

        let qname = unsafe { CStr::from_ptr(self.inner.name).to_bytes() };
        let seq = unsafe { CStr::from_ptr(self.inner.seq).to_bytes() };
        let qual = if self.inner.qual.is_null() {
            rec.set(qname, None, seq, &vec![255u8; seq.len()]);
        } else {
            rec.set(qname, None, seq, unsafe {
                CStr::from_ptr(self.inner.qual).to_bytes()
            });
        };
        rec.set_unmapped();
        rec.set_tid(-1);
        rec.set_pos(-1);
        rec.set_mapq(0);
        rec.set_mpos(-1);
        rec.set_mtid(-1);
        rec.set_insert_size(0);
        rec
    }
}

impl Aligner {
    pub fn populate_header(&self, header: &mut Header) {
        let mm_idx = MMIndex::from(self);
        for seq in mm_idx.seqs() {
            header.push_record(
                HeaderRecord::new(b"SQ")
                    .push_tag(b"SN", &seq.name)
                    .push_tag(b"LN", &seq.length),
            );
        }
    }

    pub fn map_to_sam(
        &self,
        seq: &[u8],
        qual: Option<&[u8]>,
        name: Option<&[u8]>,
        header: &HeaderView,
        max_frag_len: Option<usize>,
        extra_flags: Option<Vec<u64>>,
    ) -> Result<Vec<Record>, &'static str> {
        // Make sure index is set
        if !self.has_index() {
            return Err("No index");
        }

        let query = Query::new(seq, qual, name);
        // Number of results
        let mut n_regs: i32 = 0;
        let mut map_opt = self.mapopt.clone();

        // TODO: other flags to consider:
        // MM_F_NO_PRINT_2ND
        // MM_F_SAM_HIT_ONLY (though this seems to be the default?)
        //map_opt.flag |= mm_ffi::MM_F_OUT_SAM as i64;
        //map_opt.flag |= mm_ffi::MM_F_CIGAR as i64;

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
            //let km = unsafe { mm_ffi::mm_tbuf_get_km(buf.borrow_mut().buf) };

            let mm_reg = MaybeUninit::new(unsafe {
                mm_ffi::mm_map(
                    self.idx.as_ref().unwrap() as *const mm_ffi::mm_idx_t,
                    query.inner.l_seq,
                    query.inner.seq as *const i8,
                    &mut n_regs,
                    buf.borrow_mut().buf,
                    &map_opt,
                    query.inner.name,
                )
            });
            // FIXFIX: mm_map should return unmapped SAM records but it
            //  currently doesn't seem to work. To work around this we create the
            // record manually
            if (n_regs == 0) & ((map_opt.flag & mm_ffi::MM_F_SAM_HIT_ONLY as i64) == 0) {
                return vec![query.as_unmapped_record()];
            }

            let mut mappings = Vec::with_capacity(n_regs as usize);

            for i in 0..n_regs {
                let sam_str = unsafe {
                    let mut result: MaybeUninit<mm_ffi::kstring_t> = MaybeUninit::zeroed();
                    let reg_ptr = (*mm_reg.as_ptr()).offset(i as isize);
                    //    // println!("{:#?}", *reg_ptr);
                    let const_ptr = reg_ptr as *const mm_ffi::mm_reg1_t;
                    // TODO: use mm_write_sam3 t do the writing so that we can pass the map_opt flags
                    mm_ffi::mm_write_sam(
                        result.as_mut_ptr(),
                        self.idx.as_ref().unwrap() as *const mm_ffi::mm_idx_t,
                        &query.inner as *const mm_ffi::mm_bseq1_t,
                        const_ptr,
                        n_regs,
                        *mm_reg.as_ptr() as *const mm_ffi::mm_reg1_t,
                    );
                    //mm_ffi::mm_write_sam3(
                    //    result.as_mut_ptr(),
                    //    self.idx.as_ref().unwrap() as *const mm_ffi::mm_idx_t,
                    //    &read  as *const mm_ffi::mm_bseq1_t,
                    //    0, // seg_idx doesn't apply here (think it's a batch index)
                    //    i,
                    //    1, // only 1 segment
                    //    n_regs as *const i32,
                    //    &const_ptr,
                    //    km,
                    //    map_opt.flag,
                    //    0
                    //);
                    CStr::from_ptr((*result.as_ptr()).s)
                };
                let record = Record::from_sam(header, sam_str.to_bytes()).unwrap();
                mappings.push(record);
            }
            mappings
        });
        Ok(mappings)
    }
}

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
    let qual = qual.map_or_else(|| vec![255u8; seq.len()], Vec::from);

    let cigar: Option<CigarString> = mapping.and_then(|m| {
        m.alignment
            .as_ref()
            .and_then(|aln| aln.cigar.as_ref())
            .map(cigar_to_cigarstr)
    });

    rec.set(qname, cigar.as_ref(), seq, &qual[..]);
    match mapping {
        Some(m) => {
            if m.strand == Strand::Reverse {
                rec.set_reverse();
            }
            if !m.is_primary {
                rec.set_secondary();
            }
            if m.is_supplementary {
                rec.set_supplementary();
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

#[derive(Debug, PartialEq, Eq)]
pub struct SeqMetaData {
    pub name: String,
    pub length: u32,
    pub is_alt: bool,
}

#[derive(Debug)]
pub struct MMIndex {
    pub inner: mm_ffi::mm_idx_t,
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

    pub fn get_header(&self) -> Header {
        let mut header = Header::new();
        for seq in self.seqs() {
            header.push_record(
                HeaderRecord::new(b"SQ")
                    .push_tag(b"SN", &seq.name)
                    .push_tag(b"LN", &seq.length),
            );
        }
        header
    }
}

impl From<&Aligner> for MMIndex {
    fn from(aligner: &Aligner) -> Self {
        MMIndex {
            inner: aligner.idx.unwrap(),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "htslib")]
mod tests {
    use super::*;
    use crate::Aligner;
    use rust_htslib::bam::ext::BamRecordExtensions;
    use rust_htslib::bam::{header::Header, record::Aux, Read, Reader, Record};

    #[test]
    fn test_index() {
        let aligner = Aligner::builder()
            .with_threads(1)
            .with_index("test_data/genome.fa", None)
            .unwrap();

        let idx = MMIndex::from(&aligner);
        let seqs = idx.seqs();
        assert_eq!(
            seqs,
            vec![
                SeqMetaData {
                    name: "chr1".to_string(),
                    length: 1720u32,
                    is_alt: false
                },
                SeqMetaData {
                    name: "chr2".to_string(),
                    length: 460u32,
                    is_alt: false
                },
            ]
        );

        let header = idx.get_header();

        let records = header.to_hashmap();
        let observed = records.get("SQ").unwrap().first().unwrap();
        assert_eq!(observed.get("SN").unwrap(), "chr1");
        assert_eq!(observed.get("LN").unwrap(), "1720");
    }

    /// find all alignments for a given query
    fn get_expected_records(query_name: &str, spliced: bool) -> Vec<Record> {
        let sam_path = match spliced {
            true => "test_data/cDNA_vs_genome.sam",
            false => "test_data/gDNA_vs_genome.sam",
        };
        let mut reader = Reader::from_path(sam_path).unwrap();
        let records: Vec<Record> = reader
            .records()
            .filter_map(|r| Some(r.unwrap()))
            .filter(|r| String::from_utf8_lossy(r.qname()) == *query_name)
            .collect();
        records
    }

    /// Extract the sequence from the primary alignment
    fn get_query_sequence(records: &Vec<Record>) -> (Vec<u8>, Vec<u8>) {
        let (seq, qual) = records
            .iter()
            .find(|r| !(r.is_secondary() | r.is_supplementary()))
            .map(|r| {
                let mut seq = r.seq().as_bytes();
                let mut qual = r.qual().to_vec();
                if r.is_reverse() {
                    seq = seq
                        .iter()
                        .rev()
                        .map(|b| match b {
                            b'A' => b'T',
                            b'T' => b'A',
                            b'G' => b'C',
                            b'C' => b'G',
                            _ => panic!("Invalid base"),
                        })
                        .collect();
                    qual = qual.into_iter().rev().collect();
                };
                (seq, qual)
            })
            .unwrap();
        (seq, qual)
    }

    fn get_test_case(
        query_name: &str,
        spliced: bool,
    ) -> (Aligner, MMIndex, HeaderView, Vec<Record>, Vec<u8>, Vec<u8>) {
        let aligner = match spliced {
            false => Aligner::builder()
                .with_threads(1)
                .with_index("test_data/genome.fa", None)
                .unwrap()
                .with_cigar(),
            true => Aligner::builder()
                .splice()
                .with_threads(1)
                .with_index("test_data/genome.fa", None)
                .unwrap()
                .with_cigar(),
        };

        let idx = MMIndex::from(&aligner);
        let header = idx.get_header();
        let header_view = HeaderView::from_header(&header);
        // truth set from cli minimap
        let expected_recs = get_expected_records(query_name, spliced);

        // extract the query sequence from the primary record for this query
        let (seq, qual) = get_query_sequence(&expected_recs);
        let qual: Vec<u8> = qual.iter().map(|q| q + 33).collect();
        (aligner, idx, header_view, expected_recs, seq, qual)
    }

    fn map_test_case(query_name: &str, spliced: bool) -> (Vec<Record>, Vec<Record>) {
        let (aligner, _, header_view, expected, seq, qual) = get_test_case(query_name, spliced);
        let observed = aligner
            .map_to_sam(
                &seq,
                Some(&qual),
                Some(query_name.as_bytes()),
                &header_view,
                None,
                None,
            )
            .unwrap();
        (observed, expected)
    }

    #[test]
    fn test_fwd() {
        let query_name = "perfect_read.fwd";
        let (o, e) = map_test_case(query_name, false);
        check_single_mapper(&e, &o);
    }

    #[test]
    fn test_rev() {
        let query_name = "perfect_read.rev";
        let (o, e) = map_test_case(query_name, false);
        check_single_mapper(&e, &o);
    }

    #[test]
    fn test_mismatch() {
        let query_name = "imperfect_read.fwd";
        let (o, e) = map_test_case(query_name, false);
        check_single_mapper(&e, &o);

        let rec = o.first().unwrap();
        let nm = rec.aux(b"NM").unwrap();
        assert_eq!(nm, Aux::U8(5));
    }

    #[test]
    fn test_unmapped() {
        let query_name = "unmappable_read";
        let (o, e) = map_test_case(query_name, false);
        check_single_mapper(&e, &o);
        let rec = o.first().unwrap();
        assert!(rec.is_unmapped());
    }

    #[test]
    fn test_secondary() {
        let query_name = "perfect_inv_duplicate";
        let (o, e) = map_test_case(query_name, false);

        assert_eq!(o.len(), 2); // expect a primary and secondary mapping
        let o_fields: Vec<_> = o
            .iter()
            .map(|r| (r.tid(), r.pos(), r.is_secondary(), r.is_supplementary()))
            .collect();
        let e_fields: Vec<_> = e
            .iter()
            .map(|r| (r.tid(), r.pos(), r.is_secondary(), r.is_supplementary()))
            .collect();
        assert_eq!(
            o_fields,
            vec![(0, 540, false, false), (0, 720, true, false)]
        );
        assert_eq!(o_fields, e_fields);
    }

    #[test]
    fn test_supplementary() {
        let query_name = "split_read";
        let (o, e) = map_test_case(query_name, false);

        assert_eq!(o.len(), 2); // expect a primary and supplementary mapping
        let o_fields: Vec<_> = o
            .iter()
            .map(|r| (r.tid(), r.pos(), r.is_secondary(), r.is_supplementary()))
            .collect();
        let e_fields: Vec<_> = e
            .iter()
            .map(|r| (r.tid(), r.pos(), r.is_secondary(), r.is_supplementary()))
            .collect();
        assert_eq!(o_fields, vec![(0, 0, false, false), (0, 820, false, true)]);
        assert_eq!(o_fields, e_fields);
    }

    #[test]
    fn test_spliced() {
        let query_name = "cdna.fwd";
        let (o, e) = map_test_case(query_name, true);
        check_single_mapper(&e, &o);

        let record = o.first().unwrap();

        let ablocks: Vec<_> = record.aligned_block_pairs().collect();
        println!("{ablocks:?}");
        assert_eq!(
            ablocks,
            vec![
                ([0, 100], [540, 640]),
                ([100, 200], [900, 1000]),
                ([200, 300], [1080, 1180]),
                ([300, 400], [1260, 1360])
            ]
        );
        let introns: Vec<_> = record.introns().collect();
        assert_eq!(introns, vec![[640, 900], [1000, 1080], [1180, 1260]]);
    }

    #[test]
    fn test_spliced_rev() {
        let query_name = "cdna.rev";
        let (o, e) = map_test_case(query_name, true);
        check_single_mapper(&e, &o);
    }

    fn check_single_mapper(expected: &Vec<Record>, observed: &Vec<Record>) {
        // TODO: compare the SAM strings
        assert_eq!(expected.len(), observed.len());
        let e = expected.first().unwrap();
        let o = observed.first().unwrap();
        assert_eq!(o.seq().as_bytes(), e.seq().as_bytes());

        assert_eq!(o.cigar(), e.cigar());
        assert_eq!(o.inner().core.pos, e.inner().core.pos);
        assert_eq!(o.inner().core.mpos, e.inner().core.mpos);
        assert_eq!(o.inner().core.mtid, e.inner().core.mtid);
        assert_eq!(o.inner().core.tid, e.inner().core.tid);
        // the bin attribute is associated with BAM format, so I don't think we need to set it
        // assert_eq!(o.inner().core.bin, e.inner().core.bin);
        assert_eq!(o.inner().core.qual, e.inner().core.qual);
        assert_eq!(o.inner().core.l_extranul, e.inner().core.l_extranul);
        assert_eq!(o.inner().core.flag, e.inner().core.flag);
        assert_eq!(o.inner().core.l_qname, e.inner().core.l_qname);
        assert_eq!(o.inner().core.n_cigar, e.inner().core.n_cigar);
        assert_eq!(o.inner().core.l_qseq, e.inner().core.l_qseq);
        assert_eq!(o.inner().core.isize_, e.inner().core.isize_);
    }

    #[test]
    fn test_optional_fields() {
        let query_name = "perfect_read.fwd";
        let (aligner, _, header_view, _, seq, _qual) = get_test_case(query_name, false);

        let observed = aligner
            .map_to_sam(
                &seq,
                None,
                Some(query_name.as_bytes()),
                &header_view,
                None,
                None,
            )
            .unwrap();
        let rec = observed.first().unwrap();
        assert_eq!(rec.qual(), vec![255; seq.len()]);

        let observed = aligner
            .map_to_sam(&seq, None, None, &header_view, None, None)
            .unwrap();
        let rec = observed.first().unwrap();
        assert_eq!(rec.qual(), vec![255; seq.len()]);
        assert_eq!(rec.qname(), b"query");
    }

    #[test]
    fn test_optional_fields_unmappable() {
        let query_name = "unmappable_read";
        let (aligner, _, header_view, _, seq, _qual) = get_test_case(query_name, false);
        let observed = aligner
            .map_to_sam(&seq, None, None, &header_view, None, None)
            .unwrap();
    }

    #[test]
    fn test_doctest() {
        let aligner = Aligner::builder()
            .with_index("test_data/genome.fa", None)
            .unwrap()
            .with_cigar();
        let mut header = Header::new();
        aligner.populate_header(&mut header);
        let header_view = HeaderView::from_header(&header);

        let records = aligner
            .map_to_sam(
                b"TACGCCACACGGGCTACACTCTCGCCTTCTCGTCTCAACTACGAGATGGACTGTCGGCCTAGAGGATCTAACACGAGAAGTACTTGCCGGCAAGCCCTAA",
                Some(b"2222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222"),
                Some(b"read1"),
                &header_view, None, None)
            .unwrap();

        assert_eq!(records.len(), 1);
        let record = records.first().unwrap();
        assert_eq!((record.tid(), record.pos(), record.mapq()), (0, 180, 13));

        let nm = record.aux(b"NM").unwrap();
        assert_eq!(nm, Aux::U8(5));

        // you can also map reads with no quality/name
        let records = aligner
            .map_to_sam(
                b"TACGCCACACGGGCTACACTCTCGCCTTCTCGTCTCAACTACGAGATGGACTGTCGGCCTAGAGGATCTAACACGAGAAGTACTTGCCGGCAAGCCCTAA",
                None, None,  &header_view, None, None)
            .unwrap();

        assert_eq!(records.len(), 1);
        let record = records.first().unwrap();
        assert_eq!((record.tid(), record.pos(), record.mapq()), (0, 180, 13));
    }
}

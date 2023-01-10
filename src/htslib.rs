use rust_htslib::bam::record::{Cigar, CigarString};
use rust_htslib::bam::{Header, Record};
//    bam, bam::header::HeaderRecord, bam::record::Aux, bam::CompressionLevel, bam::Format,
//    bam::Header, bam::HeaderView, bam::Read, errors, tpool::ThreadPool,
//};
use crate::{Alignment, Mapping};

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
            let q = vec![255; seq.len()]; // Vec::with_capacity(seq.len());
            q
        }
    };

    let cigar: Option<CigarString> = mapping
        .and_then(|m| m.alignment.clone()) // FIXFIX: we probably don't need a clone here
        .and_then(|a| a.cigar)
        .map(|c| cigar_to_cigarstr(&c));

    match mapping {
        Some(m) => {
            // TODO: set strand
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
    rec.set(qname, cigar.as_ref(), seq, &qual[..]);
    // TODO: set AUX flags for cs/md if available
    rec
}

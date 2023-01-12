#[cfg(test)]
#[cfg(feature = "htslib")]
mod tests {
    use minimap2::htslib::mapping_to_record;
    use minimap2::{Aligner, Preset};
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::rc::Rc;

    // #[derive(Debug, Clone, PartialEq, Eq)]
    // pub enum AlignType {
    //     Primary,
    //     Secondary,
    //     Supplementary,
    // }

    // impl From<&u16> for AlignType {
    //     fn from(flags: &u16) -> Self {
    //         if (*flags & 256 as u16) {
    //             Self::Secondary
    //         } else if (*flags & 2048 as u16) {
    //             Self::Supplementary
    //         } else {
    //             Self::Primary
    //         }
    //     }
    // }

    //    use bitflags::bitflags;
    //    bitflags! {
    //        struct SamFlags: u16 {
    //            const PAIRED        = 0x1;
    //            const PROPER_PAIR   = 0x2;
    //            const UNMAP         = 0x4;
    //            const MUNMAP        = 0x8;
    //            const REVERSE       = 0x16;
    //            const MREVERSE      = 0x32;
    //            const READ1         = 0x64;
    //            const READ2         = 0x128;
    //            const SECONDARY     = 0x256;
    //            const QCFAIL        = 0x512;

    //            const DUP           = 0x1024;
    //            const SUPPLEMENTARY = 0x2048;
    //            const PRIMARY  = !(Self::SECONDARY | Self::SUPPLEMENTARY);
    //        }
    //    }

    use rust_htslib::bam::header::{Header, HeaderRecord};
    use rust_htslib::bam::{Format, Read, Reader, Record, Writer};

    fn gdna_records(query_name: &str) -> Vec<Record> {
        let mut reader = Reader::from_path("test_data/gDNA_vs_genome.sam").unwrap();
        let records: Vec<Record> = reader
            .records()
            .into_iter()
            .filter_map(|r| Some(r.unwrap()))
            .filter(|r| String::from_utf8_lossy(r.qname()) == *query_name)
            .collect();

        // for rec in reader.records() {
        //     let rec = rec.unwrap();
        //     if rec.qname() == q
        // }
        records
    }

    fn get_query_sequence(records: &Vec<Record>) -> Vec<u8> {
        let seq = records
            .iter()
            .find(|r| !(r.is_secondary() | r.is_supplementary()))
            .map(|r| r.seq().as_bytes())
            .unwrap();
        seq
    }

    fn get_test_case(query_name: &str) -> (Vec<Record>, Vec<Record>) {
        let aligner = Aligner::builder()
            .with_threads(1)
            .with_index("test_data/genome.fa", None)
            .unwrap()
            .with_cigar();
        let mut header = Header::new();
        // TODO: would be nice to get this from the aligner index
        header.push_record(
            HeaderRecord::new(b"SQ")
                .push_tag(b"SN", &String::from("chr1"))
                .push_tag(b"LN", &1720),
        );

        // truth set from cli minimap
        let expected_recs = gdna_records(query_name);

        // extract the query sequence from the primary record for this query
        let query = get_query_sequence(&expected_recs);
        let mappings = aligner.map(&query, true, true, None, None).unwrap();
        let observed_recs: Vec<Record> = mappings
            .iter()
            .filter_map(|m| {
                let m = mapping_to_record(
                    Some(&m),
                    &query,
                    header.clone(),
                    None,
                    Some(query_name.as_bytes()),
                );
                Some(m)
            })
            .collect();
        (expected_recs, observed_recs)
    }

    fn check_single_mapper(expected: &Vec<Record>, observed: &Vec<Record>) {
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
        // FIXFIX: renable this
        //assert_eq!(o.inner().core.flag, e.inner().core.flag);
        assert_eq!(o.inner().core.l_qname, e.inner().core.l_qname);
        assert_eq!(o.inner().core.n_cigar, e.inner().core.n_cigar);
        assert_eq!(o.inner().core.l_qseq, e.inner().core.l_qseq);
        assert_eq!(o.inner().core.isize, e.inner().core.isize);

    }

    #[test]
    fn test_perfect_fwd() {
        let (expected, observed) = get_test_case(&"perfect_read.fwd".to_string());
        check_single_mapper(&expected, &observed);
    }

    #[test]
    fn test_perfect_rev() {
        let (expected, observed) = get_test_case(&"perfect_read.rev".to_string());
        check_single_mapper(&expected, &observed);
        let e: &Record = expected.first().unwrap();
        let o: &Record = observed.first().unwrap();
        assert_eq!(e.is_reverse(), true);
        assert_eq!(o.is_reverse(), true);
    }



    #[test]
    fn test_mappy_output() {
        let seq = b"atCCTACACTGCATAAACTATTTTGcaccataaaaaaaagGGACatgtgtgGGTCTAAAATAATTTGCTGAGCAATTAATGATTTCTAAATGATGCTAAAGTGAACCATTGTAatgttatatgaaaaataaatacacaattaagATCAACACAGTGAAATAACATTGATTGGGTGATTTCAAATGGGGTCTATctgaataatgttttatttaacagtaatttttatttctatcaatttttagtaatatctacaaatattttgttttaggcTGCCAGAAGATCGGCGGTGCAAGGTCAGAGGTGAGATGTTAGGTGGTTCCACCAACTGCACGGAAGAGCTGCCCTCTGTCATTCAAAATTTGACAGGTACAAACAGactatattaaataagaaaaacaaactttttaaaggCTTGACCATTAGTGAATAGGTTATATGCTTATTATTTCCATTTAGCTTTTTGAGACTAGTATGATTAGACAAATCTGCTTAGttcattttcatataatattgaGGAACAAAATTTGTGAGATTTTGCTAAAATAACTTGCTTTGCTTGTTTATAGAGGCacagtaaatcttttttattattattataattttagattttttaatttttaaat";

        let aligner = Aligner::builder()
            .preset(Preset::MapOnt)
            .with_threads(1)
            .with_index("test_data/test_data.fasta", None)
            .unwrap()
            .with_cigar();

        let mut mappings = aligner.map(seq, true, true, None, None).unwrap();
        assert_eq!(mappings.len(), 1);

        let mut header = Header::new();
        // TODO: would be nice to get this from the aligner index
        header.push_record(
            HeaderRecord::new(b"SQ")
                .push_tag(b"SN", &String::from("contig4"))
                .push_tag(b"LN", &3360),
        );

        let observed = mappings.pop().unwrap();
        let o = mapping_to_record(Some(&observed), seq, header.clone(), None, Some(b"q1"));

        let mut sam_reader = Reader::from_path("test_data/query_vs_test_data.sam").unwrap();
        let e = sam_reader.records().next().unwrap().unwrap();

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
        assert_eq!(o.inner().core.isize, e.inner().core.isize);

        let mut writer =
            Writer::from_path("test_data/query_vs_target.bam", &header, Format::Bam).unwrap();
        writer.write(&o).unwrap();
    }
}

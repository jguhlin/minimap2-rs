#[cfg(test)]
#[cfg(feature = "htslib")]
mod tests {
    use minimap2::htslib::mapping_to_record;
    use minimap2::{Aligner, Preset};

    use rust_htslib::bam::header::{Header, HeaderRecord};
    use rust_htslib::bam::{Format, Read, Reader, Writer};

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

from minimappers2 import map_ont, Aligner

aligner = map_ont();
aligner.threads(4);
aligner.index("/mnt/data/mock/SRR21295036.fasta.gz")
seq = "CCAGAACGTACAAGGAAATATCCTCAAATTATCCCAAGAATTGTCCGCAGGAAATGGGGATAATTTCAGAAATGAGAGCCTTTAGAAATCAGGAAAAATTGAAATTTTGAGCATTTTTTAACCGA"
result = aligner.map1("Randomly Found Seq", seq)
print(result)

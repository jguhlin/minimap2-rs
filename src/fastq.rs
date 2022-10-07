use simdutf8::basic::from_utf8;

use std::io::BufRead;

use crate::Sequence;

pub struct Fastq<'fastq, R: BufRead> {
    reader: &'fastq mut R,
    buffer: Vec<u8>,
    seqbuffer: Vec<u8>,
    scores_buffer: Vec<u8>,
    seqid: Option<String>,
    header: Option<String>,
    seqlen: usize,
}

#[allow(dead_code)]
impl<'fastq, R: BufRead> Fastq<'fastq, R> {
    pub fn from_buffer(in_buf: &'fastq mut R) -> Fastq<'fastq, R> {
        Fastq {
            reader: in_buf,
            buffer: Vec::with_capacity(1024),
            seqbuffer: Vec::with_capacity(2048),
            scores_buffer: Vec::with_capacity(2048),
            seqid: None,
            header: None,
            seqlen: 0,
        }
    }

    pub fn into_reader(self) -> &'fastq mut R {
        self.reader
    }
}

// TODO: This does extra moving by using a generic buffer
// Place the "MATCH" before the while let.... (prob don't need while let anymore...)
impl<'a, R: BufRead> Iterator for Fastq<'a, R> {
    type Item = Result<Sequence, &'static str>;

    fn next(&mut self) -> Option<Result<Sequence, &'static str>> {
        if let Ok(bytes_read) = self.reader.read_until(b'\n', &mut self.buffer) {
            if bytes_read == 0 {
                if self.seqlen > 0 {
                    /*let seq = Sequence {
                        sequence: Some(self.seqbuffer[..self.seqlen].to_vec()),
                        id: Some(self.seqid.take().unwrap()),
                        header: self.header.take(),
                        scores: Some(self.scores_buffer[..self.seqlen].to_vec()),
                        offset: 0,
                    };
                    self.buffer.clear();
                    self.seqlen = 0;
                    return Some(Ok(seq)); */
                    //return Some(Err("File has ended but sequence still in buffer..."));
                    panic!("File has ended but sequence still in buffer!");
                } else {
                    return None;
                }
            } else if self.buffer[0] == b'@' && bytes_read > 1 {
                let idline = match from_utf8(&self.buffer[1..]) {
                    Ok(idline) => idline.to_string(),
                    Err(_) => return Some(Err("Invalid UTF-8 in ID line")),
                };
                let idline = idline.trim();

                let split: Vec<&str> = idline.splitn(2, ' ').collect();
                self.seqid = Some(split[0].to_string());
                if split.len() > 1 {
                    self.header = Some(split[1].to_string());
                } else {
                    self.header = Some("".to_string());
                }
                self.buffer.clear();
            } else {
                return Some(Err("Invalid FASTQ file"));
            }
        } else {
            return Some(Err("Error reading FASTQ file"));
        }

        if let Ok(bytes_read) = self.reader.read_until(b'\n', &mut self.seqbuffer) {
            if bytes_read == 0 {
                return Some(Err("Invalid FASTQ file"));
            } else {
                let mut end = self.seqbuffer.len() - 1;
                while end > 0 && self.seqbuffer[end].is_ascii_whitespace() {
                    end = end.saturating_sub(1);
                }

                if end == 0 {
                    return Some(Err("Invalid FASTQ file"));
                }

                self.seqlen = end + 1;
            }
        } else {
            return Some(Err("Error reading FASTQ file"));
        }

        if let Ok(bytes_read) = self.reader.read_until(b'\n', &mut self.buffer) {
            if bytes_read == 0 {
                return Some(Err("Invalid FASTQ file"));
            }
        } else {
            return Some(Err("Error reading FASTQ file"));
        }

        if let Ok(bytes_read) = self.reader.read_until(b'\n', &mut self.scores_buffer) {
            if bytes_read == 0 {
                Some(Err("Invalid FASTQ file"))
            } else {
                if self.seqid.is_none() {
                    return Some(Err("Invalid FASTQ file"));
                }

                let mut seqbuffer = Vec::with_capacity(self.seqlen);
                let mut scores_buffer = Vec::with_capacity(self.seqlen);
                std::mem::swap(&mut self.seqbuffer, &mut seqbuffer);
                std::mem::swap(&mut self.scores_buffer, &mut scores_buffer);
                seqbuffer.truncate(self.seqlen);
                scores_buffer.truncate(self.seqlen);

                if scores_buffer.len() != seqbuffer.len() {
                    return Some(Err("Invalid FASTQ file"));
                }

                let seq = Sequence {
                    sequence: Some(seqbuffer),
                    id: Some(self.seqid.take().unwrap()),
                    header: self.header.take(),
                    scores: Some(scores_buffer),
                    offset: 0,
                };
                self.buffer.clear();
                self.seqlen = 0;
                Some(Ok(seq))
            }
        } else {
            Some(Err("Error reading FASTQ file"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_fastq() {
        let fastq_data = b"@seq1
ACGT
+
IIII
@seq2
ACGT
+
IIII
@seq3
ACGT
+
IIII
@seq4
ACGT
+
IIII
";
        let mut cursor = Cursor::new(fastq_data);
        let mut fastq = Fastq::from_buffer(&mut cursor);
        let seq = fastq.next().unwrap().unwrap();
        assert_eq!(seq.id.unwrap(), "seq1");
        assert_eq!(seq.sequence.unwrap(), b"ACGT");
        assert_eq!(seq.scores, Some(b"IIII".to_vec()));
        let seq = fastq.next().unwrap().unwrap();
        assert_eq!(seq.id.unwrap(), "seq2");
        assert_eq!(seq.sequence.unwrap(), b"ACGT");
        assert_eq!(seq.scores.unwrap(), b"IIII");
        let seq = fastq.next().unwrap().unwrap();
        assert_eq!(seq.id.unwrap(), "seq3");
        assert_eq!(seq.sequence.unwrap(), b"ACGT");
        assert_eq!(seq.scores, Some(b"IIII".to_vec()));
        let seq = fastq.next().unwrap().unwrap();
        assert_eq!(seq.id.unwrap(), "seq4");
        assert_eq!(seq.sequence.unwrap(), b"ACGT");
        assert_eq!(seq.scores, Some(b"IIII".to_vec()));
        assert!(fastq.next().is_none());
    }
}

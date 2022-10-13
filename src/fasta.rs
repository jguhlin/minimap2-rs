use bytelines::ByteLinesReader;
use simdutf8::basic::from_utf8;

use std::io::BufRead;

use crate::Sequence;

pub struct Fasta<'fasta, R> {
    reader: &'fasta mut R,
    buffer: Vec<u8>,
    seqbuffer: Vec<u8>,
    next_seqid: Option<String>,
    next_header: Option<String>,
    seqlen: usize,
    moving_average: [u64; 8],
    moving_average_pos: usize,
}

impl<'fasta, R: BufRead> Fasta<'fasta, R> {
    pub fn from_buffer(in_buf: &mut R) -> Fasta<R> {
        Fasta {
            reader: in_buf,
            buffer: Vec::with_capacity(1024),
            seqbuffer: Vec::with_capacity(1024),
            next_seqid: None,
            next_header: None,
            seqlen: 0,
            moving_average: [1024; 8],
            moving_average_pos: 1,
        }
    }
}

impl<'fasta, R: BufRead> Iterator for Fasta<'fasta, R> {
    type Item = Result<Sequence, &'static str>;

    fn next(&mut self) -> Option<Result<Sequence, &'static str>> {
        while let Ok(bytes_read) = self.reader.read_until(b'\n', &mut self.buffer) {
            if bytes_read == 0 {
                // File is finished
                if self.seqlen > 0 && self.next_seqid.is_some() {
                    let seq = Sequence {
                        sequence: Some(self.seqbuffer[..self.seqlen].to_vec()),
                        id: Some(self.next_seqid.take().unwrap()),
                        header: self.next_header.take(),
                        scores: None,
                        offset: 0,
                    };
                    self.seqlen = 0;
                    return Some(Ok(seq));
                } else {
                    return None;
                }
            } else {
                match self.buffer[0] {
                    // 62 is a > meaning we have a new sequence id.
                    62 => {
                        let slice_end = if self.buffer[bytes_read - 1] == b'\r' {
                            bytes_read.saturating_sub(1)
                        } else {
                            bytes_read
                        };

                        let next_id = match from_utf8(&self.buffer[1..slice_end]) {
                            Ok(id) => id.to_string(),
                            Err(_) => return Some(Result::Err("Invalid UTF-8")),
                        };

                        let next_id = next_id.trim();

                        self.buffer.clear();
                        let split: Vec<&str> = next_id.splitn(2, ' ').collect();
                        let next_id = split[0].trim().to_string();
                        let mut header = if split.len() == 2 {
                            Some(split[1].trim().to_string())
                        } else {
                            None
                        };

                        let id = self.next_seqid.replace(next_id);
                        std::mem::swap(&mut header, &mut self.next_header);

                        if self.seqlen > 0 {
                            if id.is_none() {
                                return Some(Result::Err(
                                    "No sequence found - Sequence > 0 but no ID",
                                ));
                            }

                            // Use the last seqlen as the new buffer's size
                            self.moving_average[self.moving_average_pos] = self.seqlen as u64;
                            self.moving_average_pos = (self.moving_average_pos + 1) % 8;
                            let mut seqbuf = Vec::with_capacity(
                                (self.moving_average.iter().sum::<u64>() / 8) as usize,
                            );
                            std::mem::swap(&mut self.seqbuffer, &mut seqbuf);
                            seqbuf.truncate(self.seqlen);

                            let seq = Sequence {
                                sequence: Some(seqbuf),
                                id: Some(id.unwrap()),
                                header,
                                scores: None,
                                offset: 0,
                            };
                            self.seqbuffer.clear();
                            self.seqlen = 0;
                            return Some(Ok(seq));
                        }
                    }
                    _ => {
                        let mut slice_end = bytes_read;

                        if self.buffer.is_empty() {
                            return Some(Result::Err("No sequence found - Empty buffer"));
                        }

                        if self.buffer[bytes_read.saturating_sub(1)] == b'\n' {
                            slice_end = slice_end.saturating_sub(1);
                        }

                        if self.buffer[slice_end.saturating_sub(1)] == b'\r' {
                            slice_end = slice_end.saturating_sub(1);
                        }

                        // let slice_end = bytes_read; //.saturating_sub(1);
                        self.seqbuffer.extend_from_slice(&self.buffer[0..slice_end]);
                        self.seqlen = self.seqlen.saturating_add(slice_end);
                        self.buffer.clear();
                    }
                }
            }
        }
        unreachable!();
    }
}

// TODO: This needs a refresh
pub fn summarize_fasta(fasta_buf: &mut dyn BufRead) -> (usize, Vec<String>, Vec<usize>) {
    let mut entries: usize = 0;
    let mut ids: Vec<String> = Vec::with_capacity(2 * 1024 * 1024);
    let mut lengths: Vec<usize> = Vec::with_capacity(2 * 1024 * 1024);
    let mut length: usize = 0;

    let mut lines = fasta_buf.byte_lines();
    let mut first = true;
    while let Some(line) = lines.next() {
        let line = line.expect("Error parsing FASTA file");
        if line.starts_with(b">") {
            let id = from_utf8(&line[1..]).expect("Unable to convert FASTA header to string");
            ids.push(id.to_string());

            if first {
                first = false;
            } else {
                lengths.push(length);
            }

            entries += 1;
            length = 0;
        } else {
            length += line.len();
        }
    }
    lengths.push(length);

    (entries, ids, lengths)
}

pub fn count_fasta_entries(fasta_buf: &mut dyn BufRead) -> usize {
    let mut entries: usize = 0;

    let mut lines = fasta_buf.byte_lines();
    while let Some(line) = lines.next() {
        let line = line.expect("Error parsing FASTA file");
        if line.starts_with(b">") {
            entries += 1;
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    #[test]
    pub fn test_fasta_parse() {
        let fakefasta =
            b">Hello\nACTGCATCACTGACCTA\n>Second\nACTTGCAACTTGGGACACAACATGTA\n>Third  \nACTGCA\nACTGCA\nNNNNN".to_vec();
        let fakefasta_ = fakefasta.as_slice();
        let inner = &mut BufReader::new(fakefasta_);
        let mut fasta = Fasta::from_buffer(inner);
        let j = fasta.next().unwrap();
        assert!(j.unwrap().sequence.unwrap() == b"ACTGCATCACTGACCTA".to_vec());
        let _j = fasta.next().unwrap();
        let j = fasta.next().unwrap();
        assert!(j.unwrap().sequence.unwrap() == b"ACTGCAACTGCANNNNN");
    }

    #[test]
    pub fn test_fasta_parse_rest_of_the_header() {
        let fakefasta =
            b">Hello I have more information in the rest of the FASTA header\nACTGCATCACTGACCTA\n>Second\nACTTGCAACTTGGGACACAACATGTA\n".to_vec();
        let fakefasta_ = fakefasta.as_slice();
        let mut inner = BufReader::new(fakefasta_);
        let mut fasta = Fasta::from_buffer(&mut inner);
        let s = fasta.next().unwrap().unwrap();
        println!("{:#?}", s.header);
        assert!(
            s.header == Some("I have more information in the rest of the FASTA header".to_string())
        );
    }

    #[test]
    pub fn test_summarize_fasta() {
        let fakefasta =
            b">Hello\nACTGCATCACTGACCTA\n>Second\nACTTGCAACTTGGGACACAACATGTA\n".to_vec();
        let fakefasta_ = fakefasta.as_slice();
        let mut buf = BufReader::new(fakefasta_);
        let j = summarize_fasta(&mut buf);
        assert!(j.0 == 2);
        println!("{:#?}", j);
        assert!(j.1[0] == "Hello");
        assert!(j.1[1] == "Second");
        assert!(j.2[0] == 17);
        assert!(j.2[1] == 26);
    }
}

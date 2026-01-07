// src/stream_manager.rs
use std::io::{self, BufRead};
use std::mem;

/// A robust, memory-aware iterator for FASTA streams.
///
/// Features:
/// - Smart Batching: Flushes based on Item Count OR Memory Usage (prevents OOM).
/// - Robust Parsing: Handles multi-line sequences (standard FASTA) and ignores whitespace.
/// - State Persistence: Correctly handles records that span across batch boundaries.
pub struct DnaBatchIterator<R> {
    lines: io::Lines<R>,
    max_items: usize,
    max_bytes: usize,

    // Internal State
    pending_header: Option<String>,
    pending_sequence: String,
    exhausted: bool,
}

impl<R: BufRead> DnaBatchIterator<R> {
    pub fn new(reader: R, max_items: usize, max_bytes: usize) -> Self {
        Self {
            lines: reader.lines(),
            max_items,
            max_bytes,
            pending_header: None,
            pending_sequence: String::new(),
            exhausted: false,
        }
    }
}

impl<R: BufRead> Iterator for DnaBatchIterator<R> {
    type Item = io::Result<Vec<(String, String)>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }

        let mut batch = Vec::new();
        let mut current_batch_bytes = 0;

        loop {
            // Check limits BEFORE reading more to ensure we stay within RAM bounds
            if !batch.is_empty() {
                if batch.len() >= self.max_items || current_batch_bytes >= self.max_bytes {
                    return Some(Ok(batch));
                }
            }

            match self.lines.next() {
                Some(Ok(raw_line)) => {
                    let line = raw_line.trim();
                    if line.is_empty() { continue; } // Skip blank lines

                    if line.starts_with('>') {
                        // NEW HEADER FOUND
                        // If we were building a record, finalize it and push to batch.
                        if let Some(prev_header) = self.pending_header.replace(line.to_string()) {
                            let prev_seq = mem::take(&mut self.pending_sequence);

                            // Only push valid records (ignore headers with no sequence)
                            if !prev_seq.is_empty() {
                                let size_est = prev_header.len() + prev_seq.len() + 48; // Struct overhead
                                batch.push((prev_header, prev_seq));
                                current_batch_bytes += size_est;
                            }
                        }
                        // Note: self.pending_sequence is already cleared by mem::take
                    } else {
                        // SEQUENCE LINE
                        // Append to buffer (handles multi-line FASTA)
                        self.pending_sequence.push_str(line);
                    }
                }
                Some(Err(e)) => return Some(Err(e)), // Propagate I/O errors
                None => {
                    // EOF
                    self.exhausted = true;

                    // Flush the final pending record if it exists
                    if let Some(last_header) = self.pending_header.take() {
                        let last_seq = mem::take(&mut self.pending_sequence);
                        if !last_seq.is_empty() {
                            batch.push((last_header, last_seq));
                        }
                    }
                    break;
                }
            }
        }

        if batch.is_empty() {
            None
        } else {
            Some(Ok(batch))
        }
    }
}

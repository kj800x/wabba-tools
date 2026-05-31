use base64::prelude::*;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use xxhash_rust::xxh64::{Xxh64, xxh64};

pub struct Hash {}

impl Hash {
    pub fn compute(data: &[u8]) -> String {
        let hash = xxh64(data, 0);

        // u64 into bytes
        let hash_bytes = hash.to_le_bytes();

        // Format in base64

        BASE64_STANDARD.encode(hash_bytes)
    }

    /// Stream a file through xxhash64 without loading the whole file into
    /// memory. Produces the same base64 output as `compute`.
    pub fn compute_file(path: &Path) -> io::Result<String> {
        let mut hasher = Xxh64::new(0);
        let mut file = File::open(path)?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let hash_bytes = hasher.digest().to_le_bytes();
        Ok(BASE64_STANDARD.encode(hash_bytes))
    }
}

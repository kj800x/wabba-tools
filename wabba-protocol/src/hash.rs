use base64::prelude::*;
use xxhash_rust::xxh64::xxh64;

pub struct Hash {}

impl Hash {
    pub fn compute(data: &[u8]) -> String {
        let hash = xxh64(data, 0);

        // u64 into bytes
        let hash_bytes = hash.to_le_bytes();

        // Format in base64
        let hash_base64 = BASE64_STANDARD.encode(&hash_bytes);

        hash_base64
    }
}

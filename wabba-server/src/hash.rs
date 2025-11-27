pub struct Hash {}

impl Hash {
    // FIXME: xxhash64 instead of md5!
    pub fn compute(data: &[u8]) -> String {
        let hash = md5::compute(data);
        format!("{:x}", hash)
    }
}

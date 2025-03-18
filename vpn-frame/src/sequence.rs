use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use bucky_raw_codec::{RawDecode, RawEncode};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, RawEncode, RawDecode)]
pub struct Sequence(u32);

impl Sequence {
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Debug for Sequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value())
    }
}

impl From<u32> for Sequence {
    fn from(v: u32) -> Self {
        Sequence(v)
    }
}

impl Hash for Sequence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.0)
    }
}

pub struct SequenceGenerator {
    cur: AtomicU32,
}


impl SequenceGenerator {
    pub fn new() -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u32;
        Self {
            cur: AtomicU32::new(now),
        }
    }

    pub fn generate(&self) -> Sequence {
        let v = self.cur.fetch_add(1, Ordering::SeqCst);
        if v == 0 {
            Sequence(self.cur.fetch_add(1, Ordering::SeqCst))
        } else {
            Sequence(v)
        }
    }
}

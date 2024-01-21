use std::iter::Iterator;

pub mod prelude {
    pub use super::*;
}

pub struct PollardsLog {
    p: u64,
    g: u64,
    h: u64,
    k: usize,
    xi: u64,
    yi: u64,
    ai: u64,
    bi: u64,
    gi: u64,
    di: u64,
}

impl PollardsLog {
    pub fn new(p: u64, g: u64, h: u64) -> PollardsLog {
        PollardsLog {
            p, g, h,
            k: 0,
            xi: 1,
            yi: 1,
            ai: 0,
            bi: 0,
            di: 0,
            gi: 0,
        }
    }

    fn mix(&self) -> (u64, u64, u64) {
        if 0 <= self.xi && self.xi < self.p / 3 {
            ((self.xi * self.g) % self.p, (self.ai + 1) % (self.p - 1), self.bi)
        } else if self.p / 3 < self.xi && self.xi < (2 * self.p) / 3 {
            (u64::pow(self.xi, 2) % self.p, (2 * self.ai) % (self.p - 1), (2 * self.bi) % (self.p - 1))
        } else {
            ((self.xi * self.h) % self.p, self.ai, (self.bi + 1) % (self.p - 1))
        }
    }
}

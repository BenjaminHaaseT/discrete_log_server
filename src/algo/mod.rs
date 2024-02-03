use std::iter::Iterator;
use std::pin::Pin;
use std::task::{Context, Poll};
use rand::prelude::*;
use futures::stream::{FusedStream, Stream};
use futures::StreamExt;

pub mod prelude {
    pub use super::*;
}

pub use utils::*;

#[derive(Debug, PartialEq)]
pub struct PollardsLogItem {
    pub i: usize,
    pub xi: u64,
    pub ai: u64,
    pub bi: u64,
    pub yi: u64,
    pub gi: u64,
    pub di: u64,
}

#[derive(Debug, PartialEq)]
pub struct PollardsLog {
    pub p: u64,
    pub g: u64,
    pub h: u64,
    i: usize,
    xi: u64,
    yi: u64,
    ai: u64,
    bi: u64,
    gi: u64,
    di: u64,
    finished: bool,
}

impl PollardsLog {
    pub fn new(p: u64, g: u64, h: u64) -> PollardsLog {
        PollardsLog {
            p, g, h,
            i: 0,
            xi: 1,
            yi: 1,
            ai: 0,
            bi: 0,
            gi: 0,
            di: 0,
            finished: false,
        }
    }

    fn mix(&self, x: u64, a: u64, b: u64) -> (u64, u64, u64) {
        if 0 <= x && x <  self.p / 3 {
            ((self.g * x) % self.p, (a + 1) % (self.p - 1), b)
        } else if self.p / 3 <= x && x < (2 * self.p) / 3 {
            (u64::pow(x, 2) % self.p, (2 * a) % (self.p - 1), (2 * b) % (self.p - 1))
        } else {
            ((self.h * x) % self.p, a, (b + 1) % (self.p - 1))
        }
    }

    pub fn solve(&self) -> Option<u64> {
        assert!(self.xi == self.yi);
        // Compute the exponents after combining like terms
        let u = if self.ai >= self.gi {
            (self.ai - self.gi) % (self.p - 1)
        } else {
            (self.ai + (self.p - 1) - self.gi) % (self.p - 1)
        };
        let v = if self.di >= self.bi {
            (self.di - self.bi) % (self.p - 1)
        } else {
            (self.di + (self.p - 1) - self.bi) % (self.p - 1)
        };
        // Compute gcd of v and p - 1
        let d = gcd(v, self.p - 1);
        let (s, t) = gcd_weights(v, self.p - 1);

        // Find correct combination of weights that sum to d
        let v_inv = gcd_mul_inverse(self.p - 1, v, d, s, t);
        assert_eq!((v * v_inv) % (self.p - 1), d);

        // Finally solve
        let r = ((u * v_inv) % (self.p - 1)) / d;
        let mut found = None;
        for k in 0..d {
            let e = ((self.p - 1) / d) * k + r;
            let res = fast_power(self.g, e, self.p);
            if res == self.h {
                found = Some(e);
                break;
            }
        }

        found
    }

    pub fn steps_to_sqrt_mod_ratio(&self) -> f64 {
        (self.i as f64) / (f64::sqrt(self.p as f64))
    }
}

impl Iterator for PollardsLog {
    type Item = PollardsLogItem;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let (next_xi, next_ai, next_bi) = self.mix(self.xi, self.ai, self.bi);
        self.xi = next_xi;
        self.ai = next_ai;
        self.bi = next_bi;
        let (next_yi, next_gi, next_di) = self.mix(self.yi, self.gi, self.di);
        let (next_yi, next_gi, next_di) = self.mix(next_yi, next_gi, next_di);
        self.yi = next_yi;
        self.di = next_di;
        self.gi = next_gi;
        self.i += 1;
        if self.xi == self.yi {
            self.finished = true;
        }
        Some(PollardsLogItem {
            i: self.i,
            xi: self.xi,
            ai: self.ai,
            bi: self.bi,
            yi: self.yi,
            gi: self.gi,
            di: self.di
        })
    }
}

impl Stream for PollardsLog {
    type Item = PollardsLogItem;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<PollardsLogItem>> {
        if !self.finished {
            return Poll::Ready(Iterator::next(self.get_mut()));
        }
        Poll::Ready(None)
    }
}

impl FusedStream for PollardsLog {
    fn is_terminated(&self) -> bool {
        self.finished
    }
}

// impl StreamExt for PollardsLog {}

#[derive(Debug, PartialEq)]
pub struct PollardsRSAFactItem {
    pub i: usize,
    pub xi: u64,
    pub yi: u64,
    pub g: u64,
    pub n: u64
}

#[derive(Debug, PartialEq)]
pub struct PollardsRSAFact {
    n: u64,
    i: usize,
    xi: u64,
    yi: u64,
    factor: Option<u64>,
    finished: bool,
}

impl PollardsRSAFact {
    pub fn new(n: u64) -> Self {
        assert!((n - 1).checked_mul(n - 1).is_some(), "modulus too large, overflow may occur");
        Self { n, i: 0, xi: 1, yi: 1,  factor: None, finished: false }
    }

    fn mix(&self, x: u64) -> u64 {
        (((x * x) % self.n) + 1) % self.n
    }

    pub fn factor(&mut self) -> Option<u64> {
        self.factor.take()
    }

    pub fn steps_to_sqrt_mod_ratio(&self) -> f64 {
        (self.i as f64) / f64::sqrt(self.n as f64)
    }
}

impl Iterator for PollardsRSAFact {
    type Item = PollardsRSAFactItem;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        self.i += 1;
        self.xi = self.mix(self.xi);
        self.yi = self.mix(self.yi);
        self.yi = self.mix(self.yi);
        let g = gcd(self.xi.abs_diff(self.yi), self.n);
        if g != 1 && self.n % g == 0 {
            self.finished = true;
            self.factor = Some(g);
        }
        Some(PollardsRSAFactItem { i: self.i, xi: self.xi, yi: self.yi, g, n: self.n })
    }
}

pub mod utils {
    pub fn gcd(mut a: u64, mut b: u64) -> u64 {
        assert!(a != 0 && b != 0);
        let mut r = a % b;
        while r > 0 {
            a = b;
            b = r;
            r = a % b;
        }
        b
    }

    pub fn fast_power(mut g: u64, mut e: u64, n: u64) -> u64 {
        let mut r = 1;
        while e > 0 {
            if e % 2 == 1 {
                r *= g;
                r %= n;
            }
            g *= g;
            g %= n;
            e /= 2;
        }
        r
    }

    pub fn gcd_weights(mut a: u64, mut b: u64) -> (u64, u64) {
        let mut p_vec = vec![1];
        let mut q_vec = vec![0, 1];
        let mut q = a / b;
        p_vec.push(q);
        let mut r = a % b;
        while r > 0 {
            a = b;
            b = r;
            q = a / b;
            let (p1, p2) = (p_vec[p_vec.len() - 1], p_vec[p_vec.len() - 2]);
            let (q1, q2) = (q_vec[q_vec.len() - 1], q_vec[q_vec.len() - 2]);
            p_vec.push(p1 * q + p2);
            q_vec.push(q1 * q + q2);
            r = a % b;
        }
        (p_vec[p_vec.len() - 2], q_vec[q_vec.len() - 2])
    }

    pub fn gcd_mul_inverse(m: u64, v: u64, d: u64, s: u64, t: u64) -> u64 {
        let mut m = m;
        if m * s > v * t && m * s - v * t == d {
            while m < t {
                m += m;
            }
            // println!("gcd_mul_inverse, branch1");
            let v_inv = (m - t) % m;
            assert_eq!((v * v_inv) % m, d);
            (m - t) % m
        } else if m * t > v * s && m * t - v * s == d {
            while m < s {
                m += m;
            }
            // println!("gcd_mul_inverse, branch2");
            let v_inv = (m - s) % m;
            assert_eq!((v * v_inv) % m, d);
            (m - s) % m
        } else if v * t > m * s && t * v - m * s == d {
            // println!("gcd_mul_inverse, branch3");
            let v_inv = t % m;
            assert_eq!((v * v_inv) % m, d);
            t % m
        } else {
            // println!("gcd_mul_inverse, branch4");
            let v_inv = s % m;
            assert_eq!((v * v_inv) % m, d);
            s % m
        }
    }

    pub fn miller_rabin(n: u64, a: u64) -> bool {
        let d = gcd(a, n);
        if n % 2 == 0 || (1 < d && d < n) {
            return true;
        }
        let mut q = n - 1;
        let mut k = 0;
        while q % 2 == 0 {
            q /= 2;
            k += 1;
        }
        let mut a = fast_power(a, q, n);
        if a % n == 1 {
            return false;
        }
        for i in 0..k {
            if a % n == n - 1 {
                return false;
            }
            a *= a;
            a %= n;
        }
        true
    }
}

#[cfg(test)]
mod test {
    pub use super::*;

    #[test]
    fn pollards_log_iter_test() {
        let mut pollard = PollardsLog::new(48611, 19, 24717);
        while let Some(item) = Iterator::next(&mut pollard) {
            println!("{:?}", item);
        }
        println!("{:?}", pollard);
    }

    #[test]
    fn gcd_weights_test() {
        let (a, b) = (100, 80);
        let d = gcd(a, b);
        let (u, v) = gcd_weights(a, b);
        println!("a: {}, b: {}", a, b);
        println!("u: {}, v: {}", u, v);
        if a * u > b * v && a * u - b * v == d {
            println!("a * u - b * v = {}", d);
        } else if a * v > b * u && a * v - b * u == d {
            println!("a * v - b * u = {}", d);
        } else if b * v > a * u && b * v - a * u == d {
            println!("b * v - a * u = {}", d);
        } else {
            assert!(b * u > a * v);
            assert_eq!(b * u - a * v, d);
            println!("b * u - a * v = {}", d);
        }

        println!();

        let (a, b) = (9409612, 666);
        let d = gcd(a, b);
        let (u, v) = gcd_weights(a, b);
        println!("a: {}, b: {}", a, b);
        println!("u: {}, v: {}", u, v);
        if a * u > b * v && a * u - b * v == d {
            println!("a * u - b * v = {}", d);
        } else if a * v > b * u && a * v - b * u == d {
            println!("a * v - b * u = {}", d);
        } else if b * v > a * u && b * v - a * u == d {
            println!("b * v - a * u = {}", d);
        } else {
            assert!(b * u > a * v);
            assert_eq!(b * u - a * v, d);
            println!("b * u - a * v = {}", d);
        }

        println!();

        let (a, b) = (2200, 124);
        let d = gcd(a, b);
        let (u, v) = gcd_weights(a, b);
        println!("a: {}, b: {}", a, b);
        println!("u: {}, v: {}", u, v);
        if a * u > b * v && a * u - b * v == d {
            println!("a * u - b * v = {}", d);
        } else if a * v > b * u && a * v - b * u == d {
            println!("a * v - b * u = {}", d);
        } else if b * v > a * u && b * v - a * u == d {
            println!("b * v - a * u = {}", d);
        } else {
            assert!(b * u > a * v);
            assert_eq!(b * u - a * v, d);
            println!("b * u - a * v = {}", d);
        }

        println!();

        let (a, b) = (1782886218, 34478);
        let d = gcd(a, b);
        let (u, v) = gcd_weights(a, b);
        println!("a: {}, b: {}", a, b);
        println!("u: {}, v: {}", u, v);
        if a * u > b * v && a * u - b * v == d {
            println!("a * u - b * v = {}", d);
        } else if a * v > b * u && a * v - b * u == d {
            println!("a * v - b * u = {}", d);
        } else if b * v > a * u && b * v - a * u == d {
            println!("b * v - a * u = {}", d);
        } else {
            assert!(b * u > a * v);
            assert_eq!(b * u - a * v, d);
            println!("b * u - a * v = {}", d);
        }
    }

    #[test]
    fn miller_rabin_test() {
        let n = 561;
        let a = 2;
        let res = miller_rabin(n, a);
        println!("res: {}", res);

        let n = 172947529;
        // let a = 2;
        // let res = miller_rabin(n, a);
        // println!("res: {}", res);

        let a = 17;
        let res = miller_rabin(n, a);
        println!("res: {}", res);

        let a = 23;
        let res = miller_rabin(n, a);
        println!("res: {}", res);

        let mut rng = rand::thread_rng();
        let mut k = 0;
        let n = 15239131;
        let mut prime_flag = true;
        while k < 20 {
            let a = rng.gen_range(2..n);
            if a == 1 {
                continue;
            }
            if miller_rabin(n, a) {
                prime_flag = false;
                break;
            }
            k += 1;
        }

        assert!(prime_flag);
        println!("{} is prime with probability: {:2.20}", n, 1.0 - f64::powi(0.25, k));
    }

    #[test]
    fn gcd_mul_inverse_test() {
        let (a, b) = (100, 80);
        let d = gcd(a, b);
        let (u, v) = gcd_weights(a, b);
        println!("a: {}, b: {}", a, b);
        println!("u: {}, v: {}", u, v);
        let b_inv = gcd_mul_inverse(a, b, d, u, v);
        println!("b_inv = {}", b_inv);
        println!("b * b_inv mod a = {}", (b * b_inv) % a);
        assert_eq!((b * b_inv) % a, d);
        println!();

        let (a, b) = (9409612, 666);
        let d = gcd(a, b);
        let (u, v) = gcd_weights(a, b);
        println!("a: {}, b: {}", a, b);
        println!("u: {}, v: {}", u, v);
        let b_inv = gcd_mul_inverse(a, b, d, u, v);
        println!("b_inv = {}", b_inv);
        println!("b * b_inv mod a = {}", (b * b_inv) % a);
        assert_eq!((b * b_inv) % a, d);
        println!();

        let (a, b) = (2200, 124);
        let d = gcd(a, b);
        let (u, v) = gcd_weights(a, b);
        println!("a: {}, b: {}", a, b);
        println!("u: {}, v: {}", u, v);
        let b_inv = gcd_mul_inverse(a, b, d, u, v);
        println!("b_inv = {}", b_inv);
        println!("b * b_inv mod a = {}", (b * b_inv) % a);
        assert_eq!((b * b_inv) % a, d);
        println!();

        let (a, b) = (1782886218, 34478);
        let d = gcd(a, b);
        let (u, v) = gcd_weights(a, b);
        println!("a: {}, b: {}", a, b);
        println!("u: {}, v: {}", u, v);
        let b_inv = gcd_mul_inverse(a, b, d, u, v);
        println!("b_inv = {}", b_inv);
        println!("b * b_inv mod a = {}", (b * b_inv) % a);
        assert_eq!((b * b_inv) % a, d);
    }

    #[test]
    fn pollards_log_solve_test() {
        let (p, g, h) = (5011, 2, 2495);
        println!("p: {}, g: {}, h: {}", p, g, h);
        let mut pollards = PollardsLog::new(p, g, h);
        for item in &mut pollards {
            println!("{:?}", item);
        }
        let log = pollards.solve();
        println!("{:?}", log);
        assert!(log.is_some());
        let log = log.unwrap();
        let res = fast_power(g, log, p);
        println!("res: {}", res);
        println!("steps to sqrt mod ratio: {:10.10}", pollards.steps_to_sqrt_mod_ratio());
        assert_eq!(res, h);
        println!();

        let (p, g, h) = (17959, 17, 14226);
        println!("p: {}, g: {}, h: {}", p, g, h);
        let mut pollards = PollardsLog::new(p, g, h);
        for item in &mut pollards {
            println!("{:?}", item);
        }
        let log = pollards.solve();
        println!("{:?}", log);
        assert!(log.is_some());
        let log = log.unwrap();
        let res = fast_power(g, log, p);
        println!("res: {}", res);
        println!("steps to sqrt mod ratio: {:10.10}", pollards.steps_to_sqrt_mod_ratio());
        assert_eq!(res, h);
        println!();

        let (p, g, h) = (15239131, 29, 5953042);
        println!("p: {}, g: {}, h: {}", p, g, h);
        let mut pollards = PollardsLog::new(p, g, h);
        for item in &mut pollards {
            println!("{:?}", item);
        }
        let log = pollards.solve();
        println!("{:?}", log);
        assert!(log.is_some());
        let log = log.unwrap();
        let res = fast_power(g, log, p);
        println!("res: {}", res);
        println!("steps to sqrt mod ratio: {:10.10}", pollards.steps_to_sqrt_mod_ratio());
        assert_eq!(res, h);
        println!();
    }

    #[test]
    fn test_pollards_rsa_factor() {
        let mut pollards = PollardsRSAFact::new(1782886219);
        for item in &mut pollards {
            println!("{:?}", item);
        }
        let factor1 = pollards.factor();
        println!("{:?}", factor1);
        assert!(factor1.is_some());

        let factor1 = factor1.unwrap();
        assert_ne!(factor1, 1);
        assert_eq!(pollards.n % factor1, 0);

        let factor2 = pollards.n / factor1;
        println!("factor1: {}, factor2: {}", factor1, factor2);
        println!("factor1 * factor2 = {}", factor1 * factor2);
        println!("Steps to modulus sqrt ratio: {:10.10}", pollards.steps_to_sqrt_mod_ratio());
        assert_eq!(factor1 * factor2, pollards.n);

        let mut pollards = PollardsRSAFact::new(9409613);
        for item in &mut pollards {
            println!("{:?}", item);
        }
        let factor1 = pollards.factor();
        println!("{:?}", factor1);
        assert!(factor1.is_some());

        let factor1 = factor1.unwrap();
        assert_ne!(factor1, 1);
        assert_eq!(pollards.n % factor1, 0);

        let factor2 = pollards.n / factor1;
        println!("factor1: {}, factor2: {}", factor1, factor2);
        println!("factor1 * factor2 = {}", factor1 * factor2);
        println!("Steps to modulus sqrt ratio: {:10.10}", pollards.steps_to_sqrt_mod_ratio());
        assert_eq!(factor1 * factor2, pollards.n);

        let mut pollards = PollardsRSAFact::new(2201);
        for item in &mut pollards {
            println!("{:?}", item);
        }
        let factor1 = pollards.factor();
        println!("{:?}", factor1);
        assert!(factor1.is_some());

        let factor1 = factor1.unwrap();
        assert_ne!(factor1, 1);
        assert_eq!(pollards.n % factor1, 0);

        let factor2 = pollards.n / factor1;
        println!("factor1: {}, factor2: {}", factor1, factor2);
        println!("factor1 * factor2 = {}", factor1 * factor2);
        println!("Steps to modulus sqrt ratio: {:10.10}", pollards.steps_to_sqrt_mod_ratio());

        assert_eq!(factor1 * factor2, pollards.n);
    }

}



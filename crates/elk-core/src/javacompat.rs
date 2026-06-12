//! Bit-exact replicas of Java runtime behavior that layout results depend on.

/// `java.util.Random`: 48-bit LCG, identical sequences for identical seeds.
pub struct JavaRandom {
    seed: u64,
}

const MULTIPLIER: u64 = 0x5DEECE66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1 << 48) - 1;

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        JavaRandom { seed: (seed as u64 ^ MULTIPLIER) & MASK }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// `nextInt(bound)`, Java's rejection sampling.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        if (bound & -bound) == bound {
            // power of two
            return ((bound as i64).wrapping_mul(self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 {
                return val;
            }
        }
    }

    pub fn next_double(&mut self) -> f64 {
        let high = (self.next(26) as i64) << 27;
        let low = self.next(27) as i64;
        (high + low) as f64 * (1.0f64 / (1i64 << 53) as f64)
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    pub fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }
}

/// `java.util.PriorityQueue`: binary min-heap with Java's exact sift
/// semantics, so tie-breaking matches the JVM element order.
pub struct JavaPriorityQueue<T> {
    heap: Vec<T>,
    cmp: fn(&T, &T) -> std::cmp::Ordering,
}

impl<T> JavaPriorityQueue<T> {
    pub fn new(cmp: fn(&T, &T) -> std::cmp::Ordering) -> Self {
        JavaPriorityQueue { heap: Vec::new(), cmp }
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn add(&mut self, item: T) {
        self.heap.push(item);
        let i = self.heap.len() - 1;
        self.sift_up(i);
    }

    pub fn peek(&self) -> Option<&T> {
        self.heap.first()
    }

    pub fn poll(&mut self) -> Option<T> {
        if self.heap.is_empty() {
            return None;
        }
        let last = self.heap.len() - 1;
        self.heap.swap(0, last);
        let result = self.heap.pop();
        if !self.heap.is_empty() {
            self.sift_down(0);
        }
        result
    }

    fn sift_up(&mut self, mut k: usize) {
        while k > 0 {
            let parent = (k - 1) >> 1;
            if (self.cmp)(&self.heap[k], &self.heap[parent]) == std::cmp::Ordering::Less {
                self.heap.swap(k, parent);
                k = parent;
            } else {
                break;
            }
        }
    }

    fn sift_down(&mut self, mut k: usize) {
        let n = self.heap.len();
        let half = n >> 1;
        while k < half {
            let mut child = 2 * k + 1;
            let right = child + 1;
            if right < n
                && (self.cmp)(&self.heap[right], &self.heap[child]) == std::cmp::Ordering::Less
            {
                child = right;
            }
            if (self.cmp)(&self.heap[child], &self.heap[k]) == std::cmp::Ordering::Less {
                self.heap.swap(k, child);
                k = child;
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_random_known_sequence() {
        // Values verified against java.util.Random with seed 42.
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_int(), -1170105035);
        assert_eq!(r.next_int(), 234785527);
        let mut r = JavaRandom::new(42);
        assert!((r.next_double() - 0.7275636800328681).abs() < 1e-18);
        assert!((r.next_double() - 0.6832234717598454).abs() < 1e-18);
        let mut r = JavaRandom::new(0);
        assert_eq!(r.next_int_bound(100), 60);
    }

    #[test]
    fn priority_queue_poll_order() {
        let mut q: JavaPriorityQueue<i32> = JavaPriorityQueue::new(|a, b| a.cmp(b));
        for v in [5, 1, 4, 2, 3] {
            q.add(v);
        }
        let mut out = Vec::new();
        while let Some(v) = q.poll() {
            out.push(v);
        }
        assert_eq!(out, vec![1, 2, 3, 4, 5]);
    }
}

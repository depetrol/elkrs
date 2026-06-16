//! A faithful replica of `java.util.HashSet` (JDK 8+ `HashMap` backing) for
//! value types with `hashCode`/`equals`. Iteration order of these
//! hash sets leaks into layout results (e.g. the Bowyer-Watson triangulation
//! feeds a stable sort whose tie-breaking depends on set order), so we model
//! the bucket table exactly: lazy allocation at capacity 16, load factor
//! 0.75, tail insertion, and capacity-doubling resizes that split each bucket
//! into a lo/hi list preserving relative order. `clear` keeps the table
//! capacity.
//!
//! Treeification (chains of 9+ entries with table capacity >= 64) is not
//! implemented and panics; it cannot occur for the small inputs this code is
//! used for without colliding hash codes.

/// `Object.hashCode`/`equals` for a value type.
pub trait JHashEq {
    fn jhash(&self) -> i32;
    fn jeq(&self, other: &Self) -> bool;
}

/// `Double.hashCode` (via `doubleToLongBits`).
pub fn java_double_hash(d: f64) -> i32 {
    // doubleToLongBits canonicalizes NaN; layout coordinates are not NaN.
    let bits = d.to_bits() as i64;
    (bits ^ ((bits as u64) >> 32) as i64) as i32
}

/// `KVector.hashCode()`:
/// `Double.valueOf(x).hashCode() + Integer.reverse(Double.valueOf(y).hashCode())`.
pub fn java_kvector_hash(x: f64, y: f64) -> i32 {
    java_double_hash(x).wrapping_add(java_double_hash(y).reverse_bits())
}

/// JDK 8 `HashMap.hash` spreading.
fn spread(h: i32) -> i32 {
    h ^ ((h as u32) >> 16) as i32
}

struct Entry<T> {
    hash: i32, // spread hash
    value: T,
}

/// `HashSet<T>` with exact iteration order.
pub struct JavaHashSet<T> {
    /// `None` until the first insertion (lazy table allocation).
    table: Option<Vec<Vec<Entry<T>>>>,
    size: usize,
    threshold: usize,
}

impl<T: JHashEq> Default for JavaHashSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: JHashEq> JavaHashSet<T> {
    pub fn new() -> Self {
        JavaHashSet { table: None, size: 0, threshold: 0 }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// `HashSet.add`: returns false if an equal element already exists.
    pub fn add(&mut self, value: T) -> bool {
        if self.table.is_none() {
            // resize(): default capacity 16, threshold 12
            self.table = Some((0..16).map(|_| Vec::new()).collect());
            self.threshold = 12;
        }
        let hash = spread(value.jhash());
        let table = self.table.as_mut().unwrap();
        let n = table.len();
        let idx = ((n as i32 - 1) & hash) as usize;
        let bucket = &mut table[idx];
        for e in bucket.iter() {
            if e.hash == hash && e.value.jeq(&value) {
                return false; // already present; the existing key is kept
            }
        }
        bucket.push(Entry { hash, value });
        let chain_len = bucket.len();
        self.size += 1;
        // putVal: treeify check happens before the size/threshold resize.
        if chain_len >= 9 {
            if n < 64 {
                self.resize();
            } else {
                panic!("JavaHashSet: treeification not implemented (bucket chain of 9 at capacity >= 64)");
            }
        }
        if self.size > self.threshold {
            self.resize();
        }
        true
    }

    /// `HashSet.remove`.
    pub fn remove(&mut self, value: &T) -> bool {
        let Some(table) = self.table.as_mut() else { return false };
        let hash = spread(value.jhash());
        let n = table.len();
        let idx = ((n as i32 - 1) & hash) as usize;
        let bucket = &mut table[idx];
        if let Some(pos) = bucket
            .iter()
            .position(|e| e.hash == hash && e.value.jeq(value))
        {
            bucket.remove(pos);
            self.size -= 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, value: &T) -> bool {
        let Some(table) = self.table.as_ref() else { return false };
        let hash = spread(value.jhash());
        let n = table.len();
        let idx = ((n as i32 - 1) & hash) as usize;
        table[idx]
            .iter()
            .any(|e| e.hash == hash && e.value.jeq(value))
    }

    /// `HashMap.clear`: keeps the current table capacity.
    pub fn clear(&mut self) {
        if let Some(table) = self.table.as_mut() {
            for bucket in table.iter_mut() {
                bucket.clear();
            }
        }
        self.size = 0;
    }

    /// Iterates in `HashMap` iteration order (table order, chain order).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.table
            .iter()
            .flat_map(|t| t.iter())
            .flat_map(|bucket| bucket.iter().map(|e| &e.value))
    }

    fn resize(&mut self) {
        let table = self.table.as_mut().unwrap();
        let old_cap = table.len();
        let new_cap = old_cap << 1;
        self.threshold <<= 1;
        let mut new_table: Vec<Vec<Entry<T>>> = (0..new_cap).map(|_| Vec::new()).collect();
        for (j, bucket) in table.drain(..).enumerate() {
            // Split preserving order: lo stays at j, hi goes to j + old_cap.
            for e in bucket {
                let hi = (e.hash & old_cap as i32) != 0;
                let target = if hi { j + old_cap } else { j };
                new_table[target].push(e);
            }
        }
        *self.table.as_mut().unwrap() = new_table;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl JHashEq for i32 {
        fn jhash(&self) -> i32 {
            *self
        }
        fn jeq(&self, other: &Self) -> bool {
            self == other
        }
    }

    #[test]
    fn iteration_order_matches_java_small() {
        // Table 16: bucket1=[1], bucket3=[3], bucket5=[5,21].
        let mut s = JavaHashSet::new();
        for v in [5, 21, 3, 1] {
            assert!(s.add(v));
        }
        assert!(!s.add(5));
        let order: Vec<i32> = s.iter().copied().collect();
        assert_eq!(order, vec![1, 3, 5, 21]);
    }

    #[test]
    fn resize_splits_buckets() {
        // 13 entries trigger a resize to 32 (threshold 12).
        let mut s = JavaHashSet::new();
        for v in 0..13 {
            s.add(v * 16); // all collide in bucket 0 pre-resize? no: spread mixes
        }
        assert_eq!(s.len(), 13);
    }
}

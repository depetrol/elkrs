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

    /// `java.util.Random.setSeed(long)`: re-scrambles exactly like the
    /// constructor (`(seed ^ 0x5DEECE66D) & ((1 << 48) - 1)`).
    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed as u64 ^ MULTIPLIER) & MASK;
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// `nextInt(bound)`, with rejection sampling.
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

/// A binary min-heap replicating `java.util.PriorityQueue`'s exact sift-up /
/// sift-down order, so tie-breaking matches its element ordering bit-for-bit.
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

/// Faithful port of `java.util.Arrays.sort(T[], Comparator)` = TimSort, with a
/// mutating comparator (`FnMut`). The exact sequence of `compare` calls matches
/// OpenJDK's TimSort, which matters when the comparator carries state (e.g. the
/// model-order comparators' transitive-ordering maps).
///
/// `cmp(a, b)` returns negative / zero / positive like a comparator's `compare`.
pub fn tim_sort<T, F>(a: &mut [T], mut cmp: F)
where
    T: Clone,
    F: FnMut(&T, &T) -> i32,
{
    let n = a.len();
    if n < 2 {
        return;
    }
    const MIN_MERGE: usize = 32;
    if n < MIN_MERGE {
        // A "mini-TimSort" with no merges: count run then binary insertion sort.
        let init_run_len = count_run_and_make_ascending(a, &mut cmp);
        binary_sort(a, init_run_len, &mut cmp);
        return;
    }

    let mut ts = TimSortState::new(n);
    let min_run = min_run_length(n);
    let mut lo = 0usize;
    let mut n_remaining = n;
    loop {
        let mut run_len = count_run_and_make_ascending_from(a, lo, &mut cmp);
        if run_len < min_run {
            let force = n_remaining.min(min_run);
            binary_sort_range(a, lo, lo + force, lo + run_len, &mut cmp);
            run_len = force;
        }
        ts.push_run(lo, run_len);
        ts.merge_collapse(a, &mut cmp);
        lo += run_len;
        n_remaining -= run_len;
        if n_remaining == 0 {
            break;
        }
    }
    debug_assert_eq!(lo, n);
    ts.merge_force_collapse(a, &mut cmp);
    debug_assert_eq!(ts.stack_size, 1);
}

/// Binary insertion sort over the whole slice with `start` already sorted
/// (`[0, start)` is sorted). Used by the mini-TimSort path.
fn binary_sort<T: Clone, F: FnMut(&T, &T) -> i32>(a: &mut [T], start: usize, cmp: &mut F) {
    let hi = a.len();
    binary_sort_range(a, 0, hi, start, cmp);
}

fn binary_sort_range<T: Clone, F: FnMut(&T, &T) -> i32>(
    a: &mut [T],
    lo: usize,
    hi: usize,
    mut start: usize,
    cmp: &mut F,
) {
    debug_assert!(lo <= start && start <= hi);
    if start == lo {
        start += 1;
    }
    while start < hi {
        let pivot = a[start].clone();
        let mut left = lo;
        let mut right = start;
        while left < right {
            let mid = (left + right) >> 1;
            if cmp(&pivot, &a[mid]) < 0 {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        // Shift [left, start) right by one, then drop pivot at `left`.
        let mut i = start;
        while i > left {
            a[i] = a[i - 1].clone();
            i -= 1;
        }
        a[left] = pivot;
        start += 1;
    }
}

fn count_run_and_make_ascending<T: Clone, F: FnMut(&T, &T) -> i32>(
    a: &mut [T],
    cmp: &mut F,
) -> usize {
    count_run_and_make_ascending_from(a, 0, cmp)
}

fn count_run_and_make_ascending_from<T: Clone, F: FnMut(&T, &T) -> i32>(
    a: &mut [T],
    lo: usize,
    cmp: &mut F,
) -> usize {
    let hi = a.len();
    debug_assert!(lo < hi);
    let mut run_hi = lo + 1;
    if run_hi == hi {
        return 1;
    }
    if cmp(&a[run_hi], &a[lo]) < 0 {
        // descending
        run_hi += 1;
        while run_hi < hi && cmp(&a[run_hi], &a[run_hi - 1]) < 0 {
            run_hi += 1;
        }
        a[lo..run_hi].reverse();
    } else {
        // ascending
        run_hi += 1;
        while run_hi < hi && cmp(&a[run_hi], &a[run_hi - 1]) >= 0 {
            run_hi += 1;
        }
    }
    run_hi - lo
}

fn min_run_length(mut n: usize) -> usize {
    let mut r = 0usize;
    while n >= 32 {
        r |= n & 1;
        n >>= 1;
    }
    n + r
}

struct TimSortState {
    run_base: Vec<usize>,
    run_len: Vec<usize>,
    stack_size: usize,
    min_gallop: usize,
}

const MIN_GALLOP: usize = 7;

impl TimSortState {
    fn new(len: usize) -> Self {
        let stack_len = if len < 120 {
            5
        } else if len < 1542 {
            10
        } else if len < 119151 {
            24
        } else {
            49
        };
        TimSortState {
            run_base: vec![0; stack_len],
            run_len: vec![0; stack_len],
            stack_size: 0,
            min_gallop: MIN_GALLOP,
        }
    }

    fn push_run(&mut self, base: usize, len: usize) {
        self.run_base[self.stack_size] = base;
        self.run_len[self.stack_size] = len;
        self.stack_size += 1;
    }

    fn merge_collapse<T: Clone, F: FnMut(&T, &T) -> i32>(&mut self, a: &mut [T], cmp: &mut F) {
        while self.stack_size > 1 {
            let mut n = self.stack_size - 2;
            if (n > 0 && self.run_len[n - 1] <= self.run_len[n] + self.run_len[n + 1])
                || (n > 1 && self.run_len[n - 2] <= self.run_len[n - 1] + self.run_len[n])
            {
                if self.run_len[n - 1] < self.run_len[n + 1] {
                    n -= 1;
                }
            } else if n == 0 || self.run_len[n] > self.run_len[n + 1] {
                break;
            }
            self.merge_at(a, n, cmp);
        }
    }

    fn merge_force_collapse<T: Clone, F: FnMut(&T, &T) -> i32>(&mut self, a: &mut [T], cmp: &mut F) {
        while self.stack_size > 1 {
            let mut n = self.stack_size - 2;
            if n > 0 && self.run_len[n - 1] < self.run_len[n + 1] {
                n -= 1;
            }
            self.merge_at(a, n, cmp);
        }
    }

    fn merge_at<T: Clone, F: FnMut(&T, &T) -> i32>(&mut self, a: &mut [T], i: usize, cmp: &mut F) {
        let base1 = self.run_base[i];
        let mut len1 = self.run_len[i];
        let base2 = self.run_base[i + 1];
        let mut len2 = self.run_len[i + 1];

        self.run_len[i] = len1 + len2;
        if self.stack_size >= 3 && i == self.stack_size - 3 {
            self.run_base[i + 1] = self.run_base[i + 2];
            self.run_len[i + 1] = self.run_len[i + 2];
        }
        self.stack_size -= 1;

        // Where does first element of run2 go in run1?
        let k = gallop_right(&a[base2].clone(), a, base1, len1, 0, cmp);
        let base1 = base1 + k;
        len1 -= k;
        if len1 == 0 {
            return;
        }
        // Where does last element of run1 go in run2?
        len2 = gallop_left(&a[base1 + len1 - 1].clone(), a, base2, len2, len2.wrapping_sub(1), cmp);
        if len2 == 0 {
            return;
        }
        if len1 <= len2 {
            self.merge_lo(a, base1, len1, base2, len2, cmp);
        } else {
            self.merge_hi(a, base1, len1, base2, len2, cmp);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn merge_lo<T: Clone, F: FnMut(&T, &T) -> i32>(
        &mut self,
        a: &mut [T],
        base1: usize,
        len1: usize,
        base2: usize,
        len2: usize,
        cmp: &mut F,
    ) {
        let tmp: Vec<T> = a[base1..base1 + len1].to_vec();
        let mut cursor1 = 0usize; // into tmp
        let mut cursor2 = base2; // into a
        let mut dest = base1; // into a

        a[dest] = a[cursor2].clone();
        dest += 1;
        cursor2 += 1;
        let mut len2 = len2 - 1;
        if len2 == 0 {
            a[dest..dest + len1].clone_from_slice(&tmp[cursor1..cursor1 + len1]);
            return;
        }
        let mut len1 = len1;
        if len1 == 1 {
            clone_within(a, cursor2, dest, len2);
            a[dest + len2] = tmp[cursor1].clone();
            return;
        }

        let mut min_gallop = self.min_gallop;
        'outer: loop {
            let mut count1 = 0usize;
            let mut count2 = 0usize;
            // straightforward merge
            loop {
                debug_assert!(len1 > 1 && len2 > 0);
                if cmp(&a[cursor2], &tmp[cursor1]) < 0 {
                    a[dest] = a[cursor2].clone();
                    dest += 1;
                    cursor2 += 1;
                    count2 += 1;
                    count1 = 0;
                    len2 -= 1;
                    if len2 == 0 {
                        break 'outer;
                    }
                } else {
                    a[dest] = tmp[cursor1].clone();
                    dest += 1;
                    cursor1 += 1;
                    count1 += 1;
                    count2 = 0;
                    len1 -= 1;
                    if len1 == 1 {
                        break 'outer;
                    }
                }
                if (count1 | count2) >= min_gallop {
                    break;
                }
            }
            // gallop mode
            loop {
                debug_assert!(len1 > 1 && len2 > 0);
                count1 = gallop_right(&a[cursor2].clone(), &tmp, cursor1, len1, 0, cmp);
                if count1 != 0 {
                    a[dest..dest + count1].clone_from_slice(&tmp[cursor1..cursor1 + count1]);
                    dest += count1;
                    cursor1 += count1;
                    len1 -= count1;
                    if len1 <= 1 {
                        break 'outer;
                    }
                }
                a[dest] = a[cursor2].clone();
                dest += 1;
                cursor2 += 1;
                len2 -= 1;
                if len2 == 0 {
                    break 'outer;
                }
                count2 = gallop_left(&tmp[cursor1].clone(), a, cursor2, len2, 0, cmp);
                if count2 != 0 {
                    clone_within(a, cursor2, dest, count2);
                    dest += count2;
                    cursor2 += count2;
                    len2 -= count2;
                    if len2 == 0 {
                        break 'outer;
                    }
                }
                a[dest] = tmp[cursor1].clone();
                dest += 1;
                cursor1 += 1;
                len1 -= 1;
                if len1 == 1 {
                    break 'outer;
                }
                min_gallop = min_gallop.saturating_sub(1);
                if count1 < MIN_GALLOP && count2 < MIN_GALLOP {
                    break;
                }
            }
            if min_gallop < 1 {
                min_gallop = 1;
            }
            min_gallop += 1;
        }
        self.min_gallop = min_gallop.max(1);

        if len1 == 1 {
            debug_assert!(len2 > 0);
            clone_within(a, cursor2, dest, len2);
            a[dest + len2] = tmp[cursor1].clone();
        } else {
            debug_assert!(len1 != 0, "comparison method violates its general contract");
            debug_assert_eq!(len2, 0);
            a[dest..dest + len1].clone_from_slice(&tmp[cursor1..cursor1 + len1]);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn merge_hi<T: Clone, F: FnMut(&T, &T) -> i32>(
        &mut self,
        a: &mut [T],
        base1: usize,
        mut len1: usize,
        base2: usize,
        mut len2: usize,
        cmp: &mut F,
    ) {
        let tmp: Vec<T> = a[base2..base2 + len2].to_vec();
        // Cursors are kept as i64 (they legitimately reach -1 between the
        // copy and the following decrement).
        let mut cursor1: i64 = (base1 + len1 - 1) as i64; // into a
        let mut cursor2: i64 = len2 as i64 - 1; // into tmp
        let mut dest: i64 = (base2 + len2 - 1) as i64; // into a

        a[dest as usize] = a[cursor1 as usize].clone();
        dest -= 1;
        cursor1 -= 1;
        len1 -= 1;
        if len1 == 0 {
            let start = (dest - len2 as i64 + 1) as usize;
            a[start..start + len2].clone_from_slice(&tmp[0..len2]);
            return;
        }
        if len2 == 1 {
            dest -= len1 as i64;
            cursor1 -= len1 as i64;
            let src = (cursor1 + 1) as usize;
            clone_within(a, src, (dest + 1) as usize, len1);
            a[dest as usize] = tmp[cursor2 as usize].clone();
            return;
        }

        let mut min_gallop = self.min_gallop;
        'outer: loop {
            let mut count1 = 0usize;
            let mut count2 = 0usize;
            loop {
                debug_assert!(len1 > 0 && len2 > 1);
                if cmp(&tmp[cursor2 as usize], &a[cursor1 as usize]) < 0 {
                    a[dest as usize] = a[cursor1 as usize].clone();
                    dest -= 1;
                    cursor1 -= 1;
                    count1 += 1;
                    count2 = 0;
                    len1 -= 1;
                    if len1 == 0 {
                        break 'outer;
                    }
                } else {
                    a[dest as usize] = tmp[cursor2 as usize].clone();
                    dest -= 1;
                    cursor2 -= 1;
                    count2 += 1;
                    count1 = 0;
                    len2 -= 1;
                    if len2 == 1 {
                        break 'outer;
                    }
                }
                if (count1 | count2) >= min_gallop {
                    break;
                }
            }
            loop {
                debug_assert!(len1 > 0 && len2 > 1);
                let key = tmp[cursor2 as usize].clone();
                count1 = len1 - gallop_right(&key, a, base1, len1, len1 - 1, cmp);
                if count1 != 0 {
                    dest -= count1 as i64;
                    cursor1 -= count1 as i64;
                    len1 -= count1;
                    let src = (cursor1 + 1) as usize;
                    clone_within(a, src, (dest + 1) as usize, count1);
                    if len1 == 0 {
                        break 'outer;
                    }
                }
                a[dest as usize] = tmp[cursor2 as usize].clone();
                dest -= 1;
                cursor2 -= 1;
                len2 -= 1;
                if len2 == 1 {
                    break 'outer;
                }
                let key2 = a[cursor1 as usize].clone();
                count2 = len2 - gallop_left(&key2, &tmp, 0, len2, len2 - 1, cmp);
                if count2 != 0 {
                    dest -= count2 as i64;
                    cursor2 -= count2 as i64;
                    len2 -= count2;
                    let src = (cursor2 + 1) as usize;
                    a[(dest + 1) as usize..(dest + 1) as usize + count2]
                        .clone_from_slice(&tmp[src..src + count2]);
                    if len2 <= 1 {
                        break 'outer;
                    }
                }
                a[dest as usize] = a[cursor1 as usize].clone();
                dest -= 1;
                cursor1 -= 1;
                len1 -= 1;
                if len1 == 0 {
                    break 'outer;
                }
                min_gallop = min_gallop.saturating_sub(1);
                if count1 < MIN_GALLOP && count2 < MIN_GALLOP {
                    break;
                }
            }
            if min_gallop < 1 {
                min_gallop = 1;
            }
            min_gallop += 1;
        }
        self.min_gallop = min_gallop.max(1);

        if len2 == 1 {
            debug_assert!(len1 > 0);
            dest -= len1 as i64;
            cursor1 -= len1 as i64;
            let src = (cursor1 + 1) as usize;
            clone_within(a, src, (dest + 1) as usize, len1);
            a[dest as usize] = tmp[cursor2 as usize].clone();
        } else {
            debug_assert!(len2 != 0, "comparison method violates its general contract");
            debug_assert_eq!(len1, 0);
            let start = (dest - len2 as i64 + 1) as usize;
            a[start..start + len2].clone_from_slice(&tmp[0..len2]);
        }
    }
}

/// Clone `count` elements from `src_start` to `dst_start` within one slice,
/// handling overlap (equivalent to `System.arraycopy` on the same array).
fn clone_within<T: Clone>(a: &mut [T], src_start: usize, dst_start: usize, count: usize) {
    if count == 0 || src_start == dst_start {
        return;
    }
    if dst_start < src_start {
        for k in 0..count {
            a[dst_start + k] = a[src_start + k].clone();
        }
    } else {
        for k in (0..count).rev() {
            a[dst_start + k] = a[src_start + k].clone();
        }
    }
}

/// `gallopLeft`: locate `key`'s leftmost insertion point in `a[base..base+len]`.
fn gallop_left<T: Clone, F: FnMut(&T, &T) -> i32>(
    key: &T,
    a: &[T],
    base: usize,
    len: usize,
    hint: usize,
    cmp: &mut F,
) -> usize {
    debug_assert!(len > 0);
    let mut last_ofs: isize = 0;
    let mut ofs: isize = 1;
    let hint = hint as isize;
    if cmp(key, &a[base + hint as usize]) > 0 {
        let max_ofs = len as isize - hint;
        while ofs < max_ofs && cmp(key, &a[base + (hint + ofs) as usize]) > 0 {
            last_ofs = ofs;
            ofs = (ofs << 1) + 1;
            if ofs <= 0 {
                ofs = max_ofs;
            }
        }
        if ofs > max_ofs {
            ofs = max_ofs;
        }
        last_ofs += hint;
        ofs += hint;
    } else {
        let max_ofs = hint + 1;
        while ofs < max_ofs && cmp(key, &a[base + (hint - ofs) as usize]) <= 0 {
            last_ofs = ofs;
            ofs = (ofs << 1) + 1;
            if ofs <= 0 {
                ofs = max_ofs;
            }
        }
        if ofs > max_ofs {
            ofs = max_ofs;
        }
        let tmp = last_ofs;
        last_ofs = hint - ofs;
        ofs = hint - tmp;
    }
    let mut last_ofs = last_ofs + 1;
    while last_ofs < ofs {
        let m = last_ofs + ((ofs - last_ofs) >> 1);
        if cmp(key, &a[base + m as usize]) > 0 {
            last_ofs = m + 1;
        } else {
            ofs = m;
        }
    }
    ofs as usize
}

/// `gallopRight`: locate `key`'s rightmost insertion point in `a[base..base+len]`.
fn gallop_right<T: Clone, F: FnMut(&T, &T) -> i32>(
    key: &T,
    a: &[T],
    base: usize,
    len: usize,
    hint: usize,
    cmp: &mut F,
) -> usize {
    debug_assert!(len > 0);
    let mut ofs: isize = 1;
    let mut last_ofs: isize = 0;
    let hint = hint as isize;
    if cmp(key, &a[base + hint as usize]) < 0 {
        let max_ofs = hint + 1;
        while ofs < max_ofs && cmp(key, &a[base + (hint - ofs) as usize]) < 0 {
            last_ofs = ofs;
            ofs = (ofs << 1) + 1;
            if ofs <= 0 {
                ofs = max_ofs;
            }
        }
        if ofs > max_ofs {
            ofs = max_ofs;
        }
        let tmp = last_ofs;
        last_ofs = hint - ofs;
        ofs = hint - tmp;
    } else {
        let max_ofs = len as isize - hint;
        while ofs < max_ofs && cmp(key, &a[base + (hint + ofs) as usize]) >= 0 {
            last_ofs = ofs;
            ofs = (ofs << 1) + 1;
            if ofs <= 0 {
                ofs = max_ofs;
            }
        }
        if ofs > max_ofs {
            ofs = max_ofs;
        }
        last_ofs += hint;
        ofs += hint;
    }
    let mut last_ofs = last_ofs + 1;
    while last_ofs < ofs {
        let m = last_ofs + ((ofs - last_ofs) >> 1);
        if cmp(key, &a[base + m as usize]) < 0 {
            ofs = m;
        } else {
            last_ofs = m + 1;
        }
    }
    ofs as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tim_sort_matches_stable_sort() {
        for n in 0..500usize {
            for &m in &[2i32, 5, 11, 97, 1000] {
                let mut v: Vec<(i32, usize)> =
                    (0..n).map(|i| (((i * 7 + 3) % m as usize) as i32, i)).collect();
                let mut expected = v.clone();
                expected.sort_by(|x, y| x.0.cmp(&y.0)); // stable
                tim_sort(&mut v, |x, y| x.0 - y.0);
                assert_eq!(v, expected, "n={n} m={m}");
            }
        }
    }

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

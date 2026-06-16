//! 2D vector math mirroring `org.eclipse.elk.core.math`.

use std::fmt;

/// A simple 2D vector, port of `KVector`. We provide both mutating methods and
/// value-returning helpers where that reads better.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct KVector {
    pub x: f64,
    pub y: f64,
}

impl KVector {
    pub const fn new(x: f64, y: f64) -> Self {
        KVector { x, y }
    }

    /// Vector pointing from `start` to `end`.
    pub fn between(start: KVector, end: KVector) -> Self {
        KVector::new(end.x - start.x, end.y - start.y)
    }

    /// Normalized vector for the given angle in radians.
    pub fn from_angle(angle: f64) -> Self {
        KVector::new(angle.cos(), angle.sin())
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn square_length(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    pub fn reset(&mut self) -> &mut Self {
        self.x = 0.0;
        self.y = 0.0;
        self
    }

    pub fn set(&mut self, x: f64, y: f64) -> &mut Self {
        self.x = x;
        self.y = y;
        self
    }

    pub fn add(&mut self, v: KVector) -> &mut Self {
        self.x += v.x;
        self.y += v.y;
        self
    }

    pub fn add_xy(&mut self, dx: f64, dy: f64) -> &mut Self {
        self.x += dx;
        self.y += dy;
        self
    }

    pub fn sum(vs: &[KVector]) -> KVector {
        let mut sum = KVector::default();
        for v in vs {
            sum.x += v.x;
            sum.y += v.y;
        }
        sum
    }

    pub fn sub(&mut self, v: KVector) -> &mut Self {
        self.x -= v.x;
        self.y -= v.y;
        self
    }

    pub fn sub_xy(&mut self, dx: f64, dy: f64) -> &mut Self {
        self.x -= dx;
        self.y -= dy;
        self
    }

    pub fn diff(v1: KVector, v2: KVector) -> KVector {
        KVector::new(v1.x - v2.x, v1.y - v2.y)
    }

    pub fn scale(&mut self, scale: f64) -> &mut Self {
        self.x *= scale;
        self.y *= scale;
        self
    }

    pub fn scale_xy(&mut self, sx: f64, sy: f64) -> &mut Self {
        self.x *= sx;
        self.y *= sy;
        self
    }

    pub fn normalize(&mut self) -> &mut Self {
        let length = self.length();
        if length > 0.0 {
            self.x /= length;
            self.y /= length;
        }
        self
    }

    pub fn scale_to_length(&mut self, length: f64) -> &mut Self {
        self.normalize();
        self.scale(length);
        self
    }

    pub fn negate(&mut self) -> &mut Self {
        self.x = -self.x;
        self.y = -self.y;
        self
    }

    /// Angle in degrees, within [0, 360). Length must not be 0.
    pub fn to_degrees(&self) -> f64 {
        self.to_radians().to_degrees()
    }

    /// Angle in radians, within [0, 2*pi). Length must not be 0.
    pub fn to_radians(&self) -> f64 {
        let length = self.length();
        debug_assert!(length > 0.0);
        if self.x >= 0.0 && self.y >= 0.0 {
            (self.y / length).asin()
        } else if self.x < 0.0 {
            std::f64::consts::PI - (self.y / length).asin()
        } else {
            2.0 * std::f64::consts::PI + (self.y / length).asin()
        }
    }

    pub fn distance(&self, v2: KVector) -> f64 {
        let dx = self.x - v2.x;
        let dy = self.y - v2.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn dot_product(&self, v2: KVector) -> f64 {
        self.x * v2.x + self.y * v2.y
    }

    pub fn cross_product(v: KVector, w: KVector) -> f64 {
        v.x * w.y - v.y * w.x
    }

    /// Rotate anti-clockwise by `angle` radians.
    pub fn rotate(&mut self, angle: f64) -> &mut Self {
        let new_x = self.x * angle.cos() - self.y * angle.sin();
        self.y = self.x * angle.sin() + self.y * angle.cos();
        self.x = new_x;
        self
    }

    pub fn angle(&self, other: KVector) -> f64 {
        (self.dot_product(other) / (self.length() * other.length())).acos()
    }

    pub fn bound(&mut self, lowx: f64, lowy: f64, highx: f64, highy: f64) -> &mut Self {
        assert!(
            highx >= lowx && highy >= lowy,
            "The highx must be bigger then lowx and the highy must be bigger then lowy"
        );
        if self.x < lowx {
            self.x = lowx;
        } else if self.x > highx {
            self.x = highx;
        }
        if self.y < lowy {
            self.y = lowy;
        } else if self.y > highy {
            self.y = highy;
        }
        self
    }

    pub fn is_nan(&self) -> bool {
        self.x.is_nan() || self.y.is_nan()
    }

    pub fn is_infinite(&self) -> bool {
        self.x.is_infinite() || self.y.is_infinite()
    }

    pub fn equals_fuzzily(&self, other: KVector, fuzzyness: f64) -> bool {
        (self.x - other.x).abs() <= fuzzyness && (self.y - other.y).abs() <= fuzzyness
    }

    /// Parse from ELK's `(x,y)` string form.
    pub fn parse(string: &str) -> Result<KVector, String> {
        let trimmed = string
            .trim_matches(|c: char| "([{\"' \t\r\n".contains(c) || ")]}".contains(c));
        let tokens: Vec<&str> = trimmed
            .split(|c: char| c == ',' || c == ';' || c == '\r' || c == '\n')
            .collect();
        if tokens.len() != 2 {
            return Err(format!(
                "Exactly two numbers are expected, {} were found.",
                tokens.len()
            ));
        }
        let x = tokens[0].trim().parse::<f64>().map_err(|e| e.to_string())?;
        let y = tokens[1].trim().parse::<f64>().map_err(|e| e.to_string())?;
        Ok(KVector::new(x, y))
    }
}

impl fmt::Display for KVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", fmt_java_double(self.x), fmt_java_double(self.y))
    }
}

impl fmt::Debug for KVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Format an f64 per the `Double.toString` spec for the common cases
/// (integral values get a trailing `.0`). The full spec (shortest
/// roundtrip representation) is produced by Rust's `{}` for non-integral values.
pub fn fmt_java_double(v: f64) -> String {
    if v.is_finite() && v == v.trunc() && v.abs() < 1e7 {
        format!("{:.1}", v)
    } else {
        format!("{}", v)
    }
}

/// A chain of vectors, port of `KVectorChain` (a `LinkedList<KVector>`).
/// Backed by a `Vec`; ELK only relies on order, not on linked-list identity.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct KVectorChain(pub Vec<KVector>);

impl KVectorChain {
    pub fn new() -> Self {
        KVectorChain(Vec::new())
    }

    pub fn of(vectors: &[KVector]) -> Self {
        KVectorChain(vectors.to_vec())
    }

    pub fn add(&mut self, x: f64, y: f64) {
        self.0.push(KVector::new(x, y));
    }

    pub fn add_first(&mut self, v: KVector) {
        self.0.insert(0, v);
    }

    pub fn add_last(&mut self, v: KVector) {
        self.0.push(v);
    }

    pub fn first(&self) -> KVector {
        self.0[0]
    }

    pub fn last(&self) -> KVector {
        *self.0.last().unwrap()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, KVector> {
        self.0.iter()
    }

    pub fn scale(&mut self, scale: f64) -> &mut Self {
        for v in &mut self.0 {
            v.scale(scale);
        }
        self
    }

    pub fn scale_xy(&mut self, sx: f64, sy: f64) -> &mut Self {
        for v in &mut self.0 {
            v.scale_xy(sx, sy);
        }
        self
    }

    pub fn offset(&mut self, offset: KVector) -> &mut Self {
        for v in &mut self.0 {
            v.add(offset);
        }
        self
    }

    pub fn offset_xy(&mut self, dx: f64, dy: f64) -> &mut Self {
        for v in &mut self.0 {
            v.add_xy(dx, dy);
        }
        self
    }

    pub fn total_length(&self) -> f64 {
        let mut length = 0.0;
        for w in self.0.windows(2) {
            length += w[0].distance(w[1]);
        }
        length
    }

    pub fn has_nan(&self) -> bool {
        self.0.iter().any(KVector::is_nan)
    }

    pub fn has_infinite(&self) -> bool {
        self.0.iter().any(KVector::is_infinite)
    }

    /// Point on the chain at the given distance from the first point
    /// (negative: from the last point, traversing backwards).
    pub fn point_on_line(&self, dist: f64) -> KVector {
        match self.0.len() {
            0 => panic!("Cannot determine a point on an empty vector chain."),
            1 => self.0[0],
            _ => {
                let abs_distance = dist.abs();
                let mut distance_sum = 0.0;
                let forward = dist >= 0.0;
                let n = self.0.len();
                let idx = |i: usize| if forward { i } else { n - 1 - i };
                for i in 0..n - 1 {
                    let current = self.0[idx(i)];
                    let next = self.0[idx(i + 1)];
                    let old_distance_sum = distance_sum;
                    let additional = current.distance(next);
                    if additional > 0.0 {
                        distance_sum += additional;
                        if distance_sum >= abs_distance {
                            let rel = (abs_distance - old_distance_sum) / additional;
                            let mut result = next;
                            result.sub(current).scale(rel).add(current);
                            return result;
                        }
                    }
                }
                self.0[idx(n - 1)]
            }
        }
    }

    /// Angle (radians) of the segment at the given distance along the chain.
    pub fn angle_on_line(&self, dist: f64) -> f64 {
        assert!(self.0.len() >= 2, "Need at least two points to determine an angle.");
        let abs_distance = dist.abs();
        let mut distance_sum = 0.0;
        let forward = dist >= 0.0;
        let n = self.0.len();
        let idx = |i: usize| if forward { i } else { n - 1 - i };
        let mut current = self.0[idx(0)];
        let mut next = self.0[idx(1)];
        for i in 0..n - 1 {
            current = self.0[idx(i)];
            next = self.0[idx(i + 1)];
            let additional = current.distance(next);
            if additional > 0.0 {
                distance_sum += additional;
                if distance_sum >= abs_distance {
                    break;
                }
            }
        }
        let mut seg = next;
        seg.sub(current);
        seg.to_radians()
    }

    pub fn reverse(chain: &KVectorChain) -> KVectorChain {
        let mut result = chain.0.clone();
        result.reverse();
        KVectorChain(result)
    }

    /// Parse from ELK's `(x1,y1; x2,y2; ...)` string form.
    pub fn parse(string: &str) -> Result<KVectorChain, String> {
        let mut chain = KVectorChain::new();
        let tokens = string.split(|c: char| ",;()[]{} \t\n".contains(c));
        let mut xy = 0u32;
        let mut x = 0.0f64;
        for tok in tokens {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            let v = tok.parse::<f64>().map_err(|e| {
                format!("The given string does not match the expected format for vectors. {e}")
            })?;
            if xy % 2 == 0 {
                x = v;
            } else {
                chain.0.push(KVector::new(x, v));
            }
            xy += 1;
        }
        Ok(chain)
    }
}

impl fmt::Display for KVectorChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, v) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, "; ")?;
            }
            write!(f, "{},{}", fmt_java_double(v.x), fmt_java_double(v.y))?;
        }
        write!(f, ")")
    }
}

impl FromIterator<KVector> for KVectorChain {
    fn from_iter<T: IntoIterator<Item = KVector>>(iter: T) -> Self {
        KVectorChain(iter.into_iter().collect())
    }
}

impl<'a> IntoIterator for &'a KVectorChain {
    type Item = &'a KVector;
    type IntoIter = std::slice::Iter<'a, KVector>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Spacing {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

pub type ElkMargin = Spacing;
pub type ElkPadding = Spacing;

impl Spacing {
    pub const fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Spacing { top, bottom, left, right }
    }

    pub const fn uniform(any: f64) -> Self {
        Spacing::new(any, any, any, any)
    }

    pub const fn of_lr_tb(left_right: f64, top_bottom: f64) -> Self {
        Spacing::new(top_bottom, left_right, top_bottom, left_right)
    }

    pub fn set(&mut self, top: f64, right: f64, bottom: f64, left: f64) {
        self.top = top;
        self.right = right;
        self.bottom = bottom;
        self.left = left;
    }

    pub fn horizontal(&self) -> f64 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f64 {
        self.top + self.bottom
    }

    /// Expects a list of `key=value` pairs
    /// (unknown keys are ignored; an empty string yields all zeros).
    pub fn parse(string: &str) -> Result<Spacing, String> {
        let is_delim = |c: char, delims: &str| delims.contains(c);
        let bytes: Vec<char> = string.chars().collect();
        let mut start = 0usize;
        while start < bytes.len() && is_delim(bytes[start], "([{\"' \t\r\n") {
            start += 1;
        }
        let mut end = bytes.len();
        while end > 0 && is_delim(bytes[end - 1], ")]}\"' \t\r\n") {
            end -= 1;
        }
        let mut s = Spacing::default();
        if start < end {
            let inner: String = bytes[start..end].iter().collect();
            for token in inner.split([',', ';']) {
                let keyandvalue: Vec<&str> = token.split('=').collect();
                if keyandvalue.len() != 2 {
                    return Err("Expecting a list of key-value pairs.".to_string());
                }
                let key = keyandvalue[0].trim();
                let value: f64 = keyandvalue[1]
                    .trim()
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| {
                        format!("The given string contains parts that cannot be parsed as numbers.{e}")
                    })?;
                match key {
                    "top" => s.top = value,
                    "left" => s.left = value,
                    "bottom" => s.bottom = value,
                    "right" => s.right = value,
                    _ => {} // silently ignore unknown keys
                }
            }
        }
        Ok(s)
    }
}

impl fmt::Display for Spacing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[top={},left={},bottom={},right={}]",
            fmt_java_double(self.top),
            fmt_java_double(self.left),
            fmt_java_double(self.bottom),
            fmt_java_double(self.right)
        )
    }
}

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct ElkRectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ElkRectangle {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        ElkRectangle { x, y, width, height }
    }

    pub fn position(&self) -> KVector {
        KVector::new(self.x, self.y)
    }

    pub fn top_right(&self) -> KVector {
        KVector::new(self.x + self.width, self.y)
    }

    pub fn bottom_left(&self) -> KVector {
        KVector::new(self.x, self.y + self.height)
    }

    pub fn bottom_right(&self) -> KVector {
        KVector::new(self.x + self.width, self.y + self.height)
    }

    pub fn center(&self) -> KVector {
        KVector::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn union(&mut self, other: &ElkRectangle) {
        let mut x1 = self.x.min(other.x);
        let mut y1 = self.y.min(other.y);
        let mut x2 = (self.x + self.width).max(other.x + other.width);
        let mut y2 = (self.y + self.height).max(other.y + other.height);
        if x2 < x1 {
            std::mem::swap(&mut x1, &mut x2);
        }
        if y2 < y1 {
            std::mem::swap(&mut y1, &mut y2);
        }
        *self = ElkRectangle::new(x1, y1, x2 - x1, y2 - y1);
    }

    pub fn move_by(&mut self, offset: KVector) {
        self.x += offset.x;
        self.y += offset.y;
    }

    pub fn max_x(&self) -> f64 {
        self.x + self.width
    }

    pub fn max_y(&self) -> f64 {
        self.y + self.height
    }

    pub fn intersects(&self, rect: &ElkRectangle) -> bool {
        self.x < rect.x + rect.width
            && self.x + self.width > rect.x
            && self.y + self.height > rect.y
            && self.y < rect.y + rect.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kvector_basic_ops() {
        let mut v = KVector::new(3.0, 4.0);
        assert_eq!(v.length(), 5.0);
        assert_eq!(v.square_length(), 25.0);
        v.scale(2.0);
        assert_eq!(v, KVector::new(6.0, 8.0));
        v.normalize();
        assert!((v.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn kvector_to_radians_quadrants() {
        use std::f64::consts::PI;
        assert!((KVector::new(1.0, 0.0).to_radians() - 0.0).abs() < 1e-12);
        assert!((KVector::new(0.0, 1.0).to_radians() - PI / 2.0).abs() < 1e-12);
        assert!((KVector::new(-1.0, 0.0).to_radians() - PI).abs() < 1e-12);
        assert!((KVector::new(0.0, -1.0).to_radians() - 3.0 * PI / 2.0).abs() < 1e-12);
    }

    #[test]
    fn kvector_parse() {
        assert_eq!(KVector::parse("(1.5, 2.5)").unwrap(), KVector::new(1.5, 2.5));
        assert_eq!(KVector::parse("3,4").unwrap(), KVector::new(3.0, 4.0));
        assert!(KVector::parse("(1)").is_err());
    }

    #[test]
    fn kvector_display_matches_java() {
        assert_eq!(KVector::new(1.0, 2.5).to_string(), "(1.0,2.5)");
    }

    #[test]
    fn chain_total_length_and_point_on_line() {
        let chain = KVectorChain::of(&[
            KVector::new(0.0, 0.0),
            KVector::new(10.0, 0.0),
            KVector::new(10.0, 10.0),
        ]);
        assert_eq!(chain.total_length(), 20.0);
        assert_eq!(chain.point_on_line(5.0), KVector::new(5.0, 0.0));
        assert_eq!(chain.point_on_line(15.0), KVector::new(10.0, 5.0));
        assert_eq!(chain.point_on_line(-5.0), KVector::new(10.0, 5.0));
        assert_eq!(chain.point_on_line(100.0), KVector::new(10.0, 10.0));
    }

    #[test]
    fn chain_parse() {
        let chain = KVectorChain::parse("(0,0; 10,0; 10,10)").unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.last(), KVector::new(10.0, 10.0));
    }

    #[test]
    fn rectangle_union_and_intersects() {
        let mut r = ElkRectangle::new(0.0, 0.0, 10.0, 10.0);
        r.union(&ElkRectangle::new(5.0, 5.0, 10.0, 10.0));
        assert_eq!(r, ElkRectangle::new(0.0, 0.0, 15.0, 15.0));
        assert!(r.intersects(&ElkRectangle::new(14.0, 14.0, 5.0, 5.0)));
        assert!(!r.intersects(&ElkRectangle::new(15.0, 15.0, 5.0, 5.0)));
    }

    #[test]
    fn spacing_parse_forms() {
        let s = Spacing::parse("[top=1.0,left=2.0,bottom=3.0,right=4.0]").unwrap();
        assert_eq!(s, Spacing::new(1.0, 4.0, 3.0, 2.0));
        // bare numbers (not key=value pairs) are rejected
        assert!(Spacing::parse("5").is_err());
        // unknown keys are silently ignored; empty input yields all zeros
        assert_eq!(Spacing::parse("[foo=7]").unwrap(), Spacing::default());
        assert_eq!(Spacing::parse("[]").unwrap(), Spacing::default());
    }
}

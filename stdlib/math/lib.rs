#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub fn dot(self, rhs: Self) -> f64 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn magnitude(self) -> f64 {
        self.dot(self).sqrt()
    }
}

pub fn checked_add(a: i64, b: i64) -> Option<i64> {
    a.checked_add(b)
}

pub fn checked_mul(a: i64, b: i64) -> Option<i64> {
    a.checked_mul(b)
}

pub fn clamp_f64(v: f64, min: f64, max: f64) -> f64 {
    v.max(min).min(max)
}

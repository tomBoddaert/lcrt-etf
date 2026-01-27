use glam::DVec3;

#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub o: DVec3,
    pub r: f64,
}

impl Sphere {
    #[must_use]
    #[inline]
    pub const fn new(o: DVec3, r: f64) -> Self {
        Self { o, r }
    }

    #[must_use]
    #[inline]
    // If the two spheres intersect with a lens of non-zero volume, returns the distance between their centres.
    pub fn intersection_distance(&self, other: &Self) -> Option<f64> {
        let r = self.r + other.r;
        let d2 = self.o.distance_squared(other.o);
        (d2 < r * r).then(|| d2.sqrt())
    }

    #[must_use]
    #[inline]
    pub fn contains(&self, p: DVec3) -> bool {
        self.o.distance_squared(p) < self.r * self.r
    }

    #[must_use]
    #[inline]
    pub fn distance_to(&self, p: DVec3) -> f64 {
        (self.o.distance(p) - self.r).max(0.)
    }

    #[must_use]
    #[inline]
    pub fn intersection_midpoint(&self, other: &Self) -> DVec3 {
        let r = self.r + other.r;
        let sr = self.r / r;
        let or = other.r / r;
        self.o * or + other.o * sr
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub a: DVec3,
    pub abl: f64,
    pub abn: DVec3,
}

#[derive(Clone, Copy, Debug)]
pub struct SecantResult {
    pub tm: f64,
    pub l2: f64,
}

#[inline]
/// Returns the square root of `n` (as per [`f64::sqrt`]) when `n > 0`.
///
/// Note that `n` must be **strictly** greater than `0`.
fn try_sqrt(n: f64) -> Option<f64> {
    (n > 0.).then(|| n.sqrt())
}

impl Line {
    #[must_use]
    #[inline]
    /// Construct the line containing the segment from `a` to `b`.
    pub fn new(a: DVec3, b: DVec3) -> Self {
        let ab = b - a;
        let abl = ab.length();
        let abn = ab / abl;
        Self { a, abl, abn }
    }

    #[must_use]
    #[inline]
    /// Get the point on the line at **distance** `t` to `a`.
    pub fn interpolate(&self, t: f64) -> DVec3 {
        self.a + t * self.abn
    }

    // #[must_use]
    // #[inline]
    // /// Get the secant intersection with the `sphere`.
    // ///
    // /// Returns [`None`] if no such intersection exists.
    // pub fn sphere_intersection(&self, sphere: Sphere) -> Option<Secant> {
    //     let ao = sphere.o - self.a;
    //     let tm = ao.dot(self.abn);
    //     let m = self.interpolate(tm);
    //     let k2 = (m - sphere.o).length_squared();
    //     let l2 = sphere.r.mul_add(sphere.r, -k2);

    //     try_sqrt(l2).map(|l| Secant {
    //         tc: tm - l,
    //         td: tm + l,
    //     })
    // }

    #[must_use]
    #[inline]
    pub fn sphere_intersection(&self, sphere: &Sphere) -> SecantResult {
        let ao = sphere.o - self.a;
        let tm = ao.dot(self.abn);
        let m = self.interpolate(tm);
        let k2 = (m - sphere.o).length_squared();
        let l2 = sphere.r.mul_add(sphere.r, -k2);
        SecantResult { tm, l2 }
    }

    #[must_use]
    #[inline]
    /// Returns whether the line **segment** from `a` to `b` intersects with the secant, which **must** be computed from `self`.
    pub fn intersects_with_secant(&self, Secant { tc, td }: &Secant) -> bool {
        *td > 0. && *tc < self.abl
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Secant {
    pub tc: f64,
    pub td: f64,
}

impl SecantResult {
    #[must_use]
    #[inline]
    pub fn try_get(self) -> Option<Secant> {
        let Self { tm, l2 } = self;
        try_sqrt(l2).map(|l| Secant {
            tc: tm - l,
            td: tm + l,
        })
    }

    #[must_use]
    #[inline]
    pub fn get_first_unchecked(self) -> f64 {
        let Self { tm, l2 } = self;
        tm - l2.sqrt()
    }
}

impl Secant {
    #[must_use]
    #[inline]
    pub fn first_total_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.tc.total_cmp(&other.tc)
    }

    #[must_use]
    #[inline]
    pub fn second_total_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.td.total_cmp(&other.td)
    }
}

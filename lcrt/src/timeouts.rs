use std::{mem, time};

#[derive(Clone, Debug)]
pub struct Timeouts<const N: usize> {
    active: usize,
    ts: [Option<time::Duration>; N],
}

impl<const N: usize> Timeouts<N> {
    pub const fn new() -> Self {
        Self {
            active: usize::MAX,
            ts: [None; N],
        }
    }

    fn iter(&self) -> impl Iterator<Item = (usize, time::Duration)> {
        self.ts
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(i, t)| t.map(|t| (i, t)))
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut time::Duration> {
        self.ts.iter_mut().flatten()
    }

    pub fn set(&mut self, i: usize, t: time::Duration) -> Option<time::Duration> {
        self.ts[i] = Some(t);

        let (a, t) = self
            .iter()
            .min_by_key(|(_, t)| *t)
            .expect("expected at last one timer to be active");

        (a != self.active).then(|| {
            self.active = a;
            t
        })
    }

    pub fn handle(&mut self) -> (usize, Option<time::Duration>) {
        debug_assert_ne!(self.active, usize::MAX, "no timer running");

        let passed = mem::take(&mut self.ts[self.active]).expect("expected timer to be active");
        self.iter_mut().for_each(|t| *t -= passed);

        let (a, t) = self
            .iter()
            .min_by_key(|(_, t)| *t)
            .map_or((usize::MAX, None), |(a, t)| (a, Some(t)));

        debug_assert_ne!(a, self.active);
        (mem::replace(&mut self.active, a), t)
    }
}

impl<const N: usize> Default for Timeouts<N> {
    fn default() -> Self {
        Self::new()
    }
}

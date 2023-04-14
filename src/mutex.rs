use crate::sem::{SemGuard, SemVar};
use std::cell::UnsafeCell;

/// A Semaphore-based Mutex.
pub struct Mutex<T> {
    inner: SemVar<UnsafeCell<T>>,
}

/// It's safe to share across threads since single access
/// is enforced.
unsafe impl<T> Sync for Mutex<T> where T: Send {}

/// A guard that represents exclusive access to the guarded value.
pub struct MutexGuard<'a, T>(SemGuard<'a, UnsafeCell<T>>);

impl<T> Mutex<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: SemVar::new(1, UnsafeCell::new(value)),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        let guard = self.inner.access();
        MutexGuard(guard)
    }
}

use std::ops::{Deref, DerefMut};

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.0.deref().get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.deref().get() }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::Instant;

    #[test]
    fn mutex_test_single_thread() {
        let m = Mutex::new(0);
        std::hint::black_box(&m);
        let start = Instant::now();
        for _ in 0..100 {
            *m.lock() += 1;
        }
        let duration = start.elapsed();
        assert_eq!(*m.lock(), 100);
    }

    #[test]
    fn mutex_test_multi_threads() {
        let m = Mutex::new(0);
        std::hint::black_box(&m);
        let start = Instant::now();
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    for _ in 0..100 {
                        *m.lock() += 1;
                    }
                });
            }
        });
        let duration = start.elapsed();
        assert_eq!(*m.lock(), 400);
    }
}

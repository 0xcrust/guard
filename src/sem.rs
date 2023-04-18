use atomic_wait::{wait, wake_one};
use std::sync::atomic::{AtomicU32, Ordering};

/// A type representing a semaphore-protected value.
pub(crate) struct SemVar<T> {
    /// The maximum allowed accesses at a time.
    capacity: u32,
    /// Number of active accesses.
    count: AtomicU32,
    /// The value being guarded.
    value: T,
}

/// A guard that represents shared access to the inner value.
pub(crate) struct SemGuard<'a, T> {
    inner: &'a SemVar<T>,
}

impl<T> SemVar<T> {
    /// Create a new semvar with the maximum access limit set
    /// to `capacity`.
    pub fn new(capacity: u32, value: T) -> Self {
        Self {
            capacity,
            count: AtomicU32::new(0),
            value,
        }
    }

    /// Try to gain access to the protected value. Returns
    /// a [SemGuard].
    pub fn access(&self) -> SemGuard<T> {
        let mut value = self.count.load(Ordering::Relaxed);

        loop {
            if value < self.capacity {
                match self.count.compare_exchange(
                    value,
                    value + 1,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return SemGuard { inner: self },
                    Err(e) => value = e,
                }
            }

            if value == self.capacity {
                wait(&self.count, value);
                value = self.count.load(Ordering::Relaxed);
            }
        }
    }
}

impl<T> Drop for SemGuard<'_, T> {
    fn drop(&mut self) {
        self.inner.count.fetch_sub(1, Ordering::Release);
        wake_one(&self.inner.count);
    }
}

impl<T> std::ops::Deref for SemGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner.value
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn waits_when_max_guards_active() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        let value = 5;
        // So we can pass the guards around for testing.
        let sem = Box::leak::<'static>(Box::new(SemVar::new(10, value)));

        std::thread::scope(|s| {
            let mut first_set = vec![];
            let mut second_set = vec![];

            for _ in 0..10 {
                let handle = s.spawn(|| {
                    let guard = sem.access();
                    _ = &COUNT.fetch_add(1, Ordering::SeqCst);
                    guard
                });
                first_set.push(handle);
            }

            for _ in 0..10 {
                let handle = s.spawn(|| {
                    let guard = sem.access();
                    _ = &COUNT.fetch_add(1, Ordering::SeqCst);
                    guard
                });
                second_set.push(handle);
            }

            let mut guards = vec![];

            for handle in first_set {
                guards.push(handle.join().unwrap());
            }
            std::thread::sleep(Duration::from_secs(1));
            // Since we took ownership of the guards to prevent them
            // being dropped, only the first 10 threads should have run.
            assert_eq!(COUNT.load(Ordering::SeqCst), 10);

            for guard in guards {
                // Release each guard
                drop(guard);
            }
            for handle in second_set {
                handle.join().unwrap();
            }

            // Now the second set should be able to access the
            // value
            assert_eq!(COUNT.load(Ordering::SeqCst), 20);
        });

        _ = unsafe { Box::from_raw(sem) };
    }

    #[test]
    fn everyone_gets_their_chance() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        let value = 5;
        let sem = Arc::new(SemVar::new(3, value));

        let mut handles = Vec::with_capacity(100);

        for _ in 0..100 {
            let sem = Arc::clone(&sem);
            let handle = std::thread::spawn(move || {
                let _guard = sem.access();
                _ = &COUNT.fetch_add(1, Ordering::SeqCst);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(COUNT.load(Ordering::SeqCst), 100);
    }
}

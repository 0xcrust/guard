#![allow(dead_code)]

use atomic_wait::{wait, wake_one};
use std::sync::atomic::{AtomicU32, Ordering};

/// A type representing a semaphore-protected value.
pub struct SemVar<T> {
    capacity: u32,
    // Number of active users
    count: AtomicU32,
    value: T,
}

pub struct SemGuard<'a, T> {
    inner: &'a SemVar<T>,
}

impl<T> Drop for SemGuard<'_, T> {
    fn drop(&mut self) {
        let res = self.inner.count.fetch_sub(1, Ordering::SeqCst);
        println!(
            "{:?} Access dropped. count is now {}",
            std::thread::current(),
            res - 1
        );
        wake_one(&self.inner.count);
    }
}

impl<T> std::ops::Deref for SemGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner.value
    }
}

impl<T> SemVar<T> {
    pub fn new(capacity: u32, value: T) -> Self {
        Self {
            capacity,
            count: AtomicU32::new(0),
            value,
        }
    }

    pub fn access(&self) -> SemGuard<T> {
        let mut value = self.count.load(Ordering::SeqCst);

        loop {
            if value < self.capacity {
                match self.count.compare_exchange(
                    value,
                    value + 1,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        println!(
                            "{:?}. Access acquired. count is now {}",
                            std::thread::current(),
                            value + 1
                        );
                        return SemGuard { inner: self };
                    }
                    Err(e) => value = e,
                }
            }

            if value == self.capacity {
                println!("{:?}. Waiting.", std::thread::current());
                wait(&self.count, value);
                value = self.count.load(Ordering::SeqCst);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn cant_access_when_max_guards_active() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        let value = 5;
        let sem = Box::leak::<'static>(Box::new(SemVar::new(10, value)));
        std::hint::black_box(&sem);

        for _ in 0..100 {
            std::thread::spawn(|| {
                std::hint::black_box(());
                let _guard: &'static mut _ = Box::leak(Box::new(sem.access()));
                _ = &COUNT.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_secs(3));
            });
        }

        assert_eq!(COUNT.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn waits_when_max_guards_active() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        let value = 5;
        let sem = Box::leak::<'static>(Box::new(SemVar::new(10, value)));
        std::hint::black_box(&sem);

        let mut first_set = vec![];
        let mut second_set = vec![];

        for _ in 0..10 {
            let handle = std::thread::spawn(|| {
                std::hint::black_box(());
                let guard = sem.access();
                let x = &COUNT.fetch_add(1, Ordering::SeqCst);
                guard
            });
            first_set.push(handle);
        }

        for _ in 0..10 {
            let handle = std::thread::spawn(|| {
                std::hint::black_box(());
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
        let count = COUNT.load(Ordering::SeqCst);
        println!("Count before first assertion: {}", count);
        assert_eq!(count, 10);

        for guard in guards {
            drop(guard);
        }

        for handle in second_set {
            handle.join().unwrap();
        }
        let count = COUNT.load(Ordering::SeqCst);
        println!("Count before first assertion: {}", count);
        assert_eq!(count, 20);
    }

    #[test]
    fn everyone_gets_their_chance() {
        use std::sync::Arc;

        static COUNT: AtomicUsize = AtomicUsize::new(0);

        let value = 5;
        let sem = Arc::new(SemVar::new(3, value));
        std::hint::black_box(&sem);

        let mut handles = Vec::with_capacity(100);

        for _ in 0..100 {
            let sem = Arc::clone(&sem);
            let handle = std::thread::spawn(move || {
                std::hint::black_box(());
                let _guard = sem.access();
                _ = &COUNT.fetch_add(1, Ordering::SeqCst);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let count = COUNT.load(Ordering::SeqCst);
        println!("Count before first assertion: {}", count);
        assert_eq!(count, 100);
    }
}

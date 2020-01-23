#![cfg_attr(feature = "no_std", no_std)]
#[cfg(feature = "no_std")]
extern crate alloc;

#[cfg(feature = "no_std")]
use alloc::sync::Arc;
use core::{
    hash::Hash,
    mem::swap,
    task::{RawWaker, RawWakerVTable, Waker},
};
use futures::task::AtomicWaker;
#[cfg(feature = "no_std")]
use hashbrown::HashSet;
use lock_api::{Mutex, RawMutex};
#[cfg(not(feature = "no_std"))]
use std::{collections::HashSet, sync::Arc};

pub struct SetWaker<M: RawMutex, K> {
    inner: WakerPointer<M, K>,
}

impl<M: RawMutex, K> Clone for SetWaker<M, K> {
    fn clone(&self) -> Self {
        SetWaker {
            inner: self.inner.clone(),
        }
    }
}

struct SetWakerInner<K> {
    wakeups: HashSet<K>,
    waker: AtomicWaker,
}

struct SetWakerInstance<M: RawMutex, K> {
    handle: WakerPointer<M, K>,
    key: K,
}

impl<M: RawMutex, K: Eq + Hash + Clone + 'static> SetWaker<M, K> {
    pub fn new() -> Self {
        SetWaker {
            inner: Arc::new(Mutex::new(SetWakerInner {
                wakeups: HashSet::new(),
                waker: AtomicWaker::new(),
            })),
        }
    }
    pub fn register(&self, waker: &Waker) {
        self.inner.lock().waker.register(waker)
    }
    pub fn with_key(&self, key: K) -> Waker {
        let vtable = {
            fn clone<M: RawMutex, K: Eq + Hash + Clone>(data: *const ()) -> RawWaker {
                let waker: Arc<SetWakerInstance<M, K>> = unsafe { Arc::from_raw(data as *const _) };
                let cloned = RawWaker::new(
                    Arc::into_raw(waker.clone()) as *const _,
                    &RawWakerVTable::new(
                        clone::<M, K>,
                        wake::<M, K>,
                        wake_by_ref::<M, K>,
                        drop::<M, K>,
                    ),
                );
                Arc::into_raw(waker);
                cloned
            }
            fn wake<M: RawMutex, K: Eq + Hash + Clone>(data: *const ()) {
                let waker: Arc<SetWakerInstance<M, K>> = unsafe { Arc::from_raw(data as *const _) };
                waker.handle.lock().wake(&waker.key);
            }
            fn wake_by_ref<M: RawMutex, K: Eq + Hash + Clone>(data: *const ()) {
                let waker: Arc<SetWakerInstance<M, K>> = unsafe { Arc::from_raw(data as *const _) };
                waker.handle.lock().wake(&waker.key);
                Arc::into_raw(waker);
            }
            fn drop<M: RawMutex, K>(data: *const ()) {
                unsafe { <Arc<SetWakerInstance<M, K>>>::from_raw(data as *const _) };
            }
            &RawWakerVTable::new(
                clone::<M, K>,
                wake::<M, K>,
                wake_by_ref::<M, K>,
                drop::<M, K>,
            )
        };
        unsafe {
            Waker::from_raw(RawWaker::new(
                Arc::<SetWakerInstance<M, K>>::into_raw(Arc::new(SetWakerInstance {
                    handle: self.inner.clone(),
                    key,
                })) as *const _,
                vtable,
            ))
        }
    }
    pub fn keys(&self) -> impl Iterator<Item = K> {
        let map = &mut self.inner.lock().wakeups;
        let mut set = HashSet::new();
        swap(map, &mut set);
        set.into_iter()
    }
}

impl<K: Eq + Hash + Clone> SetWakerInner<K> {
    fn wake(&mut self, key: &K) {
        if !self.wakeups.contains(key) {
            self.wakeups.insert(key.clone());
        }
        self.waker.wake();
    }
}

type WakerPointer<M, K> = Arc<Mutex<M, SetWakerInner<K>>>;

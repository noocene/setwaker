#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, sync::Arc};
use core::{
    hash::Hash,
    mem::swap,
    task::{RawWaker, RawWakerVTable, Waker},
};
use futures::task::AtomicWaker;
#[cfg(not(feature = "std"))]
use hashbrown::HashSet;
use lock_api::{Mutex, RawMutex};
#[cfg(feature = "std")]
use std::{collections::HashSet, sync::Arc};

pub struct SetWaker<M: RawMutex, K> {
    inner: WakerPointer<M, K>,
}

struct SetWakerInner<K> {
    wakeups: HashSet<K>,
    waker: AtomicWaker,
}

struct SetWakerInstance<M: RawMutex, K> {
    handle: WakerPointer<M, K>,
    key: K,
}

trait WakeRef {
    fn wake(&self);
}

impl<M: RawMutex, K: Eq + Hash + Clone> WakeRef for SetWakerInstance<M, K> {
    fn wake(&self) {
        self.handle.lock().wake(self.key.clone())
    }
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
        let waker: Arc<Box<dyn WakeRef>> = Arc::new(Box::new(SetWakerInstance {
            handle: self.inner.clone(),
            key,
        }));
        unsafe {
            Waker::from_raw(RawWaker::new(
                Arc::<Box<dyn WakeRef>>::into_raw(waker) as *const _,
                VTABLE,
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

impl<K: Eq + Hash> SetWakerInner<K> {
    fn wake(&mut self, key: K) {
        self.wakeups.insert(key);
        self.waker.wake();
    }
}

type WakerPointer<M, K> = Arc<Mutex<M, SetWakerInner<K>>>;

static VTABLE: &'static RawWakerVTable = {
    fn clone(data: *const ()) -> RawWaker {
        let waker: Arc<Box<dyn WakeRef>> = unsafe { Arc::from_raw(data as *const _) };
        RawWaker::new(Arc::into_raw(waker.clone()) as *const _, VTABLE)
    }
    fn wake(data: *const ()) {
        let waker: Arc<Box<dyn WakeRef>> = unsafe { Arc::from_raw(data as *const _) };
        waker.wake();
    }
    fn wake_by_ref(data: *const ()) {
        let waker: Arc<Box<dyn WakeRef>> = unsafe { Arc::from_raw(data as *const _) };
        waker.wake();
        Arc::into_raw(waker);
    }
    fn drop(data: *const ()) {
        unsafe { Arc::<Box<dyn WakeRef>>::from_raw(data as *const _) };
    }
    &RawWakerVTable::new(clone, wake, wake_by_ref, drop)
};

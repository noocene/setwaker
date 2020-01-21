use futures::task::AtomicWaker;
use std::{
    collections::HashSet,
    hash::Hash,
    mem::swap,
    sync::{Arc, Mutex},
    task::{RawWaker, RawWakerVTable, Waker},
};

pub struct SetWaker<K> {
    inner: WakerPointer<K>,
}

struct SetWakerInner<K> {
    wakeups: HashSet<K>,
    waker: AtomicWaker,
}

struct SetWakerInstance<K> {
    handle: WakerPointer<K>,
    key: K,
}

trait WakeRef {
    fn wake(&self);
}

impl<K: Eq + Hash + Clone> WakeRef for SetWakerInstance<K> {
    fn wake(&self) {
        self.handle.lock().unwrap().wake(self.key.clone())
    }
}

impl<K: Eq + Hash + Clone + 'static> SetWaker<K> {
    pub fn new() -> Self {
        SetWaker {
            inner: Arc::new(Mutex::new(SetWakerInner {
                wakeups: HashSet::new(),
                waker: AtomicWaker::new(),
            })),
        }
    }
    pub fn register(&self, waker: &Waker) {
        self.inner.lock().unwrap().waker.register(waker)
    }
    pub fn with_key(&self, key: K) -> Waker {
        let waker: InstancePointer = Arc::new(Mutex::new(Box::new(SetWakerInstance {
            handle: self.inner.clone(),
            key,
        })));
        unsafe { Waker::from_raw(RawWaker::new(Arc::into_raw(waker) as *const _, VTABLE)) }
    }
    pub fn keys(&self) -> impl Iterator<Item = K> {
        let map = &mut self.inner.lock().unwrap().wakeups;
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

type WakerPointer<K> = Arc<Mutex<SetWakerInner<K>>>;
type InstancePointer = Arc<Mutex<Box<dyn WakeRef>>>;

static VTABLE: &'static RawWakerVTable = {
    fn clone(data: *const ()) -> RawWaker {
        let waker: InstancePointer = unsafe { Arc::from_raw(data as *const _) };
        RawWaker::new(Arc::into_raw(waker.clone()) as *const _, VTABLE)
    }
    fn wake(data: *const ()) {
        let waker: InstancePointer = unsafe { Arc::from_raw(data as *const _) };
        waker.lock().unwrap().wake();
    }
    fn wake_by_ref(data: *const ()) {
        let waker: InstancePointer = unsafe { Arc::from_raw(data as *const _) };
        waker.lock().unwrap().wake();
        Arc::into_raw(waker);
    }
    fn drop(data: *const ()) {
        unsafe { Arc::from_raw(data as *const _) };
    }
    &RawWakerVTable::new(clone, wake, wake_by_ref, drop)
};

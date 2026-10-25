//! Blocks decoded off the interface's thread and kept while they are drawn.
//! DESIGN.md §3.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use arrow_array::{Array, ArrayRef};

use crate::{BlockId, Table};

/// How much decoded data is kept before the least recently drawn goes.
const CACHE_BYTES: usize = 512 * 1024 * 1024;
const WORKERS: usize = 4;

type Decoded = Result<ArrayRef, String>;

struct Wanted {
    /// What the window last asked for, first wanted first.
    queue: Vec<BlockId>,
    /// What a worker is decoding now.
    busy: HashSet<BlockId>,
    closed: bool,
}

struct Shared {
    wanted: Mutex<Wanted>,
    ready: Condvar,
}

struct Entry {
    decoded: Decoded,
    bytes: usize,
    drawn: u64,
}

pub struct Blocks {
    shared: Arc<Shared>,
    arrived: Receiver<(BlockId, Decoded)>,
    cache: HashMap<BlockId, Entry>,
    bytes: usize,
    frame: u64,
}

impl Blocks {
    /// Start the workers for `table`; `wake` is called when a block arrives.
    pub fn new(table: &Arc<Table>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = Arc::new(Shared {
            wanted: Mutex::new(Wanted { queue: Vec::new(), busy: HashSet::new(), closed: false }),
            ready: Condvar::new(),
        });
        let (send, arrived) = mpsc::channel();
        let wake = Arc::new(wake);
        for _ in 0..WORKERS {
            let (shared, table, send, wake) = (shared.clone(), table.clone(), send.clone(), wake.clone());
            std::thread::spawn(move || work(&shared, &table, &send, &*wake));
        }
        Self { shared, arrived, cache: HashMap::new(), bytes: 0, frame: 0 }
    }

    /// Begin a frame: take in what has arrived since the last.
    pub fn begin(&mut self) {
        self.frame += 1;
        while let Ok((id, decoded)) = self.arrived.try_recv() {
            let bytes = decoded.as_ref().map_or(0, Array::get_array_memory_size);
            self.bytes += bytes;
            if let Some(old) = self.cache.insert(id, Entry { decoded, bytes, drawn: self.frame }) {
                self.bytes -= old.bytes;
            }
        }
        self.evict();
    }

    /// A block if it has arrived, marked as drawn this frame.
    pub fn get(&mut self, id: BlockId) -> Option<&Decoded> {
        let frame = self.frame;
        self.cache.get_mut(&id).map(|entry| {
            entry.drawn = frame;
            &entry.decoded
        })
    }

    /// Ask for these blocks, in this order, in place of whatever was asked before.
    pub fn want(&self, ids: Vec<BlockId>) {
        let mut wanted = self.shared.wanted.lock().unwrap_or_else(PoisonError::into_inner);
        wanted.queue = ids.into_iter().filter(|id| !self.cache.contains_key(id) && !wanted.busy.contains(id)).collect();
        if !wanted.queue.is_empty() {
            self.shared.ready.notify_all();
        }
    }

    fn evict(&mut self) {
        if self.bytes <= CACHE_BYTES {
            return;
        }
        let mut entries: Vec<(u64, BlockId)> = self.cache.iter().map(|(id, e)| (e.drawn, *id)).collect();
        entries.sort_unstable_by_key(|(drawn, _)| *drawn);
        for (drawn, id) in entries {
            if self.bytes <= CACHE_BYTES || drawn + 1 >= self.frame {
                break;
            }
            if let Some(entry) = self.cache.remove(&id) {
                self.bytes -= entry.bytes;
            }
        }
    }
}

impl Drop for Blocks {
    fn drop(&mut self) {
        self.shared.wanted.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
        self.shared.ready.notify_all();
    }
}

fn work(shared: &Shared, table: &Table, send: &Sender<(BlockId, Decoded)>, wake: &(dyn Fn() + Send + Sync)) {
    loop {
        let id = {
            let mut wanted = shared.wanted.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if wanted.closed {
                    return;
                }
                if !wanted.queue.is_empty() {
                    let id = wanted.queue.remove(0);
                    wanted.busy.insert(id);
                    break id;
                }
                wanted = shared.ready.wait(wanted).unwrap_or_else(PoisonError::into_inner);
            }
        };
        let decoded = table.read(id);
        if send.send((id, decoded)).is_err() {
            return;
        }
        shared.wanted.lock().unwrap_or_else(PoisonError::into_inner).busy.remove(&id);
        wake();
    }
}

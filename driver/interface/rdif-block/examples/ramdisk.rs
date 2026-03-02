use dma_api::Osal;
use rdif_block::*;
use std::boxed::Box;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::vec::Vec;

struct ExampleOsal;

impl Osal for ExampleOsal {
    fn map(&self, addr: NonNull<u8>, _size: usize, _direction: dma_api::Direction) -> u64 {
        addr.as_ptr() as u64
    }

    fn unmap(&self, _addr: NonNull<u8>, _size: usize) {}
}

static EX_OSAL: ExampleOsal = ExampleOsal;

/// A simple in-memory ramdisk that implements `rdif_block::Interface`.
///
/// Internally it keeps a vector of blocks. Each block is initialized
/// so every byte equals the block id as u8 (truncated).
/// It spawns a worker thread that processes requests pushed by the
/// ReadQueue implementation and sets an event bit to signal completion.
struct RamDisk {
    block_size: usize,
    num_blocks: usize,
    // storage: Vec<[u8]> is not possible, use single Vec<u8>
    _storage: Arc<Vec<u8>>,

    // Shared request/response queue between interface and worker
    inner: Arc<Mutex<RamInner>>,
}

struct RamInner {
    // map request id -> (queue_id, block_id, buffer pointer/size)
    // we keep a simple Vec of pending requests
    pending: Vec<(usize, RequestId, usize, usize, usize)>,
    // pending write requests: (queue_id, req_id, block_id, src_ptr, size)
    pending_writes: Vec<(usize, RequestId, usize, usize, usize)>,
    // set when there is new data to be processed
    irq_rx: IdList,
    irq_enabled: bool,
    next_req_id: usize,
    // next queue id to hand out from create_queue()
    next_queue_id: usize,
    completed: Vec<RequestId>,
    completed_writes: Vec<RequestId>,
}

impl RamDisk {
    pub fn new(block_size: usize, num_blocks: usize) -> Self {
        // fill storage so that each block's bytes == block_id as u8
        let mut storage = Vec::with_capacity(block_size * num_blocks);
        for i in 0..num_blocks {
            let v = i as u8;
            storage.extend(std::iter::repeat_n(v, block_size));
        }

        let storage = Arc::new(storage);

        let inner = Arc::new(Mutex::new(RamInner {
            pending: Vec::new(),
            pending_writes: Vec::new(),
            irq_rx: IdList::none(),
            irq_enabled: true,
            next_req_id: 1,
            next_queue_id: 0,
            completed: Vec::new(),
            completed_writes: Vec::new(),
        }));

        // spawn worker thread to process requests
        let storage_cloned = storage.clone();
        let inner_cloned = inner.clone();
        std::thread::spawn(move || {
            loop {
                // take a snapshot of pending requests
                let (reqs, writes) = {
                    let mut guard = inner_cloned.lock().unwrap();
                    if guard.pending.is_empty() && guard.pending_writes.is_empty() {
                        // no work - sleep briefly
                        drop(guard);
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }

                    // take requests and release lock immediately
                    let reqs = core::mem::take(&mut guard.pending);
                    let writes = core::mem::take(&mut guard.pending_writes);
                    (reqs, writes)
                    // lock is automatically released here
                };

                // process all pending read requests without holding the lock
                let mut completed_reads = Vec::new();
                // reqs contain (queue_id, req_id, block_id, buf_ptr, size)
                for (_q_id, req_id, block_id, buf_ptr_usize, sz) in &reqs {
                    // copy block data into user buffer
                    let start = block_id * sz;
                    let buf_ptr = *buf_ptr_usize as *mut u8;
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            storage_cloned.as_ptr().add(start),
                            buf_ptr,
                            *sz,
                        );
                    }
                    completed_reads.push(*req_id);
                    // mark that this queue has an event
                    // we'll insert queue id into irq_rx when updating the guard
                }

                // process pending write requests without holding the lock
                let mut completed_writes = Vec::new();
                for (_q_id, req_id, block_id, src_ptr_usize, sz) in &writes {
                    let start = block_id * sz;
                    let src_ptr = *src_ptr_usize as *const u8;
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            src_ptr,
                            storage_cloned.as_ptr().add(start) as *mut u8,
                            *sz,
                        );
                    }
                    completed_writes.push(*req_id);
                    // same as reads, queue id will be inserted into irq bits below
                }

                // acquire lock again only to update completion status
                {
                    let mut guard = inner_cloned.lock().unwrap();
                    guard.completed.extend(completed_reads);
                    guard.completed_writes.extend(completed_writes);
                    // insert queue ids for all processed requests into irq_rx
                    for (q_id, _req_id, _blk, _p, _s) in reqs.iter() {
                        guard.irq_rx.insert(*q_id);
                    }
                    for (q_id, _req_id, _blk, _p, _s) in writes.iter() {
                        guard.irq_rx.insert(*q_id);
                    }
                }

                // small delay to simulate device latency
                std::thread::sleep(Duration::from_millis(1));
            }
        });

        Self {
            block_size,
            num_blocks,
            _storage: storage,
            inner,
        }
    }
}

impl rdif_base::DriverGeneric for RamDisk {
    fn open(&mut self) -> Result<(), KError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), KError> {
        Ok(())
    }
}

impl Interface for RamDisk {
    fn create_queue(&mut self) -> Option<Box<dyn IQueue>> {
        let mut g = self.inner.lock().unwrap();
        let id = g.next_queue_id;
        g.next_queue_id += 1;
        Some(Box::new(RamQueue::new(
            id,
            self.block_size,
            self.num_blocks,
            self.inner.clone(),
        )))
    }

    fn enable_irq(&mut self) {
        let mut g = self.inner.lock().unwrap();
        g.irq_enabled = true;
    }

    fn disable_irq(&mut self) {
        let mut g = self.inner.lock().unwrap();
        g.irq_enabled = false;
    }

    fn is_irq_enabled(&self) -> bool {
        let g = self.inner.lock().unwrap();
        g.irq_enabled
    }

    fn handle_irq(&mut self) -> Event {
        let mut g = self.inner.lock().unwrap();
        let mut ev = Event::none();
        core::mem::swap(&mut ev.queue, &mut g.irq_rx);
        ev
    }
}

struct RamQueue {
    id: usize,
    block_size: usize,
    num_blocks: usize,
    inner: Arc<Mutex<RamInner>>,
}

impl RamQueue {
    fn new(id: usize, block_size: usize, num_blocks: usize, inner: Arc<Mutex<RamInner>>) -> Self {
        Self {
            id,
            block_size,
            num_blocks,
            inner,
        }
    }
}

impl IQueue for RamQueue {
    fn id(&self) -> usize {
        self.id
    }

    fn num_blocks(&self) -> usize {
        self.num_blocks
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    fn buff_config(&self) -> BuffConfig {
        BuffConfig {
            dma_mask: !0u64,
            align: 1,
            size: self.block_size,
        }
    }

    fn submit_request(&mut self, request: Request<'_>) -> Result<RequestId, BlkError> {
        let block_id = request.block_id;
        if block_id >= self.num_blocks {
            return Err(BlkError::InvalidBlockIndex(block_id));
        }

        let mut g = self.inner.lock().unwrap();
        let req_id = RequestId::new(g.next_req_id);
        g.next_req_id += 1;

        match request.kind {
            RequestKind::Read(buff) => {
                g.pending
                    .push((self.id, req_id, block_id, buff.virt as usize, buff.size));
            }
            RequestKind::Write(slice) => {
                g.pending_writes.push((
                    self.id,
                    req_id,
                    block_id,
                    slice.as_ptr() as usize,
                    slice.len(),
                ));
            }
        }

        // Indicate that the device has data for rx (so handle_irq can wake)
        g.irq_rx.insert(self.id);

        Ok(req_id)
    }

    fn poll_request(&mut self, request: RequestId) -> Result<(), BlkError> {
        let mut g = self.inner.lock().unwrap();
        if let Some(pos) = g.completed.iter().position(|r| *r == request) {
            g.completed.remove(pos);
            Ok(())
        } else if let Some(pos) = g.completed_writes.iter().position(|r| *r == request) {
            g.completed_writes.remove(pos);
            Ok(())
        } else {
            Err(BlkError::Retry)
        }
    }
}

#[tokio::main]
async fn main() {
    // initialize dma-api osal
    dma_api::init(&EX_OSAL);

    // create a ram device with 16 byte blocks and 1024 blocks
    let mut ram = Block::new(RamDisk::new(16, 1024));

    // open device (no-op here)
    let _ = ram.open();

    // get a read queue via the new Interface API
    let mut sq = ram.create_queue().expect("read queue");

    // spawn a thread that polls the device handle and prints events
    let handle = ram.irq_handler();
    std::thread::spawn(move || {
        loop {
            handle.handle();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });

    // request blocks 3 and 4 and asynchronously poll for completion
    let res = sq.read_blocks(3, 2).await;

    for b in res {
        println!("block: {:?}", b.unwrap());
    }
    let size = sq.block_size();

    // prepare data for blocks 3 and 4: fill with 0xAA and 0xBB respectively
    let mut data = vec![0xAAu8; size];
    data.extend(vec![0xBBu8; size]);

    let res = sq.write_blocks(3, &data).await;

    for r in res {
        println!("write block result: {:?}", r);
    }

    let res = sq.read_blocks(3, 2).await;

    for b in res {
        println!("block: {:?}", b.unwrap());
    }

    println!("done");

    // test blocking
    println!("test blocking read");

    let res = sq.read_blocks_blocking(3, 2);
    for b in res {
        println!("block: {:?}", b.unwrap());
    }
}

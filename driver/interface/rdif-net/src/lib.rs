#![no_std]

extern crate alloc;

use core::ops::{Deref, DerefMut};

use alloc::boxed::Box;
pub use dma_api;
pub use rdif_base::{DriverGeneric, KError, io};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors that can occur during network device operations.
#[derive(thiserror::Error, Debug)]
pub enum NetError {
    /// The requested operation is not supported by the device.
    #[error("Operation not supported")]
    NotSupported,

    /// The operation should be retried later (e.g. queue full).
    #[error("Operation should be retried")]
    Retry,

    /// Insufficient memory to complete the operation.
    #[error("Insufficient memory")]
    NoMemory,

    /// The network link is down.
    #[error("Link down")]
    LinkDown,

    /// An unspecified error occurred.
    #[error("Other error: {0}")]
    Other(Box<dyn core::error::Error>),
}

impl From<NetError> for io::ErrorKind {
    fn from(value: NetError) -> Self {
        match value {
            NetError::NotSupported => io::ErrorKind::Unsupported,
            NetError::Retry => io::ErrorKind::Interrupted,
            NetError::NoMemory => io::ErrorKind::OutOfMemory,
            NetError::LinkDown => io::ErrorKind::NotAvailable,
            NetError::Other(e) => io::ErrorKind::Other(e),
        }
    }
}

impl From<dma_api::DmaError> for NetError {
    fn from(value: dma_api::DmaError) -> Self {
        match value {
            dma_api::DmaError::NoMemory => NetError::NoMemory,
            e => NetError::Other(Box::new(e)),
        }
    }
}

// ---------------------------------------------------------------------------
// DMA buffer helpers
// ---------------------------------------------------------------------------

/// Configuration for DMA buffer allocation.
pub struct BuffConfig {
    /// DMA addressing mask for the device.
    pub dma_mask: u64,

    /// Required alignment for buffer addresses (in bytes).
    pub align: usize,

    /// Maximum buffer size in bytes (typically MTU + Ethernet header).
    pub size: usize,
}

/// A DMA-capable buffer described by both a virtual and bus address.
#[derive(Clone, Copy)]
pub struct Buffer {
    pub virt: *mut u8,
    pub bus: u64,
    pub size: usize,
}

impl Deref for Buffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.virt, self.size) }
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.virt, self.size) }
    }
}

// ---------------------------------------------------------------------------
// Request / response identifiers and event bitmask
// ---------------------------------------------------------------------------

/// Opaque identifier for a submitted request.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(usize);

impl RequestId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

impl From<RequestId> for usize {
    fn from(value: RequestId) -> Self {
        value.0
    }
}

/// Bitmask tracking up to 64 queue identifiers.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct IdList(u64);

impl IdList {
    pub const fn none() -> Self {
        Self(0)
    }

    pub fn contains(&self, id: usize) -> bool {
        (self.0 & (1 << id)) != 0
    }

    pub fn insert(&mut self, id: usize) {
        self.0 |= 1 << id;
    }

    pub fn remove(&mut self, id: usize) {
        self.0 &= !(1 << id);
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> {
        let bits = self.0;
        (0..64).filter(move |i| (bits & (1 << i)) != 0)
    }
}

/// Event returned by [`Interface::handle_irq`] indicating which queues have
/// completed operations.
#[derive(Debug, Clone, Copy)]
pub struct Event {
    /// Bitmask of TX queue IDs that have completion events.
    pub tx_queue: IdList,
    /// Bitmask of RX queue IDs that have completion events.
    pub rx_queue: IdList,
}

impl Event {
    pub const fn none() -> Self {
        Self {
            tx_queue: IdList::none(),
            rx_queue: IdList::none(),
        }
    }
}

// ---------------------------------------------------------------------------
// TX request / RX request & response
// ---------------------------------------------------------------------------

/// A transmit request: a packet to send.
pub struct TxRequest<'a> {
    /// Raw packet data (including Ethernet header) to transmit.
    pub data: &'a [u8],
}

/// A receive request: a pre-allocated buffer submitted to hardware.
pub struct RxRequest {
    /// DMA buffer that hardware will fill with received data.
    pub buffer: Buffer,
}

/// Result of a completed receive operation.
pub struct RxResponse {
    /// Actual number of bytes received into the buffer.
    pub len: usize,
}

// ---------------------------------------------------------------------------
// Device-level interface
// ---------------------------------------------------------------------------

/// Core interface that network device drivers must implement.
///
/// Provides device-level operations: queue creation, interrupt management,
/// and MAC address retrieval. Individual packet I/O goes through the queue
/// traits ([`ITxQueue`] / [`IRxQueue`]).
pub trait Interface: DriverGeneric {
    /// Returns the device's 6-byte MAC address.
    fn mac_address(&self) -> [u8; 6];

    /// Create a new transmit queue. Returns `None` if no more queues are
    /// available.
    fn create_tx_queue(&mut self) -> Option<Box<dyn ITxQueue>>;

    /// Create a new receive queue. Returns `None` if no more queues are
    /// available.
    fn create_rx_queue(&mut self) -> Option<Box<dyn IRxQueue>>;

    /// Enable device interrupts.
    fn enable_irq(&mut self);

    /// Disable device interrupts.
    fn disable_irq(&mut self);

    /// Check whether device interrupts are currently enabled.
    fn is_irq_enabled(&self) -> bool;

    /// Handle a device interrupt, returning which queues have events.
    fn handle_irq(&mut self) -> Event;
}

// ---------------------------------------------------------------------------
// Transmit queue
// ---------------------------------------------------------------------------

/// Transmit queue interface.
///
/// A driver creates one or more TX queues via [`Interface::create_tx_queue`].
/// The caller submits packets with [`submit_request`](ITxQueue::submit_request)
/// and later polls for completion with [`poll_request`](ITxQueue::poll_request).
pub trait ITxQueue: Send + 'static {
    /// Queue identifier (unique within the device).
    fn id(&self) -> usize;

    /// Maximum transmission unit in bytes (payload only, excluding Ethernet
    /// header).
    fn mtu(&self) -> usize;

    /// DMA buffer configuration for transmit buffers.
    fn buff_config(&self) -> BuffConfig;

    /// Submit a packet for transmission.
    ///
    /// Returns a [`RequestId`] that can be polled via [`poll_request`](ITxQueue::poll_request).
    /// Returns [`NetError::Retry`] when the hardware queue is full.
    fn submit_request(&mut self, request: TxRequest<'_>) -> Result<RequestId, NetError>;

    /// Poll for completion of a previously submitted transmit request.
    ///
    /// Returns `Ok(())` when the packet has been sent, or
    /// [`NetError::Retry`] if the request is still in progress.
    fn poll_request(&mut self, request: RequestId) -> Result<(), NetError>;
}

// ---------------------------------------------------------------------------
// Receive queue
// ---------------------------------------------------------------------------

/// Receive queue interface.
///
/// A driver creates one or more RX queues via [`Interface::create_rx_queue`].
/// The caller pre-allocates DMA buffers and submits them with
/// [`submit_request`](IRxQueue::submit_request). When a packet arrives the
/// hardware fills the buffer; the caller retrieves results via
/// [`poll_request`](IRxQueue::poll_request).
pub trait IRxQueue: Send + 'static {
    /// Queue identifier (unique within the device).
    fn id(&self) -> usize;

    /// Maximum transmission unit in bytes.
    fn mtu(&self) -> usize;

    /// DMA buffer configuration for receive buffers.
    fn buff_config(&self) -> BuffConfig;

    /// Submit a pre-allocated receive buffer to hardware.
    ///
    /// The buffer will be filled when a packet is received. Returns a
    /// [`RequestId`] for later polling.
    /// Returns [`NetError::Retry`] when the hardware queue is full.
    fn submit_request(&mut self, request: RxRequest) -> Result<RequestId, NetError>;

    /// Poll for completion of a previously submitted receive request.
    ///
    /// Returns [`RxResponse`] with the actual received byte count on success,
    /// or [`NetError::Retry`] if no packet has been received yet.
    fn poll_request(&mut self, request: RequestId) -> Result<RxResponse, NetError>;
}

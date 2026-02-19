use crate::polymarket::OrderBook;
use crossbeam::queue::ArrayQueue;
use std::sync::Arc;

/// Lock-free orderbook update queue
/// Uses SPSC (Single Producer Single Consumer) pattern
/// WebSocket thread produces, Strategy thread consumes
pub struct OrderBookQueue {
    queue: Arc<ArrayQueue<OrderBook>>,
}

impl OrderBookQueue {
    /// Create a new lock-free queue with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
        }
    }

    /// Push an orderbook update (non-blocking)
    /// Returns true if successful, false if queue is full
    #[inline(always)]
    pub fn push(&self, orderbook: OrderBook) -> bool {
        self.queue.push(orderbook).is_ok()
    }

    /// Pop an orderbook update (non-blocking)
    /// Returns None if queue is empty
    #[inline(always)]
    pub fn pop(&self) -> Option<OrderBook> {
        self.queue.pop()
    }

    /// Get current queue length
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if queue is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get queue capacity
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }

    /// Clone the queue handle (for sharing between threads)
    pub fn clone_handle(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
        }
    }
}

impl Clone for OrderBookQueue {
    fn clone(&self) -> Self {
        self.clone_handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_push_pop() {
        let queue = OrderBookQueue::new(10);
        let ob = OrderBook::new();

        assert!(queue.push(ob));
        assert_eq!(queue.len(), 1);

        let popped = queue.pop();
        assert!(popped.is_some());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_full() {
        let queue = OrderBookQueue::new(2);

        assert!(queue.push(OrderBook::new()));
        assert!(queue.push(OrderBook::new()));
        assert!(!queue.push(OrderBook::new())); // Should fail (full)
    }
}

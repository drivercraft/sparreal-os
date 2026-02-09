use crate::{RangeError, RangeOp, VecOp};
use heapless::Vec;

impl<T: RangeOp + Send + 'static, const N: usize> VecOp<T> for Vec<T, N> {
    fn push(&mut self, item: T) -> Result<(), RangeError<T>> {
        self.push(item).map_err(|_| RangeError::Capacity)
    }

    fn as_slice(&self) -> &[T] {
        self.as_slice()
    }

    fn drain<R>(&mut self, range: R) -> impl Iterator<Item = T>
    where
        R: core::ops::RangeBounds<usize>,
    {
        self.drain(range)
    }

    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn remove(&mut self, index: usize) -> T {
        self.remove(index)
    }

    fn insert(&mut self, index: usize, item: T) -> Result<(), RangeError<T>> {
        self.insert(index, item).map_err(|_| RangeError::Capacity)
    }

    fn clear(&mut self) {
        self.clear();
    }
}

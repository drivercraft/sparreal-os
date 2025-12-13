//! 禁止中断的Spinlock实现
//!
//! 提供中断安全的互斥锁，在持有锁期间会自动禁用中断。
//! 适用于保护临界区，防止中断处理程序与主线程之间的竞争条件。

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::os::irq::NoIrqGuard;

/// 禁止中断的原始Spinlock实现
///
/// 使用原子操作和中断禁用来提供线程安全。
/// 在获取锁时会禁用中断，释放锁时恢复中断状态。
pub struct IrqRawSpinlock {
    locked: AtomicBool,
}

impl IrqRawSpinlock {
    /// 创建一个新的IrqRawSpinlock实例
    #[inline]
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }
}

impl Default for IrqRawSpinlock {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl IrqRawSpinlock {
    /// 获取锁（阻塞直到获取成功）
    ///
    /// 纯粹的锁获取操作，不涉及中断管理。
    /// 中断管理应该在调用层面处理。
    #[inline]
    pub fn lock(&self) {
        // 自旋获取锁，使用退避策略优化性能
        let mut spin_count = 0;

        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // 提示CPU我们正在自旋等待
            core::hint::spin_loop();

            // 简单的退避策略：避免CPU过度占用
            spin_count = (spin_count + 1) & 0xFFF;
            if spin_count == 0 {
                // 每自旋4096次，给CPU一点休息时间
                core::hint::spin_loop();
            }
        }
    }

    /// 尝试获取锁（非阻塞）
    ///
    /// 纯粹的锁获取尝试，不涉及中断管理。
    ///
    /// # Returns
    ///
    /// * `true` - 成功获取锁
    /// * `false` - 锁已被占用
    #[inline]
    pub fn try_lock(&self) -> bool {
        // 尝试获取锁
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// 释放锁
    ///
    /// 此方法会恢复中断状态（如果通过lock获取的）。
    ///
    /// # Safety
    ///
    /// 调用者必须确保当前线程持有该锁。
    #[inline]
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }

    /// 检查锁是否被占用
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Acquire)
    }
}

/// 中断安全的互斥锁
///
/// 在持有锁期间会自动禁用中断，适用于内核临界区保护。
pub struct IrqSpinlock<T> {
    raw: IrqRawSpinlock,
    data: UnsafeCell<T>,
    // PhantomData不再需要，因为UnsafeCell已经提供了正确的不变性
}

// IrqSpinlock是Send（如果T是Send）
unsafe impl<T: Send> Send for IrqSpinlock<T> {}

// IrqSpinlock是Sync（如果T是Send）
unsafe impl<T: Send> Sync for IrqSpinlock<T> {}

/// IrqSpinlock的锁守卫
///
/// 当这个守卫被drop时，锁会自动释放，中断状态也会恢复。
pub struct IrqMutexGuard<'a, T> {
    lock: &'a IrqSpinlock<T>,
    _irq_guard: NoIrqGuard, // 确保中断在整个守卫生命周期内被禁用
}

impl<T> IrqSpinlock<T> {
    /// 创建一个新的IrqSpinlock实例
    ///
    /// # Arguments
    ///
    /// * `data` - 要保护的数据
    ///
    /// # Returns
    ///
    /// 返回一个包含指定数据的IrqSpinlock实例
    ///
    /// # Examples
    ///
    /// ```
    /// use sparreal_kernel::os::sync::spinlock::IrqSpinlock;
    ///
    /// let lock = IrqSpinlock::new(42);
    /// ```
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            raw: IrqRawSpinlock::new(),
            data: UnsafeCell::new(data),
        }
    }

    /// 创建一个空的IrqSpinlock
    #[inline]
    pub const fn empty() -> IrqSpinlock<()> {
        IrqSpinlock::new(())
    }

    /// 获取锁，如果锁被占用则自旋等待
    ///
    /// 调用此方法会禁用中断，直到返回的守卫被drop。
    ///
    /// # Returns
    ///
    /// 返回一个守卫，通过它可以访问受保护的数据
    ///
    /// # Safety
    ///
    /// 在持有锁期间，中断会被禁用，因此调用者需要确保：
    /// 1. 尽快释放锁
    /// 2. 不要在持有锁期间执行可能长时间阻塞的操作
    /// 3. 避免嵌套获取同一个锁
    #[inline]
    pub fn lock(&self) -> IrqMutexGuard<'_, T> {
        // 禁用中断（这会在整个守卫生命周期内保持）
        let irq_guard = NoIrqGuard::new();

        // 获取锁
        self.raw.lock();

        IrqMutexGuard {
            lock: self,
            _irq_guard: irq_guard,
        }
    }

    /// 尝试获取锁，如果失败则立即返回
    ///
    /// 这个方法不会自旋等待，而是立即返回结果。
    /// 如果成功获取锁，中断也会被禁用，直到守卫被drop。
    ///
    /// # Returns
    ///
    /// * `Some(guard)` - 成功获取锁时的守卫，中断已禁用
    /// * `None` - 锁已被占用时，中断状态不变
    #[inline]
    pub fn try_lock(&self) -> Option<IrqMutexGuard<'_, T>> {
        let irq_guard = NoIrqGuard::new();
        if self.raw.try_lock() {
            Some(IrqMutexGuard {
                lock: self,
                _irq_guard: irq_guard,
            })
        } else {
            None
        }
    }

    /// 检查锁是否被占用
    ///
    /// # Returns
    ///
    /// * `true` - 锁被占用
    /// * `false` - 锁空闲
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// 获取内部数据的不可变引用
    ///
    /// # Safety
    ///
    /// 此方法不提供同步保证，调用者需要确保没有其他地方在并发访问数据
    #[inline]
    pub unsafe fn get(&self) -> *mut T {
        self.data.get()
    }

    /// 获取内部数据的可变引用
    ///
    /// # Safety
    ///
    /// 此方法不提供同步保证，调用者需要确保没有其他地方在并发访问数据
    #[inline]
    pub unsafe fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    /// 消费IrqSpinlock并返回内部数据
    ///
    /// # Returns
    ///
    /// 返回受保护的数据
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    /// 获取锁但不使用RAII守卫（高级用法）
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// 1. 手动管理中断禁用
    /// 2. 在适当的时机调用unlock
    /// 3. 不会发生死锁或竞态条件
    #[inline]
    pub unsafe fn lock_raw(&self) -> &IrqRawSpinlock {
        &self.raw
    }
}

impl<T> Drop for IrqMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.lock.raw.unlock();
        }
    }
}

impl<T> Deref for IrqMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for IrqMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::string::String;

    #[test]
    fn test_irq_raw_spinlock_creation() {
        let spinlock = IrqRawSpinlock::new();
        assert!(!spinlock.is_locked());
    }

    #[test]
    fn test_irq_raw_spinlock_default() {
        let spinlock = IrqRawSpinlock::default();
        assert!(!spinlock.is_locked());
    }

    #[test]
    fn test_irq_raw_spinlock_try_lock() {
        let spinlock = IrqRawSpinlock::new();

        // 首次尝试获取锁应该成功
        assert!(spinlock.try_lock());
        assert!(spinlock.is_locked());

        // 第二次尝试获取锁应该失败
        assert!(!spinlock.try_lock());

        // 释放锁
        unsafe {
            spinlock.unlock();
        }
        assert!(!spinlock.is_locked());

        // 再次尝试获取锁应该成功
        assert!(spinlock.try_lock());
        unsafe {
            spinlock.unlock();
        }
    }

    #[test]
    fn test_irq_spinlock_creation() {
        let lock = IrqSpinlock::new(42);
        assert!(!lock.is_locked());

        // 测试into_inner
        let value = lock.into_inner();
        assert_eq!(value, 42);
    }

    #[test]
    fn test_irq_spinlock_empty() {
        let lock: IrqSpinlock<()> = IrqSpinlock::<()>::empty();
        assert!(!lock.is_locked());

        let value: () = lock.into_inner();
        assert_eq!(value, ());
    }

    #[test]
    fn test_irq_spinlock_try_lock() {
        let lock = IrqSpinlock::new(42);

        // 首次尝试获取锁应该成功
        {
            let guard = lock.try_lock().unwrap();
            assert_eq!(*guard, 42);
            assert!(lock.is_locked());
        } // guard被drop，锁被释放

        assert!(!lock.is_locked());

        // 再次尝试获取锁应该成功
        {
            let guard = lock.try_lock().unwrap();
            assert_eq!(*guard, 42);
        }
    }

    #[test]
    fn test_irq_spinlock_try_lock_failure() {
        let lock = IrqSpinlock::new(42);

        // 获取锁
        let guard1 = lock.try_lock().unwrap();
        assert!(lock.is_locked());

        // 第二次尝试获取锁应该失败
        let guard2 = lock.try_lock();
        assert!(guard2.is_none());

        // 释放第一个锁
        drop(guard1);
        assert!(!lock.is_locked());
    }

    #[test]
    fn test_irq_spinlock_lock_and_modify() {
        let lock = IrqSpinlock::new(0);

        {
            let mut guard = lock.lock();
            *guard = 100;
            assert_eq!(*guard, 100);
        } // guard被drop，锁被释放

        assert!(!lock.is_locked());

        // 验证值被修改
        let guard = lock.lock();
        assert_eq!(*guard, 100);
    }

    #[test]
    fn test_multiple_types() {
        // 测试不同类型的数据
        let string_lock = IrqSpinlock::new(String::from("hello"));
        {
            let mut guard = string_lock.lock();
            guard.push_str(" world");
            assert_eq!(guard.as_str(), "hello world");
        }

        let array_lock = IrqSpinlock::new([1, 2, 3]);
        {
            let guard = array_lock.lock();
            assert_eq!(*guard, [1, 2, 3]);
        }

        let struct_lock = IrqSpinlock::new((42, "test"));
        {
            let guard = struct_lock.lock();
            assert_eq!(guard.0, 42);
            assert_eq!(guard.1, "test");
        }
    }
}

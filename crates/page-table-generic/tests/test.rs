//! Mock implementations for testing
//!
//! This module provides mock implementations used in tests for the page-table-generic crate.
#![cfg(not(target_os = "none"))]

use page_table_generic::*;
use std::alloc::{self, Layout};
use std::vec::Vec;

mod mocks;

use mocks::*;

#[test]
fn test_pte() {
    let mut want = PteImpl(0);
    want.set_valid(true);
    assert!(want.valid());

    let addr = PhysAddr::from(0xff123456000usize);
    want.set_paddr(addr);
    assert_eq!(want.paddr(), addr);
}

fn test_high<T: TableGeneric, A: FramAllocator>(pte: T::P, alloc: A) {
    let mut pg = PageTable::<T, A>::new(alloc).unwrap();
    pg.map(&MapConfig {
        vaddr: 0xfffff00000000000usize.into(),
        paddr: 0x0000usize.into(),
        size: 0x2000,
        pte,
        allow_huge: false,
        flush: false,
    })
    .unwrap();
    let mut count = 0;
    for p in pg.walk_valid() {
        println!("l: {}, va: {:?}, pte: {:?}", p.level, p.vaddr, p.pte);
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn test_new() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .try_init();

    test_high::<T4kL4, Fram4k>(PteImpl(0), Fram4k);

}

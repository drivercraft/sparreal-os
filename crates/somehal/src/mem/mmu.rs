use page_table_generic::PageTable;

use crate::mem::ram::Ram;

pub(crate) type ArchPageTable<A> = PageTable<<crate::arch::Arch as crate::ArchTrait>::P, A>;
#[allow(unused)]
pub(crate) type ArchPte =
    <<crate::arch::Arch as crate::ArchTrait>::P as page_table_generic::TableGeneric>::P;

static BOOT_TABLE: spin::Once<ArchPageTable<Ram>> = spin::Once::new();

pub(crate) fn new_boot_table() -> ArchPageTable<Ram> {
    ArchPageTable::<Ram>::new(Ram).unwrap()
}

pub(crate) fn set_boot_table(table: ArchPageTable<Ram>) {
    BOOT_TABLE.call_once(|| table);
}

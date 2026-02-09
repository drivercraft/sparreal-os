use crate::RangeOp;
use core::ops::Range;

/// 测试用的 Range 类型
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestRange {
    pub start: u64,
    pub end: u64,
    pub kind: RangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeKind {
    TypeA,
    TypeB,
    TypeC,
}

impl Default for RangeKind {
    fn default() -> Self {
        RangeKind::TypeA
    }
}

impl TestRange {
    pub fn new(start: u64, end: u64, kind: RangeKind) -> Self {
        Self { start, end, kind }
    }
}

impl RangeOp for TestRange {
    type Kind = RangeKind;
    type Type = u64;

    fn range(&self) -> Range<Self::Type> {
        self.start..self.end
    }

    fn kind(&self) -> Self::Kind {
        self.kind.clone()
    }

    fn overwritable(&self, _other: Self::Type) -> bool {
        true
    }

    fn clone_with_range(&self, range: Range<Self::Type>) -> Self {
        Self {
            start: range.start,
            end: range.end,
            kind: self.kind.clone(),
        }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRequest {
    pub offset: usize,
    pub limit: usize,
}

impl PageRequest {
    pub const DEFAULT_LIMIT: usize = 100;
    pub const MAX_LIMIT: usize = 1000;

    pub fn normalized(self) -> Self {
        Self {
            offset: self.offset,
            limit: self.limit.clamp(1, Self::MAX_LIMIT),
        }
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self { offset: 0, limit: Self::DEFAULT_LIMIT }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub has_more: bool,
}

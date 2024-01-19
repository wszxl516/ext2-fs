use alloc::vec::Vec;

use crate::fs::error::Error;

#[derive(Debug)]
pub enum Offset {
    Block {
        block_size: u64,
        block_num: u64,
    },
    BlockOffset {
        block_size: u64,
        block_num: u64,
        offset: u64,
    },
}

impl Offset {
    pub const fn new(block_size: u64, block_num: u64) -> Self {
        Self::Block { block_size, block_num }
    }
    pub const fn new_offset(block_size: u64, block_num: u64, offset: u64) -> Self {
        Self::BlockOffset { block_size, block_num, offset }
    }
    pub fn value(&self) -> u64 {
        match self {
            Offset::Block {
                block_size,
                block_num,
            } => (*block_num) * (*block_size),
            Offset::BlockOffset {
                block_size,
                block_num,
                offset,
            } => *block_num * *block_size + *offset,
        }
    }
}

pub trait Disk {
    fn read(&self, buffer: &mut [u8]) -> Result<usize, Error>;
    fn write(&self, buffer: &[u8]) -> Result<usize, Error>;

    fn read_at(&self, offset: &Offset, size: u64) -> Result<Vec<u8>, Error>;
    fn write_at(&self, offset: &Offset, buffer: &[u8]) -> Result<usize, Error>;

    fn seek(&self, offset: u64) -> Result<(), Error>;
}

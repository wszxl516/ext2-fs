#![allow(dead_code)]

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::{Display, Formatter};

use crate::ext2::Ext2Filesystem;
use crate::ext2::inode::{EXT2_NDIR_BLOCKS, Ext2Inode};
use crate::fs::disk::Offset;
use crate::fs::error::Error;
use crate::fs::io::CoreRead;
use crate::fs::stat::Stat;

pub struct FsFile<'a> {
    name: String,
    fs: &'a mut Ext2Filesystem,
    pub inode: Ext2Inode,
    blocks: Vec<u64>,
    pos: u64,
    stat: Stat,
}

impl Display for FsFile<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?} {}", self.inode, self.pos)
    }
}

impl FsFile<'_> {
    pub fn new(fs: &mut Ext2Filesystem, inode: Ext2Inode, blocks: Vec<u64>, name: String) -> FsFile {
        let meta = inode.metadata();
        FsFile {
            name,
            fs,
            inode,
            blocks,
            pos: 0,
            stat: meta,
        }
    }
    pub fn inode(&self) -> u64 {
        self.inode.inode()
    }
    fn read_block(&mut self, file_block_num: u64) -> Result<Vec<u8>, Error> {
        let offset = Offset::new(
            self.inode.get_block_size(),
            self.blocks[file_block_num as usize],
        );
        self.fs.disk.read_at(&offset, self.inode.get_block_size())
    }

    fn write_block(&mut self, file_block_num: u64, offset: u64, buffer: &[u8]) -> Result<usize, Error> {
        assert!(file_block_num < EXT2_NDIR_BLOCKS as u64);
        //TODO: size > 12k file
        let mut inode = self.inode;
        if inode.ext2_inode.i_block[file_block_num as usize] == 0 {
            if let Some(new_block) = self.fs.alloc_block() {
                self.blocks.push(new_block as u64);
                inode.ext2_inode.i_block[file_block_num as usize] = new_block;
                inode.ext2_inode.i_blocks += 1;
                self.inode.data_blocks_count += 1;
                inode.write(&self.fs.disk, &self.fs.block_groups)
            }
        }
        self.inode = inode;
        let offset = Offset::new_offset(
            self.inode.get_block_size(),
            self.blocks[file_block_num as usize],
            offset,
        );
        self.fs.disk.write_at(&offset, buffer)
    }
    fn how_many_bytes(&self, buffer_len: usize) -> usize {
        if self.pos + buffer_len as u64 > self.inode.get_size() {
            (self.inode.get_size() - self.pos) as usize
        } else {
            buffer_len
        }
    }

    fn zero_padding(&self, read_bytes: usize, buffer_len: usize, buffer: &mut Vec<u8>) {
        if read_bytes < buffer_len {
            let zero: Vec<u8> = vec![0; buffer_len - read_bytes];
            buffer.extend_from_slice(&zero);
        }
    }

    fn is_eol(&self) -> bool {
        self.pos >= self.inode.get_size()
    }
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if self.is_eol() {
            // End of file
            let zero: Vec<u8> = vec![0; buf.len()];
            buf.copy_from_slice(&zero[..]);
            Ok(0)
        } else {
            let buffer_len = buf.len();
            let read_bytes = self.how_many_bytes(buffer_len);
            let block_num = self.pos / self.inode.get_block_size();
            let mut buffer = self.read_block(block_num)?;
            self.zero_padding(read_bytes, buffer_len, &mut buffer);
            let block_pos: usize = (self.pos - block_num * self.inode.get_block_size()) as usize;
            buf.copy_from_slice(&buffer[block_pos..block_pos + buffer_len]);
            self.pos += read_bytes as u64;
            Ok(read_bytes)
        }
    }
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        let block_size = self.inode.get_block_size();
        let mut write_bytes = 0;
        let mut buffer = buf;
        loop {
            let write_buf = match buffer.len() <= block_size as usize {
                true => {
                    buffer
                }
                false => {
                    let b = buffer.split_at(block_size as usize);
                    buffer = b.1;
                    b.0
                }
            };
            let blk_num = self.pos / block_size;
            let blk_pos = self.pos % block_size;
            let size = self.write_block(blk_num, blk_pos, write_buf)?;
            self.pos += size as u64;
            write_bytes += size;
            if write_bytes == buf.len() {
                break;
            }
        }
        let mut inode = self.inode;
        inode.ext2_inode.i_size += write_bytes as u32;
        inode.size += write_bytes as u64;
        self.inode = inode;
        inode.write(&self.fs.disk, &self.fs.block_groups);
        Ok(write_bytes)
    }
    pub fn seek(&mut self, offset: u64) {
        self.pos = offset
    }
    pub fn stat(&self) -> Stat {
        self.stat
    }
    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn blocks(&self) -> &Vec<u64> {
        &self.blocks
    }
}

impl CoreRead for FsFile<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.read(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, Error> {
        let file_size = self.inode.metadata().size as usize;
        let mut tmp = [0u8; 32];
        let mut n = 0;
        loop {
            let rn = self.read(&mut tmp).unwrap();
            buf.extend_from_slice(&tmp[0..rn]);
            n += rn;
            if n >= file_size { break; }
        }
        Ok(n)
    }
}


#![allow(dead_code)]

use alloc::{format, vec};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::slice::SlicePattern;
use core::str;

use crate::{align_up, int_get, to_slice};
use crate::ext2::dir::{Ext2DirEntry, Ext2DirEntryStruct};
use crate::ext2::group::{EXT2_GROUP_DESC_SIZE, Ext2BlockGroups, Ext2GroupDesc};
use crate::ext2::inode::{Ext2Inode, Ext2InodeStruct};
use crate::ext2::superblock::Ext2SuperBlock;
use crate::fs::{base_dir, base_file};
use crate::fs::disk::{Disk, Offset};
use crate::fs::error::Error;
use crate::fs::file::FsFile;
use crate::fs::io::CoreRead;
use crate::fs::stat::Stat;

pub mod dir;
pub mod group;
pub mod inode;
pub mod superblock;

const EXT2_ROOT_INO: u64 = 2;

pub struct Ext2Filesystem {
    pub disk: Box<dyn Disk>,
    super_block: Ext2SuperBlock,
    pub block_groups: Ext2BlockGroups,
}

impl Ext2Filesystem {
    pub fn mount(disk: Box<dyn Disk>) -> Result<Ext2Filesystem, Error> {
        let super_block = Ext2SuperBlock::new(disk.as_ref())?;
        let block_groups = Ext2BlockGroups::new(&super_block.clone())?;
        Ok(Ext2Filesystem {
            disk,
            super_block,
            block_groups,
        })
    }

    /// Get inode by number
    pub fn read_inode(&self, inode_num: u64) -> Result<Ext2Inode, Error> {
        Ext2Inode::new(
            &self.disk,
            self.super_block.s_inode_size as u64,
            self.super_block.get_block_size(),
            &self.block_groups,
            inode_num,
        )
    }

    /// Get inode by path
    fn resolve<'a>(&'a self, path: &'a str) -> Result<(Ext2Inode, String), Error> {
        let root_inode = self.read_inode(EXT2_ROOT_INO)?;
        self.resolve_relative(path, root_inode, false)
    }

    /// Get inode by relative path
    fn resolve_relative<'a>(
        &'a self,
        path: &'a str,
        mut inode: Ext2Inode,
        link: bool,
    ) -> Result<(Ext2Inode, String), Error> {
        if path.starts_with("/") {
            // if the path is absolute, resolve from root inode
            inode = self.read_inode(EXT2_ROOT_INO)?;
        }
        let path_parts: Vec<_> = path.split("/").collect();
        let last = path_parts.len() - 1;
        let mut file_name = String::new();
        for (i, part) in path_parts.iter().enumerate() {
            file_name.clear();
            file_name.push_str(part);
            if !part.is_empty() {
                match inode.get_child(&self.disk, self, &self.block_groups, part) {
                    Some(child) => {
                        let resolve_symlink = child.metadata().is_symlink() && (!link || i != last);
                        if resolve_symlink {
                            let target = child.read_link(&self.disk)?;
                            (inode, file_name) = self.resolve_relative(&target, inode, link)?;
                        } else {
                            inode = child
                        }
                    }
                    None => {
                        return Err(Error::NotFound(format!(
                            "{} No such file or directory",
                            path
                        )));
                    }
                }
            }
        }
        Ok((inode, file_name.to_string()))
    }
}

impl Ext2Filesystem {
    pub fn open(&mut self, path: &str) -> Result<FsFile, Error> {
        let (inode, name) = self.resolve(path)?;
        if inode.metadata().is_dir() {
            Err(Error::InvalidInput(format!("{} Is a directory", path)))
        } else {
            let blocks = inode.get_blocks(&self.disk)?;
            Ok(FsFile::new(self, inode, blocks, name))
        }
    }

    /// Get block size
    fn get_block_size(&self) -> u64 {
        self.super_block.get_block_size()
    }

    fn get_groups_count(&self) -> usize {
        self.super_block.get_groups_count()
    }
    /// Get the number of blocks in file system
    fn get_blocks_count(&self) -> u64 {
        self.super_block.s_blocks_count as u64
    }

    /// Get the number of unallocated blocks
    fn get_free_blocks_count(&self) -> u64 {
        self.super_block.s_free_blocks_count as u64
    }

    /// Read the contents of a given directory
    pub fn read_dir(&self, path: &str) -> Result<BTreeMap<String, Ext2DirEntry>, Error> {
        let (inode, _) = self.resolve(path)?;
        inode.read_dir(&self.disk, self, path)
    }
    fn mk_default_dir(&self, path: &str) -> Result<(), Error> {
        let (parent_inode, _) = self.resolve(&base_dir(path))?;
        let (current_inode, _) = self.resolve(path)?;
        let (block_num, _) = current_inode.find_last_dir_entry(&self.disk)?;
        let mut current_dir = Ext2DirEntryStruct::default();
        let mut parent_dir = Ext2DirEntryStruct::default();
        // dir .
        current_dir.rec_len = align_up!(8 + 1, 4) as u16;
        current_dir.inode_num = current_inode.inode_num as u32;
        current_dir.file_type = 2;
        current_dir.name_len = 1;
        // dir ..
        parent_dir.rec_len = (self.get_block_size() - current_dir.rec_len as u64) as u16;
        parent_dir.inode_num = parent_inode.inode_num as u32;
        parent_dir.file_type = 2;
        parent_dir.name_len = 2;
        self.write_block(block_num, 0, to_slice!(&current_dir, Ext2DirEntryStruct))?;
        self.write_block(block_num, 8, ".".as_bytes())?;
        self.write_block(
            block_num,
            current_dir.rec_len as u64,
            to_slice!(&parent_dir, Ext2DirEntryStruct),
        )?;
        self.write_block(block_num, (current_dir.rec_len + 8) as u64, "..".as_bytes())?;
        Ok(())
    }
    pub fn mk_dir(&mut self, path: &str, perm: u16) -> Result<(), Error> {
        self.new_dir_entry(path, perm, false)?;
        self.mk_default_dir(path)?;
        Ok(())
    }
    pub fn new_file(&mut self, path: &str, perm: u16) -> Result<FsFile, Error> {
        let (inode, name) = self.new_dir_entry(path, perm, true)?;
        Ok(FsFile::new(self, inode, vec![inode.blocks()[0] as u64], name))
    }
    pub fn new_dir_entry(&mut self, path: &str, perm: u16, is_file: bool) -> Result<(Ext2Inode, String), Error> {
        match self.is_exist(path) {
            true => Err(Error::FileExists(format!("{}", path))),
            false => {
                let (parent_inode, _) = self.resolve(&base_dir(path))?;
                let block_size = self.super_block.get_block_size();
                let (block_num, offset) = parent_inode.find_last_dir_entry(&self.disk)?;
                let buffer = self.read_block(block_num).unwrap();
                let entry_size = core::mem::size_of::<Ext2DirEntryStruct>();
                let new_inum = self.alloc_inode_num().unwrap();
                let new_block_num = self.alloc_block().unwrap();
                let inode_new = Ext2Inode {
                    inode_num: new_inum,
                    ext2_inode: match is_file {
                        true => Ext2InodeStruct::new_file(perm, new_block_num, block_size as u32),
                        false => Ext2InodeStruct::new_dir(perm, new_block_num, block_size as u32),
                    },
                    inode_size: self.super_block.s_inode_size as u64,
                    block_size,
                    size: block_size,
                    data_blocks_count: 1,
                };
                inode_new.write(&self.disk, &self.block_groups);
                let mut entry = buffer[offset..]
                    .as_slice()
                    .read_struct::<Ext2DirEntryStruct>()?;
                entry.rec_len = align_up!(entry_size + entry.name_len as usize, 4) as u16;
                let mut new_entry = Ext2DirEntryStruct::default();
                let new_name = base_file(path);
                new_entry.rec_len = (block_size - offset as u64 - entry.rec_len as u64) as u16;
                new_entry.inode_num = new_inum as u32;
                new_entry.file_type = match is_file {
                    true => 1,
                    false => 2,
                };
                new_entry.name_len = new_name.len() as u8;
                self.write_block(
                    block_num,
                    offset as u64,
                    to_slice!(&entry, Ext2DirEntryStruct),
                )?;
                self.write_block(
                    block_num,
                    offset as u64 + entry.rec_len as u64,
                    to_slice!(&new_entry, Ext2DirEntryStruct),
                )?;
                self.write_block(
                    block_num,
                    offset as u64 + entry.rec_len as u64 + entry_size as u64,
                    new_name.as_bytes(),
                )?;
                Ok((inode_new, new_name))
            }
        }
    }
    pub fn is_exist(&self, path: &str) -> bool {
        match self.resolve(path) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    /// Given a path, query the file system to get information about a file, directory, etc.
    fn metadata(&self, path: &str) -> Result<Stat, Error> {
        let root_inode = self.read_inode(EXT2_ROOT_INO)?;
        let (inode, _) = self.resolve_relative(path, root_inode, true)?;
        Ok(inode.metadata())
    }

    /// Reads a symbolic link, returning the file that the link points to
    fn read_link(&self, path: &str) -> Result<String, Error> {
        // Read value of a symbolic link
        let root_inode = self.read_inode(EXT2_ROOT_INO)?;
        let (inode, _) = self.resolve_relative(path, root_inode, true)?;
        inode.read_link(&self.disk)
    }
    pub fn read_block(&self, block_num: u64) -> Result<Vec<u8>, Error> {
        let block_size = self.get_block_size();
        let offset = Offset::new(block_size, block_num);
        self.disk.read_at(&offset, block_size)
    }
    pub fn write_block(&self, block_num: u64, offset: u64, buffer: &[u8]) -> Result<usize, Error> {
        let block_size = self.get_block_size();
        let offset = Offset::BlockOffset {
            block_num,
            block_size,
            offset,
        };
        self.disk.write_at(&offset, buffer)
    }
}

impl Ext2Filesystem {
    fn get_block_bitmap(&self, num: u64) -> Result<Vec<u8>, Error> {
        let group = self.block_groups.get_group(num, &self.disk)?;
        let bitmap_block_num = group.ext2_group_desc.bg_block_bitmap as u64;
        let block_size = self.get_block_size();
        let offset = Offset::new(block_size, bitmap_block_num);
        self.disk.read_at(&offset, block_size)
    }
    pub fn alloc_block(&mut self) -> Option<u32> {
        for i in 0..self.get_groups_count() {
            match self.alloc_block_group(i as u64) {
                None => continue,
                Some(block_num) => return Some(block_num),
            }
        }
        None
    }
    pub fn alloc_block_group(&mut self, group_num: u64) -> Option<u32> {
        let mut bitmap = self.get_block_bitmap(group_num).ok()?;
        let mut chunk = u8::MAX;
        let mut bnum = 0u32;
        for i in 0..self.get_block_size() as usize {
            chunk = bitmap[i];
            if chunk != u8::MAX {
                break;
            }
            bnum += 8;
        }
        if chunk != u8::MAX {
            while chunk & 1 != 0 {
                chunk >>= 1;
                bnum += 1;
            }
        } else {
            return None;
        }
        bnum += 1;
        self.bitmap_set_bit(&mut bitmap, bnum, true).ok()?;
        self.set_block_bitmap(group_num, &bitmap).ok()?;
        self.set_group_free(group_num as u32, 0, -1).ok()?;
        self.set_sb_free(0, -1);
        Some(bnum)
    }

    fn set_block_bitmap(&self, num: u64, bitmap: &Vec<u8>) -> Result<(), Error> {
        let group = self.block_groups.get_group(num, &self.disk)?;
        let bitmap_block_num = group.ext2_group_desc.bg_block_bitmap as u64;
        let block_size = self.get_block_size();
        let offset = Offset::new(block_size, bitmap_block_num);
        match self.disk.write_at(&offset, bitmap.as_slice()) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
    fn bitmap_set_bit(
        &self,
        bitmap: &mut Vec<u8>,
        bnum: u32,
        set: bool,
    ) -> Result<(), Error> {
        let index = ((bnum - 1) / 8) as usize;
        let bit_n = (bnum - 1) % 8;
        let mut c = bitmap[index];
        if set {
            c |= 1 << bit_n;
        } else {
            c &= !(1 << bit_n);
        }
        bitmap[index] = c;
        Ok(())
    }

    pub fn get_inode_bitmap(&self, num: u64) -> Result<Vec<u8>, Error> {
        let group = self.block_groups.get_group(num, &self.disk)?;
        let bitmap_block_num = group.ext2_group_desc.bg_inode_bitmap as u64;
        let block_size = self.get_block_size();
        let offset = Offset::new(block_size, bitmap_block_num);
        self.disk.read_at(&offset, block_size)
    }
    fn set_inode_bitmap(&self, inode_num: u64, bitmap: &Vec<u8>) -> Result<(), Error> {
        let group = self.block_groups.get_inode_group(inode_num, &self.disk)?;
        let bitmap_block_num = group.ext2_group_desc.bg_inode_bitmap as u64;
        let block_size = self.get_block_size();
        let offset = Offset::new(block_size, bitmap_block_num);
        match self.disk.write_at(&offset, bitmap.as_slice()) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
    pub fn alloc_inode_num(&mut self) -> Option<u64> {
        for i in 0..self.get_groups_count() {
            match self.alloc_inode_num_group(i as u64) {
                None => continue,
                Some(inode_num) => return Some(inode_num),
            }
        }
        None
    }
    pub fn alloc_inode_num_group(&mut self, group_num: u64) -> Option<u64> {
        let mut bitmap = self.get_inode_bitmap(group_num).ok()?;
        let mut chunk = u8::MAX;
        let mut inum = 8u32;
        //reserved inode 1~10
        for i in 1..self.get_block_size() as usize {
            chunk = bitmap[i];
            if chunk != u8::MAX {
                break;
            }
            inum += 8;
        }
        if chunk != u8::MAX {
            while chunk & 1 != 0 {
                chunk >>= 1;
                inum += 1;
            }
        } else {
            return None;
        }
        inum += 1;
        self.bitmap_set_bit(&mut bitmap, inum, true).ok()?;
        self.set_inode_bitmap(inum as u64, &bitmap).ok()?;
        self.set_group_free(group_num as u32, -1, 0).ok()?;
        self.set_sb_free(-1, 0);
        Some(inum as u64)
    }

    pub fn set_group_free(
        &self,
        group_num: u32,
        inode_free: i64,
        block_free: i64,
    ) -> Result<(), Error> {
        let size = EXT2_GROUP_DESC_SIZE as u64;
        let block_size = self.get_block_size();
        let offset = Offset::new_offset(
            block_size,
            if block_size == 1024 { 2 } else { 1 },
            group_num as u64 * size,
        );
        let buffer = self.disk.read_at(&offset, size)?;
        let mut desc = buffer.as_slice().read_struct::<Ext2GroupDesc>()?;
        let mut bg_free_blocks_count = desc.bg_free_blocks_count as i64;
        if bg_free_blocks_count != 0 {
            bg_free_blocks_count += block_free
        }
        desc.bg_free_blocks_count = bg_free_blocks_count as u16;

        let mut bg_free_inodes_count = desc.bg_free_inodes_count as i64;
        if bg_free_inodes_count != 0 {
            bg_free_inodes_count += inode_free
        }
        desc.bg_free_inodes_count = bg_free_inodes_count as u16;
        self.disk
            .write_at(&offset, to_slice!(&desc, Ext2GroupDesc))?;
        Ok(())
    }

    pub fn get_block_num(&self, block_num: u64, level: u32) -> Vec<u64> {
        assert!(level > 0 && level <= 3);
        let buffer = self.read_block(block_num).unwrap();
        let mut blocks = Vec::new();
        let mut bytes = buffer.as_slice();
        for _ in 0..buffer.len() / 4 {
            let block = int_get!(bytes, u32);
            if block == 0 {
                break;
            }
            if level == 1 {
                blocks.push(block as u64)
            }
            if level == 2 {
                let b = self.get_block_num(block as u64, 1);
                blocks.extend(b)
            }
            if level == 3 {
                for blk_num in self.get_block_num(block as u64, 1) {
                    let b = self.get_block_num(blk_num, 1);
                    blocks.extend(b)
                }
            }
        }
        blocks
    }
    pub fn indirect_block_table_offset(&self, block_table: [u64; 3]) -> Option<(u64, usize)> {
        let blk_num_size = core::mem::size_of::<u32>();
        let b1 = self.get_block_num(block_table[0], 1);
        if b1.len() < 1024 / blk_num_size {
            return Some((block_table[0], (b1.len() - 1) * blk_num_size));
        }
        let b1 = self.get_block_num(block_table[1], 1);
        for b2 in b1 {
            let blocks = self.get_block_num(b2, 1);
            if blocks.len() < 1024 / blk_num_size {
                return Some((b2, (blocks.len()) * blk_num_size));
            }
        }
        let b1 = self.get_block_num(block_table[2], 1);
        for b2 in b1 {
            let blocks2 = self.get_block_num(b2, 1);
            for b3 in &blocks2 {
                if blocks2.len() < 1024 / blk_num_size {
                    return Some((*b3, (blocks2.len()) * blk_num_size));
                }
            }
        }
        None
    }
    pub fn set_sb_free(&mut self, inode_free: i64, block_free: i64) {
        self.super_block.s_free_blocks_count = (self.super_block.s_free_blocks_count as i64 + block_free) as u32;
        self.super_block.s_free_inodes_count = (self.super_block.s_free_inodes_count as i64 + inode_free) as u32;
        self.super_block.write(self.disk.as_ref());
    }
}

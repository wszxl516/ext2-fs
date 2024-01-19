use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem;

use crate::ext2::superblock::Ext2SuperBlock;
use crate::fs::disk::{Disk, Offset};
use crate::fs::error::Error;
use crate::fs::io::CoreRead;

pub const EXT2_GROUP_DESC_SIZE: usize = mem::size_of::<Ext2GroupDesc>();

/// Blocks are divided up into block groups.
/// A block group is a contiguous groups of blocks
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Ext2GroupDesc {
    pub bg_block_bitmap: u32,
    // The block which contains the block bitmap for the group.
    pub bg_inode_bitmap: u32,
    // The block contains the inode bitmap for the group.
    pub bg_inode_table: u32,
    // The block contains the inode table first block (the starting block of the inode table.).
    pub bg_free_blocks_count: u16,
    // Number of free blocks in the group.
    pub bg_free_inodes_count: u16,
    // Number of free inodes in the group.
    pub bg_used_dirs_count: u16,
    // Number of inodes allocated to the directories.
    pub bg_flags: u16,
    pub bg_exclude_bitmap_lo: u32,
    // Exclude bitmap for snapshots
    pub bg_block_bitmap_csum_lo: u16,
    // crc32c(s_uuid+grp_num+bitmap) LSB
    pub bg_inode_bitmap_csum_lo: u16,
    // crc32c(s_uuid+grp_num+bitmap) LSB
    pub bg_itable_unused: u16,
    // Unused inodes count
    pub bg_checksum: u16,
    // crc16(s_uuid+group_num+group_desc)
}

impl Ext2GroupDesc {
    pub fn new(group_num: usize, buffer: &Vec<u8>) -> Ext2GroupDesc {
        let mut buf =
            &buffer[EXT2_GROUP_DESC_SIZE * group_num..EXT2_GROUP_DESC_SIZE * (group_num + 1)];
        buf.read_struct::<Ext2GroupDesc>().unwrap()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct GroupDesc {
    pub group_num: usize,
    // Group number
    pub ext2_group_desc: Ext2GroupDesc,
    // Ext2 group desc struct
    pub first_inode_num: u64,
    // Fist inode in the group
}

impl GroupDesc {
    pub fn new(group_num: usize, buffer: &Vec<u8>, inodes_per_group: u32) -> GroupDesc {
        GroupDesc {
            group_num,
            ext2_group_desc: Ext2GroupDesc::new(group_num, &buffer),
            first_inode_num: group_num as u64 * inodes_per_group as u64 + 1,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Ext2BlockGroups {
    block_size: u64,
    group_count: u64,
    inodes_per_group: u64,
}

impl Ext2BlockGroups {
    /// Read the Block Groups
    pub fn new(super_block: &Ext2SuperBlock) -> Result<Ext2BlockGroups, Error> {
        let result = Ext2BlockGroups {
            // block_groups,
            block_size: super_block.get_block_size(),
            group_count: super_block.get_groups_count() as u64,
            inodes_per_group: super_block.s_inodes_per_group as u64,
        };
        Ok(result)
    }

    /// Determine which block group the inode belongs to and return the group
    pub fn get_inode_group(&self, inode_num: u64, disk: &Box<dyn Disk>) -> Result<GroupDesc, Error> {
        let group_num = (inode_num - 1) / self.inodes_per_group;
        let desc = self.fetch_group_desc(group_num, disk)?;
        Ok(GroupDesc {
            group_num: group_num as _,
            ext2_group_desc: desc,
            first_inode_num: group_num * self.inodes_per_group + 1,
        })
    }
    pub fn get_group(&self, group_num: u64, disk: &Box<dyn Disk>) -> Result<GroupDesc, Error> {
        let desc = self.fetch_group_desc(group_num, disk)?;
        Ok(GroupDesc {
            group_num: group_num as _,
            ext2_group_desc: desc,
            first_inode_num: group_num * self.inodes_per_group + 1,
        })
    }
    pub fn fetch_group_desc(&self, group_num: u64, disk: &Box<dyn Disk>) -> Result<Ext2GroupDesc, Error> {
        let size = EXT2_GROUP_DESC_SIZE as u64;
        let block_size = self.block_size;
        let offset = Offset::new_offset(
            block_size,
            if block_size == 1024 { 2 } else { 1 },
            group_num * size,
        );
        let buffer = disk.read_at(&offset, size)?;
        buffer.as_slice().read_struct::<Ext2GroupDesc>()
    }
}

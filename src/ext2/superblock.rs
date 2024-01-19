use alloc::string::ToString;
use core::mem;

use crate::fs::disk::{Disk, Offset};
use crate::fs::error::Error;
use crate::fs::io::CoreRead;
use crate::to_slice;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Ext2SuperBlock {
    pub s_inodes_count: u32,
    // Total number of inodes in file system
    pub s_blocks_count: u32,
    // Total number of blocks in file system
    pub s_r_blocks_count: u32,
    // Number of blocks reserved for superuser
    pub s_free_blocks_count: u32,
    // Total number of unallocated blocks
    pub s_free_inodes_count: u32,
    // Total number of unallocated inodes
    pub s_first_data_block: u32,
    // First Data Block
    pub s_log_block_size: u32,
    // Block size
    pub s_log_frag_size: u32,
    // Allocation cluster size
    pub s_blocks_per_group: u32,
    // Number of blocks in each block group
    pub s_frags_per_group: u32,
    // Number of fragments in each block group
    pub s_inodes_per_group: u32,
    // Number of inodes in each block group
    pub s_mtime: u32,
    // Last mount time
    pub s_wtime: u32,
    // Last written time
    pub s_mnt_count: u16,
    // Mounts since its last consistency check
    pub s_max_mnt_count: u16,
    // Mounts before a consistency check
    pub s_magic: u16,
    // Ext2 signature (0xef53),
    pub s_state: u16,
    // File system state
    pub s_pad: u16,
    // What to do when an error is detected
    pub s_minor_rev_level: u16,
    // Minor portion of version
    pub s_lastcheck: u32,
    // Time of last consistency check
    pub s_checkinterval: u32,
    // Interval between forced consistency checks
    pub s_creator_os: u32,
    // Operating system ID
    pub s_rev_level: u32,
    // Major portion of version
    pub s_def_resuid: u16,
    // User ID that can use reserved blocks
    pub s_def_regid: u16,
    // Group ID that can use reserved blocks
    // -- EXT2_DYNAMIC_REV superblocks only ---
    pub s_first_ino: u32,
    // First non-reserved inode
    pub s_inode_size: u16,
    // Size of inode structure
    pub s_block_group_nr: u16,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: [u8; 16],
    // 128-bit uuid for volume
    pub s_volume_name: [u8; 16],
    pub s_last_mounted: [u8; 64],
    pub s_algorithm_usage_bitmap: u32,
    pub s_prealloc_blocks: u8,
    // Nr of blocks to try to preallocate
    pub s_prealloc_dir_blocks: u8,
    // Nr to preallocate for dirs
    pub s_reserved_gdt_blocks: u16,
    // Per group table for online growth
    s_reserved: [u32; 204],
}

impl Ext2SuperBlock {
    pub const SUPER_BLOCK_SIZE: u64 = 1024;
    pub const SUPER_BLOCK: u64 = 1;
    pub const MAGIC: u16 = 0xef53;
    pub fn get_groups_count(&self) -> usize {
        let count = (self.s_blocks_count / self.s_blocks_per_group) as usize;
        match self.s_blocks_count % self.s_blocks_per_group == 0 {
            true => count,
            false => count + 1
        }
    }
    // Get block size
    pub fn get_block_size(&self) -> u64 {
        1024 << self.s_log_block_size as u64
    }
    // Read the Superblock
    pub fn new(disk: &dyn Disk) -> Result<Ext2SuperBlock, Error> {
        assert_eq!(mem::size_of::<Ext2SuperBlock>(), Self::SUPER_BLOCK_SIZE as usize);
        let offset = Offset::new(Self::SUPER_BLOCK_SIZE, Self::SUPER_BLOCK);
        let buffer = disk.read_at(&offset, Self::SUPER_BLOCK_SIZE)?;
        let super_block = buffer.as_slice().read_struct::<Ext2SuperBlock>()?;
        // Check ext2 signature
        if super_block.s_magic == Self::MAGIC {
            Ok(super_block)
        } else {
            Err(Error::InvalidData("Invalid filesystem".to_string()))
        }
    }
    pub fn write(&self, disk: &dyn Disk) {
        let offset = Offset::new(Self::SUPER_BLOCK_SIZE, Self::SUPER_BLOCK);
        disk.write_at(&offset, to_slice!(self, Self)).unwrap();
    }
}

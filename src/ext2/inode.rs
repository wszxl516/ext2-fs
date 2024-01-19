use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::mem;
use core::str;

use crate::{align_up, to_slice};
use crate::ext2::dir::{Ext2DirEntry, Ext2DirEntryStruct};
use crate::ext2::Ext2Filesystem;
use crate::ext2::group::Ext2BlockGroups;
use crate::fs::disk::{Disk, Offset};
use crate::fs::error::Error;
use crate::fs::io::CoreRead;
use crate::fs::stat::{FileFlags, Mode, Stat};

// Constants relative to the data blocks
pub const EXT2_NDIR_BLOCKS: usize = 12;
pub const EXT2_IND_BLOCK: usize = EXT2_NDIR_BLOCKS;
pub const EXT2_DOUBLY_IND_BLOCK: usize = EXT2_IND_BLOCK + 1;
pub const EXT2_TRIPLY_IND_BLOCK: usize = EXT2_DOUBLY_IND_BLOCK + 1;
pub const EXT2_N_BLOCKS: usize = EXT2_TRIPLY_IND_BLOCK + 1;
pub const I_BLOCKS_SIZE: usize = EXT2_N_BLOCKS * 4;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Ext2InodeStruct {
    pub i_mode: u16,
    /* File mode */
    pub i_uid: u16,
    /* Low 16 bits of Owner Uid */
    pub i_size: u32,
    /* Lower 32 bits of size in bytes */
    pub i_atime: u32,
    /* Access time */
    pub i_ctime: u32,
    /* Creation time */
    pub i_mtime: u32,
    /* Modification time */
    pub i_dtime: u32,
    /* Deletion Time */
    pub i_gid: u16,
    /* Low 16 bits of Group Id */
    pub i_links_count: u16,
    /* Links count */
    pub i_blocks: u32,
    /* Count of disk sectors (not Ext2 blocks) in use by this inode */
    pub i_flags: u32,
    /* File flags */
    pub l_i_reserved1: u32,
    pub i_block: [u32; EXT2_N_BLOCKS],
    /* Pointers to blocks (12) */
    pub i_generation: u32,
    /* File version (for NFS) */
    pub i_file_acl: u32,
    /* File ACL */
    pub i_size_high: u32,
    pub i_faddr: u32,
    /* Fragment address */
    pub l_i_frag: u8,
    /* Fragment number */
    pub l_i_fsize: u8,
    /* Fragment size */
    pub i_pad1: u16,
    pub l_i_uid_high: u16,
    /* these 2 fields    */
    pub l_i_gid_high: u16,
    /* were reserved2[0] */
    pub l_i_reserved2: u32,
}

impl Ext2InodeStruct {
    pub fn new_dir(perm: u16, first_block: u32, size: u32) -> Self {
        let mut new = Self::default();
        new.i_mode = Mode::DIRECTORY.bits() | perm;
        new.i_links_count = 1;
        new.i_block[0] = first_block;
        new.i_blocks = 2;
        new.i_size = size;
        new
    }
    pub fn new_file(perm: u16, first_block: u32, size: u32) -> Self {
        let mut new = Self::default();
        new.i_mode = Mode::FILE.bits() | perm;
        new.i_links_count = 1;
        new.i_block[0] = first_block;
        new.i_blocks = 2;
        new.i_size = size;
        new
    }
}

impl Ext2InodeStruct {
    pub fn default() -> Ext2InodeStruct {
        let ionode: Ext2InodeStruct = unsafe { mem::zeroed() };
        ionode
    }
    pub fn is_file(&self) -> bool {
        Mode::from_bits_truncate(self.i_mode).is_file()
    }
    pub fn size(&self) -> u64 {
        // Calculate the size in bytes
        if self.is_file() {
            self.i_size as u64 | ((self.i_size_high as u64) << 32)
        } else {
            self.i_size as u64
        }
    }
    pub const fn blocks(&self) -> [u32; EXT2_N_BLOCKS] {
        self.i_block
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Ext2Inode {
    pub(crate) inode_num: u64,
    // Inode number
    pub(crate) ext2_inode: Ext2InodeStruct,
    // Ext2 inode struct
    pub(crate) inode_size: u64,
    // Inode size
    pub(crate) block_size: u64,
    // Block size
    pub(crate) size: u64,
    // Size in bytes
    pub(crate) data_blocks_count: u64, // Number of data blocks
}

impl Ext2Inode {
    pub fn new(
        disk: &Box<dyn Disk>,
        inode_size: u64,
        block_size: u64,
        block_groups: &Ext2BlockGroups,
        inode_num: u64,
    ) -> Result<Ext2Inode, Error> {
        // Determinate the block group
        let group = block_groups.get_inode_group(inode_num, disk)?;
        // Calculate the offset
        let offset = Offset::new_offset(
            block_size,
            group.ext2_group_desc.bg_inode_table as u64,
            (inode_num - group.first_inode_num) * inode_size,
        );
        // Read the inode from the disk
        let buffer = disk.read_at(&offset, inode_size)?;
        let inode = buffer.as_slice().read_struct::<Ext2InodeStruct>()?;
        // Calculate the size
        let size = inode.size();
        // Calculate the number of data blocks
        let data_blocks_count = size / block_size;
        let data_blocks_count = match size % block_size == 0 {
            true => data_blocks_count,
            false => data_blocks_count + 1
        };
        Ok(Ext2Inode {
            inode_num,
            ext2_inode: inode,
            inode_size,
            block_size,
            size,
            data_blocks_count,
        })
    }

    pub fn write(&self, disk: &Box<dyn Disk>, block_groups: &Ext2BlockGroups) {
        // Determinate the block group
        let group = block_groups.get_inode_group(self.inode_num, disk).unwrap();
        // Calculate the offset
        let offset = Offset::new_offset(
            self.block_size,
            group.ext2_group_desc.bg_inode_table as u64,
            (self.inode_num - group.first_inode_num) * self.inode_size,
        );
        self.ext2_inode;
        disk.write_at(&offset, to_slice!(&self.ext2_inode, Ext2InodeStruct))
            .unwrap();
    }
    pub fn blocks(&self) -> [u32; EXT2_N_BLOCKS] {
        self.ext2_inode.blocks()
    }
    pub fn inode(&self) -> u64 {
        self.inode_num
    }
    /// Read blocks iterator
    pub fn read_blocks_iter<'a>(&'a self, disk: &'a Box<dyn Disk>) -> Result<ReadBlock<'a>, Error> {
        Ok(ReadBlock {
            disk: &Box::new(disk),
            block_size: self.block_size,
            blocks: self.get_blocks_iter(&disk)?,
        })
    }

    /// Read file content
    pub fn read(&self, disk: &Box<dyn Disk>) -> Result<Vec<u8>, Error> {
        let mut buffer: Vec<u8> = Vec::new();
        for block in self.read_blocks_iter(disk)? {
            buffer.extend(&block?);
        }
        Ok(buffer)
    }

    /// Block numbers iterator
    pub fn get_blocks_iter<'a>(
        &'a self,
        disk: &'a Box<dyn Disk>,
    ) -> Result<ReadBlockNum<'a>, Error> {
        Ok(ReadBlockNum::new(
            disk,
            &self.ext2_inode.i_block,
            self.block_size,
            self.data_blocks_count,
        ))
    }

    /// Resolve a child by name - return the child's inode
    pub fn get_child(
        &self,
        disk: &Box<dyn Disk>,
        fs: &Ext2Filesystem,
        block_groups: &Ext2BlockGroups,
        name: &str,
    ) -> Option<Ext2Inode> {
        match self.read_dir(disk, fs, name) {
            Ok(entries) => match entries.get(name) {
                Some(dir_entry) => Some(
                    Ext2Inode::new(
                        disk,
                        self.inode_size,
                        self.block_size,
                        block_groups,
                        dir_entry.inode_num(),
                    )
                        .ok()?,
                ),
                None => None,
            },
            Err(_) => None,
        }
    }

    /// Read value of a symbolic link
    pub fn read_link(&self, disk: &Box<dyn Disk>) -> Result<String, Error> {
        if !self.metadata().is_symlink() {
            return Err(Error::InvalidData("is not a symbolic link".to_string()));
        }
        if self.size <= I_BLOCKS_SIZE as u64 {
            let buffer: [u8; I_BLOCKS_SIZE] = unsafe { mem::transmute(self.ext2_inode.i_block) };
            let target = &buffer[0..self.size as usize];
            match str::from_utf8(target) {
                Ok(result) => Ok(String::from(result)),
                Err(_) => Err(Error::InvalidData("read link name failed.".to_string())),
            }
        } else {
            match String::from_utf8(self.read(disk)?) {
                Ok(result) => Ok(result),
                Err(_) => Err(Error::InvalidData("read link name failed.".to_string())),
            }
        }
    }
    pub fn read_dir(
        &self,
        disk: &Box<(dyn Disk + 'static)>,
        fs: &Ext2Filesystem,
        path: &str,
    ) -> Result<BTreeMap<String, Ext2DirEntry>, Error> {
        if !self.metadata().is_dir() {
            Err(Error::InvalidInput(format!("{} Not a directory", path)))
        } else {
            let mut entries: BTreeMap<String, Ext2DirEntry> = BTreeMap::new();
            // Iterate over blocks
            for buffer in self.read_blocks_iter(disk)? {
                let buffer = buffer?;
                let mut offset: usize = 0;
                // Iterate over block directory entries
                while offset < self.block_size as usize {
                    let (mut dir_entry, rec_len) = Ext2DirEntry::new(&buffer, offset);
                    dir_entry.get_inode(fs)?;
                    offset += rec_len;
                    entries.insert(dir_entry.file_name(), dir_entry);
                }
            }
            Ok(entries)
        }
    }
    pub fn find_last_dir_entry(
        &self,
        disk: &Box<(dyn Disk + 'static)>,
    ) -> Result<(u64, usize), Error> {
        if !self.metadata().is_dir() {
            Err(Error::InvalidInput(format!(
                "inode {} Not a directory",
                self.inode_num
            )))
        } else {
            let mut block_num = 0usize;
            let mut offset_block = 0usize;
            let size = mem::size_of::<Ext2DirEntryStruct>();
            'all_block: for buffer in self.read_blocks_iter(disk)? {
                let buffer = buffer?;
                let mut offset: usize = 0;
                // Iterate over block directory entries
                while offset < self.block_size as usize {
                    let mut buf = &buffer[offset..offset + size];
                    let entry = buf.read_struct::<Ext2DirEntryStruct>().unwrap();
                    let entry_size = align_up!(entry.name_len as usize + size, 4);
                    if entry_size < entry.rec_len.into() || entry.inode_num == 0 {
                        offset_block = offset;
                        break 'all_block;
                    }
                    offset += entry.rec_len as usize;
                }
                block_num += 1;
            }
            Ok((self.blocks()[block_num] as u64, offset_block))
        }
    }

    /// Block numbers
    pub fn get_blocks(&self, disk: &Box<dyn Disk>) -> Result<Vec<u64>, Error> {
        match self.get_blocks_iter(disk) {
            Ok(iterator) => iterator.collect::<Result<Vec<_>, _>>(),
            Err(x) => Err(x),
        }
    }

    /// Block size in bytes
    pub fn get_block_size(&self) -> u64 {
        self.block_size
    }

    /// Size in bytes
    pub fn get_size(&self) -> u64 {
        self.size
    }

    /// Given a path, query the file system to get information about a file, directory, etc.
    pub fn metadata(&self) -> Stat {
        Stat {
            dev: 0,
            ino: self.inode_num,
            mode: Mode::from_bits_truncate(self.ext2_inode.i_mode),
            nlink: self.ext2_inode.i_links_count as u64,
            uid: self.ext2_inode.i_uid as u32,
            gid: self.ext2_inode.i_gid as u32,
            rdev: 0,
            size: self.size,
            atime: self.ext2_inode.i_atime as i64,
            atime_nsec: self.ext2_inode.i_atime as i64 * 1_000_000,
            mtime: self.ext2_inode.i_mtime as i64,
            mtime_nsec: self.ext2_inode.i_mtime as i64 * 1_000_000,
            ctime: self.ext2_inode.i_ctime as i64,
            ctime_nsec: self.ext2_inode.i_ctime as i64 * 1_000_000,
            blksize: self.block_size,
            blocks: self.ext2_inode.i_blocks as u64,
            flags: FileFlags::from_bits_truncate(self.ext2_inode.i_flags),
        }
    }
}

pub struct ReadBlockNum<'a> {
    blocks_per_block: u64,
    // number of block number (each block number is sizeof u32) in a block
    i_block: &'a [u32; EXT2_N_BLOCKS],
    data_blocks_count: u64,
    block_size: u64,
    first_indirect_block: u64,
    first_doubly_indirect_block: u64,
    first_triply_indirect_block: u64,
    curr: u64,
    disk: &'a Box<dyn Disk>,
}

impl ReadBlockNum<'_> {
    pub fn new<'a>(
        disk: &'a Box<dyn Disk>,
        i_block: &'a [u32; EXT2_N_BLOCKS],
        block_size: u64,
        data_blocks_count: u64,
    ) -> ReadBlockNum<'a> {
        let blocks_per_block = block_size / mem::size_of::<u32>() as u64;
        ReadBlockNum {
            blocks_per_block,
            i_block,
            data_blocks_count,
            block_size,
            first_indirect_block: EXT2_NDIR_BLOCKS as u64,
            first_doubly_indirect_block: EXT2_NDIR_BLOCKS as u64 + blocks_per_block,
            first_triply_indirect_block: EXT2_NDIR_BLOCKS as u64
                + blocks_per_block
                + (blocks_per_block * blocks_per_block),
            curr: 0,
            disk,
        }
    }

    /// Get direct block
    fn get_direct_block(&self, i: u64) -> Result<u64, Error> {
        Ok(self.i_block[i as usize] as u64)
    }

    /// Get singly indirect block
    fn get_indirect_block(&mut self, i: u64, indirect_block_num: u64) -> Result<u64, Error> {
        let offset = Offset::new(self.block_size, indirect_block_num);
        let indirect_blocks = self.disk.read_at(&offset, self.block_size)?;
        let addr: usize = i as usize * mem::size_of::<u32>();
        let bytes: [u8; 4] = indirect_blocks[addr..addr + 4]
            .try_into()
            .expect("incorrect length");
        Ok(u32::from_le_bytes(bytes) as u64)
    }

    /// Get doubly indirect block
    fn get_doubly_indirect_block(
        &mut self,
        i: u64,
        doubly_indirect_block_num: u64,
    ) -> Result<u64, Error> {
        let indirect_block_num_i = i / self.blocks_per_block;
        let indirect_block_num =
            self.get_indirect_block(indirect_block_num_i, doubly_indirect_block_num)?;
        let i = i - indirect_block_num_i * self.blocks_per_block;
        self.get_indirect_block(i, indirect_block_num)
    }

    /// Get triply indirect block
    fn get_triply_indirect_block(
        &mut self,
        i: u64,
        triply_indirect_block_num: u64,
    ) -> Result<u64, Error> {
        let doubly_indirect_block_num_i = i / self.blocks_per_block / self.blocks_per_block;
        let doubly_indirect_block_num =
            self.get_indirect_block(doubly_indirect_block_num_i, triply_indirect_block_num)?;
        let i = i - doubly_indirect_block_num_i * self.blocks_per_block * self.blocks_per_block;
        self.get_doubly_indirect_block(i, doubly_indirect_block_num)
    }
}

impl Iterator for ReadBlockNum<'_> {
    // Everything is wrapped in a Result so that we can pass IO errors the caller
    type Item = Result<u64, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.curr;
        if self.curr >= self.data_blocks_count {
            None
        } else {
            self.curr = self.curr + 1;
            if i < self.first_indirect_block {
                Some(self.get_direct_block(i))
            } else if i < self.first_doubly_indirect_block {
                let i = i - self.first_indirect_block;
                let indirect_block_num = self.i_block[EXT2_IND_BLOCK] as u64;
                Some(self.get_indirect_block(i, indirect_block_num))
            } else if i < self.first_triply_indirect_block {
                let i = i - self.first_doubly_indirect_block;
                let doubly_indirect_block_num = self.i_block[EXT2_DOUBLY_IND_BLOCK] as u64;
                Some(self.get_doubly_indirect_block(i, doubly_indirect_block_num))
            } else {
                let i = i - self.first_triply_indirect_block;
                let triply_indirect_block_num = self.i_block[EXT2_TRIPLY_IND_BLOCK] as u64;
                Some(self.get_triply_indirect_block(i, triply_indirect_block_num))
            }
        }
    }
}

pub struct ReadBlock<'a> {
    disk: &'a Box<dyn Disk>,
    block_size: u64,
    blocks: ReadBlockNum<'a>,
}

impl ReadBlock<'_> {
    fn prepare_block_result(&mut self, block_num: u64) -> Option<Result<Vec<u8>, Error>> {
        if block_num == 0 {
            None
        } else {
            Some(self.read_block(block_num))
        }
    }

    fn read_block(&mut self, block_num: u64) -> Result<Vec<u8>, Error> {
        let offset = Offset::new(self.block_size, block_num);
        self.disk.read_at(&offset, self.block_size)
    }
}

impl Iterator for ReadBlock<'_> {
    // Everything is wrapped in a Result so that we can pass IO errors the caller
    type Item = Result<Vec<u8>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.blocks.next() {
            Some(block) => self.prepare_block_result(block.ok()?),
            None => None,
        }
    }
}

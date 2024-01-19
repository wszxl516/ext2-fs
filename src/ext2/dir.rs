use alloc::string::String;
use alloc::vec::Vec;
use core::mem;
use core::str;

use crate::ext2::Ext2Filesystem;
use crate::ext2::inode::Ext2Inode;
use crate::fs::error::Error;
use crate::fs::io::CoreRead;
use crate::fs::stat::Stat;

#[repr(C)]
#[derive(Debug, Default)]
pub struct Ext2DirEntryStruct {
    pub inode_num: u32,
    // Inode number
    pub rec_len: u16,
    // Directory entry length
    pub name_len: u8,
    // Name length
    pub file_type: u8,
    // Type indicator
}

// Directory entry
#[derive(Debug)]
pub struct Ext2DirEntry {
    file_name: String,
    // file name
    inode_num: u64,
    // inode number
    inode: Ext2Inode,
}

impl Ext2DirEntry {
    pub fn new(buffer: &Vec<u8>, offset: usize) -> (Ext2DirEntry, usize) {
        let size = mem::size_of::<Ext2DirEntryStruct>();
        let mut buf = &buffer[offset..offset + size];
        let ext2_dir_entry = buf.read_struct::<Ext2DirEntryStruct>().unwrap();
        let name_slice = &buffer[offset + size..offset + size + ext2_dir_entry.name_len as usize];
        let name = match str::from_utf8(name_slice) {
            Ok(v) => v,
            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
        };
        let dir_entry = Ext2DirEntry {
            file_name: String::from(name),
            inode_num: ext2_dir_entry.inode_num as u64,
            inode: Default::default(),
        };
        (dir_entry, ext2_dir_entry.rec_len as usize)
    }
    pub fn get_inode(&mut self, fs: &Ext2Filesystem) -> Result<(), Error> {
        Ok(self.inode = fs.read_inode(self.inode_num)?)
    }


    /// Returns the bare file name of this directory entry without any other leading path component
    pub fn file_name(&self) -> String {
        return self.file_name.clone();
    }

    /// Returns the inode number
    pub fn inode_num(&self) -> u64 {
        self.inode_num
    }
    pub fn inode(&self) -> Ext2Inode {
        self.inode
    }

    pub fn stat(&self) -> Stat {
        self.inode.metadata()
    }
    pub fn is_dir(&self) -> bool {
        self.inode.metadata().is_dir()
    }
}

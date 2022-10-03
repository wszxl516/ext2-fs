use chrono::prelude::*;
use std::fs::File;
use std::io::prelude::*;
use std::io::Read;
use std::io::SeekFrom;
use std::mem;
use std::slice;
use std::str;

// const FILENAME: &str = "sysgng.dsk";
const FILENAME: &str = "root";
const BLOCK_SIZE: usize = 1024;
const EXT2_ROOT_INO: u32 = 2; /* Root inode */
// Constants relative to the data blocks
const EXT2_NDIR_BLOCKS: usize = 12;
const EXT2_IND_BLOCK: usize = EXT2_NDIR_BLOCKS;
const EXT2_DIND_BLOCK: usize = EXT2_IND_BLOCK + 1;
const EXT2_TIND_BLOCK: usize = EXT2_DIND_BLOCK + 1;
const EXT2_N_BLOCKS: usize = EXT2_TIND_BLOCK + 1;

struct FS {
    file: File,
    block_size: usize,
    super_block: Ext2SuperBlock,
    block_groups: Vec<Ext2GroupDesc>,
}

impl FS {
    fn open(filename: &str) -> FS {
        let file = match File::open(filename) {
            Ok(file) => file,
            Err(why) => panic!("Error opening file: {why}"),
        };
        FS {
            file: file,
            block_size: BLOCK_SIZE,
            super_block: Ext2SuperBlock::default(),
            block_groups: Vec::new(),
        }
    }
    fn offset(&mut self, base_sector_num: u32, delta: u64) -> u64 {
        base_sector_num as u64 * self.block_size as u64 + delta
    }
    fn read(&mut self, size: usize, offset: u64) -> (Vec<u8>, usize) {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.resize(size, 0);
        match self.file.seek(SeekFrom::Start(offset)) {
            Ok(r) => r,
            Err(why) => panic!("Error seeking file: {why}"),
        };
        let nbytes: usize = match self.file.read(&mut buffer) {
            Ok(nbytes) => nbytes,
            Err(why) => panic!("Error reading file: {why}"),
        };
        (buffer, nbytes)
    }
    fn read_sector(&mut self, sector_num: u32) -> (Vec<u8>, usize) {
        let offset = self.offset(sector_num, 0);
        self.read(self.block_size, offset)
    }
    fn read_superblock(&mut self) {
        // Read the Superblock
        self.block_size = BLOCK_SIZE;
        let (buffer, _) = self.read_sector(1);
        let mut buf = buffer.as_slice();
        unsafe {
            let block_slice =
                slice::from_raw_parts_mut(&mut self.super_block as *mut _ as *mut u8, BLOCK_SIZE);
            match buf.read_exact(block_slice) {
                Ok(r) => r,
                Err(why) => panic!("Error reading file: {why}"),
            };
        }
        // Get block size
        self.block_size = self.super_block.s_blocksize();
        // Check ext2 signature
        assert_eq!(0xef53, self.super_block.s_magic);
        self.read_groups()
    }
    fn read_groups(&mut self) {
        // Read the Block Groups
        let group_desc_size = mem::size_of::<Ext2GroupDesc>();
        let size: usize = group_desc_size * self.super_block.s_groups_count();
        // Read from disk
        let offset = self.offset(2, 0);
        let (buffer, _) = self.read(size, offset);
        // Prepare the Ext2GroupDesc instances
        self.block_groups.clear();
        for i in 0..self.super_block.s_groups_count() {
            let mut group = Ext2GroupDesc::default();
            let mut buf = &buffer[group_desc_size * i..group_desc_size * (i + 1)];
            unsafe {
                let group_slice =
                    slice::from_raw_parts_mut(&mut group as *mut _ as *mut u8, group_desc_size);
                buf.read_exact(group_slice).unwrap();
            }
            self.block_groups.push(group);
        }
        // println!("{:#?}", self.block_groups);
    }
    fn get_inode_group(&self, inode: u32) -> Ext2GroupDesc {
        // Determine which block group the inode belongs to and return the group
        self.block_groups[((inode - 1) / self.super_block.s_inodes_per_group) as usize]
    }
    fn read_inode(&mut self, inode: u32) -> Ext2Inode {
        let group = self.get_inode_group(inode);
        let size: usize = self.super_block.s_inode_size as usize;
        let offset = self.offset(group.bg_inode_table, (inode - 1) as u64 * size as u64);
        let (buffer, _) = self.read(size, offset);
        let mut inode = Ext2Inode::default();
        let mut buf = buffer.as_slice();
        unsafe {
            let inode_slice = slice::from_raw_parts_mut(&mut inode as *mut _ as *mut u8, size);
            buf.read_exact(inode_slice).unwrap();
        }
        inode
    }
    fn read_(&mut self, inode: u32) {
        let inode = self.read_inode(inode);
        println!("{inode:#?}");
        let (buffer, _) = self.read_sector(inode.i_block[0]);
        let mut offset = 0;
        while offset < self.block_size {
            let (dir_entry, rec_len) = DirEntry::read(&buffer, offset);
            offset += rec_len;
            println!("{:#?}", dir_entry);
        }
        println!("done");

        // let mut buf = buffer.as_slice();
        // let mut inode: u32;
        // buf.read_exact(&mut inode).unwrap();
        // println!("{res:#?} {s}");
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct Ext2SuperBlock {
    s_inodes_count: u32,      // Total number of inodes in file system
    s_blocks_count: u32,      // Total number of blocks in file system
    s_r_blocks_count: u32,    // Number of blocks reserved for superuser
    s_free_blocks_count: u32, // Total number of unallocated blocks
    s_free_inodes_count: u32, // Total number of unallocated inodes
    s_first_data_block: u32,  // Block number of the block containing the superblock
    s_log_block_size: u32,
    s_log_frag_size: u32,
    s_blocks_per_group: u32, // Number of blocks in each block group
    s_frags_per_group: u32,  // Number of fragments in each block group
    s_inodes_per_group: u32, // Number of inodes in each block group
    s_mtime: u32,            // Last mount time
    s_wtime: u32,            // Last written time
    s_mnt_count: u16, // Number of times the volume has been mounted since its last consistency check
    s_max_mnt_count: u16, // umber of mounts allowed before a consistency check must be done
    s_magic: u16, // Ext2 signature (0xef53), used to help confirm the presence of Ext2 on a volume
    s_state: u16, // File system state
    s_pad: u16,   // What to do when an error is detected
    s_minor_rev_level: u16, // Minor portion of version
    s_lastcheck: u32, // Time of last consistency check
    s_checkinterval: u32, // Interval (in POSIX time) between forced consistency checks
    s_creator_os: u32, // Operating system ID from which the filesystem on this volume was created
    s_rev_level: u32, // Major portion of version
    s_def_resuid: u16, // User ID that can use reserved blocks
    s_def_regid: u16, // Group ID that can use reserved blocks
    s_first_ino: u32,
    s_inode_size: u16,
    s_block_group_nr: u16,
    s_feature_compat: u32,
    s_feature_incompat: u32,
    s_feature_ro_compat: u32,
    s_uuid: [u8; 16],
    s_volume_name: [u8; 16],
    s_last_mounted: [u8; 64],
    s_algorithm_usage_bitmap: u32,
    s_prealloc_blocks: u8,
    s_prealloc_dir_blocks: u8,
    s_padding1: u16,
    // s_reserved: [u32; 204]
}
impl Ext2SuperBlock {
    fn default() -> Ext2SuperBlock {
        let super_block: Ext2SuperBlock = unsafe { mem::zeroed() };
        super_block
    }
    // Number of groups in the fs
    fn s_groups_count(&self) -> usize {
        (self.s_blocks_count as f64 / self.s_blocks_per_group as f64).ceil() as usize
    }
    // Block size
    fn s_blocksize(&self) -> usize {
        (1024 << self.s_log_block_size) as usize
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct Ext2GroupDesc {
    bg_block_bitmap: u32, // The block which contains the block bitmap for the group.
    bg_inode_bitmap: u32, // The block contains the inode bitmap for the group.
    bg_inode_table: u32, // The block contains the inode table first block (the starting block of the inode table.).
    bg_free_blocks_count: u16, // Number of free blocks in the group.
    bg_free_inodes_count: u16, // Number of free inodes in the group.
    bg_used_dirs_count: u16, // Number of inodes allocated to the directories.
    bg_pad: u16,         // Padding (reserved).
    bg_reserved: [u32; 3], // Reserved.
}
impl Ext2GroupDesc {
    fn default() -> Ext2GroupDesc {
        let group: Ext2GroupDesc = unsafe { mem::zeroed() };
        group
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct Ext2Inode {
    i_mode: u16,        /* File mode */
    i_uid: u16,         /* Low 16 bits of Owner Uid */
    i_size: u32,        /* Size in bytes */
    i_atime: u32,       /* Access time */
    i_ctime: u32,       /* Creation time */
    i_mtime: u32,       /* Modification time */
    i_dtime: u32,       /* Deletion Time */
    i_gid: u16,         /* Low 16 bits of Group Id */
    i_links_count: u16, /* Links count */
    i_blocks: u32, /* Blocks count - Count of disk sectors (not Ext2 blocks) in use by this inode */
    i_flags: u32,  /* File flags */
    l_i_reserved1: u32,
    i_block: [u32; EXT2_N_BLOCKS], /* Pointers to blocks (12) +
                                   1 Singly Indirect Block Pointer (Points to a block that is a list of block pointers to data)
                                   1 Doubly Indirect Block Pointer (Points to a block that is a list of block pointers to Singly Indirect Blocks)
                                   1 Triply Indirect Block Pointer (Points to a block that is a list of block pointers to Doubly Indirect Blocks) */
    i_generation: u32, /* File version (for NFS) */
    i_file_acl: u32,   /* File ACL */
    i_dir_acl: u32,    /* Directory ACL */
    i_faddr: u32,      /* Fragment address */
    l_i_frag: u8,      /* Fragment number */
    l_i_fsize: u8,     /* Fragment size */
    i_pad1: u16,
    l_i_uid_high: u16, /* these 2 fields    */
    l_i_gid_high: u16, /* were reserved2[0] */
    l_i_reserved2: u32,
}
impl Ext2Inode {
    fn default() -> Ext2Inode {
        let ionode: Ext2Inode = unsafe { mem::zeroed() };
        ionode
    }
}

struct Inode {
    ext2_inode: Ext2Inode
}

#[repr(C)]
#[derive(Debug)]
struct Ext2DirEntry {
    inode: u32,   /* Inode number */
    rec_len: u16, /* Directory entry length */
    name_len: u8, /* Name length */
    file_type: u8, /* Type indicator (only if the feature bit for "directory entries have file type byte" is set) */
}
impl Ext2DirEntry {
    fn default() -> Ext2DirEntry {
        let dir: Ext2DirEntry = unsafe { mem::zeroed() };
        dir
    }
}

#[derive(Debug)]
struct DirEntry {
    file_name: String,
    inode: u32,
}
impl DirEntry {
    fn read(buffer: &Vec<u8>, offset: usize) -> (DirEntry, usize) {
        let mut ext2_dir_entry = Ext2DirEntry::default();
        let size = mem::size_of::<Ext2DirEntry>();
        unsafe {
            let mut buf = &buffer[offset..offset + size];
            let dir_slice =
                slice::from_raw_parts_mut(&mut ext2_dir_entry as *mut _ as *mut u8, size);
            buf.read_exact(dir_slice).unwrap();
        }
        let name_slice = &buffer[offset + size..offset + size + ext2_dir_entry.name_len as usize];
        let name = match str::from_utf8(name_slice) {
            Ok(v) => v,
            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
        };
        let dir_entry = DirEntry { file_name: String::from(name), inode: ext2_dir_entry.inode };
        (dir_entry, ext2_dir_entry.rec_len as usize)
    }
}

#[allow(dead_code)]
fn format_time(time: u32) -> String {
    let naive = NaiveDateTime::from_timestamp(time.into(), 0);
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn main() {
    // assert_eq!(BLOCK_SIZE, mem::size_of::<Ext2SuperBlock>());

    // // let mut f = File::open(FILENAME);
    // let mut buffer = [0u8; 512];
    //
    // let mut file = match File::open(FILENAME) {
    //     Ok(file) => file,
    //     Err(why) => panic!("Error opening file: {why}"),
    // };
    //
    // let nbytes = match file.read(&mut buffer) {
    //     Ok(nbytes) => nbytes,
    //     Err(why) => panic!("Error reading file: {why}"),
    // };
    // println!("{nbytes}");

    let mut fs = FS::open(FILENAME);
    fs.read_superblock();
    fs.read_groups();

    // let super_block = fs.super_block;
    // println!("Read structure: {:#?}", super_block);

    // println!("s_mtime: {}", format_time(super_block.s_mtime));
    // println!("s_wtime: {}", format_time(super_block.s_wtime));
    // println!("s_lastcheck: {}", format_time(super_block.s_lastcheck));
    // println!("version: {}.{}", super_block.s_rev_level, super_block.s_minor_rev_level);
    // println!("{} {}", super_block.s_blocks_count, super_block.s_blocks_per_group);
    // println!("s_groups_count: {}", super_block.s_groups_count());
    // println!("s_inodes_per_group: {}", super_block.s_inodes_per_group);

    // let inode = fs.read_inode(EXT2_ROOT_INO);
    // println!("{inode:#?}");
    // fs.read_(EXT2_ROOT_INO);
    // fs.read_(12); // bin
    // fs.read_(525); // etc
    fs.read_(13); // bin

    // let group_desc_size = mem::size_of::<Ext2GroupDesc>();
    // println!("{group_desc_size}");

    // let mut buffer = [0; 512];

    // let mut handle = f.take(512);
    // handle.read(&mut buffer);
}

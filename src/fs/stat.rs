#![allow(dead_code)]

use core::fmt::{Display, Formatter};

#[derive(Debug, Default, Copy, Clone)]
pub struct Stat {
    pub dev: u64,
    pub ino: u64,
    pub mode: Mode,
    pub nlink: u64,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u64,
    pub size: u64,
    pub atime: i64,
    pub atime_nsec: i64,
    pub mtime: i64,
    pub mtime_nsec: i64,
    pub ctime: i64,
    pub ctime_nsec: i64,
    pub blksize: u64,
    pub blocks: u64,
    pub flags: FileFlags,
}

impl Stat {
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Tests whether this inode is a directory
    pub fn is_dir(&self) -> bool {
        self.mode().is_dir()
    }

    /// Tests whether this inode is a regular file
    pub fn is_file(&self) -> bool {
        self.mode().is_file()
    }

    /// Tests whether this inode is a symbolic link
    pub fn is_symlink(&self) -> bool {
        self.mode().is_symlink()
    }

    /// Returns the size of the file, in bytes
    pub fn len(&self) -> usize {
        self.size as usize
    }
    fn dev(&self) -> u64 {
        self.dev
    }
    fn ino(&self) -> u64 {
        self.ino
    }
    fn nlink(&self) -> u64 {
        self.nlink
    }
    fn uid(&self) -> u32 {
        self.uid
    }
    fn gid(&self) -> u32 {
        self.gid
    }
    fn rdev(&self) -> u64 {
        self.rdev
    }
    fn size(&self) -> u64 {
        self.size
    }
    fn atime(&self) -> i64 {
        self.atime
    }
    fn atime_nsec(&self) -> i64 {
        self.atime_nsec
    }
    fn mtime(&self) -> i64 {
        self.mtime
    }
    fn mtime_nsec(&self) -> i64 {
        self.mtime_nsec
    }
    fn ctime(&self) -> i64 {
        self.ctime
    }
    fn ctime_nsec(&self) -> i64 {
        self.ctime_nsec
    }
    fn blksize(&self) -> u64 {
        self.blksize
    }
    fn blocks(&self) -> u64 {
        self.blocks
    }
    pub fn flags(&self) -> FileFlags {
        self.flags
    }
}

bitflags::bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct Mode: u16 {
        /// FIFO
        const FIFO = 0x1000;
        /// Character device
        const CHAR_DEVICE = 0x2000;
        /// Directory
        const DIRECTORY = 0x4000;
        /// Block device
        const BLOCK_DEVICE = 0x6000;
        /// Regular file
        const FILE = 0x8000;
        /// Symbolic link
        const SYMLINK = 0xA000;
        /// Unix socket
        const SOCKET = 0xC000;
        /// Other—execute permission
        const O_EXEC = 0x001;
        /// Other—write permission
        const O_WRITE = 0x002;
        /// Other—read permission
        const O_READ = 0x004;
        /// Group—execute permission
        const G_EXEC = 0x008;
        /// Group—write permission
        const G_WRITE = 0x010;
        /// Group—read permission
        const G_READ = 0x020;
        /// User—execute permission
        const U_EXEC = 0x040;
        /// User—write permission
        const U_WRITE = 0x080;
        /// User—read permission
        const U_READ = 0x100;
        /// Sticky Bit
        const STICKY = 0x200;
        /// Set group ID
        const SET_GID = 0x400;
        /// Set user ID
        const SET_UID = 0x800;
    }
}
impl Display for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        const TYPE_NAME: [&'static str; 7] = ["f", "c", "d", "b", "-", "l", "s"];
        const PERM_NAME: [&'static str; 5] = ["-", "r", "w", "", "x"];
        write!(
            f,
            "{} ",
            TYPE_NAME[(self.file_type().bits() / 0x2000) as usize]
        )?;
        let perm = self.perm();
        for i in (0..3).rev() {
            let p = (perm >> (i * 3)) as usize;
            write!(f, "{}{}{}", PERM_NAME[p & 0b001], PERM_NAME[p & 0b010], PERM_NAME[p & 0b100])?
        }
        Ok(())
    }
}

impl Mode {
    pub fn is_dir(&self) -> bool {
        self.contains(Self::DIRECTORY)
    }

    pub fn is_file(&self) -> bool {
        self.contains(Self::FILE)
    }

    pub fn is_symlink(&self) -> bool {
        self.contains(Self::SYMLINK)
    }

    /// Returns true if this mode represents a fifo, also known as a named pipe.
    pub fn is_fifo(&self) -> bool {
        self.contains(Self::FIFO)
    }

    /// Returns true if this mode represents a character device.
    pub fn is_char_device(&self) -> bool {
        self.contains(Self::CHAR_DEVICE)
    }

    /// Returns true if this mode represents a block device.
    pub fn is_block_device(&self) -> bool {
        self.contains(Self::BLOCK_DEVICE)
    }

    /// Returns true if this mode represents a Unix-domain socket.
    pub fn is_socket(&self) -> bool {
        self.contains(Self::SOCKET)
    }
    pub fn file_type(&self) -> Mode {
        Mode::from_bits_truncate(self.bits() & 0xf000)
    }
    pub fn perm(&self) -> u32 {
        self.bits() as u32 & 0x1ff
    }
}

bitflags::bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct FileFlags: u32 {
        /// Secure deletion (not used)
        const SECURE_DEL = 0x00000001;
        /// Keep a copy of data when deleted (not used)
        const KEEP_COPY = 0x00000002;
        /// File compression (not used)
        const COMPRESSION = 0x00000004;
        /// Synchronous updates—new data is written immediately to disk
        const SYNC_UPDATE = 0x00000008;
        /// Immutable file (content cannot be changed)
        const IMMUTABLE = 0x00000010;
        /// Append only
        const APPEND_ONLY = 0x00000020;
        /// File is not included in 'dump' command
        const NODUMP = 0x00000040;
        /// Last accessed time should not updated
        const DONT_ATIME = 0x00000080;
        /// Hash indexed directory
        const HASH_DIR = 0x00010000;
        /// AFS directory
        const AFS_DIR = 0x00020000;
        /// Journal file data
        const JOURNAL_DATA = 0x00040000;
    }
}

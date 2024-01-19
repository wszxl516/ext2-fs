use std::cell::UnsafeCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

use ext2;
use ext2::fs;
use ext2::fs::disk::{Disk, Offset};
use ext2::fs::error::Error;

fn main() {
    let disk = FileDisk::open("/data/works/ext2-fs/hd.img").unwrap();
    let mut fs = fs::mount(Box::new(disk)).unwrap();

    match fs.mk_dir("/test", 0o755) {
        Ok(_) => {}
        Err(e) => println!("{:?}", e)
    }
    match fs.new_file("/test/test.txt", 0o755) {
        Ok(mut fd) => {
            for i in 'a'..'l' {
                let buf = vec![i as u8; 1024];
                fd.write(buf.as_slice()).unwrap();
            }
        }
        Err(e) => println!("{:?}", e)
    }
    println!("list /");
    let dir = fs.read_dir("/").unwrap();
    for (name, d) in dir {
        println!("{} {} {:?} {}", d.stat().mode(), name, d.inode_num(), d.stat().size);
    }
    println!("list /test");
    let dir = fs.read_dir("/test").unwrap();
    for (name, d) in dir {
        println!("{} {} {:?} {}", d.stat().mode(), name, d.inode_num(), d.stat().size);
    }
}


pub struct FileDisk {
    file: UnsafeCell<File>,
}

impl FileDisk {
    pub fn open(filename: &str) -> Result<Self, Error> {
        match File::options().write(true).read(true).open(filename) {
            Ok(file) => Ok(Self { file: file.into() }),
            Err(_) => Err(Error::IOError("disk open failed!".to_string())),
        }
    }
}

impl Disk for FileDisk {
    fn read(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        unsafe {
            match (*self.file.get()).read(buffer) {
                Ok(n) => Ok(n),
                Err(_) => Err(Error::IOError("FileDisk read failed".to_string())),
            }
        }
    }

    fn write(&self, buffer: &[u8]) -> Result<usize, Error> {
        unsafe {
            match (*self.file.get()).write(buffer) {
                Ok(n) => Ok(n),
                Err(e) => {
                    println!("{:?}", e);
                    Err(Error::IOError("FileDisk read failed".to_string()))
                }
            }
        }
    }

    fn read_at(&self, offset: &Offset, size: u64) -> Result<Vec<u8>, Error> {
        let offset: u64 = offset.value();
        let mut buffer: Vec<u8> = Vec::new();
        buffer.resize(size as usize, 0);
        match self.seek(offset) {
            Ok(_) => {}
            Err(_) => return Err(Error::IOError("seek disk failed!".to_string())),
        };
        let n_bytes: usize = match self.read(&mut buffer) {
            Ok(n) => n,
            Err(_) => return Err(Error::IOError("read disk failed!".to_string())),
        };
        if n_bytes != size as usize {
            Err(Error::UnexpectedEof("Not enough bytes".to_string()))
        } else {
            Ok(buffer)
        }
    }

    fn write_at(&self, offset: &Offset, buffer: &[u8]) -> Result<usize, Error> {
        let offset: u64 = offset.value();
        match self.seek(offset) {
            Ok(_) => {}
            Err(e) => return Err(e),
        };
        match self.write(&buffer) {
            Ok(n) => Ok(n),
            Err(_) => Err(Error::IOError("write disk failed!".to_string())),
        }
    }

    fn seek(&self, offset: u64) -> Result<(), Error> {
        unsafe {
            match (*self.file.get()).seek(SeekFrom::Start(offset)) {
                Ok(_) => Ok(()),
                Err(_) => Err(Error::IOError("seek disk failed".to_string())),
            }
        }
    }
}
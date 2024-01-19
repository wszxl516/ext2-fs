use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::ext2::Ext2Filesystem;
use crate::fs::disk::Disk;
use crate::fs::error::Error;

pub mod disk;
pub mod error;
pub mod file;
pub mod io;
pub mod stat;

pub fn mount(disk: Box<(dyn Disk + 'static)>) -> Result<Ext2Filesystem, Error> {
    Ok(Ext2Filesystem::mount(disk)?)
}

pub fn base_dir(path: &str) -> String {
    let mut path_vector = path.split("/").collect::<Vec<&str>>();
    path_vector.pop();
    let path = path_vector.join("/");
    match path.is_empty() {
        true => "/".to_string(),
        false => path,
    }
}

pub fn base_file(path: &str) -> String {
    let mut path_vector = path.split("/").collect::<Vec<&str>>();
    path_vector.pop().unwrap().to_string()
}
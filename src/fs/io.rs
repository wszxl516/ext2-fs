use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::fs::error::Error;

#[macro_export]
macro_rules! to_slice {
    ($name: expr, $input_type: tt) => {
        unsafe { core::slice::from_raw_parts($name as *const $input_type as *const u8, core::mem::size_of::<$input_type>())}
    };
}
pub trait CoreRead {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;
    #[inline]
    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<(), Error> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => buf = &mut buf[n..],
                Err(e) => return Err(e),
            }
        }
        if buf.is_empty() {
            Ok(())
        } else {
            Err(Error::UnexpectedEof("".to_string()))
        }
    }
    fn read_struct<T: Sized>(&mut self) -> Result<T, Error> {
        let mut buf = vec![0u8; core::mem::size_of::<T>()];
        self.read_exact(buf.as_mut_slice())?;
        unsafe { Ok((buf.as_ptr() as *const T).read()) }
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, Error>;
    fn read_to_string(&mut self, buf: &mut String) -> Result<usize, Error> {
        unsafe { self.read_to_end(buf.as_mut_vec()) }
    }
}

pub trait CoreWrite {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error>;

    fn write_struct<T: Sized>(&mut self, buf: &T) -> Result<usize, Error> {
        let buf = to_slice!(buf, T);
        self.write(buf)
    }
    fn write_string(&mut self, buf: &String) -> Result<usize, Error> {
        self.write(buf.as_bytes())
    }
}

impl CoreRead for &[u8] {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let amt = core::cmp::min(buf.len(), self.len());
        let (a, b) = self.split_at(amt);
        if amt == 1 {
            buf[0] = a[0];
        } else {
            buf[..amt].copy_from_slice(a);
        }

        *self = b;
        Ok(amt)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() > self.len() {
            return Err(Error::UnexpectedEof("failed to fill whole buffer".to_string()));
        }
        let (a, b) = self.split_at(buf.len());

        if buf.len() == 1 {
            buf[0] = a[0];
        } else {
            buf.copy_from_slice(a);
        }

        *self = b;
        Ok(())
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, Error> {
        buf.extend_from_slice(*self);
        let len = self.len();
        *self = &self[len..];
        Ok(len)
    }
}

impl CoreWrite for &[u8] {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        if buf.len() > self.len() {
            return Err(Error::UnexpectedEof("failed to fill whole buffer".to_string()));
        }
        unsafe {
            core::slice::from_raw_parts_mut(self.as_ptr() as *mut u8, buf.len())
        }.copy_from_slice(buf);
        *self = &self[buf.len()..];
        Ok(buf.len())
    }
}
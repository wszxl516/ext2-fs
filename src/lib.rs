#![feature(slice_pattern)]
#![feature(stmt_expr_attributes)]
#![feature(slice_first_last_chunk)]
#![no_std]
extern crate alloc;
extern crate core;

pub mod ext2;
pub mod fs;

#[macro_export]
macro_rules! int_get {
    ($slice: ident,$value_type: ty) => {{
        let b: (&[u8; core::mem::size_of::<$value_type>()], &[u8]) =
            $slice.split_first_chunk().unwrap();
        #[allow(unused_assignments)]
        $slice = b.1;
        <$value_type>::from_le_bytes(*b.0)
    }};
}

#[macro_export]
macro_rules! align_up {
    ($len:expr, $size:expr) => {
        (($len as u64) + ($size as u64) - 1) & !(($size as u64) - 1)
    };
}
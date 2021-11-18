
use crate::{include_bytes_align_as};
use include_dir::{InitDir, include_dir};

// TODO: proper init directory management, we won't use only initproc
//pub static INIT_DATA: &'static [u8] = include_bytes_align_as!(u64, r"../../../initproc/target/x86_64-smoke/debug/initproc");

pub static INIT_DIR: InitDir = include_dir!("${CARGO_MANIFEST_DIR}/../initdir");

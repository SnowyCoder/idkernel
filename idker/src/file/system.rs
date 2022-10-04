use core::iter::FusedIterator;

use alloc::{boxed::Box, sync::Arc};
use itertools::Itertools;

/// We view a folder-like structure as a tree where the leafs are files and everything else is a
/// folder, and here everything's ok. The "strange" thing is that folders are also readable
/// and when you read them you get back a list of files.
/// A PathHandle then is just a "pointer" to any node in the tree, it might point to a file or to a folder
///

pub enum FileHandleError {
    NotSeekable,
    SeekOutOfRange,
}

pub enum PathOpenError {
    InvalidPath,
    FolderNotFound,
    PathIsFile,
    ParentingNotSupported,// a/.. not supported, canonicalize your path first please
}

pub enum PartialPathResult<'a, H> {
    Done(H),
    Partial(H, &'a str)
}

pub enum PathType {
    Folder,
    File,
    // others? links?
}

pub trait PathHandle : Send + Sync {
    fn open_dir<'a>(&self, path: &'a str) -> Result<PartialPathResult<'a, Arc<dyn PathHandle>>, PathOpenError>;

    fn ptype(&self) -> PathType;

    fn read(&self) -> Box<dyn FileHandle>;
}

pub fn open_path_full(x: Arc<dyn PathHandle>, mut path: &str) -> Result<Arc<dyn PathHandle>, PathOpenError> {
    let mut curr = x;
    loop {
        match curr.open_dir(path)? {
            PartialPathResult::Done(x) => return Ok(x),
            PartialPathResult::Partial(h, x) => {
                curr = h;
                path = x;
            }
        }
    }
}


pub trait FileHandle : Send + Sync {
    fn seek(&mut self, at: usize) -> Result<(), FileHandleError>;

    fn read(&mut self, at: &mut [u8]) -> usize;
}

pub trait PathNavigator {
    fn path_step<'a>(&'a self) -> (PathNavigatorStep<'a>, &'a str);
}

#[derive(Clone, Copy, Debug)]
pub enum PathNavigatorStep<'a> {
    Current,
    Parent,
    RelCh(&'a str),
}

impl<'a> PathNavigator for str {
    fn path_step(&self) -> (PathNavigatorStep, &str) {
        let (curr, rem) = match self.split_once('/') {
            Some((a, b)) => (a, b),
            None => (self, ""),
        };
        let step = match curr {
            "" | "." => PathNavigatorStep::Current,
            ".." => PathNavigatorStep::Parent,
            x => PathNavigatorStep::RelCh(x),
        };
        (step, rem)
    }
}



pub fn dir_names_to_handle(x: impl Iterator<Item = &'static str> + 'static + Send + Sync) -> Box<dyn FileHandle> {
    const SEPAR_SLICE: &'static[&'static[u8]] = &[b"\n".as_slice()];

    let iter = x.map(|x| x.as_bytes())
            .interleave_shortest(SEPAR_SLICE.iter().map(|x| *x).cycle())
            .flatten()
            .cloned()
            .fuse();
    Box::new(IteratorFileHandle(iter))
}

pub struct IteratorFileHandle<T: Iterator<Item = u8> + FusedIterator + Send>(T);


impl<T> FileHandle for IteratorFileHandle<T>
        where T : Iterator<Item = u8> + FusedIterator + Send + Sync {
    fn seek(&mut self, _at: usize) -> Result<(), FileHandleError> {
        Err(FileHandleError::NotSeekable)
    }

    fn read(&mut self, at: &mut [u8]) -> usize {
        let mut i = 0;
        if let Some(x) = self.0.next() {
            at[i] = x;
            i += 1;
        }
        i
    }
}

use core::cmp::min;

use alloc::{boxed::Box, string::String, sync::Arc};
use include_dir::{InitDir, InitDirEntry};

use crate::context::init::INIT_DIR;

use super::system::{FileHandle, FileHandleError, PartialPathResult, PathHandle, PathNavigator, PathNavigatorStep, PathOpenError, PathType, dir_names_to_handle};

#[derive(Clone)]
pub struct InitFsFolderHandle {
    path: String,
    folder: InitDir,
    file: Option<(&'static str, &'static [u8])>,
}

impl InitFsFolderHandle {
    pub fn from_init_dir(mount_path: String) -> Self {
        InitFsFolderHandle {
            path: mount_path,
            folder: INIT_DIR,
            file: None,
        }
    }

    fn rel_open(self, x: &str) -> Result<Self, PathOpenError> {
        if self.file.is_none() {
            return Err(PathOpenError::PathIsFile);
        }

        let entry = &self.folder.0.iter()
                .find(|y| y.0 == x)
                .ok_or(PathOpenError::FolderNotFound)?;

        let (folder, file) = match entry.1 {
            InitDirEntry::File(x) => (self.folder, Some((entry.0, x))),
            InitDirEntry::Folder(x) => (x, None),
        };

        Ok(InitFsFolderHandle {
            path: self.path + x,
            folder,
            file,
        })
    }
}

impl PathHandle for InitFsFolderHandle {
    fn open_dir<'a>(&self, path: &'a str) -> Result<PartialPathResult<'a, Arc<dyn PathHandle>>, PathOpenError> {
        let mut remaining = path;
        let mut folder = self.clone();

        while !remaining.is_empty() {
            let (step, rem2) = remaining.path_step();
            remaining = rem2;
            match step {
                PathNavigatorStep::Current => {},
                PathNavigatorStep::Parent => return Err(PathOpenError::ParentingNotSupported),
                PathNavigatorStep::RelCh(rel) => {
                    folder = folder.rel_open(rel)?;
                }
            }
        }
        Ok(PartialPathResult::Done(Arc::new(folder)))
    }

    fn ptype(&self) -> PathType {
        match &self.file {
            Some(_) => PathType::File,
            None => PathType::Folder,
        }
    }

    fn read(&self) -> Box<dyn FileHandle> {
        match self.file {
            Some(file) => {
                Box::new(InitFsFileHandle {
                    //path: self.path.clone() + file.0,
                    file: file.1,
                    index: 0,
                })
            },
            None => dir_names_to_handle(
                self.folder.0.iter().map(|x| x.0)
            ),
        }
    }
}

pub struct InitFsFileHandle {
    //path: String,
    file: &'static [u8],
    index: usize,
}

impl FileHandle for InitFsFileHandle {
    fn seek(&mut self, at: usize) -> Result<(), FileHandleError> {
        if at > self.file.len() {
            Err(FileHandleError::SeekOutOfRange)
        } else {
            self.index = at;
            Ok(())
        }
    }

    fn read(&mut self, at: &mut [u8]) -> usize {
        let len = min(at.len(), self.file.len() - self.index);
        at[..len].copy_from_slice(&self.file[self.index..len]);
        len
    }
}

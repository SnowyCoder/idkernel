use alloc::{boxed::Box, sync::Arc, vec::Vec};
use spin::RwLock;

use crate::context::TaskId;

use super::system::{FileHandle, PartialPathResult, PathHandle, PathNavigator, PathNavigatorStep, PathOpenError, PathType, dir_names_to_handle};

pub struct ProcRootFs {
    proc: TaskId,
    mounts: Vec<(&'static str, Arc<dyn PathHandle>)>,
}

impl ProcRootFs {
    pub fn new(task: TaskId) -> ProcRootFs {
        ProcRootFs {
            proc: task,
            mounts: Vec::new(),
        }
    }

    pub fn mount(&mut self, name: &'static str, path_handle: Arc<dyn PathHandle>) {
        self.mounts.push((name, path_handle));
    }

    pub fn unmount(&mut self, name: &'static str) -> Option<Arc<dyn PathHandle>> {
        self.mounts.iter()
                .position(|x| x.0 == name)
                .map(|i| self.mounts.remove(i).1)
    }
}

#[derive(Clone)]
pub struct ProcRootPathHandle(pub Arc<RwLock<ProcRootFs>>);

impl PathHandle for ProcRootPathHandle {
    fn open_dir<'a>(&self, path: &'a str) -> Result<PartialPathResult<'a, Arc<dyn PathHandle>>, PathOpenError> {
        let mut remaining = path;
        while !remaining.is_empty() {
            let (step, rem) = remaining.path_step();
            match step {
                PathNavigatorStep::Current => {},// Next step
                PathNavigatorStep::Parent => return Err(PathOpenError::ParentingNotSupported),
                PathNavigatorStep::RelCh(rel) => {
                    let lock_guard = self.0.read();
                    let ch = lock_guard.mounts.iter().find(|x| x.0 == rel);
                    return match ch {
                        Some(x) => Ok(PartialPathResult::Partial(x.1.clone(), rem)),
                        None => Err(PathOpenError::InvalidPath),
                    }
                }
            }
            remaining = rem;
        }
        Ok(PartialPathResult::Done(Arc::new(self.clone())))
    }

    fn ptype(&self) -> PathType {
        PathType::Folder
    }

    fn read(&self) -> Box<dyn FileHandle> {
        let names: Vec<_> = self.0.read().mounts.iter().map(|x| x.0).collect();
        dir_names_to_handle(names.into_iter())
    }
}
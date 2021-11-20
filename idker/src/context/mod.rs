pub mod task;
pub mod elf;
pub mod init;
pub mod page_table;
pub mod registry;
pub mod switch;

use core::{cell::Cell, num::NonZeroU64, sync::atomic::{AtomicU64, Ordering}};

use alloc::sync::Arc;
use spin::{Once, RwLock, RwLockReadGuard, RwLockWriteGuard};
pub use task::{TaskId, TaskContext};
pub use page_table::UserPageTable;

use crate::{context::task::TaskState};

use self::{registry::TaskRegistry, switch::switch_task};

static TASKS: Once<RwLock<TaskRegistry>> = Once::new();

#[thread_local]
static TASK_ID: AtomicU64 = AtomicU64::new(0);

#[thread_local]
static TASK_SWITCH_LOCKS: Cell<Option<[Arc<RwLock<TaskContext>>; 2]>> = Cell::new(None);

fn init_tasks() -> RwLock<TaskRegistry> {
    RwLock::new(TaskRegistry::new())
}

pub fn tasks() -> RwLockReadGuard<'static, TaskRegistry> {
    TASKS.call_once(init_tasks).read()
}

pub fn tasks_mut() -> RwLockWriteGuard<'static, TaskRegistry> {
    TASKS.call_once(init_tasks).write()
}

pub fn current_task_id() -> TaskId {
    let id = NonZeroU64::new(TASK_ID.load(Ordering::SeqCst));
    match id {
        Some(x) => TaskId(x),
        None => panic!("Task ID not sed yet"),
    }
}

pub fn current_task() -> Arc<RwLock<TaskContext>> {
    tasks().get(current_task_id())
        .expect("Current task not in registry")
        .clone()
}

pub fn switch_to_next_task() -> bool {
    let mut tasks = tasks_mut();

    let from = current_task_id();

    let fctx = tasks.get(from).unwrap().clone();
    let from_task = fctx.write();

    if from_task.state == TaskState::User {
        tasks.executable_tasks.push_back(from);
    }

    let next = tasks.executable_tasks.pop_front();

    // task 1 will wait for interrupts
    let next = next.unwrap_or_else(|| TaskId(NonZeroU64::new(1).unwrap()));
    if next == from {
        return false;
    }

    let tctx = tasks.get(next).unwrap().clone();
    drop(tasks);
    let flock = RwLockWriteGuard::leak(from_task) as *mut TaskContext;
    let tlock = RwLockWriteGuard::leak(tctx.write()) as *mut TaskContext;

    set_current_task_id(next);
    TASK_SWITCH_LOCKS.set(Some([fctx, tctx]));
    unsafe {
        // Here we should hold no locks except for the from and to tasks (that we forgot)
        switch_task(&(& *flock).arch_regs, &(& *tlock).arch_regs);
    }
    return true;
}

pub fn set_current_task_id(id: TaskId) {
    TASK_ID.store(id.0.get(), Ordering::SeqCst)
}

extern "C" fn after_task_switch() {
    // Called after a switch, we need to unlock the task contexts
    let arcs = TASK_SWITCH_LOCKS.take();
    unsafe {
        let [farc, tarc] = arcs.unwrap_unchecked();

        // Safety: well, right now the lock is "forgot" but we hold it
        let farc2 = &mut *farc.as_mut_ptr();
        if farc2.state == TaskState::Dying {
            tasks_mut().remove(farc2.id);
        }

        farc.force_write_unlock();
        tarc.force_write_unlock();
    };
}

use alloc::{collections::{BTreeMap, VecDeque}, sync::Arc};
use spin::RwLock;

use super::{TaskContext, TaskId};

pub struct TaskRegistry {
    tasks: BTreeMap<TaskId, Arc<RwLock<TaskContext>>>,
    pub executable_tasks: VecDeque<TaskId>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        TaskRegistry {
            tasks: BTreeMap::new(),
            executable_tasks: VecDeque::new(),
        }
    }

    pub fn add(&mut self, ctx: TaskContext) {
        let id = ctx.id;
        let parent = ctx.parent;
        let wrapped = Arc::new(RwLock::new(ctx));
        if self.tasks.insert(id, wrapped).is_some() {
            panic!("Task with same ID already present");
        }
        if let Some(p) = parent {
            let mut parent = self.tasks.get(&p).unwrap().write();
            parent.children.push(id);
        }
    }

    pub fn get(&self, id: TaskId) -> Option<&Arc<RwLock<TaskContext>>> {
        self.tasks.get(&id)
    }

    pub fn remove(&mut self, id: TaskId) -> Option<Arc<RwLock<TaskContext>>> {
        let task = self.tasks.remove(&id);
        if let Some(t) = &task {
            let task = t.read();

            if let Some(parent) = task.parent.and_then(|id| self.tasks.get(&id)) {
                let mut p = parent.write();
                if let Some(index) = p.children.iter().position(|x| *x == task.id) {
                    p.children.remove(index);
                }
            }
        }
        task
    }

    pub fn queue_for_execution(&mut self, id: TaskId) {
        self.executable_tasks.push_back(id);
    }
}
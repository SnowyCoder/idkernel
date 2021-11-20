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
        let wrapped = Arc::new(RwLock::new(ctx));
        if self.tasks.insert(id, wrapped).is_some() {
            panic!("Task with same ID already present");
        }
        if id.0.get() as u64 != 1 {
            self.executable_tasks.push_back(id);
        }
    }

    pub fn get(&self, id: TaskId) -> Option<&Arc<RwLock<TaskContext>>> {
        self.tasks.get(&id)
    }

    pub fn remove(&mut self, id: TaskId) -> Option<Arc<RwLock<TaskContext>>> {
        self.tasks.remove(&id)
    }
}
use alloc::collections::BTreeMap;
use crate::task::{TaskId, Task};
use crossbeam_queue::ArrayQueue;
use alloc::sync::Arc;
use futures_util::task::Waker;
use core::task::{Context, Poll};
use alloc::task::Wake;

struct ExecutorTaskData {
    task: Task,
    waker_cache: Option<Waker>,
}

pub struct Executor {
    tasks: BTreeMap<TaskId, ExecutorTaskData>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    spawner: Spawner,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(128)),
            spawner: Spawner::new(32),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;

        let task_data = ExecutorTaskData {
            task,
            waker_cache: None,
        };

        if self.tasks.insert(task_id, task_data).is_some() {
            panic!("Task with same ID already present");
        }
        self.task_queue.push(task_id).expect("task queue full");
    }

    fn run_ready_tasks(&mut self) {
        while let Some(task) = self.spawner.queue.pop() {
            self.spawn(task);
        }

        // Destructure to avoid closures capture of "self" (https://github.com/rust-lang/rust/issues/53488)
        let Self {
            tasks,
            task_queue,
            ..
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task_data = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // task no longer exists
            };
            let waker = task_data.waker_cache
                .get_or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));

            let mut context = Context::from_waker(waker);
            match task_data.task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    tasks.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

    pub fn spawner(&self) -> &Spawner {
        return &self.spawner;
    }

    pub fn run_sync(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts;

        interrupts::disable();
        if self.task_queue.is_empty() {
            interrupts::enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id, task_queue
        }))
    }

    fn wake_task(&self) {
        self.task_queue.push(self.task_id)
            .expect("Failed to wake task, task_queue full");
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}

#[derive(Clone)]
pub struct Spawner {
    queue: Arc<ArrayQueue<Task>>
}

impl Spawner {
    fn new(queue_cap: usize) -> Self {
        Spawner {
            queue: Arc::new(ArrayQueue::new(queue_cap))
        }
    }

    pub fn spawn<T: Into<Task>>(&self, task: T) {
        self.queue.push(task.into())
            .unwrap_or_else(|_task| panic!("Task spawn queue is full"));
    }
}

pub fn batches<T: Clone>(items: &[T], size: usize) -> Vec<Vec<T>> {
    items.chunks(size).map(|chunk| chunk.to_vec()).collect()
}

pub struct Task {
    pub title: String,
    pub task: TaskFn,
}

pub type TaskFn = std::sync::Arc<
    dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync,
>;

pub fn batched_tasks<T: Clone>(
    items: &[T],
    batch_size: usize,
    f: impl Fn(&[T], usize) -> Task + Send + Sync + 'static,
) -> Vec<Task> {
    let mut tasks = Vec::new();
    let f = std::sync::Arc::new(f);
    for (i, chunk) in items.chunks(batch_size).enumerate() {
        let batch = chunk.to_vec();
        let index = i * batch_size;
        let task = f(&batch, index);
        tasks.push(Task {
            title: task.title,
            task: std::sync::Arc::new(move || {
                let task_fn = task.task.clone();
                Box::pin(async move {
                    (task_fn)().await;
                })
            }),
        });
    }
    tasks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_splits_items_by_size() {
        let items: Vec<i32> = (0..10).collect();
        let result = batches(&items, 3);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], vec![0, 1, 2]);
        assert_eq!(result[1], vec![3, 4, 5]);
        assert_eq!(result[2], vec![6, 7, 8]);
        assert_eq!(result[3], vec![9]);
    }

    #[test]
    fn batches_returns_single_batch_when_items_fit() {
        let items = vec!["a", "b"];
        let result = batches(&items, 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec!["a", "b"]);
    }

    #[tokio::test]
    async fn batched_tasks_creates_correct_number_of_tasks() {
        let items: Vec<i32> = (0..7).collect();
        let tasks = batched_tasks(&items, 3, |batch, _| Task {
            title: format!("batch of {}", batch.len()),
            task: std::sync::Arc::new(|| Box::pin(async {})),
        });
        assert_eq!(tasks.len(), 3);
    }

    #[tokio::test]
    async fn batched_tasks_executes_all_batches() {
        let items: Vec<i32> = (0..5).collect();
        let counter = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let tasks = batched_tasks(&items, 2, {
            let counter = counter.clone();
            move |batch, _| {
                let mut c = counter.lock().unwrap();
                *c += batch.len();
                Task {
                    title: String::new(),
                    task: std::sync::Arc::new(|| Box::pin(async {})),
                }
            }
        });
        for task in tasks {
            (task.task)().await;
        }
        assert_eq!(*counter.lock().unwrap(), 5);
    }
}

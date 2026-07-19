#[derive(Debug, Clone, Default, PartialEq)]
pub enum TaskStatus {
    #[default]
    Pending,
    Running,
    Success,
    Failed(String),
    Skipped,
}

#[derive(Debug, Clone, Default)]
pub struct Task {
    pub title: String,
    pub status: TaskStatus,
    pub subtasks: Vec<Task>,
    pub retry: Option<u32>,
    pub retry_count: u32,
    pub errors: Vec<String>,
}

impl Task {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: TaskStatus::Pending,
            subtasks: Vec::new(),
            retry: None,
            retry_count: 0,
            errors: Vec::new(),
        }
    }
}

pub enum OutputMode {
    Auto,
    Silent,
    Verbose,
}

#[allow(clippy::too_many_arguments)]
pub fn format_task_tree(
    tasks: &[Task],
    indent: usize,
    show_status: bool,
    completed_icon: &str,
    failed_icon: &str,
    running_icon: &str,
    pending_icon: &str,
    skipped_icon: &str,
) -> String {
    let mut out = String::new();
    for task in tasks {
        let prefix = "  ".repeat(indent);
        let icon = match task.status {
            TaskStatus::Success => completed_icon,
            TaskStatus::Failed(_) => failed_icon,
            TaskStatus::Running => running_icon,
            TaskStatus::Pending => pending_icon,
            TaskStatus::Skipped => skipped_icon,
        };
        let status_str = if show_status {
            match &task.status {
                TaskStatus::Failed(msg) => format!(" {icon} {}: {msg}", task.title),
                _ => format!(" {icon} {}", task.title),
            }
        } else {
            task.title.clone()
        };
        out.push_str(&format!("{prefix}{status_str}\n"));

        if !task.subtasks.is_empty() {
            out.push_str(&format_task_tree(
                &task.subtasks,
                indent + 1,
                show_status,
                completed_icon,
                failed_icon,
                running_icon,
                pending_icon,
                skipped_icon,
            ));
        }
    }
    out
}

pub fn format_task_status(tasks: &[Task], output_mode: OutputMode) -> String {
    let (completed, failed, running, pending, skipped) = match output_mode {
        OutputMode::Silent => ("", "", "", "", ""),
        OutputMode::Auto | OutputMode::Verbose => {
            ("✓", "✗", "●", "○", "—")
        }
    };
    format_task_tree(tasks, 0, true, completed, failed, running, pending, skipped)
}

pub fn run_tasks<F>(tasks: &mut [Task], silent: bool, mut task_fn: F)
where
    F: FnMut(&mut Task),
{
    for task in tasks.iter_mut() {
        task.status = TaskStatus::Running;

        if !silent {
            eprintln!(" ● {}", task.title);
        }

        let max_retries = task.retry.map(|r| r + 1).unwrap_or(1);
        for attempt in 0..max_retries {
            if attempt > 0 {
                task.retry_count = attempt;
                if !silent {
                    eprintln!("   retry {attempt}/{max_retries}");
                }
            }

            task_fn(task);

            match &task.status {
                TaskStatus::Failed(msg) => {
                    task.errors.push(msg.clone());
                    if attempt < max_retries - 1 {
                        continue;
                    }
                    if !silent {
                        eprintln!(" ✗ {}", task.title);
                    }
                }
                _ => {
                    if !silent {
                        eprintln!(" ✓ {}", task.title);
                    }
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_new() {
        let task = Task::new("build");
        assert_eq!(task.title, "build");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.subtasks.is_empty());
    }

    #[test]
    fn test_task_status_equality() {
        assert_eq!(TaskStatus::Pending, TaskStatus::Pending);
        assert_eq!(TaskStatus::Running, TaskStatus::Running);
        assert_eq!(TaskStatus::Success, TaskStatus::Success);
        assert_eq!(TaskStatus::Skipped, TaskStatus::Skipped);
        assert!(TaskStatus::Failed("a".into()) == TaskStatus::Failed("a".into()));
        assert!(TaskStatus::Failed("a".into()) != TaskStatus::Failed("b".into()));
    }

    #[test]
    fn test_format_task_tree_basic() {
        let tasks = vec![
            Task {
                title: "install".into(),
                status: TaskStatus::Success,
                ..Default::default()
            },
        ];
        let result = format_task_tree(&tasks, 0, true, "✓", "✗", "●", "○", "—");
        assert!(result.contains("✓"));
        assert!(result.contains("install"));
    }

    #[test]
    fn test_format_task_tree_with_subtasks() {
        let tasks = vec![Task {
            title: "parent".into(),
            status: TaskStatus::Running,
            subtasks: vec![Task {
                title: "child".into(),
                status: TaskStatus::Pending,
                ..Default::default()
            }],
            ..Default::default()
        }];
        let result = format_task_tree(&tasks, 0, true, "✓", "✗", "●", "○", "—");
        assert!(result.contains("parent"));
        assert!(result.contains("child"));
    }

    #[test]
    fn test_format_task_status_silent() {
        let tasks = vec![Task::new("x")];
        let result = format_task_status(&tasks, OutputMode::Silent);
        assert!(!result.contains('✓'));
    }

    #[test]
    fn test_format_task_tree_indented() {
        let tasks = vec![Task::new("nested")];
        let result = format_task_tree(&tasks, 2, false, "", "", "", "", "");
        assert!(result.starts_with("    "));
    }

    #[test]
    fn test_run_tasks_all_succeed() {
        let mut tasks = vec![Task::new("task1"), Task::new("task2")];
        run_tasks(&mut tasks, true, |t| {
            t.status = TaskStatus::Success;
        });
        assert_eq!(tasks[0].status, TaskStatus::Success);
        assert_eq!(tasks[1].status, TaskStatus::Success);
    }

    #[test]
    fn test_run_tasks_failed() {
        let mut tasks = vec![Task::new("fails")];
        run_tasks(&mut tasks, true, |t| {
            t.status = TaskStatus::Failed("error".into());
        });
        assert_eq!(tasks[0].status, TaskStatus::Failed("error".into()));
        assert_eq!(tasks[0].errors.len(), 1);
    }

    #[test]
    fn test_task_with_retry() {
        let mut tasks = vec![Task {
            title: "retry".into(),
            retry: Some(2),
            ..Default::default()
        }];
        let mut call_count = 0u32;
        run_tasks(&mut tasks, true, |t| {
            call_count += 1;
            if call_count < 3 {
                t.status = TaskStatus::Failed("retry".into());
            } else {
                t.status = TaskStatus::Success;
            }
        });
        assert_eq!(tasks[0].status, TaskStatus::Success);
        assert_eq!(call_count, 3);
    }
}

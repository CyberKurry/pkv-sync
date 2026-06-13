use std::future::Future;
use std::time::Duration;
use tokio::task::{JoinError, JoinHandle};

pub(crate) fn spawn_supervised<F, Fut>(
    name: &'static str,
    restart_delay: Duration,
    mut task_factory: F,
) -> JoinHandle<()>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            match run_worker(tokio::spawn(task_factory())).await {
                Ok(()) => {
                    tracing::warn!(task = name, "background task exited; restarting");
                }
                Err(err) if err.is_panic() => {
                    tracing::error!(task = name, error = %err, "background task panicked; restarting");
                }
                Err(err) => {
                    tracing::warn!(task = name, error = %err, "background task stopped; restarting");
                }
            }
            tokio::time::sleep(restart_delay).await;
        }
    })
}

async fn run_worker(handle: JoinHandle<()>) -> Result<(), JoinError> {
    let mut handle = AbortOnDrop(handle);
    (&mut handle.0).await
}

struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::Notify;

    #[tokio::test]
    async fn supervised_task_restarts_after_panic() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let restarted = Arc::new(Notify::new());
        let handle = spawn_supervised("test", Duration::from_millis(1), {
            let attempts = attempts.clone();
            let restarted = restarted.clone();
            move || {
                let attempts = attempts.clone();
                let restarted = restarted.clone();
                async move {
                    if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        panic!("intentional supervised task panic");
                    }
                    restarted.notify_one();
                    std::future::pending::<()>().await;
                }
            }
        });

        let observed = tokio::time::timeout(Duration::from_secs(1), restarted.notified()).await;
        handle.abort();

        assert!(observed.is_ok(), "supervisor did not restart the task");
        assert!(attempts.load(Ordering::SeqCst) >= 2);
    }
}

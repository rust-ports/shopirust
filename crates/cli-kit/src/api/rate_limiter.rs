use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use governor::state::direct::NotKeyed;
use governor::state::InMemoryState;
use governor::{Quota, RateLimiter};
use std::sync::Arc;
use std::time::Duration;

type GovernorInner = RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>;

#[derive(Clone)]
pub struct ApiRateLimiter {
    inner: Arc<GovernorInner>,
    semaphore: Option<Arc<tokio::sync::Semaphore>>,
}

impl ApiRateLimiter {
    pub fn new(min_time: Duration, max_concurrent: Option<usize>) -> Self {
        let quota = Quota::with_period(min_time).expect("valid duration for rate limiter");
        Self {
            inner: Arc::new(RateLimiter::direct(quota)),
            semaphore: max_concurrent.map(|n| Arc::new(tokio::sync::Semaphore::new(n))),
        }
    }

    pub fn shopify_default() -> Self {
        Self::new(Duration::from_millis(150), Some(10))
    }

    pub async fn acquire(&self) -> RateLimitPermit {
        self.inner.until_ready().await;
        let owned = match self.semaphore.as_ref() {
            Some(sem) => Arc::clone(sem).acquire_owned().await.ok(),
            None => None,
        };
        RateLimitPermit { _permit: owned }
    }
}

pub struct RateLimitPermit {
    _permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl std::fmt::Debug for ApiRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiRateLimiter").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shopify_default_creates() {
        let limiter = ApiRateLimiter::shopify_default();
        let permit = limiter.acquire().await;
        drop(permit);
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_sequential() {
        let limiter = ApiRateLimiter::new(Duration::from_millis(10), None);
        let permit1 = limiter.acquire().await;
        drop(permit1);
        let permit2 = limiter.acquire().await;
        drop(permit2);
    }

    #[tokio::test]
    async fn test_rate_limiter_concurrent_limit() {
        let limiter = Arc::new(ApiRateLimiter::new(Duration::from_millis(1), Some(2)));
        let l1 = limiter.clone();
        let l2 = limiter.clone();

        let (tx1, rx1) = tokio::sync::oneshot::channel();
        let (tx2, rx2) = tokio::sync::oneshot::channel();

        let h1 = tokio::spawn(async move {
            let p = l1.acquire().await;
            tx1.send(()).unwrap();
            rx2.await.unwrap();
            drop(p);
        });

        let h2 = tokio::spawn(async move {
            let p = l2.acquire().await;
            tx2.send(()).unwrap();
            rx1.await.unwrap();
            drop(p);
        });

        let (_, _) = tokio::join!(h1, h2);
    }

    #[tokio::test]
    async fn test_rate_limiter_clone() {
        let limiter = ApiRateLimiter::new(Duration::from_millis(10), Some(5));
        let cloned = limiter.clone();
        let permit1 = limiter.acquire().await;
        let permit2 = cloned.acquire().await;
        drop(permit1);
        drop(permit2);
    }
}

use futures::{ready, Stream};
use pin_project::pin_project;
use rand::{self, Rng};
use std::{
    cmp,
    convert::TryInto,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::Duration,
};
use tokio::time::{sleep, Sleep};

struct BackoffHandleInner {
    retry: u64,
    last_sleep: Option<Duration>,
    done: bool,
    #[allow(dead_code)]
    wake: Option<Waker>,
    min_sleep: Duration,
}

#[derive(Clone)]
pub struct BackoffHandle {
    inner: Arc<Mutex<BackoffHandleInner>>,
}

impl BackoffHandle {
    pub fn reset(&self) {
        let mut data = self.inner.lock().unwrap();
        data.retry = 0;
        data.last_sleep = Some(data.min_sleep);
    }

    // pub fn last_sleep(&self) -> Option<Duration> {
    //     (*self.inner.lock()).last_sleep
    // }
}

#[pin_project(project = BackoffProj)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    data: BackoffHandle,

    #[pin]
    delay: Option<Sleep>,
    need_next: bool,
}

impl Backoff {
    pub fn new(base: Duration, max: Duration) -> Backoff {
        Backoff {
            base,
            max,
            data: BackoffHandle {
                inner: Arc::new(Mutex::new(BackoffHandleInner {
                    retry: 0,
                    last_sleep: None,
                    done: false,
                    wake: None,
                    min_sleep: base,
                })),
            },
            delay: None,
            need_next: false,
        }
    }

    pub fn reset(&self) {
        self.data.reset();
    }
}

impl BackoffProj<'_> {
    fn next_sleep_duration(&self) -> Option<Duration> {
        let mut data = self.data.inner.lock().unwrap();
        match data.last_sleep {
            None => {
                data.last_sleep = Some(Duration::from_secs(0));
                data.retry += 1;
                None
            }
            Some(last_sleep) => {
                let from = self.base.as_millis();
                let to = last_sleep.as_millis() * 3;
                let r = if to <= from {
                    from
                } else {
                    rand::thread_rng().gen_range(from..to)
                };

                let sleep: Duration = cmp::min(
                    *self.max,
                    Duration::from_millis(r.try_into().expect("backoff delay overflow")),
                );
                data.last_sleep = Some(sleep);
                data.retry += 1;
                Some(sleep)
            }
        }
    }
}

impl Stream for Backoff {
    type Item = BackoffHandle;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut project = self.project();

        if project.data.inner.lock().unwrap().done {
            return Poll::Ready(None);
        }
        if *project.need_next {
            project.delay.set(project.next_sleep_duration().map(sleep));
            *project.need_next = false;
        };

        if let Some(delay) = project.delay.as_mut().as_pin_mut() {
            project.data.inner.lock().unwrap().wake = Some(cx.waker().clone());
            ready!(Sleep::poll(delay, cx));
        };

        project.data.inner.lock().unwrap().wake = None;
        project.delay.set(None);
        *project.need_next = true;

        Poll::Ready(Some(project.data.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{pin_mut, stream::StreamExt};
    use tokio::time::Instant;

    #[tokio::test]
    async fn test_backoff() {
        let min = Duration::from_millis(50);
        let max = Duration::from_secs(1);

        let backoff = Backoff::new(min, max);

        pin_mut!(backoff);

        let started_at = Instant::now();
        let _first = backoff.next().await;
        let first_at = Instant::now();
        assert!(first_at - started_at < min);

        let _second = backoff.next().await;
        let second_at = Instant::now();

        assert!(first_at - started_at < min);

        let third = backoff.next().await;
        let third_at = Instant::now();

        assert!(third_at - second_at < max + Duration::from_millis(50));
        assert!(third_at - second_at > min);

        third.unwrap().reset();

        let _fourth = backoff.next().await;
        let fourth_at = Instant::now();

        assert!(fourth_at - third_at < max + Duration::from_millis(50));

        // fourth.unwrap().finish();
        // assert!(backoff.next().await.is_none());
    }
}

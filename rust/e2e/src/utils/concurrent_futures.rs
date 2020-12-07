use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    stream::{FuturesUnordered, Stream},
    Future,
};
use tokio::task::{JoinError, JoinHandle};

/// `ConcurrentFutures` can keep a capped number of futures running concurrently, and yield their
/// result as they finish. When the max number of concurrent futures is reached, new tasks are
/// queued until some in-flight futures finish.
pub struct ConcurrentFutures<T>
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    /// In-flight futures.
    running: FuturesUnordered<JoinHandle<T::Output>>,
    /// Buffered tasks.
    queued: VecDeque<T>,
    /// Max number of concurrent futures.
    max_in_flight: usize,
}

impl<T> ConcurrentFutures<T>
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    pub fn new(max_in_flight: usize) -> Self {
        Self {
            running: FuturesUnordered::new(),
            queued: VecDeque::new(),
            max_in_flight,
        }
    }

    pub fn push(&mut self, task: T) {
        self.queued.push_back(task)
    }
}

impl<T> Unpin for ConcurrentFutures<T>
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
}

impl<T> Stream for ConcurrentFutures<T>
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    type Item = Result<T::Output, JoinError>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        while this.running.len() < this.max_in_flight {
            if let Some(queued) = this.queued.pop_front() {
                let handle = tokio::spawn(queued);
                this.running.push(handle);
            } else {
                break;
            }
        }
        Pin::new(&mut this.running).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::stream::StreamExt;
    use tokio::time::delay_for;

    use super::*;

    #[tokio::test]
    async fn test() {
        let mut stream =
            ConcurrentFutures::<Pin<Box<dyn Future<Output = u8> + Send + 'static>>>::new(2);

        stream.push(Box::pin(async {
            delay_for(Duration::from_millis(10_u64)).await;
            1_u8
        }));

        stream.push(Box::pin(async {
            delay_for(Duration::from_millis(28_u64)).await;
            2_u8
        }));

        stream.push(Box::pin(async {
            delay_for(Duration::from_millis(8_u64)).await;
            3_u8
        }));

        stream.push(Box::pin(async {
            delay_for(Duration::from_millis(2_u64)).await;
            4_u8
        }));

        // poll_next() hasn't been called yet so all futures are queued
        assert_eq!(stream.running.len(), 0);
        assert_eq!(stream.queued.len(), 4);

        // future 1 and 2 are spawned, then future 1 is ready
        assert_eq!(stream.next().await.unwrap().unwrap(), 1);

        // future 2 is pending, futures 3 and 4 are queued
        assert_eq!(stream.running.len(), 1);
        assert_eq!(stream.queued.len(), 2);

        // future 3 is spawned, then future 3 is ready
        assert_eq!(stream.next().await.unwrap().unwrap(), 3);

        // future 2 is pending, future 4 is queued
        assert_eq!(stream.running.len(), 1);
        assert_eq!(stream.queued.len(), 1);

        // future 4 is spawned, then future 4 is ready
        assert_eq!(stream.next().await.unwrap().unwrap(), 4);

        // future 2 is pending, then future 2 is ready
        assert_eq!(stream.next().await.unwrap().unwrap(), 2);

        // all futures have been resolved
        assert_eq!(stream.running.len(), 0);
        assert_eq!(stream.queued.len(), 0);
    }
}

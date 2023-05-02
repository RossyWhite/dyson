/// Convert a `tokio::task::JoinSet` to a `tokio_stream::Stream`.
pub fn try_join_set_to_stream<T: 'static, E: 'static>(
    mut tasks: tokio::task::JoinSet<Result<T, E>>,
) -> impl tokio_stream::Stream<Item = Result<T, E>> + 'static {
    Box::pin(async_stream::stream! {
        while let Some(r) = tasks.join_next().await {
            match r {
                Ok(Ok(r)) => yield Ok(r),
                Ok(Err(e)) => {
                    tasks.shutdown().await;
                    yield Err(e);
                    break;
                },
                Err(_) => {
                    tasks.shutdown().await;
                    break
                },
            }
        }
    })
}

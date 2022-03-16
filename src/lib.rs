use std::future::Future;
use warp::reject::Reject;

pub mod app;
pub mod oauth;
pub mod preset;
pub mod presets {
    pub mod demo;
    pub mod lab3;
    pub mod lab4;
}

#[derive(Debug)]
struct AnyHowError(pub anyhow::Error);
impl Reject for AnyHowError {}

pub fn with_anyhow<T>(
    inner: impl Future<Output = anyhow::Result<T>>,
) -> impl Future<Output = Result<T, warp::Rejection>> {
    async move { inner.await.map_err(|error| AnyHowError(error).into()) }
}

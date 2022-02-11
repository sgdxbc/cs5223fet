use warp::reject::Reject;

pub mod oauth;
pub mod app;

#[derive(Debug)]
pub struct AnyHowError(anyhow::Error);
impl Reject for AnyHowError {}

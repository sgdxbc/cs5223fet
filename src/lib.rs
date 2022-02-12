use warp::reject::Reject;

pub mod app;
pub mod oauth;
pub mod preset;
pub mod presets {
    pub mod demo;
}

#[derive(Debug)]
pub struct AnyHowError(pub anyhow::Error);
impl Reject for AnyHowError {}

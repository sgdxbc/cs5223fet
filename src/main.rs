use cs5223fet::app::{App, Task};
use cs5223fet::oauth::{login_recover, redirect, user_id, OAuth};
use cs5223fet::AnyHowError;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::reply;
use warp::Filter;

use cs5223fet::presets::demo::Preset;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let oauth = Arc::new(OAuth::new()?);
    let app = Arc::new(Mutex::new(App::<Preset>::new().await));
    let home_prompt = String::from(r#"<a href="/">Home</a>"#);

    let home_app = app.clone();
    let route = user_id().and(warp::path::end()).then(move |id| {
        let home_app = home_app.clone();
        async move {
            reply::html(format!(
                r#"
            <p>CS5223 Slow and Hard Test</p>
            <p>System status: {}</p>
            <p>GitHub id: {}</p>
            "#,
                home_app.lock().await.status,
                id
            ))
        }
    });
    let submit_app = app.clone();
    let route = route.or(user_id()
        .and(warp::path("task/submit"))
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |id, preset: Preset| {
            let submit_app = submit_app.clone();
            async move {
                let result = submit_app
                    .lock()
                    .await
                    .push_task(Task {
                        user_id: id,
                        preset,
                        upload: Vec::new(), // TODO
                        canceled: false,
                    })
                    .await;
                let task_id = match result {
                    Ok(task_id) => task_id,
                    Err(error) => return Err(Into::<warp::Rejection>::into(AnyHowError(error))),
                };
                Ok(reply::html(format!("Task #{} submitted", task_id)))
            }
        }));

    let route = route.or(redirect(oauth.clone(), home_prompt.clone()));

    let websocket_app = app.clone();
    let route = route.or(warp::path("websocket")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let websocket_app = websocket_app.clone();
            ws.on_upgrade(|websocket| App::connect_worker(websocket_app, websocket))
        }));

    let route = login_recover(route, oauth);
    warp::serve(route).run(([0, 0, 0, 0], 8080)).await;
    Ok(())
}

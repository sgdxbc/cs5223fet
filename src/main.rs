use anyhow::anyhow;
use bytes::BufMut;
use cs5223fet::app::{App, Task};
use cs5223fet::oauth::OAuth;
use cs5223fet::with_anyhow;
use futures::prelude::*;
use serde_json::from_slice;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::multipart::FormData;
use warp::{reply, Filter};

use cs5223fet::presets::demo::Preset;

const UNIVERSAL: &'static str = r#"
<link href="https://fonts.googleapis.com/css2?family=Fira+Sans&display=swap" rel="stylesheet">
<style>html { font-family: 'Fira Sans', sans-serif; }</style>
"#;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let oauth = Arc::new(OAuth::new()?);
    let app = Arc::new(Mutex::new(App::<Preset>::new().await?));

    let home_prompt = format!(r#"{}<a href="/">Home</a>"#, UNIVERSAL);
    let login_prompt = format!(r#"{}<a href="{}">Login</a>"#, UNIVERSAL, oauth.url);

    let home_app = app.clone();
    let route = oauth.user_id().and(warp::path::end()).then(move |id| {
        let home_app = home_app.clone();
        async move {
            reply::html(format!(
                r#"
{}
<p>CS5223 Slow and Hard Test</p>
<p>System status: {}</p>
<p>GitHub id: {}</p>
"#,
                UNIVERSAL,
                home_app.lock().await.status,
                id
            ))
        }
    });
    let submit_app = app.clone();
    let route = route.or(oauth
        .user_id()
        .and(warp::path!("task" / "submit"))
        .and(warp::post())
        .and(warp::multipart::form().max_length(50_000))
        .and_then(move |id, form: FormData| {
            let submit_app = submit_app.clone();
            with_anyhow(async move {
                let mut form: HashMap<_, _> = form
                    .map_ok(|part| (part.name().to_string(), part.stream()))
                    .try_collect()
                    .await?;
                let preset = form
                    .remove("preset")
                    .ok_or(anyhow!("no preset in submission"))?
                    .try_fold(Vec::new(), |mut preset, data| {
                        preset.put(data);
                        async { Ok(preset) }
                    })
                    .await?;
                let preset = from_slice(&preset)?;
                let upload = form
                    .remove("upload")
                    .ok_or(anyhow!("no upload in submission"))?
                    .try_fold(Vec::new(), |mut upload, data| {
                        upload.put(data);
                        async { Ok(upload) }
                    })
                    .await?;

                let task_id = submit_app
                    .lock()
                    .await
                    .push_task(Task {
                        user_id: id,
                        preset,
                        upload,
                        canceled: false,
                    })
                    .await?;
                Ok(reply::html(format!(
                    "{}<p>Task #{} submitted</p>",
                    UNIVERSAL, task_id
                )))
            })
        }));

    let route = route.or(oauth.redirect(home_prompt.clone()));

    let websocket_app = app.clone();
    let route = route.or(warp::path("websocket")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let websocket_app = websocket_app.clone();
            ws.on_upgrade(|websocket| App::connect_worker(websocket_app, websocket))
        }));

    let route = OAuth::recover(route, login_prompt);
    warp::serve(route).run(([0, 0, 0, 0], 8080)).await;
    Ok(())
}

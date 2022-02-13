use anyhow::anyhow;
use bytes::BufMut;
use cs5223fet::app::{App, Task, TaskId, TaskStatus};
use cs5223fet::oauth::OAuth;
use cs5223fet::preset::Preset as _;
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
            let task_navigation: Vec<_> = home_app
                .lock()
                .await
                .user_table
                .get(&id)
                .map(|task_list| {
                    task_list
                        .iter()
                        .map(|task_id| format!(r#"<a href="/task/{0}">#{0}</a>"#, task_id))
                        .collect()
                })
                .unwrap_or_default();
            reply::html(format!(
                r#"
{}
<p>CS5223 Slow and Hard Test</p>
<p>System status: {}</p>
<p>GitHub id: {}</p>
<form id="submit-form" action="/task/submit" method="post" enctype="multipart/form-data">
    <input type="file" name="upload">
    <input id="submit-preset" type="hidden" name="preset">
    {}
    <button id="submit-button" type="submit" disabled>Submit</button>
</form>
<script>
let form;
function start() {{
    form = document.querySelector('#submit-form');
    form.addEventListener('submit', onSubmit);
    
    const button = document.querySelector('#submit-button');
    button.disabled = false;
}}
function onSubmit(e) {{
    const preset = new Object;
    for (let child of form.childNodes) {{
        if (!child.name || !child.name.startsWith(':')) {{
            continue;
        }}
        preset[child.name] = child.value;
    }}
    const presetNode = document.querySelector('#submit-preset');
    presetNode.value = JSON.stringify(preset);
}}
window.addEventListener('DOMContentLoaded', start);
</script>
{}
"#,
                UNIVERSAL,
                home_app.lock().await.status,
                id,
                Preset::render_html(),
                task_navigation.join(" ")
            ))
        }
    });

    let submit_app = app.clone();
    let submit_home_prompt = home_prompt.clone();
    let route = route.or(oauth
        .user_id()
        .and(warp::path!("task" / "submit"))
        .and(warp::post())
        .and(warp::multipart::form().max_length(50_000))
        .and_then(move |id, form: FormData| {
            let submit_app = submit_app.clone();
            let submit_home_prompt = submit_home_prompt.clone();
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
                let preset: HashMap<String, String> = from_slice(&preset)?;
                let preset: Preset = preset.try_into()?;
                let upload = form
                    .remove("upload")
                    .ok_or(anyhow!("no upload in submission"))?
                    .try_fold(Vec::new(), |mut upload, data| {
                        upload.put(data);
                        async { Ok(upload) }
                    })
                    .await?;
                if upload.is_empty() {
                    return Err(anyhow!("submission is empty"));
                }

                let task_id = submit_app
                    .lock()
                    .await
                    .push_task(Task {
                        user_id: id,
                        preset,
                        upload,
                        status: TaskStatus::Pending,
                    })
                    .await?;
                Ok(reply::html(format!(
                    "{}<p>Task #{} submitted</p>",
                    submit_home_prompt, task_id
                )))
            })
        }));

    let task_app = app.clone();
    let task_home_prompt = home_prompt.clone();
    let route = route.or(oauth.user_id().and(warp::path!("task" / TaskId)).and_then(
        move |user_id, task_id| {
            let task_app = task_app.clone();
            let task_home_prompt = task_home_prompt.clone();
            with_anyhow(async move {
                let mut task_app = task_app.lock().await;
                let task = task_app.get_task(task_id).await?;
                if task.user_id != user_id {
                    return Err(anyhow!("task id not accessible"));
                }
                let output_prompt = if task.status == TaskStatus::Finished {
                    format!(r#"<a href="/task/{0}/output/{0}">output</a>"#, task_id)
                } else {
                    format!("")
                };
                let replace_prompt = if task.status == TaskStatus::Pending {
                    format!(
                        r#"
<form action="/task/{}/replace" method="post" enctype="multipart/form-data">
    <input type="file" name="upload">
    <button type="submit">Replace upload</button>
</form>
"#,
                        task_id
                    )
                } else {
                    format!("")
                };
                Ok(reply::html(format!(
                    r#"
{}
<p>#{} {}</p>
<p>{:?}</p>
{}
{}
"#,
                    task_home_prompt,
                    task_id,
                    task.preset,
                    task.status,
                    output_prompt,
                    replace_prompt
                )))
            })
        },
    ));

    let replace_app = app.clone();
    let replace_home_prompt = home_prompt.clone();
    let route = route.or(oauth
        .user_id()
        .and(warp::path!("task" / TaskId / "replace"))
        .and(warp::post())
        .and(warp::multipart::form().max_length(50_000))
        .and_then(move |user_id, task_id, form: FormData| {
            let replace_app = replace_app.clone();
            let replace_home_prompt = replace_home_prompt.clone();
            with_anyhow(async move {
                let mut form: HashMap<_, _> = form
                    .map_ok(|part| (part.name().to_string(), part.stream()))
                    .try_collect()
                    .await?;
                let upload = form
                    .remove("upload")
                    .ok_or(anyhow!("no upload in submission"))?
                    .try_fold(Vec::new(), |mut upload, data| {
                        upload.put(data);
                        async { Ok(upload) }
                    })
                    .await?;
                if upload.is_empty() {
                    return Err(anyhow!("submission is empty"));
                }

                let task = replace_app.lock().await.get_task(task_id).await?;
                if task.user_id != user_id || task.status != TaskStatus::Pending {
                    return Err(anyhow!("update upload rejected"));
                }

                replace_app.lock().await.replace_upload(task_id, upload);
                Ok(reply::html(format!(
                    "{}<p>Task #{} upload updated.</p>",
                    replace_home_prompt, task_id
                )))
            })
        }));

    let output_app = app.clone();
    let route = route.or(oauth
        .user_id()
        .and(warp::path("task"))
        .and(warp::path::param())
        .and(warp::path("output"))
        .and(warp::path::peek())
        .and_then(move |user_id, task_id, peek: warp::path::Peek| {
            let output_app = output_app.clone();
            with_anyhow(async move {
                let peek: TaskId = peek.as_str().parse()?;
                if peek != task_id {
                    return Err(anyhow!("invalid url"));
                }
                let task = output_app.lock().await.get_task(task_id).await?;
                if task.user_id != user_id {
                    return Err(anyhow!("task id not accessible"));
                }
                if task.status != TaskStatus::Finished {
                    return Err(anyhow!("no available output"));
                }
                Ok(())
            })
        })
        .untuple_one()
        .and(warp::fs::dir("_fs/output")));

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

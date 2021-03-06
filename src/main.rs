use anyhow::anyhow;
use bytes::BufMut;
use cs5223fet::app::{App, Task, TaskId, TaskStatus};
use cs5223fet::oauth::OAuth;
use cs5223fet::preset::Preset as _;
use cs5223fet::with_anyhow;
use futures::prelude::*;
use serde_json::from_slice;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use warp::multipart::FormData;
use warp::{reply, Filter};

use cs5223fet::presets::lab4::Preset;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    fn universal() -> &'static str {
        r#"
<link href="https://fonts.googleapis.com/css2?family=Fira+Sans&display=swap" rel="stylesheet">
<style>html { font-family: 'Fira Sans', sans-serif; }</style>
"#
    }
    fn home_prompt() -> String {
        format!(r#"{}<a href="/">Home</a>"#, universal())
    }

    let oauth = Arc::new(OAuth::new()?);
    let app = Arc::new(App::<Preset>::new().await?);

    let home_app = app.clone();
    let route = oauth.user_id().and(warp::path::end()).then(move |id| {
        let home_app = home_app.clone();
        async move {
            let task_navigation: Vec<_> = home_app
                .data
                .read()
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
<p>CS5223 Slow and Hard Test<sup>beta</sup></p>
<p>System status: {} GitHub ID: {}</p>
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
<ul>
    <li>At most one outstanding (i.e., pending or running) task is allowed for 
    one GitHub ID.</li>
    <li>You can replace upload file for a pending task, but you are not allowed 
    to change to another set of settings.</li>
    <li>Upload file size limits to about 50KB.</li>
</ul>
"#,
                universal(),
                home_app.status.read().await,
                id,
                Preset::render_html(),
                task_navigation.join(" ")
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
                    .push_task(Task {
                        user_id: id,
                        preset,
                        upload,
                        status: TaskStatus::Pending,
                    })
                    .await?;
                Ok(reply::html(format!(
                    "{}<p>Task #{} submitted</p>",
                    home_prompt(),
                    task_id
                )))
            })
        }));

    let task_app = app.clone();
    let route = route.or(oauth.user_id().and(warp::path!("task" / TaskId)).and_then(
        move |user_id: String, task_id| {
            let task_app = task_app.clone();
            with_anyhow(async move {
                if !task_app.allow_access(&user_id, task_id).await {
                    return Err(anyhow!("task id not accessible"));
                }
                let task = task_app.get_task(task_id).await?;
                let output_prompt = if task.status == TaskStatus::Finished {
                    format!(r#"<a href="/task/{0}/output/{0}">output</a>"#, task_id)
                } else {
                    format!("")
                };
                let wait_time_prompt = if task.status == TaskStatus::Pending {
                    format!(
                        r", maximum waiting time: {:?}",
                        task_app.get_wait_time(task_id).await
                    )
                } else {
                    format!("")
                };
                let edit_prompt = if task.status == TaskStatus::Pending {
                    format!(
                        r#"
<form action="/task/{0}/replace" method="post" enctype="multipart/form-data">
    <input type="file" name="upload">
    <button type="submit">Replace upload</button>
</form>
<form action="/task/{0}/cancel" method="post">
    <button type="submit">Cancel</button>
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
<p>{:?}{}</p>
{}
{}
<ul>
    <li>Server does not store upload file on disk, so upon system failure it has
    to cancel pending/running task if upload file is lost. Sorry for 
    inconvenience if that happens.</li>
    <li>Test output is trimmed and only the last 10MB is available for 
    downloading.</li>
</ul>
"#,
                    home_prompt(),
                    task_id,
                    task.preset,
                    task.status,
                    wait_time_prompt,
                    output_prompt,
                    edit_prompt
                )))
            })
        },
    ));

    let replace_app = app.clone();
    let route = route.or(oauth
        .user_id()
        .and(warp::path!("task" / TaskId / "replace"))
        .and(warp::post())
        .and(warp::multipart::form().max_length(50_000))
        .and_then(move |user_id: String, task_id, form: FormData| {
            let replace_app = replace_app.clone();
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

                if !replace_app.allow_access(&user_id, task_id).await {
                    return Err(anyhow!("update upload reject"));
                }

                replace_app.replace_upload(task_id, upload).await?;

                Ok(reply::html(format!(
                    "{}<p>Task #{} upload updated.</p>",
                    home_prompt(),
                    task_id
                )))
            })
        }));

    let cancel_app = app.clone();
    let route = route.or(oauth
        .user_id()
        .and(warp::path!("task" / TaskId / "cancel"))
        .and(warp::post())
        .and_then(move |user_id: String, task_id| {
            let cancel_app = cancel_app.clone();
            with_anyhow(async move {
                if !cancel_app.allow_access(&user_id, task_id).await {
                    return Err(anyhow!("cancel rejected"));
                }

                cancel_app.cancel_task(task_id).await?;
                Ok(reply::html(format!(
                    "{}<p> Task #{} canceled.</p>",
                    home_prompt(),
                    task_id
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
        .and_then(move |user_id: String, task_id, peek: warp::path::Peek| {
            let output_app = output_app.clone();
            with_anyhow(async move {
                let peek: TaskId = peek.as_str().parse()?;
                if peek != task_id {
                    return Err(anyhow!("invalid url"));
                }
                if !output_app.allow_access(&user_id, task_id).await {
                    return Err(anyhow!("task id not accessible"));
                }
                if output_app.get_task(task_id).await?.status != TaskStatus::Finished {
                    return Err(anyhow!("no available output"));
                }
                Ok(())
            })
        })
        .untuple_one()
        .and(warp::fs::dir("_fs/output")));

    let route = route.or(oauth.redirect(home_prompt()));

    let websocket_app = app.clone();
    let route = route.or(warp::path("websocket")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let websocket_app = websocket_app.clone();
            ws.on_upgrade(move |websocket| async move {
                websocket_app.connect_worker(websocket).await;
            })
        }));

    let login_prompt = format!(r#"{}<a href="{}">Login</a>"#, universal(), oauth.url);
    let route = OAuth::recover(route, login_prompt);
    warp::serve(route)
        .run(([0, 0, 0, 0], env::var("CS5223FET_PORT")?.parse()?))
        .await;
    Ok(())
}

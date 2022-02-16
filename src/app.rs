use crate::preset::Preset;
use anyhow::anyhow;
use futures::prelude::*;
use redis::{AsyncCommands, Client};
use rmp_serde::{from_slice, to_vec_named};
use serde::Deserialize as Deser;
use serde_derive::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::mem::take;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::{select, spawn};
use warp::ws::{Message, WebSocket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStatus {
    Disconnected(u32, u32), // next pending task id, last task id
    Running(u32, u32),      // running task id, last task id
    StandBy(u32),           // last task id
}

impl Display for AppStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected(pending_id, last_id) => {
                write!(f, "disconnected, {} waiting", last_id + 1 - pending_id)
            }
            Self::Running(task_id, last_id) => {
                write!(f, "running(#{}), {} waiting", task_id, last_id - task_id)
            }
            Self::StandBy(_) => write!(f, "free"),
        }
    }
}

pub struct App<Preset> {
    pub status: RwLock<AppStatus>,
    worker_tx: Mutex<Option<mpsc::Sender<ToWorker>>>,
    pub data: RwLock<AppData<Preset>>,
    client: Client,
}

pub struct AppData<Preset> {
    task_table: HashMap<TaskId, Task<Preset>>,
    pub user_table: HashMap<String, Vec<TaskId>>,
}

pub type TaskId = u32;
#[derive(Debug, Clone)]
pub struct Task<Preset> {
    pub user_id: String,
    pub preset: Preset,
    pub upload: Vec<u8>,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Finished,
    Canceled,
}

#[derive(Debug, Serialize)]
struct ToWorker {
    task_id: TaskId,
    command: String,
    upload: Vec<u8>,
    timeout: u64, // in second
}

#[derive(Debug, Deserialize)]
struct FromWorker {
    task_id: TaskId,
    output: String,
    // anything else?
}

impl<P> App<P> {
    pub async fn new() -> anyhow::Result<Self> {
        let client = Client::open("redis://localhost")?;
        let mut conn = client.get_async_connection().await?;
        let mut last_id = 0;
        let mut user_table: HashMap<_, Vec<_>> = HashMap::new();
        loop {
            let mut query: HashMap<String, String> =
                conn.hgetall(format!("task:{}", last_id + 1)).await?;
            if query.is_empty() {
                break;
            }
            last_id += 1;
            user_table
                .entry(query.remove("user-id").unwrap())
                .or_default()
                .push(last_id);

            let status: TaskStatus = from_str(query.get("status").unwrap()).unwrap();
            if status != TaskStatus::Finished {
                // println!("[app] Cancel task #{}", last_id);
                conn.hset(
                    format!("task:{}", last_id),
                    "status",
                    to_string(&TaskStatus::Canceled).unwrap(),
                )
                .await?;
            }
        }
        println!("[app] Initialized with {} past tasks", last_id);

        Ok(Self {
            status: RwLock::new(AppStatus::Disconnected(last_id + 1, last_id)),
            worker_tx: Mutex::new(None),
            data: RwLock::new(AppData {
                task_table: HashMap::new(),
                user_table,
            }),
            client,
        })
    }

    async fn disconnect_worker(&self) {
        let mut status = self.status.write().await;
        let mut worker_tx = self.worker_tx.lock().await;
        *status = match *status {
            AppStatus::Disconnected(_, _) => unreachable!(),
            AppStatus::Running(task_id, last_id) => AppStatus::Disconnected(task_id, last_id),
            AppStatus::StandBy(last_id) => AppStatus::Disconnected(last_id + 1, last_id),
        };
        *worker_tx = None;
    }

    pub async fn get_task(&self, task_id: TaskId) -> anyhow::Result<Task<P>>
    where
        P: Clone + for<'a> Deser<'a>,
    {
        if let Some(task) = self.data.read().await.task_table.get(&task_id) {
            return Ok(Task {
                user_id: task.user_id.clone(),
                preset: task.preset.clone(),
                upload: Vec::new(), // any better way?
                status: task.status,
            });
        }
        let mut query: HashMap<String, String> = self
            .client
            .get_async_connection()
            .await?
            .hgetall(format!("task:{}", task_id))
            .await?;
        if query.is_empty() {
            return Err(anyhow!("task not found"));
        }
        Ok(Task {
            user_id: query.remove("user-id").unwrap(),
            preset: from_str(query.get("preset").unwrap()).unwrap(),
            upload: Vec::new(),
            status: from_str(query.get("status").unwrap()).unwrap(),
        })
    }

    pub async fn replace_upload(&self, task_id: TaskId, upload: Vec<u8>) -> anyhow::Result<()> {
        let mut data = self.data.write().await;
        let task = data.task_table.get_mut(&task_id).unwrap();
        if task.status != TaskStatus::Pending {
            return Err(anyhow!("task is not pending"));
        }
        task.upload = upload;
        Ok(())
    }

    pub async fn cancel_task(&self, task_id: TaskId) -> anyhow::Result<()> {
        let mut data = self.data.write().await;
        let task = data.task_table.get_mut(&task_id).unwrap();
        if task.status != TaskStatus::Pending {
            return Err(anyhow!("task is not pending"));
        }
        task.status = TaskStatus::Canceled;

        let _: () = self
            .client
            .get_async_connection()
            .await?
            .hset(
                format!("task:{}", task_id),
                "status",
                to_string(&TaskStatus::Canceled).unwrap(),
            )
            .await
            .unwrap();

        Ok(())
    }

    // more efficient version of `app.get_task(task_id).user_id == user_id`
    pub async fn allow_access(&self, user_id: &str, task_id: TaskId) -> bool {
        self.data
            .read()
            .await
            .user_table
            .get(user_id)
            .map(|task_set| task_set.contains(&task_id))
            .unwrap_or(false)
    }
}

impl<P: Preset> App<P> {
    pub async fn get_wait_time(&self, task_id: TaskId) -> Duration
    where
        P: Preset,
    {
        let start_task = match *self.status.read().await {
            AppStatus::Disconnected(pending_id, _) => pending_id,
            AppStatus::Running(task_id, _) => task_id,
            AppStatus::StandBy(_) => return Duration::from_secs(0),
        };
        let wait_time = self
            .data
            .read()
            .await
            .task_table
            .iter()
            .filter(|(id, task)| {
                (start_task..task_id).contains(id) && task.status == TaskStatus::Pending
            })
            .map(|(_, task)| task.preset.get_timeout())
            .sum();
        Duration::from_secs(wait_time)
    }

    pub async fn connect_worker(self: &Arc<Self>, mut websocket: WebSocket)
    where
        P: 'static + Send,
    {
        let mut status = self.status.write().await;
        let mut worker_tx = self.worker_tx.lock().await;

        if worker_tx.is_some() {
            unimplemented!("multiple worker");
        }
        let (worker_tx0, mut worker_rx) = mpsc::channel(1);
        *worker_tx = Some(worker_tx0);
        drop(worker_tx); // transfer to `send_task`

        *status = if let AppStatus::Disconnected(pending_id, last_id) = *status {
            if pending_id <= last_id {
                if let Some(task_id) = self.send_task(pending_id).await {
                    AppStatus::Running(task_id, last_id)
                } else {
                    AppStatus::StandBy(last_id)
                }
            } else {
                AppStatus::StandBy(last_id)
            }
        } else {
            unreachable!();
        };

        let app = self.clone();
        spawn(async move {
            loop {
                select! {
                    Some(to_worker) = worker_rx.recv() => {
                        websocket.send(Message::binary(to_vec_named(&to_worker).unwrap())).await.unwrap();
                    }
                    Some(Ok(message)) = websocket.next() => {
                        if message.is_close() {
                            break;
                        }
                        if !message.is_binary()   {
                            if message.is_text() {
                                println!("[app] warning: text message from worker: {:?}", message);
                            }
                            continue;
                        }
                        let from_worker: FromWorker = from_slice(&message.into_bytes()).unwrap();
                        app.finish_task(from_worker).await;
                    }
                    else => break
                }
            }
            websocket.close().await.unwrap();
            app.disconnect_worker().await;
        });
    }

    async fn finish_task(&self, from_worker: FromWorker) {
        let mut status = self.status.write().await;
        let mut data = self.data.write().await;
        if let AppStatus::Running(task_id, last_id) = *status {
            assert_eq!(task_id, from_worker.task_id);

            fs::write(format!("_fs/output/{}", task_id), from_worker.output)
                .await
                .unwrap();
            println!("[app] finish write _fs/output/{}", task_id);

            data.task_table.get_mut(&task_id).unwrap().status = TaskStatus::Finished;
            drop(data); // transfer to `send_task`

            let _: () = self
                .client
                .get_async_connection()
                .await
                .unwrap()
                .hset(
                    format!("task:{}", task_id),
                    "status",
                    to_string(&TaskStatus::Finished).unwrap(),
                )
                .await
                .unwrap();

            let pending_id = task_id + 1;
            *status = if pending_id > last_id {
                AppStatus::StandBy(last_id)
            } else if let Some(task_id) = self.send_task(pending_id).await {
                AppStatus::Running(task_id, last_id)
            } else {
                AppStatus::StandBy(last_id)
            };
        } else {
            unreachable!();
        }
    }

    async fn send_task(&self, mut pending_id: TaskId) -> Option<TaskId> {
        let mut data = self.data.write().await;
        let task_table = &mut data.task_table;
        let worker_tx = self.worker_tx.lock().await;

        let to_worker = loop {
            if let Some(task) = task_table.get_mut(&pending_id) {
                match task.status {
                    TaskStatus::Pending => {
                        break ToWorker {
                            task_id: pending_id,
                            command: task.preset.get_command(),
                            upload: take(&mut task.upload),
                            timeout: task.preset.get_timeout(),
                        }
                    }
                    TaskStatus::Canceled => pending_id += 1,
                    _ => unreachable!(),
                }
            } else {
                return None;
            }
        };
        task_table.get_mut(&pending_id).unwrap().status = TaskStatus::Running;

        let _: () = self
            .client
            .get_async_connection()
            .await
            .unwrap()
            .hset(
                format!("task:{}", pending_id),
                "status",
                to_string(&TaskStatus::Running).unwrap(),
            )
            .await
            .unwrap(); // internal communication must success

        worker_tx.as_ref().unwrap().send(to_worker).await.unwrap();
        Some(pending_id)
    }

    pub async fn push_task(&self, task: Task<P>) -> anyhow::Result<TaskId> {
        let mut status = self.status.write().await;
        let data = self.data.read().await;

        let user_last = data
            .user_table
            .get(&task.user_id)
            .and_then(|task_list| task_list.last())
            .and_then(|last_task| {
                // assert anything not present in `task_table` is unrelated
                let status = data.task_table.get(&last_task)?.status;
                if status == TaskStatus::Pending || status == TaskStatus::Running {
                    Some(last_task)
                } else {
                    None
                }
            })
            .cloned()
            .unwrap_or(0);
        drop(data); // transfer lock to `register_task`

        let task_id = match *status {
            AppStatus::Disconnected(pending_id, last_id) => {
                if user_last >= pending_id {
                    return Err(anyhow!("already pending for #{}", user_last));
                }
                last_id
            }
            AppStatus::Running(task_id, last_id) => {
                if user_last >= task_id {
                    return Err(anyhow!("already pending/running for #{}", user_last));
                }
                last_id
            }
            AppStatus::StandBy(last_id) => last_id,
        } + 1;
        self.register_task(task_id, task).await;

        *status = match *status {
            AppStatus::Disconnected(id, _) => AppStatus::Disconnected(id, task_id),
            AppStatus::Running(id, _) => AppStatus::Running(id, task_id),
            AppStatus::StandBy(_) => {
                let id = self.send_task(task_id).await;
                assert_eq!(id, Some(task_id));
                AppStatus::Running(task_id, task_id)
            }
        };
        Ok(task_id)
    }

    async fn register_task(&self, task_id: u32, task: Task<P>) {
        assert_eq!(task.status, TaskStatus::Pending);

        let mut data = self.data.write().await;
        let prev = data.task_table.insert(task_id, task.clone());
        assert!(prev.is_none());

        data.user_table
            .entry(task.user_id.clone())
            .or_default()
            .push(task_id);

        let _: () = self
            .client
            .get_async_connection()
            .await
            .unwrap()
            .hset_multiple(
                format!("task:{}", task_id),
                &[
                    ("user-id", &task.user_id),
                    ("preset", &to_string(&task.preset).unwrap()),
                    ("status", &to_string(&task.status).unwrap()),
                ],
            )
            .await
            .unwrap();
    }
}

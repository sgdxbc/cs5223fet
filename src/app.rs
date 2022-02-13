use crate::preset::Preset;
use anyhow::anyhow;
use futures::prelude::*;
use redis::aio::Connection;
use redis::{AsyncCommands, Client};
use serde::Deserialize as Deser;
use serde_derive::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::mem::take;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{mpsc, Mutex};
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
    pub status: AppStatus,
    worker_tx: Option<mpsc::Sender<ToWorker>>,
    task_table: HashMap<TaskId, Task<Preset>>,
    pub user_table: HashMap<String, Vec<TaskId>>,
    conn: Connection,
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
    timeout: u32, // in second
}

#[derive(Debug, Deserialize)]
struct FromWorker {
    task_id: TaskId,
    output: String,
    // anything else?
}

impl<P> App<P> {
    pub async fn new() -> anyhow::Result<Self> {
        let mut conn = Client::open("redis://localhost")?
            .get_async_connection()
            .await?;
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
            status: AppStatus::Disconnected(last_id + 1, last_id),
            worker_tx: None,
            task_table: HashMap::new(),
            user_table,
            conn,
        })
    }

    fn disconnect_worker(&mut self) {
        self.status = match self.status {
            AppStatus::Disconnected(_, _) => unreachable!(),
            AppStatus::Running(task_id, last_id) => AppStatus::Disconnected(task_id, last_id),
            AppStatus::StandBy(last_id) => AppStatus::Disconnected(last_id + 1, last_id),
        };
        self.worker_tx = None;
    }
}

impl<P> App<P> {
    pub async fn get_task(&mut self, task_id: TaskId) -> anyhow::Result<Task<P>>
    where
        P: Clone + for<'a> Deser<'a>,
    {
        if let Some(task) = self.task_table.get(&task_id) {
            return Ok(Task {
                user_id: task.user_id.clone(),
                preset: task.preset.clone(),
                upload: Vec::new(), // any better way?
                status: task.status,
            });
        }
        let mut query: HashMap<String, String> =
            self.conn.hgetall(format!("task:{}", task_id)).await?;
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

    pub fn replace_upload(&mut self, task_id: TaskId, upload: Vec<u8>) {
        self.task_table.get_mut(&task_id).unwrap().upload = upload;
    }
}

impl<P: Preset> App<P> {
    pub async fn connect_worker(app0: Arc<Mutex<Self>>, mut websocket: WebSocket)
    where
        P: 'static + Send,
    {
        let mut app = app0.lock().await;
        if app.worker_tx.is_some() {
            unimplemented!("multiple worker");
        }

        let (worker_tx, mut worker_rx) = mpsc::channel(1);
        app.worker_tx = Some(worker_tx);
        app.status = if let AppStatus::Disconnected(pending_id, last_id) = app.status {
            if pending_id <= last_id {
                if let Some(task_id) = app.send_task(pending_id).await {
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
        drop(app);

        spawn(async move {
            loop {
                select! {
                    Some(to_worker) = worker_rx.recv() => {
                        websocket.send(Message::text(to_string(&to_worker).unwrap())).await.unwrap();
                    }
                    Some(Ok(message)) = websocket.next() => {
                        if message.is_close() {
                            break;
                        }
                        if !message.is_text()   {
                            if message.is_binary() {
                                println!("[app] warning: binary message from worker: {:?}", message);
                            }
                            continue;
                        }
                        let from_worker: FromWorker = from_str(&message.to_str().unwrap()).unwrap();
                        app0.lock().await.finish_task(from_worker).await;
                    }
                    else => break
                }
            }
            websocket.close().await.unwrap();
            app0.lock().await.disconnect_worker();
        });
    }

    async fn finish_task(&mut self, from_worker: FromWorker) {
        if let AppStatus::Running(task_id, last_id) = self.status {
            assert_eq!(task_id, from_worker.task_id);

            fs::write(format!("_fs/output/{}", task_id), from_worker.output)
                .await
                .unwrap();
            println!("[app] finish write _fs/output/{}", task_id);

            self.task_table.get_mut(&task_id).unwrap().status = TaskStatus::Finished;
            let _: () = self
                .conn
                .hset(
                    format!("task:{}", task_id),
                    "status",
                    to_string(&TaskStatus::Finished).unwrap(),
                )
                .await
                .unwrap();

            let pending_id = task_id + 1;
            self.status = if pending_id > last_id {
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

    async fn send_task(&mut self, mut pending_id: TaskId) -> Option<TaskId> {
        let to_worker = loop {
            if let Some(task) = self.task_table.get_mut(&pending_id) {
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
        self.task_table.get_mut(&pending_id).unwrap().status = TaskStatus::Running;
        let _: () = self
            .conn
            .hset(
                format!("task:{}", pending_id),
                "status",
                to_string(&TaskStatus::Running).unwrap(),
            )
            .await
            .unwrap(); // internal communication must success
        self.worker_tx
            .as_ref()
            .unwrap()
            .send(to_worker)
            .await
            .unwrap();
        Some(pending_id)
    }

    pub async fn push_task(&mut self, task: Task<P>) -> anyhow::Result<TaskId> {
        let user_last = self
            .user_table
            .get(&task.user_id)
            .and_then(|user_set| user_set.iter().max())
            .cloned()
            .unwrap_or(0);
        let last_id = match self.status {
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
        };
        let task_id = last_id + 1;
        self.register_task(task_id, task).await?;
        self.status = match self.status {
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

    async fn register_task(&mut self, task_id: u32, task: Task<P>) -> anyhow::Result<()> {
        assert_eq!(task.status, TaskStatus::Pending);
        let prev = self.task_table.insert(task_id, task.clone());
        assert!(prev.is_none());

        self.user_table
            .entry(task.user_id.clone())
            .or_default()
            .push(task_id);

        self.conn
            .hset_multiple(
                format!("task:{}", task_id),
                &[
                    ("user-id", &task.user_id),
                    ("preset", &to_string(&task.preset).unwrap()),
                    ("status", &to_string(&task.status).unwrap()),
                ],
            )
            .await?;
        Ok(())
    }
}

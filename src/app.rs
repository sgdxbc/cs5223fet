use futures::prelude::*;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::{select, spawn};
use warp::ws::WebSocket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStatus {
    Disconnected(u32, u32), // next pending task id, last task id
    Running(u32, u32),      // running task id, last task id
    StandBy(u32),           // last task id
}

impl Display for AppStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected(_, _) => write!(f, "disconnected"),
            Self::Running(task_id, _) => write!(f, "running(#{})", task_id),
            Self::StandBy(_) => write!(f, "stand by"),
        }
    }
}

pub struct App {
    pub status: AppStatus,
    worker_tx: Option<mpsc::Sender<()>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            status: AppStatus::Disconnected(1, 0), // TODO
            worker_tx: None,
        }
    }

    pub async fn connect_worker(app0: Arc<Mutex<Self>>, mut websocket: WebSocket) {
        let mut app = app0.lock().await;
        if app.worker_tx.is_some() {
            unimplemented!("multiple worker");
        }

        let (worker_tx, mut worker_rx) = mpsc::channel(1);
        app.worker_tx = Some(worker_tx);
        app.status = if let AppStatus::Disconnected(pending_id, last_id) = app.status {
            if pending_id <= last_id {
                app.send_task(pending_id).await;
                AppStatus::Running(pending_id, last_id)
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
                    Some(_) = worker_rx.recv() => {
                        // TODO send task
                    }
                    Some(Ok(message)) = websocket.next() => {
                        if message.is_close() {
                            break;
                        }
                        // TODO parse message
                        app0.lock().await.finish_task().await;
                    }
                    else => break
                }
            }
            websocket.close().await.unwrap();
            app0.lock().await.disconnect_worker();
        });
    }

    async fn send_task(&mut self, pending_id: u32) {
        self.worker_tx.as_ref().unwrap().send(()).await.unwrap();
    }

    async fn finish_task(&mut self) {
        // TODO assert finished current pending task
        self.send_task(1).await;
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

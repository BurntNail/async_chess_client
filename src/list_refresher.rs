use crate::{
    error_ext::{ErrorExt, ToAnyhowDebug, ToAnyhowDisplay},
    server_interface::{Board, JSONMove, JSONPieceList},
    sync_ext::RwLockExt,
    time_based_structs::{DoOnInterval, ScopedTimer},
};
use anyhow::{Context as _, Result};
use reqwest::Error as RError;
use reqwest::{
    blocking::{ClientBuilder, Response},
    StatusCode,
};
use std::{
    sync::{
        mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
        Arc, Mutex, RwLock,
    },
    thread::JoinHandle,
    time::{Duration},
};

//TODO: More flexible calling API - eg. give an endpoint and an either
//TODO: Local move, until server refresh for movepiece

#[derive(Debug, PartialEq, Eq)]
pub enum MessageToWorker {
    UpdateList,
    UpdateNOW,
    RestartBoard,
    InvalidateKill,
    MakeMove(JSONMove),
}

#[derive(Debug)]
pub enum MessageToGame {
    Response(Result<Response, RError>),
}

pub struct ListRefresher {
    handle: Option<JoinHandle<()>>,
    tx: Sender<MessageToWorker>,
    rx: Receiver<MessageToGame>,
}

enum UpdateAction {
    NewList(Board),
    ReqwestError(reqwest::Error),
    UseExisting(Option<reqwest::Error>),
}

fn run_loop(
    mtw_rx: Receiver<MessageToWorker>,
    mtg_tx: Sender<MessageToGame>,
    id: u32,
    cached_pieces: Arc<RwLock<Board>>,
) -> Result<()> {
    let inflight = Mutex::new(());
    let client = ClientBuilder::default()
        .user_agent("JackyBoi/AsyncChess")
        .build()
        .context("building client")
        .unwrap_log_error();

    let mut refresh_timer = DoOnInterval::new(Duration::from_millis(500));
    let mut reqwest_error_at_last_refresh = false;

    while let Ok(msg) = mtw_rx.recv() {
        let _lock = inflight
            .lock()
            .to_ae_display()
            .context("locking inflight mutex")?;

        match msg {
            MessageToWorker::UpdateList | MessageToWorker::UpdateNOW => {
                let _doiu = {
                    if msg == MessageToWorker::UpdateNOW {
                        continue;
                    }
                    let doiu = refresh_timer.can_do();
                    if let Some(doiu) = doiu {
                        doiu
                    } else {
                        continue;
                    }
                };

                let _st = ScopedTimer::new("Updating List");

                let result_rsp = client
                    .get(format!("http://109.74.205.63:12345/games/{}", id))
                    .send();

                let list = match result_rsp {
                    Ok(rsp) => {
                        let rsp = rsp.error_for_status()?;
                        reqwest_error_at_last_refresh = false;

                        if rsp.status() == StatusCode::ALREADY_REPORTED {
                            UpdateAction::UseExisting(None)
                        } else {
                            UpdateAction::NewList(rsp.json::<JSONPieceList>()?.into_game_list()?)
                        }
                    }
                    Err(e) => {
                        if reqwest_error_at_last_refresh {
                            UpdateAction::UseExisting(Some(e))
                        } else {
                            reqwest_error_at_last_refresh = true;
                            UpdateAction::ReqwestError(e)
                        }
                    }
                };

                match cached_pieces.write() {
                    Ok(mut lock) => match list {
                        UpdateAction::NewList(nl) => {
                            *lock = nl;
                        }
                        UpdateAction::ReqwestError(e) => {
                            *lock = JSONPieceList::no_connection_list();
                            error!(%e, "Error for reqwest");
                        }
                        UpdateAction::UseExisting(e) => {
                            if let Some(e) = e {
                                warn!(%e, "Using existing list");
                            }
                        }
                    },
                    Err(e) => {
                        bail!("Unable to populate due to posion error: {e}");
                    }
                }
            }
            MessageToWorker::RestartBoard => {
                match client
                    .post("http://109.74.205.63:12345/newgame")
                    .body(id.to_string())
                    .send()
                {
                    Ok(rsp) => match rsp.error_for_status() {
                        Ok(rsp) => info!(update=?rsp.text(), "Update from server on restarting"),
                        Err(e) => warn!(%e, "Error code from server on restarting"),
                    },
                    Err(e) => error!(%e, "Error restarting"),
                }
            }
            MessageToWorker::MakeMove(m) => {
                let rsp = client
                    .post("http://109.74.205.63:12345/movepiece")
                    .json(&m)
                    .send();

                if let Ok(mut lock) = cached_pieces.write_timeout(Duration::from_millis(50)) {
                    lock[(m.ny * 8 + m.nx) as usize] = lock[(m.y * 8 + m.x) as usize];
                    lock[(m.y * 8 + m.x) as usize] = None;
                }

                mtg_tx
                    .send(MessageToGame::Response(rsp))
                    .context("sending msg to game re moving piece")
                    .warn();
            }
            MessageToWorker::InvalidateKill => {
                info!("InvalidateKill msg sent");

                match client
                    .post("http://109.74.205.63:12345/invalidate")
                    .body(id.to_string())
                    .send()
                {
                    Ok(rsp) => match rsp.error_for_status() {
                        Ok(rsp) => info!(update=?rsp.text(), "Update from server on invalidating"),
                        Err(e) => warn!(%e, "Error code from server on invalidating"),
                    },
                    Err(e) => error!(%e, "Error invalidating"),
                }

                info!("Ending refresher");
                break;
            }
        }
    }

    Ok(())
}

impl ListRefresher {
    pub fn new(cached_pieces: Arc<RwLock<Board>>, id: u32) -> Self {
        let (mtw_tx, mtw_rx) = channel();
        let (mtg_tx, mtg_rx) = channel();

        let thread = std::thread::spawn(move || {
            run_loop(mtw_rx, mtg_tx, id, cached_pieces)
                .context("error running refresh loop")
                .error();
        });

        Self {
            handle: Some(thread),
            tx: mtw_tx,
            rx: mtg_rx,
        }
    }

    pub fn send_msg(&self, m: MessageToWorker) -> Result<(), SendError<MessageToWorker>> {
        self.tx.send(m)
    }
    pub fn try_recv(&self) -> Result<MessageToGame, TryRecvError> {
        self.rx.try_recv()
    }
}

impl Drop for ListRefresher {
    fn drop(&mut self) {
        if let Some(h) = std::mem::take(&mut self.handle) {
            h.join()
                .to_ae_debug()
                .context("joining refresher handle")
                .error_exit();
        }
    }
}

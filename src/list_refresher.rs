use crate::{
    eyre,
    server_interface::{Board, JSONMove, JSONPieceList},
    time_based_structs::{DoOnInterval, ScopedTimer},
};
use color_eyre::Report;
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
    time::Duration,
};
use reqwest::Error as RError;

//TODO: More flexible calling API - eg. give an endpoint and an either
//TODO: Local move, until server refresh for movepiece

#[derive(Debug)]
pub enum MessageToWorker {
    UpdateList,
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

fn run_loop (mtw_rx: Receiver<MessageToWorker>, mtg_tx: Sender<MessageToGame>, id: u32, cached_pieces: Arc<RwLock<Board>>) -> Result<(), Report> {
    let inflight = Mutex::new(());
            let client = ClientBuilder::default()
                .user_agent("JackyBoi/AsyncChess")
                .build().expect("Unable to build client due to async runtime");
            let mut refresh_timer = DoOnInterval::new(Duration::from_millis(250));
            let mut reqwest_error_at_last_refresh = false;
    
    loop {
        if let Ok(msg) = mtw_rx.try_recv() {
            let _ = inflight.lock().expect("Unable to unlock IF mutex");

            match msg {
                MessageToWorker::UpdateList => {
                    if !refresh_timer.do_check() {
                        continue;
                    }
                    info!("Passed timer, refreshing");
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
                                UpdateAction::NewList(
                                    rsp.json::<JSONPieceList>()?.into_game_list()?,
                                )
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
                            UpdateAction::UseExisting(e) => match e {
                                Some(e) => {
                                    warn!(%e, "Using existing list");
                                }
                                _ => {}
                            },
                        },
                        Err(e) => {
                            break Err(eyre!(
                                "Unable to populate due to posion error: {e}"
                            ));
                        }
                    }
                }
                MessageToWorker::RestartBoard => {
                    match client
                    .post("http://109.74.205.63:12345/newgame")
                    .body(id.to_string()).send() {
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
                    
                    if let Err(e) = mtg_tx.send(MessageToGame::Response(rsp)) {
                        warn!(%e, "Error sending message to game re moving piece");
                    }
                }
                MessageToWorker::InvalidateKill => {
                    match client
                    .post("http://109.74.205.63:12345/invalidate")
                    .body(id.to_string()).send() {
                        Ok(rsp) => match rsp.error_for_status() {
                            Ok(rsp) => info!(update=?rsp.text(), "Update from server on invalidating"),
                            Err(e) => warn!(%e, "Error code from server on invalidating"),
                        },
                        Err(e) => error!(%e, "Error invalidating"),
                    }     

                    info!("Ending refresher");
                    break Ok(());
                }
            }
        }
    }
}

impl ListRefresher {
    pub fn new(cached_pieces: Arc<RwLock<Board>>, id: u32) -> Result<Self, Report> {
        let (mtw_tx, mtw_rx) = channel();
        let (mtg_tx, mtg_rx) = channel();

        let thread = std::thread::spawn(move || {
            if let Err(e) = run_loop(mtw_rx, mtg_tx, id, cached_pieces) {
                error!(%e, "Error in refresh loop");
            }
        });

        Ok(Self {
            handle: Some(thread),
            tx: mtw_tx,
            rx: mtg_rx,
        })
    }

    pub fn send_msg(&self, m: MessageToWorker) -> Result<(), SendError<MessageToWorker>> {
        self.tx.send(m)
    }
    pub fn try_recv (&self) -> Result<MessageToGame, TryRecvError> {
        self.rx.try_recv()
    }
}

impl Drop for ListRefresher {
    fn drop(&mut self) {
        if let Some(h) = std::mem::take(&mut self.handle) {
            h.join().expect("Unable to join handle");
        }
    }
}

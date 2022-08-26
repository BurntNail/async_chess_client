use crate::{
    error_ext::{ErrorExt, ToAnyhowPoisonErr, ToAnyhowThreadErr},
    server_interface::{JSONMove, JSONPieceList},
    time_based_structs::{DoOnInterval, MemoryTimedCacher, ThreadSafeScopedToListTimer},
};
use anyhow::{Context as _, Result};
use reqwest::{blocking::ClientBuilder, StatusCode};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

//TODO: More flexible calling API - eg. give an endpoint and an either

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
    UpdateBoard(BoardMessage),
}

#[derive(Debug)]
pub enum BoardMessage {
    TmpMove(JSONMove),
    Move(MoveOutcome),
    UseExisting,
    NoConnectionList,
    ///Make sure to have 65 elements
    NewList(JSONPieceList),
}

#[derive(Debug)]
pub enum MoveOutcome {
    Worked,
    Invalid,
    ReqwestFailed,
}

pub struct ListRefresher {
    handle: Option<JoinHandle<()>>,
    tx: Sender<MessageToWorker>,
    rx: Receiver<MessageToGame>,
}

fn run_loop(
    mtw_rx: Receiver<MessageToWorker>,
    mtg_tx: Sender<MessageToGame>,
    id: u32,
) -> Result<()> {
    let inflight = Arc::new(Mutex::new(()));
    let client = ClientBuilder::default()
        .user_agent("JackyBoi/AsyncChess")
        .build()
        .context("building client")
        .unwrap_log_error();
    let mut handles: Vec<JoinHandle<Result<()>>> = vec![]; //technically could be an option but easier for it to be a vec

    let mut refresh_timer = DoOnInterval::new(Duration::from_millis(500));
    let reqwest_error_at_last_refresh = Arc::new(AtomicBool::new(false));

    let request_timer = Arc::new(Mutex::new(MemoryTimedCacher::<_, 150>::new(None)));
    let mut request_print_timer = DoOnInterval::new(Duration::from_millis(2500));

    while let Ok(msg) = mtw_rx.recv() {
        {
            let rt = request_timer.clone();
            let lock = rt
                .lock()
                .ae()
                .context("unlocking mtc mutex")
                .unwrap_log_error();
            if let Some(_doiu) = request_print_timer.can_do() {
                let avg_ttr = lock.average_u32();
                info!(?avg_ttr, "Average time for response");
            }
        }

        {
            let mut finished_indicies = vec![];
            for (index, handle) in handles.iter().enumerate() {
                if handle.is_finished() {
                    finished_indicies.push(index - finished_indicies.len()); //to account for removing indicies and making the vec smaller
                }
            }

            for index in finished_indicies {
                let handle = handles.remove(index);
                handle
                    .join()
                    .ae()
                    .context("error joining handle")?
                    .context("error from handle")?;
            }
        }

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

                let (inflight, reqwest_error_at_last_refresh, mtg_tx, client, request_timer) = (
                    inflight.clone(),
                    reqwest_error_at_last_refresh.clone(),
                    mtg_tx.clone(),
                    client.clone(),
                    request_timer.clone(),
                );
                handles.push(std::thread::spawn(move || {
                    let _lock = inflight
                        .lock()
                        .ae()
                        .context("locking inflight mutex")
                        .unwrap_log_error();

                    let _st = ThreadSafeScopedToListTimer::new(request_timer);

                    let result_rsp = client
                        .get(format!("http://109.74.205.63:12345/games/{}", id))
                        .send();

                    let msg = match result_rsp {
                        Ok(rsp) => {
                            let rsp = rsp.error_for_status()?;
                            reqwest_error_at_last_refresh.store(false, Ordering::SeqCst);

                            if rsp.status() == StatusCode::ALREADY_REPORTED {
                                BoardMessage::UseExisting
                            } else {
                                BoardMessage::NewList(rsp.json::<JSONPieceList>()?)
                            }
                        }
                        Err(e) => {
                            if reqwest_error_at_last_refresh.load(Ordering::SeqCst) {
                                warn!(%e, "Using existing list due to errors");
                                BoardMessage::UseExisting
                            } else {
                                reqwest_error_at_last_refresh.store(true, Ordering::SeqCst);
                                error!(%e, "Error refreshing list - sending NCL");
                                BoardMessage::NoConnectionList
                            }
                        }
                    };

                    mtg_tx
                        .send(MessageToGame::UpdateBoard(msg))
                        .context("sending update list msg")
                        .error();

                    Ok(())
                }));
            }
            MessageToWorker::RestartBoard => {
                let (client, rt) = (client.clone(), request_timer.clone());
                //not added to the handles list because I don't care about the results
                std::thread::spawn(move || {
                    let _st = ThreadSafeScopedToListTimer::new(rt);

                    match client
                        .post("http://109.74.205.63:12345/newgame")
                        .body(id.to_string())
                        .send()
                    {
                        Ok(rsp) => match rsp.error_for_status() {
                            Ok(rsp) => {
                                info!(update=?rsp.text(), "Update from server on restarting")
                            }
                            Err(e) => warn!(%e, "Error code from server on restarting"),
                        },
                        Err(e) => error!(%e, "Error restarting"),
                    }
                });
            }
            MessageToWorker::MakeMove(m) => {
                let (mtg_tx, client, rt) = (mtg_tx.clone(), client.clone(), request_timer.clone());
                handles.push(std::thread::spawn(move || {
                    let _st = ThreadSafeScopedToListTimer::new(rt);

                    mtg_tx
                        .send(MessageToGame::UpdateBoard(BoardMessage::TmpMove(m)))
                        .context("sending msg to game re moving piece temp")
                        .warn();

                    let rsp = client
                        .post("http://109.74.205.63:12345/movepiece")
                        .json(&m)
                        .send();

                    let outcome = match rsp {
                        Ok(rsp) => match rsp.error_for_status() {
                            Ok(rsp) => {
                                info!(update=?rsp.text(), "Update from server on moving");
                                MoveOutcome::Worked
                            }
                            Err(e) => {
                                if let Some(sc) = e.status() {
                                    if sc == StatusCode::PRECONDITION_FAILED {
                                        error!("Invalid move");
                                        MoveOutcome::Invalid
                                    } else {
                                        error!(%e, %sc, "Error in input response status code");
                                        MoveOutcome::ReqwestFailed
                                    }
                                } else {
                                    MoveOutcome::ReqwestFailed
                                }
                            }
                        },
                        Err(e) => {
                            error!(%e, "Error in input response");
                            MoveOutcome::ReqwestFailed
                        }
                    };

                    mtg_tx
                        .send(MessageToGame::UpdateBoard(BoardMessage::Move(outcome)))
                        .context("piece move result")
                        .warn();

                    Ok(())
                }));
            }
            MessageToWorker::InvalidateKill => {
                let (client, rt) = (client, request_timer);
                std::thread::spawn(move || {
                    info!("InvalidateKill msg sent");
                    let _st = ThreadSafeScopedToListTimer::new(rt);

                    match client
                        .post("http://109.74.205.63:12345/invalidate")
                        .body(id.to_string())
                        .send()
                    {
                        Ok(rsp) => match rsp.error_for_status() {
                            Ok(rsp) => {
                                info!(update=?rsp.text(), "Update from server on invalidating")
                            }
                            Err(e) => warn!(%e, "Error code from server on invalidating"),
                        },
                        Err(e) => error!(%e, "Error invalidating"),
                    }

                    info!("Ending refresher");
                });
                break;
            }
        }
    }

    Ok(())
}

impl ListRefresher {
    pub fn new(id: u32) -> Self {
        let (mtw_tx, mtw_rx) = channel();
        let (mtg_tx, mtg_rx) = channel();

        let thread = std::thread::spawn(move || {
            run_loop(mtw_rx, mtg_tx, id)
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
                .ae()
                .context("ending list refresher")
                .unwrap_log_error();
        }
    }
}

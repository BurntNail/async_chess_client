use crate::{
    either::Either,
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

///Enum for sending a message to the worker
#[derive(Debug, PartialEq, Eq)]
pub enum MessageToWorker {
    ///Ask the server if the list has changed, if the [`DoOnInterval`] allows so
    UpdateList,
    ///Ask the server if the list has changed, and reset the [`DoOnInterval`]
    UpdateNOW,
    ///Ask the server to clear the board for a new game
    RestartBoard,
    ///Ask the server to invalidate all caches for that game
    InvalidateKill,
    ///Ask the server to make a move
    MakeMove(JSONMove),
}

///Enum for sending a message back to the game
#[derive(Debug)]
pub enum MessageToGame {
    ///Update the board
    UpdateBoard(BoardMessage),
}

///Enum for messages to the game, relating to the board
#[derive(Debug)]
pub enum BoardMessage {
    ///This move has been approved by the client, but not the server, but move it anyway to reduce perception of internet speed
    TmpMove(JSONMove),
    ///Response from the server on a move made
    Move(MoveOutcome),
    ///The board hasn't changed since the last update
    UseExisting,
    ///No connection - use the [`no_connection_list`]
    NoConnectionList,
    ///The board has changed, use all of these pieces
    NewList(JSONPieceList),
}

///The outcome of a move from the server
#[derive(Debug)]
pub enum MoveOutcome {
    ///The move worked and was successful
    Worked,
    ///The move is invalid, and should be undone
    Invalid,
    ///The request from `reqwest` failed
    ReqwestFailed,
}

///Struct to refresh the board and deal with requests to the server, using multi-threading and channels
pub struct ListRefresher {
    ///Handle to hold the main thread.
    ///
    ///It is an `Option` because that makes it ownable for [`Drop::drop`] using [`std::mem::take`] as you need to own a [`JoinHandle`] to [`JoinHandle::join`] it to receive any errors.
    handle: Option<JoinHandle<()>>,
    ///Sender to send messages to the main thread
    tx: Sender<MessageToWorker>,
    ///Receiver for messages sent from the main thread to send them to the game.
    rx: Receiver<MessageToGame>,
}

///Run the loop - this should be called from a new thread as it blocks heavily until the [`Receiver`] is closed
///
/// # Errors
/// Can return an error if the board is upating and the response cannot be marshalled into [`JSONPieceList`] or if there are errors joining threads.
///
/// NB: Threads can still be running when this function ends so be careful about the receiver
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
                        None //continue regardless
                    } else {
                        let doiu = refresh_timer.can_do();
                        if let Some(doiu) = doiu {
                            Some(doiu) //doi says we can
                        } else {
                            continue; //next in loop
                        }
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
                        .get(format!("http://109.74.205.63:12345/games/{id}"))
                        .send();

                    let msg = match result_rsp {
                        Ok(rsp) => {
                            let rsp = rsp.error_for_status();
                            match rsp {
                                Ok(rsp) => {
                                    reqwest_error_at_last_refresh.store(false, Ordering::SeqCst);

                                    if rsp.status() == StatusCode::ALREADY_REPORTED {
                                        Either::Left(BoardMessage::UseExisting)
                                    } else {
                                        Either::Left(BoardMessage::NewList(
                                            rsp.json::<JSONPieceList>()?,
                                        ))
                                    }
                                }
                                Err(e) => {
                                    warn!(%e, "Error updating list");

                                    Either::Right(e)
                                }
                            }
                        }
                        Err(e) => Either::Right(e),
                    };

                    let msg = match msg {
                        Either::Left(m) => m,
                        Either::Right(e) => {
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
                                info!(update=?rsp.text(), "Update from server on restarting");
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
                info!("InvalidateKill msg sending");

                let rsp = client
                    .post("http://109.74.205.63:12345/invalidate")
                    .body(id.to_string())
                    .send();

                match rsp {
                    Ok(rsp) => match rsp.error_for_status() {
                        Ok(rsp) => {
                            info!(update=?rsp.text(), "Update from server on invalidating");
                        }
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
    ///Create a new `ListRefresher`, and start up the main thread
    #[must_use]
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

    ///Sends a message to the main thread
    ///
    /// # Errors
    /// Can error if there is an error sending the message
    pub fn send_msg(&self, m: MessageToWorker) -> Result<(), SendError<MessageToWorker>> {
        self.tx.send(m)
    }
    ///Tries to receive a message from the main thread in a non-blocking fashion
    ///
    /// # Errors
    /// - There is no message
    /// - The sender has been closed
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

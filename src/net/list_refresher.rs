use anyhow::{Context as _, Result};
use reqwest::{
    blocking::{Client, ClientBuilder},
    StatusCode,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};
use epac_utils::either::Either;
use epac_utils::error_ext::{ErrorExt, MutexExt, ToAnyhowThreadErr};
use epac_utils::time_based_structs::do_on_interval::DoOnInterval;
use epac_utils::time_based_structs::memcache::MemoryTimedCacher;
use epac_utils::time_based_structs::scoped_timers::ThreadSafeScopedToListTimer;

use crate::{
    prelude::{DoOnInterval, Either, ErrorExt},
    util::{
        error_ext::{MutexExt, ToAnyhowThreadErr},
        time_based_structs::{
            memcache::MemoryTimedCacher, scoped_timers::ThreadSafeScopedToListTimer,
        },
    },
};

use super::server_interface::{JSONMove, JSONPieceList};

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
    ///No connection - use the [`crate::server_interface::no_connection_list`]
    NoConnectionList,
    ///The board has changed, use all of these pieces
    NewList(JSONPieceList),
}

///The outcome of a move from the server
#[derive(Debug)]
pub enum MoveOutcome {
    ///The move worked and was successful. Bool signifies whether or not a piece was taken
    Worked(bool),
    ///The move is invalid, and should be undone
    Invalid,
    ///The request from `reqwest` failed
    CouldntProcessMove,
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
    let update_req_inflight = Arc::new(AtomicBool::new(false));
    let move_req_inflight = Arc::new(AtomicBool::new(false));

    let client = ClientBuilder::default()
        .user_agent("JackyBoi/AsyncChess")
        .build()
        .context("building client")
        .unwrap_log_error();
    let mut handles: Vec<JoinHandle<Result<()>>> = vec![]; //technically could be an option but easier for it to be a vec

    let refresh_timer = Arc::new(Mutex::new(DoOnInterval::new(Duration::from_millis(500)))); //timer for updating board
    let reqwest_error_at_last_refresh = Arc::new(AtomicBool::new(false));

    let request_timer = Arc::new(Mutex::new(MemoryTimedCacher::<_, 150>::new(None))); //cacher for printing av requests ttr
    let mut request_print_timer = DoOnInterval::new(Duration::from_millis(2500)); //timer for when to print av request ttr

    while let Ok(msg) = mtw_rx.recv() {
        {
            let rt = request_timer.clone();
            let lock = rt.lock_panic("unlocking mtc mutex");

            if let Some(_doiu) = request_print_timer.get_updater() {
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
                let can = if msg == MessageToWorker::UpdateNOW {
                    true
                } else {
                    refresh_timer.lock_panic("refresh timer").can_do()
                };
                if !can {
                    continue;
                }

                let (
                    update_req_inflight,
                    reqwest_error_at_last_refresh,
                    mtg_tx,
                    client,
                    request_timer,
                    refresh_timer,
                ) = (
                    update_req_inflight.clone(),
                    reqwest_error_at_last_refresh.clone(),
                    mtg_tx.clone(),
                    client.clone(),
                    request_timer.clone(),
                    refresh_timer.clone(),
                );

                std::thread::spawn(move || {
                    if !update_req_inflight.load(Ordering::SeqCst) {
                        update_req_inflight.store(true, Ordering::SeqCst);
                        let _st = ThreadSafeScopedToListTimer::new(request_timer);

                        do_update_list(id, reqwest_error_at_last_refresh, mtg_tx, client);

                        update_req_inflight.store(false, Ordering::SeqCst);
                        refresh_timer.lock_panic("refresh timer").update_timer();
                    }
                });
            }
            MessageToWorker::RestartBoard => {
                let (client, rt) = (client.clone(), request_timer.clone());
                //not added to the handles list because I don't care about the results
                std::thread::spawn(move || {
                    let _st = ThreadSafeScopedToListTimer::new(rt);
                    do_restart_board(id, client);
                });
            }
            MessageToWorker::MakeMove(m) => {
                let (mtg_tx, client, rt, mr_inflight) = (
                    mtg_tx.clone(),
                    client.clone(),
                    request_timer.clone(),
                    move_req_inflight.clone(),
                );
                std::thread::spawn(move || {
                    if mr_inflight.load(Ordering::SeqCst) {
                        mtg_tx
                            .send(MessageToGame::UpdateBoard(BoardMessage::Move(
                                MoveOutcome::CouldntProcessMove,
                            )))
                            .context("piece move result")
                            .warn();
                    } else {
                        mr_inflight.store(true, Ordering::SeqCst);

                        let _st = ThreadSafeScopedToListTimer::new(rt);
                        do_make_move(m, mtg_tx, client);

                        mr_inflight.store(false, Ordering::SeqCst);
                    }
                });
            }
            MessageToWorker::InvalidateKill => {
                do_invalidate_exit(id, client);
                break;
            }
        }

        //NB: Can have no logic here as there are continue statements
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

///Function to be run on a separate thread to update the list and send a message to a [`Sender`]
fn do_update_list(
    id: u32,
    reqwest_error_at_last_refresh: Arc<AtomicBool>,
    mtg_tx: Sender<MessageToGame>,
    client: Client,
) {
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
                        match rsp.json::<JSONPieceList>() {
                            Ok(l) => Either::Left(BoardMessage::NewList(l)),
                            Err(e) => {
                                error!(%e, "Unable to parse JSON list from reqwest");
                                Either::Right(e)
                            }
                        }
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
}

///Utility function to be run on a separate thread to restart the board
fn do_restart_board(id: u32, client: Client) {
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
}

///Utility function to be run on a separate thread to make a move.
///
/// NB: Make sure not to call this method again until it has finished
fn do_make_move(m: JSONMove, mtg_tx: Sender<MessageToGame>, client: Client) {
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
                let txt = rsp.text();
                info!(update=?txt, "Update from server on moving");
                let taken = txt.map_or(false, |txt| !txt.contains("not"));
                MoveOutcome::Worked(taken)
            }
            Err(e) => {
                if let Some(sc) = e.status() {
                    if sc == StatusCode::PRECONDITION_FAILED {
                        error!("Invalid move");
                        MoveOutcome::Invalid
                    } else {
                        error!(%e, %sc, "Error in input response status code");
                        MoveOutcome::CouldntProcessMove
                    }
                } else {
                    MoveOutcome::CouldntProcessMove
                }
            }
        },
        Err(e) => {
            error!(%e, "Error in input response");
            MoveOutcome::CouldntProcessMove
        }
    };

    mtg_tx
        .send(MessageToGame::UpdateBoard(BoardMessage::Move(outcome)))
        .context("piece move result")
        .warn();
}

///Utility function to send the invalidate-kill message
fn do_invalidate_exit(id: u32, client: Client) {
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

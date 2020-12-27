extern crate pretty_env_logger;
#[macro_use]
extern crate log;

pub mod ws_messages;
use ws_messages::*;

use futures::{FutureExt, StreamExt};
use std::sync::Arc;
use std::{collections::HashMap, convert::Infallible};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use warp::ws::{Message, WebSocket};
use warp::Filter;

pub struct User {
    pub id: String,
    pub sender: mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>,
}
type Users = Arc<RwLock<HashMap<String, User>>>;

pub struct ServerUser {
    pub user: User,
    pub peers: Users,
}
type ServerUsers = Arc<RwLock<HashMap<String, ServerUser>>>;

const PEEROLATOR_ID: &str = "0";

fn main() -> anyhow::Result<()> {
    // TODO: Add command-line parameters
    pretty_env_logger::init();

    tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { run().await })
}

async fn run() -> anyhow::Result<()> {
    let server_users = ServerUsers::default();

    // FIXME: Validate user/password if enabled
    let server_route = warp::path("server")
        .and(warp::ws())
        .and(warp::path::end())
        .and(
            warp::header("x-forwarded-host")
                .or(warp::header("host"))
                .unify(),
        )
        .and(with_users(server_users.clone()))
        .map(|ws: warp::ws::Ws, host: String, server_users| {
            ws.on_upgrade(move |socket| server_connected(socket, host, server_users))
        });

    let client_route = warp::path("client")
        .and(warp::ws())
        .and(warp::path::param())
        .and(warp::path::end())
        .and(with_users(server_users.clone()))
        .map(|ws: warp::ws::Ws, params: String, server_users| {
            ws.on_upgrade(move |socket| client_connected(socket, params, server_users))
        });

    let routes = server_route
        .or(client_route)
        .with(warp::cors().allow_any_origin());

    // FIXME: Get address and port from config
    trace!("Starting the warp server...");
    // FIXME: Disable HTTP Keep-Alive (no reason for a client to make more than one request, reduces DoS)
    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;

    Ok(())
}

async fn server_connected(ws: WebSocket, host: String, server_users: ServerUsers) {
    info!("New server connection at {}", host);

    let id = generate_server_id();
    debug!("server id: {}", id);

    // Split the socket into a sender and receive of messages.
    let (user_ws_tx, mut user_ws_rx) = ws.split();

    // Use an unbounded channel to handle buffering and flushing of messages to the websocket...
    // FIXME: Use bounded channel to avoid memory overuse. There's no reason to queue too many messages here.
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::task::spawn(rx.forward(user_ws_tx).map(|result| {
        if let Err(e) = result {
            error!("websocket send error: {}", e);
        }
    }));

    let user = User {
        id: id.clone(),
        sender: tx,
    };

    // Send a HELLO message to the server
    {
        let hello = HelloMessage { id: id.clone() };
        send_message(
            String::from(PEEROLATOR_ID),
            String::from("HELLO"),
            serde_json::to_value(hello).unwrap(),
            &user,
        )
        .await;
    }

    {
        // Save the server in our list of connected servers.
        let server = ServerUser {
            user: user,
            peers: Users::default(),
        };
        server_users.write().await.insert(id.clone(), server);
    }

    // Every time the server sends a message, send it to the peer
    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                error!("websocket error(uid={}): {}", id.clone(), e);
                break;
            }
        };

        let msg = if let Ok(s) = msg.to_str() {
            s
        } else {
            debug!("invalid message type");
            break;
        };

        let deserialized: PeerMessage = match serde_json::from_str(&msg) {
            Ok(msg) => msg,
            Err(e) => {
                debug!("message validation error(uid={}): {}", id.clone(), e);
                debug!("{:?}", msg);
                break;
            }
        };

        if deserialized.from != id {
            debug!("invalid message source");
            break;
        }

        {
            let server_read = server_users.read().await;
            match server_read.get(&id) {
                None => {}
                Some(server) => {
                    let client_read = server.peers.read().await;
                    match client_read.get(&deserialized.to) {
                        None => {
                            info!("Received a message for an unknown client");
                        } // Drop the message if the client has disconnected
                        Some(client) => {
                            send_message(
                                id.clone(),
                                deserialized.msg_type,
                                deserialized.message,
                                client,
                            )
                            .await;
                        }
                    }
                }
            }
        }
    }

    {
        // user_ws_rx stream will keep processing as long as the server stays
        // connected. Once it disconnects, then...
        debug!("server disconnected: {}", &id);
        let mut server_write = server_users.write().await;
        match server_write.remove(&id) {
            None => {}
            Some(server) => {
                drop(server_write);
                // Send a PEER_GONE message to all the peers to let them know
                let client_read = server.peers.read().await;
                for peer in client_read.values() {
                    let gone = PeerGoneMessage {
                        id: peer.id.clone(),
                    };
                    send_message(
                        String::from(PEEROLATOR_ID),
                        String::from("PEER_GONE"),
                        serde_json::to_value(gone).unwrap(),
                        peer,
                    )
                    .await;
                }
            }
        }
    }
}

async fn client_connected(ws: WebSocket, server_id: String, server_users: ServerUsers) {
    info!("New client connection with server id: {}", &server_id);

    // The client must request an existing server. If not, close the connection.
    // FIXME: This could probably be a filter.
    {
        if !server_users.read().await.contains_key(&server_id) {
            debug!("Unknown server id: {}", &server_id);
            let _ = ws.close().await;
            return;
        };
    }

    let id = generate_client_id();
    debug!("New client id: {}", &id);

    // Split the socket into a sender and receiver of messages.
    let (user_ws_tx, mut user_ws_rx) = ws.split();

    // Use an unbounded channel to handle buffering and flushing of messages to the websocket...
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::task::spawn(rx.forward(user_ws_tx).map(|result| {
        if let Err(e) = result {
            error!("websocket send error: {}", e);
        }
    }));

    let user = User {
        id: id.clone(),
        sender: tx,
    };

    // Send a HELLO message to the client
    {
        let hello = HelloMessage { id: id.clone() };
        send_message(
            String::from(PEEROLATOR_ID),
            String::from("HELLO"),
            serde_json::to_value(hello).unwrap(),
            &user,
        )
        .await;
    }

    // Save the user in the server list of connected clients.
    {
        let server_read = server_users.read().await;
        match server_read.get(&server_id) {
            None => {} // FIXME: Server already disconnected. We should move this code up to avoid thie situation
            Some(server_user) => {
                server_user.peers.write().await.insert(id.clone(), user);
            }
        }
    }

    // Send a message to the server indicating a new client
    {
        let server_read = server_users.read().await;
        match server_read.get(&server_id) {
            None => {}
            Some(server) => {
                let client_msg = PeerJoinedMessage { id: id.clone() };
                send_message(
                    String::from(PEEROLATOR_ID),
                    String::from("PEER_JOINED"),
                    serde_json::to_value(client_msg).unwrap(),
                    &server.user,
                )
                .await;
            }
        }
    }

    // Every time the user sends a message, send it to the peer
    // FIXME: Add timeout for client connections.
    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                error!("websocket error(uid={}): {}", id.clone(), e);
                break;
            }
        };

        let msg = if let Ok(s) = msg.to_str() {
            s
        } else {
            error!("invalid message type");
            break;
        };

        let deserialized: PeerMessage = match serde_json::from_str(&msg) {
            Ok(msg) => msg,
            Err(e) => {
                error!("message validation error(uid={}): {}", id.clone(), e);
                break;
            }
        };

        if deserialized.from != id {
            error!("invalid message source");
            break;
        }
        if deserialized.to != server_id {
            error!("invalid message destination");
            break;
        }

        {
            let server_read = server_users.read().await;
            match server_read.get(&server_id) {
                None => {} // Drop the message if the server just left
                Some(server) => {
                    send_message(
                        id.clone(),
                        deserialized.msg_type,
                        deserialized.message,
                        &server.user,
                    )
                    .await;
                }
            }
        }
    }

    {
        // user_ws_rx stream will keep processing as long as the peer stays
        // connected. Once it disconnects, then...
        let server_read = server_users.read().await;
        match server_read.get(&server_id) {
            None => {} // FIXME: Server already disconnected. We should move this code up to avoid thie situation
            Some(server_user) => {
                client_disconnected(id.clone(), &server_user.peers).await;
            }
        }
    }

    // Send a PEER_GONE message to the server to let it know
    {
        let server_read = server_users.read().await;
        match server_read.get(&server_id) {
            Some(server) => {
                let gone = PeerGoneMessage { id: id.clone() };
                send_message(
                    String::from(PEEROLATOR_ID),
                    String::from("PEER_GONE"),
                    serde_json::to_value(gone).unwrap(),
                    &server.user,
                )
                .await;
            }
            None => {}
        }
    }
}

async fn send_message(sender_id: String, msg_type: String, msg: serde_json::Value, peer: &User) {
    // Skip any non-Text messages...
    // FIXME: Close messages need to be handled explicitly: usually by closing the `Sink` end of the
    // `WebSocket`.
    let peer_msg = PeerMessage {
        to: peer.id.clone(),
        from: sender_id,
        msg_type,
        message: msg,
    };
    let serialized = serde_json::to_string(&peer_msg).unwrap();

    if let Err(_disconnected) = peer.sender.send(Ok(Message::text(serialized))) {
        // The tx is disconnected, our `client_disconnected` code
        // should be happening in another task, nothing more to
        // do here.
    }
}

async fn client_disconnected(my_id: String, users: &Users) {
    debug!("client disconnected: {}", my_id);

    // Stream closed up, so remove from the user list
    users.write().await.remove(&my_id);
}

fn with_users(
    server_users: ServerUsers,
) -> impl Filter<Extract = (ServerUsers,), Error = Infallible> + Clone {
    warp::any().map(move || server_users.clone())
}

/// Return a URL-friendly string that contains a new unique idenfitier
fn generate_server_id() -> String {
    // FIXME: For debugging only
    //String::from("1234567890")

    // TODO: Add option to generate human-readable string as a ID
    generate_id()
}

/// Return a string that contains a new unique idenfitier
fn generate_client_id() -> String {
    generate_id()
}

/// Return a URL-friendly string that contains a new unique idenfitier
fn generate_id() -> String {
    // FIXME: Improve the quality of the secret
    Uuid::new_v4().to_simple().to_string()
}

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
    let server_users = Users::default();
    let client_users = Users::default();

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
        .and(with_users(client_users.clone()))
        .map(
            |ws: warp::ws::Ws, host: String, server_users, client_users| {
                ws.on_upgrade(move |socket| {
                    server_connected(socket, host, server_users, client_users)
                })
            },
        );

    let client_route = warp::path("client")
        .and(warp::ws())
        .and(warp::path::param())
        .and(warp::path::end())
        .and(with_users(server_users.clone()))
        .and(with_users(client_users.clone()))
        .map(
            |ws: warp::ws::Ws, params: String, server_users, client_users| {
                ws.on_upgrade(move |socket| {
                    client_connected(socket, params, server_users, client_users)
                })
            },
        );

    let routes = server_route
        .or(client_route)
        .with(warp::cors().allow_any_origin());

    // FIXME: Get address and port from config
    trace!("Starting the warp server...");
    // FIXME: Disable HTTP Keep-Alive (no reason for a client to make more than one request, reduces DoS)
    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;

    Ok(())
}

async fn server_connected(ws: WebSocket, host: String, server_users: Users, client_users: Users) {
    info!("New server connection at {}", host);

    // FIXME: Debug !!!!!!!!!!!!
    let id = String::from("1234567890"); //generate_user_id();
    debug!("server id: {}", id);

    // Split the socket into a sender and receive of messages.
    let (user_ws_tx, mut user_ws_rx) = ws.split();

    // Use an unbounded channel to handle buffering and flushing of messages to the websocket...
    // FIXME: Bound channel
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
            String::from("0"),
            String::from("HELLO"),
            serde_json::to_value(hello).unwrap(),
            &user,
        )
        .await;
    }

    // Save the sender in our list of connected servers.
    server_users.write().await.insert(id.clone(), user);

    // Make an extra clone to give to our disconnection handler...
    let users2 = server_users.clone();

    // Every time the user sends a message, send it to the peer
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
                error!("{:?}", msg);
                break;
            }
        };

        if deserialized.from != id {
            error!("invalid message source");
            break;
        }

        if !client_users.read().await.contains_key(&deserialized.to) {
            error!("invalid message destination");
        };

        {
            let client_read = client_users.read().await;
            send_message(
                id.clone(),
                deserialized.msg_type,
                deserialized.message,
                client_read.get(&deserialized.to).unwrap(),
            )
            .await;
        }
    }

    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    user_disconnected(id, &users2).await;
}

async fn client_connected(ws: WebSocket, params: String, server_users: Users, client_users: Users) {
    info!("New client connection with params: {}", &params);

    // The client must request an existing server. If not, close the connection.
    // FIXME: This could probably be a filter.
    if !server_users.read().await.contains_key(&params) {
        warn!("Unknown server id: {}", params);
        let _ = ws.close().await;
        return;
    };

    let id = generate_user_id();
    debug!("client id: {}", id);

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
            String::from("0"),
            String::from("HELLO"),
            serde_json::to_value(hello).unwrap(),
            &user,
        )
        .await;
    }

    // Save the user in our list of connected clients.
    client_users.write().await.insert(id.clone(), user);

    // Make an extra clone to give to our disconnection handler...
    let users2 = client_users.clone();

    // Send a message to the server indicating a new client
    {
        let server_read = server_users.read().await;
        let client_msg = ClientConnectedMessage { id: id.clone() };
        send_message(
            String::from("0"),
            String::from("CLIENT_CONNECTION"),
            serde_json::to_value(client_msg).unwrap(),
            server_read.get(&params).unwrap(),
        )
        .await;
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
        if deserialized.to != params {
            error!("invalid message destination");
            break;
        }

        {
            let server_read = server_users.read().await;
            send_message(
                id.clone(),
                deserialized.msg_type,
                deserialized.message,
                server_read.get(&params).unwrap(),
            )
            .await;
        }
    }

    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    user_disconnected(id, &users2).await;

    // FIXME: Send a message to the peer to let it know
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
        // The tx is disconnected, our `user_disconnected` code
        // should be happening in another task, nothing more to
        // do here.
    }
}

async fn user_disconnected(my_id: String, users: &Users) {
    info!("user disconnected: {}", my_id);

    // Stream closed up, so remove from the user list
    users.write().await.remove(&my_id);
}

fn with_users(users: Users) -> impl Filter<Extract = (Users,), Error = Infallible> + Clone {
    warp::any().map(move || users.clone())
}

/// Return a URL-friendly string that contains a new unique idenfitier
fn generate_user_id() -> String {
    // FIXME: Improve the quality of the secret
    Uuid::new_v4().to_simple().to_string()
}

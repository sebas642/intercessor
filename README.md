# Intercessor

[![CI](https://github.com/sebas642/intercessor/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/sebas642/intercessor/actions/workflows/ci.yml)

## Peer Messaging Service

Intercessor is a relay server to exchange messages between two peers. Intercessor is initially designed as a signaling server for WebRTC. It is similar to SignalMaster.

A _server_ first connects to the Intercessor (the _relay_), and receives back a unique token that can be used by the _client(s)_ to connect to the Intercessor.

- Server connection: wss://somehost/server
- Client connection: wss://somehost/client/SERVER_ID

## Developing

To submit changes for review, push them to a branch in a fork and submit a pull request to merge that branch into main.

## Logging

Log level can be specified as an environmnt variable such as:

```console
RUST_LOG=trace cargo run
```

## Protocol

### Messsage Format

Peers must send messages in the following format:

```json
{
    "to": PEER_ID,
    "type": MESSAGE_TYPE,
    "message": MESSAGE
}
```

Messages sent from the relay have the following format:

```json
{
    "from": PEER_ID,
    "type": MESSAGE_TYPE,
    "message": MESSAGE
}
```

Where:

- PEER_ID (string): The unique identifier of the peer.
- MESSAGE_TYPE (string): The type of content of the message. Either 'HELLO', 'PEER_JOINED', "PEER_GONE", or application specific.
- MESSAGE (Object): The actual message content (HELLO, PEER_JOINED, PEER_GONE, or other message).

### HELLO message

When a peer connects to the Intercessor, it will receive a HELLO message with the following content:

```json
{
    "id": USER_ID,
    "serverInfo": SERVER_INFO
}
```

Where:

- USER_ID (string): The unique ID attributed to the peer.
- SERVER_INFO (string): Optional application specific information configured on the server.

### PEER_JOINED message

This message is sent to a server when a new client connects using its server ID.

```json
{
    "id": USER_ID
}
```

Where:

- USER_ID (string): The unique ID of the new client.

### PEER_GONE message

This message is sent to a peer when their peer leaves.

```json
{
    "id": USER_ID
}
```

Where:

- USER_ID (string): The unique ID of the peer that has left.

## Future

This project is a work in progress. Future work includes the following:

- SSL Support
- Command-line & file configuration
- Server extra info in HELLO message
- Passphrase for server logins
- Replace UUID with something stronger and/or easier to remember
- Client timeouts
- Apply optional (configurable) schema validation on messages sent by the peers
- Valgrind
- Docker image

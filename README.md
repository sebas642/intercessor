# Peerolator

## Peer messaging service

The Peerolator is a relay server to exchange messages between two peers. Peerolator is initially designed as a signaling server for WebRTC. It is similar to SignalMaster.

A *server* first connects to the Peerolator (the *relay*), and receives back a unique token that can be used by the *client(s)* to connect to the Peerolator. 

* Server connection: wss://www.peerolator.com/server
* Client connection: wss://www.peerolator.com/client/SERVER_ID

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

* PEER_ID (string): The unique identifier of the peer.
* MESSAGE_TYPE (string): The type of content of the message. Either 'HELLO', 'PEER_JOINED', "PEER_GONE", or application specific.
* MESSAGE (Object): The actual message content (HELLO, PEER_JOINED, PEER_GONE, or other message).

### HELLO message

When a peer connects to the Peerolator, it will receive a HELLO message with the following content:

```json
{
    "id": USER_ID,
    "serverInfo": SERVER_INFO
}
```

Where:

* USER_ID (string): The unique ID attributed to the peer.
* SERVER_INFO (string): Optional application specific information configured on the server.

### PEER_JOINED message

This message is sent to a server when a new client connects using its server ID.

```json
{
    "id": USER_ID
}
```

Where:

* USER_ID (string): The unique ID of the new client.

### PEER_GONE message

This message is sent to a peer when their peer leaves.

```json
{
    "id": USER_ID
}
```

Where:

* USER_ID (string): The unique ID of the peer that has left.

## Future

This project is a work in progress. Future work includes tht following:

* SSL Support
* Command-line & file configuration
* Server extra info in HELLO message
* Passphrase for server logins
* Replace UUID with something stronger and/or easier to remember
* Client timeouts
* Apply optional (configurable) schema validation on messages sent by the peers
* Valgrind
* Docker image

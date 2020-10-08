# Peerolator

A peer messaging service.

A relay server to exchange messages between two peers. Peerolator is initially designed as a signaling server for WebRTC (similar to SignalMaster).

A *server* first connects to the Peerolator and receives back a unique token that can be used by the *client(s)* to connect to the Peerolator. Each message is sent to a unique peer.

* Server connection: wss://www.peerolator.com/server
* Client connection: wss://www.peerolator.com/client/SERVER_ID

## Protocol

### Messsage Format

Clients must send messages in the following format:

```json
{
    "to": PEER_ID,
    "type": MESSAGE_TYPE,
    "message": MESSAGE
}
```

Messages sent or relayed from the server have the following format:

```json
{
    "from": PEER_ID,
    "type": MESSAGE_TYPE,
    "message": MESSAGE
}
```

Where:

* PEER_ID (string): The unique identifier of the peer.
* MESSAGE_TYPE (string): The type of content of the message. Either 'HELLO', 'CLIENT_CONNECTION', or application specific.
* MESSAGE (Object): The actual message content (HELLO, CLIENT_CONNECTION, or other message).

### HELLO message

When a client connects to the server, it will receive a HELLO message with the following content:

```json
{
    "id": USER_ID,
    "serverInfo": SERVER_INFO
}
```

Where:

* USER_ID (string): The unique ID attributed to the user.
* SERVER_INFO (string): Application specific information configured on the server.

### CLIENT_CONNECTION message

```json
{
    "id": USER_ID
}
```

Where:

* USER_ID (string): The unique ID of the new client.

## Future

* Command-line & file configuration
* Server extra info in HELLO message
* SSL Support
* Passphrase for server logins
* Replace UUID with something stronger and base64 encoded
* Client timeout
* Apply optional (configurable) schema validation on messages sent by the client
* Valgrind
* Docker image

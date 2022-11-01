use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct PeerMessage {
    pub from: String, // FIXME: Set max size
    pub to: String,   // FIXME: Set max size
    #[serde(rename = "type")]
    pub msg_type: String,

    // FIXME: Ideally this would be an enum with a default value
    // but serde(other) works only on unit variants
    pub message: Value,
}

/*
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type", content = "message")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageContent {
    Hello(HelloMessage),
    PeerJoined(PeerJoinedMessage),
    PeerGone(PeerGoneMessage),
    #[serde(other)]
    Other
}
*/

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct HelloMessage {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct PeerJoinedMessage {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct PeerGoneMessage {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_a_hello_message() {
        let msg_str = "{
            \"from\": \"source\", 
            \"to\": \"dest\",
            \"type\": \"HELLO\",
            \"message\": {
                \"id\": \"123456\"
            }
        }";

        let deserialized: PeerMessage = serde_json::from_str(msg_str).unwrap();
        assert_eq!(deserialized.from, "source");
        assert_eq!(deserialized.to, "dest");
        assert_eq!(deserialized.msg_type, "HELLO");

        let hello: HelloMessage = serde_json::from_value(deserialized.message).unwrap();
        assert_eq!(hello.id, "123456");
    }

    #[test]
    fn parsing_an_unknown_message() {
        let msg_str = "{
            \"from\": \"source\", 
            \"to\": \"dest\",
            \"type\": \"SOME_OTHER_TYPE\",
            \"message\": {
                \"id\": \"123456\"
            }
        }";

        let deserialized: PeerMessage = serde_json::from_str(msg_str).unwrap();
        assert_eq!(deserialized.from, "source");
        assert_eq!(deserialized.to, "dest");
        assert_eq!(deserialized.msg_type, "SOME_OTHER_TYPE");
        assert_eq!(deserialized.message["id"], "123456");
    }

    #[test]
    fn generating_a_hello_message() {
        let mut msg_str = String::from(
            "{
            \"from\": \"source\", 
            \"to\": \"dest\",
            \"type\": \"HELLO\",
            \"message\": {
                \"id\": \"123456\"
            }
        }",
        );

        let hello = HelloMessage {
            id: String::from("123456"),
        };
        let msg = PeerMessage {
            from: String::from("source"),
            to: String::from("dest"),
            msg_type: String::from("HELLO"),
            message: serde_json::to_value(hello).unwrap(),
        };

        let serialized = serde_json::to_string(&msg)
            .unwrap()
            .retain(|c| !c.is_whitespace());
        assert_eq!(serialized, msg_str.retain(|c| !c.is_whitespace()));
    }

    #[test]
    fn generating_an_unknown_message() {
        let mut msg_str = String::from(
            "{
            \"from\": \"source\", 
            \"to\": \"dest\",
            \"type\": \"SOME_OTHER_MESSAGE\",
            \"message\": {
                \"id\": \"123456\"
            }
        }",
        );

        let extra: Value = serde_json::from_str("{\"id\": \"123456\"}").unwrap();
        let msg = PeerMessage {
            from: String::from("source"),
            to: String::from("dest"),
            msg_type: String::from("SOME_OTHER_MESSAGE"),
            message: extra,
        };

        let serialized = serde_json::to_string(&msg)
            .unwrap()
            .retain(|c| !c.is_whitespace());
        assert_eq!(serialized, msg_str.retain(|c| !c.is_whitespace()));
    }
}

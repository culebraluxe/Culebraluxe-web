//! Side effects as data. A screen's `update` returns a `Cmd`; only the shell's executor (`exec.rs`) performs it.
//!
//! THERE IS NO ESCAPE HATCH. No `Cmd::Custom(closure)`, no "run this future". A capability a screen needs is a named
//! variant here, reviewed once and executed in one place — that is what keeps every screen's HTTP, errors and
//! navigation identical. See docs/agent/UI-SCREEN-ARCHITECTURE.md.

use serde::de::DeserializeOwned;

/// A failed request, in one shape for every screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    /// The HTTP status, or 0 when the request never got an answer.
    pub status: u16,
    /// The server's error code (`GUEST_CODE_INVALID`), or the executor's own (`NETWORK`, `DECODE`).
    pub code: String,
    /// Words fit to show a person.
    pub message: String,
}

impl ApiError {
    pub fn network(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: "NETWORK".into(),
            message: message.into(),
        }
    }

    pub fn decode(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: "DECODE".into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One endpoint in the API catalogue (`app/api.rs`). The catalogue is the only place that knows URLs.
pub trait Endpoint {
    const METHOD: Method;
    /// What a successful answer decodes into. The executor unwraps the `{ ok, value }` envelope when there is one.
    type Response: DeserializeOwned + 'static;
    fn path(&self) -> String;
    fn body(&self) -> Option<serde_json::Value> {
        None
    }
}

/// A request a screen asked for, with the message its answer becomes.
pub struct Request<Msg> {
    pub method: Method,
    pub path: String,
    pub body: Option<serde_json::Value>,
    // The answer arrives as JSON TEXT and is decoded with `from_str`, the same instantiation the rest of the crate
    // uses for these types. Decoding from a `Value` instead doubled the wasm size: a second full deserializer for every
    // large payload type.
    reply: Box<dyn FnOnce(Result<String, ApiError>) -> Msg>,
}

impl<Msg> Request<Msg> {
    /// Turn the raw answer into the screen's message: decode on success, pass the failure through. The executor calls
    /// this; so do tests, which is how an `update` is tested without a network.
    pub fn respond(self, answer: Result<serde_json::Value, ApiError>) -> Msg {
        (self.reply)(answer.map(|value| value.to_string()))
    }
}

/// Everything a screen may ask the shell to do.
pub enum Cmd<Msg> {
    None,
    Batch(Vec<Cmd<Msg>>),
    Request(Request<Msg>),
    /// In-app navigation through the router: no page load.
    Navigate(String),
    /// A full document load: another area of the app (site ↔ portal), or a server-owned route such as Auth.js.
    Load(String),
    /// Read one value from the device's storage; `None` when absent or storage is unavailable.
    StorageRead {
        key: String,
        reply: Box<dyn FnOnce(Option<String>) -> Msg>,
    },
    /// Write (or, with `None`, remove) one value in the device's storage.
    StorageWrite {
        key: String,
        value: Option<String>,
    },
    /// Deliver `msg` after `millis` — how a search waits for typing to pause. The screen compares a token it carried
    /// in the message with its current one, so a superseded timer does nothing.
    After {
        millis: u32,
        msg: Msg,
    },
}

impl<Msg: 'static> Cmd<Msg> {
    pub fn none() -> Self {
        Cmd::None
    }

    pub fn batch(cmds: impl IntoIterator<Item = Cmd<Msg>>) -> Self {
        let cmds: Vec<_> = cmds
            .into_iter()
            .filter(|cmd| !matches!(cmd, Cmd::None))
            .collect();
        if cmds.is_empty() {
            Cmd::None
        } else {
            Cmd::Batch(cmds)
        }
    }

    /// Call an endpoint; its typed answer (or the failure) becomes a message.
    pub fn request<E: Endpoint>(
        endpoint: E,
        to_msg: impl FnOnce(Result<E::Response, ApiError>) -> Msg + 'static,
    ) -> Self {
        Cmd::Request(Request {
            method: E::METHOD,
            path: endpoint.path(),
            body: endpoint.body(),
            reply: Box::new(move |answer| {
                to_msg(answer.and_then(|text| {
                    serde_json::from_str::<E::Response>(&text)
                        .map_err(|error| ApiError::decode(error.to_string()))
                }))
            }),
        })
    }

    pub fn navigate(path: impl Into<String>) -> Self {
        Cmd::Navigate(path.into())
    }

    pub fn load(href: impl Into<String>) -> Self {
        Cmd::Load(href.into())
    }

    pub fn storage_read(
        key: impl Into<String>,
        to_msg: impl FnOnce(Option<String>) -> Msg + 'static,
    ) -> Self {
        Cmd::StorageRead {
            key: key.into(),
            reply: Box::new(to_msg),
        }
    }

    pub fn after(millis: u32, msg: Msg) -> Self {
        Cmd::After { millis, msg }
    }

    pub fn storage_write(key: impl Into<String>, value: Option<String>) -> Self {
        Cmd::StorageWrite {
            key: key.into(),
            value,
        }
    }

    /// Re-address every message this command will produce. How a composed piece (a list inside a screen) hands its
    /// commands up to the screen that owns it.
    pub fn map<B: 'static>(self, f: impl Fn(Msg) -> B + Clone + 'static) -> Cmd<B> {
        match self {
            Cmd::None => Cmd::None,
            Cmd::Batch(cmds) => {
                Cmd::Batch(cmds.into_iter().map(|cmd| cmd.map(f.clone())).collect())
            }
            Cmd::Request(Request {
                method,
                path,
                body,
                reply,
            }) => Cmd::Request(Request {
                method,
                path,
                body,
                reply: Box::new(move |answer| f(reply(answer))),
            }),
            Cmd::Navigate(path) => Cmd::Navigate(path),
            Cmd::Load(href) => Cmd::Load(href),
            Cmd::StorageRead { key, reply } => Cmd::StorageRead {
                key,
                reply: Box::new(move |value| f(reply(value))),
            },
            Cmd::StorageWrite { key, value } => Cmd::StorageWrite { key, value },
            Cmd::After { millis, msg } => Cmd::After {
                millis,
                msg: f(msg),
            },
        }
    }

    /// The requests this command makes, flattened — for tests that assert what an `update` asked for.
    pub fn into_requests(self) -> Vec<Request<Msg>> {
        match self {
            Cmd::Request(request) => vec![request],
            Cmd::Batch(cmds) => cmds.into_iter().flat_map(Cmd::into_requests).collect(),
            _ => Vec::new(),
        }
    }
}

impl<Msg> std::fmt::Debug for Cmd<Msg> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cmd::None => write!(f, "None"),
            Cmd::Batch(cmds) => f.debug_list().entries(cmds).finish(),
            Cmd::Request(request) => write!(f, "Request({:?} {})", request.method, request.path),
            Cmd::Navigate(path) => write!(f, "Navigate({path})"),
            Cmd::Load(href) => write!(f, "Load({href})"),
            Cmd::StorageRead { key, .. } => write!(f, "StorageRead({key})"),
            Cmd::StorageWrite { key, value } => write!(f, "StorageWrite({key}, {value:?})"),
            Cmd::After { millis, .. } => write!(f, "After({millis}ms)"),
        }
    }
}

/// Data that arrives over the network, in the four states every screen draws the same way (`template::remote`).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Remote<T> {
    #[default]
    NotAsked,
    Loading,
    Loaded(T),
    Failed(ApiError),
}

impl<T> Remote<T> {
    pub fn from_result(result: Result<T, ApiError>) -> Self {
        match result {
            Ok(value) => Remote::Loaded(value),
            Err(error) => Remote::Failed(error),
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Remote::Loading)
    }

    pub fn loaded(&self) -> Option<&T> {
        match self {
            Remote::Loaded(value) => Some(value),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    enum Msg {
        Got(Result<Vec<u32>, ApiError>),
        Wrapped(Box<Msg>),
    }

    struct Numbers;
    impl Endpoint for Numbers {
        const METHOD: Method = Method::Get;
        type Response = Vec<u32>;
        fn path(&self) -> String {
            "/api/numbers".into()
        }
    }

    #[test]
    fn a_request_decodes_its_answer_into_the_screens_message() {
        let mut requests = Cmd::request(Numbers, Msg::Got).into_requests();
        assert_eq!(requests.len(), 1);
        let request = requests.remove(0);
        assert_eq!(
            (request.method, request.path.as_str()),
            (Method::Get, "/api/numbers")
        );
        assert_eq!(
            request.respond(Ok(serde_json::json!([1, 2]))),
            Msg::Got(Ok(vec![1, 2]))
        );
    }

    #[test]
    fn a_wrong_shape_is_a_decode_failure_not_a_panic() {
        let request = Cmd::request(Numbers, Msg::Got).into_requests().remove(0);
        let Msg::Got(Err(error)) = request.respond(Ok(serde_json::json!({ "no": "list" }))) else {
            panic!("expected a decode failure");
        };
        assert_eq!(error.code, "DECODE");
    }

    #[test]
    fn map_readdresses_the_reply_and_batch_drops_nothing_but_none() {
        let cmd = Cmd::batch([Cmd::None, Cmd::request(Numbers, Msg::Got)])
            .map(|msg| Msg::Wrapped(Box::new(msg)));
        let request = cmd.into_requests().remove(0);
        assert_eq!(
            request.respond(Err(ApiError::network("offline"))),
            Msg::Wrapped(Box::new(Msg::Got(Err(ApiError::network("offline")))))
        );
        assert!(matches!(Cmd::<Msg>::batch([Cmd::None]), Cmd::None));
    }
}

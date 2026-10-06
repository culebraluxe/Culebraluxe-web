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
    Put,
    /// A real delete. Never a POST with a verb in its path.
    Delete,
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

/// An endpoint that takes a FILE: the chunked-upload protocol (`Cmd::upload`) or one multipart form (`Cmd::post_form`).
/// Like `Endpoint`, it lives in the catalogue (`app/api.rs`), so a screen names it and never writes a path.
pub trait FileEndpoint {
    fn path(&self) -> String;
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
    /// Replace the browser URL without dispatching a router transition or remounting the current Screen.
    ///
    /// Stateful workspaces use this after swapping their selected record in-place. The MVI Model stays mounted while
    /// the address bar still tracks the record a reload should reopen.
    ReplacePath(String),
    /// Invoke the browser's native share sheet with an already-rendered PDF.
    ///
    /// This is an executor-owned browser capability, not screen logic. Keeping it a first-class Cmd means Forms can
    /// preserve the immediate user gesture Apple/Safari requires without adding a JavaScript escape hatch.
    SharePdf {
        data_uri: String,
        filename: String,
        reply: Box<dyn FnOnce(Result<(), ApiError>) -> Msg>,
    },
    /// Listen once through the browser's speech recognition (Safari's included) and answer what was said.
    Listen {
        reply: Box<dyn FnOnce(Result<String, ApiError>) -> Msg>,
    },
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
    /// Send one file with the chunked-upload protocol (see `Upload`).
    Upload(Upload<Msg>),
    /// A property film, sent from the browser straight to Mux (see `exec::upload_video`).
    VideoUpload(VideoUpload<Msg>),
    /// One form post with a file (a signed contract): fields, the file, and the answer — with the way out.
    PostForm {
        path: String,
        fields: Vec<(String, String)>,
        file: web_sys::File,
        reply: Box<dyn FnOnce(Result<serde_json::Value, ApiError>) -> Msg>,
    },
    /// Render the Google property map into the container with this id: load
    /// the Maps JS once, then mount a styled map with the CL pin. The
    /// container is looked up when the command runs (the host executes
    /// commands before the re-render commits), so the executor waits for it
    /// briefly rather than failing a race it created.
    InitMap {
        key: String,
        lat: f64,
        lng: f64,
        title: String,
        container_id: String,
        reply: Box<dyn FnOnce(Result<(), ApiError>) -> Msg>,
    },
}

/// A file sent in pieces, so no single request reaches the gateway's body limit: `init` declares it (the executor adds
/// `filename`, `mimeType`, `byteSize`, `chunkCount`, `chunkSize`) and answers an `uploadId`; each `chunk` carries one
/// piece in order; `complete` assembles and attaches it. `fields` go with every step, `init_fields` with the first only.
pub struct Upload<Msg> {
    pub file: web_sys::File,
    pub path: String,
    pub fields: Vec<(String, String)>,
    pub init_fields: Vec<(String, String)>,
    pub reply: Box<dyn FnOnce(Result<(), ApiError>) -> Msg>,
}

/// How far a film has got: bytes sent of the total, and the stage (`uploading`, then `preparing` while Mux encodes).
#[derive(Debug, Clone, PartialEq)]
pub struct VideoProgress {
    pub sent: f64,
    pub total: f64,
    pub stage: &'static str,
}

pub struct VideoUpload<Msg> {
    pub file: web_sys::File,
    pub property_id: String,
    pub role: String,
    pub caption: String,
    pub progress: std::rc::Rc<dyn Fn(VideoProgress) -> Msg>,
    pub reply: Box<dyn FnOnce(Result<(), ApiError>) -> Msg>,
}

impl<Msg: 'static> Cmd<Msg> {
    pub fn video_upload(
        file: web_sys::File,
        property_id: String,
        role: String,
        caption: String,
        progress: impl Fn(VideoProgress) -> Msg + 'static,
        reply: impl FnOnce(Result<(), ApiError>) -> Msg + 'static,
    ) -> Self {
        Cmd::VideoUpload(VideoUpload {
            file,
            property_id,
            role,
            caption,
            progress: std::rc::Rc::new(progress),
            reply: Box::new(reply),
        })
    }

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

    pub fn replace_path(path: impl Into<String>) -> Self {
        Cmd::ReplacePath(path.into())
    }

    pub fn share_pdf(
        data_uri: impl Into<String>,
        filename: impl Into<String>,
        to_msg: impl FnOnce(Result<(), ApiError>) -> Msg + 'static,
    ) -> Self {
        Cmd::SharePdf {
            data_uri: data_uri.into(),
            filename: filename.into(),
            reply: Box::new(to_msg),
        }
    }

    pub fn post_form(
        endpoint: impl FileEndpoint,
        fields: Vec<(String, String)>,
        file: web_sys::File,
        to_msg: impl FnOnce(Result<serde_json::Value, ApiError>) -> Msg + 'static,
    ) -> Self {
        Cmd::PostForm {
            path: endpoint.path(),
            fields,
            file,
            reply: Box::new(to_msg),
        }
    }

    pub fn listen(to_msg: impl FnOnce(Result<String, ApiError>) -> Msg + 'static) -> Self {
        Cmd::Listen {
            reply: Box::new(to_msg),
        }
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

    pub fn upload(
        file: web_sys::File,
        endpoint: impl FileEndpoint,
        fields: Vec<(String, String)>,
        init_fields: Vec<(String, String)>,
        to_msg: impl FnOnce(Result<(), ApiError>) -> Msg + 'static,
    ) -> Self {
        Cmd::Upload(Upload {
            file,
            path: endpoint.path(),
            fields,
            init_fields,
            reply: Box::new(to_msg),
        })
    }

    pub fn storage_write(key: impl Into<String>, value: Option<String>) -> Self {
        Cmd::StorageWrite {
            key: key.into(),
            value,
        }
    }

    pub fn init_map(
        key: impl Into<String>,
        lat: f64,
        lng: f64,
        title: impl Into<String>,
        container_id: impl Into<String>,
        to_msg: impl FnOnce(Result<(), ApiError>) -> Msg + 'static,
    ) -> Self {
        Cmd::InitMap {
            key: key.into(),
            lat,
            lng,
            title: title.into(),
            container_id: container_id.into(),
            reply: Box::new(to_msg),
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
            Cmd::ReplacePath(path) => Cmd::ReplacePath(path),
            Cmd::SharePdf {
                data_uri,
                filename,
                reply,
            } => Cmd::SharePdf {
                data_uri,
                filename,
                reply: Box::new(move |answer| f(reply(answer))),
            },
            Cmd::PostForm {
                path,
                fields,
                file,
                reply,
            } => Cmd::PostForm {
                path,
                fields,
                file,
                reply: Box::new(move |answer| f(reply(answer))),
            },
            Cmd::InitMap {
                key,
                lat,
                lng,
                title,
                container_id,
                reply,
            } => Cmd::InitMap {
                key,
                lat,
                lng,
                title,
                container_id,
                reply: Box::new(move |answer| f(reply(answer))),
            },
            Cmd::Listen { reply } => Cmd::Listen {
                reply: Box::new(move |answer| f(reply(answer))),
            },
            Cmd::StorageRead { key, reply } => Cmd::StorageRead {
                key,
                reply: Box::new(move |value| f(reply(value))),
            },
            Cmd::StorageWrite { key, value } => Cmd::StorageWrite { key, value },
            Cmd::After { millis, msg } => Cmd::After {
                millis,
                msg: f(msg),
            },
            Cmd::Upload(upload) => Cmd::Upload(Upload {
                file: upload.file,
                path: upload.path,
                fields: upload.fields,
                init_fields: upload.init_fields,
                reply: {
                    let reply = upload.reply;
                    Box::new(move |answer| f(reply(answer)))
                },
            }),
            Cmd::VideoUpload(upload) => {
                let (progress, g) = (upload.progress, f.clone());
                Cmd::VideoUpload(VideoUpload {
                    file: upload.file,
                    property_id: upload.property_id,
                    role: upload.role,
                    caption: upload.caption,
                    progress: std::rc::Rc::new(move |step| g(progress(step))),
                    reply: {
                        let reply = upload.reply;
                        Box::new(move |answer| f(reply(answer)))
                    },
                })
            }
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
            Cmd::ReplacePath(path) => write!(f, "ReplacePath({path})"),
            Cmd::SharePdf { filename, .. } => write!(f, "SharePdf({filename})"),
            Cmd::Listen { .. } => write!(f, "Listen"),
            Cmd::PostForm { path, .. } => write!(f, "PostForm({path})"),
            Cmd::InitMap { lat, lng, .. } => write!(f, "InitMap({lat},{lng})"),
            Cmd::StorageRead { key, .. } => write!(f, "StorageRead({key})"),
            Cmd::StorageWrite { key, value } => write!(f, "StorageWrite({key}, {value:?})"),
            Cmd::After { millis, .. } => write!(f, "After({millis}ms)"),
            Cmd::Upload(upload) => write!(f, "Upload({})", upload.path),
            Cmd::VideoUpload(upload) => write!(f, "VideoUpload({})", upload.file.name()),
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

/// A command answered over HTTP is 200 with the verdict INSIDE the body (`value.outcome`). `None` means it
/// succeeded (or the body carries no verdict); `Some(message)` is what to tell the person.
pub fn command_refusal(body: &serde_json::Value) -> Option<String> {
    let result = body.get("value").unwrap_or(body);
    let outcome = result.get("outcome")?.as_str()?;
    if outcome == "success" {
        return None;
    }
    let detail = result
        .pointer("/error/message")
        .and_then(|v| v.as_str())
        .or_else(|| result.get("message").and_then(|v| v.as_str()))
        .filter(|text| !text.trim().is_empty());
    Some(match detail {
        Some(text) => text.to_string(),
        None => format!("That was not accepted ({outcome})."),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_command_is_reported_and_a_success_is_not() {
        let ok = serde_json::json!({"value": {"outcome": "success"}});
        assert_eq!(command_refusal(&ok), None);
        let refused = serde_json::json!({"value": {"outcome": "rejected", "error": {"message": "Consent first"}}});
        assert_eq!(command_refusal(&refused).as_deref(), Some("Consent first"));
        let bare = serde_json::json!({"outcome": "denied"});
        assert_eq!(
            command_refusal(&bare).as_deref(),
            Some("That was not accepted (denied).")
        );
        assert_eq!(command_refusal(&serde_json::json!({})), None);
    }

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

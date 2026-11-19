//! The clipboard between this machine and a remote desktop.
//!
//! MS-RDPECLIP is a pull protocol. Each side announces what it has when it
//! changes, and the other side asks for the bytes only when someone pastes.
//! IronRDP speaks the protocol; what it leaves to the embedder is the local
//! end — reading and writing this machine's clipboard, and the files — and
//! that is what lives here.
//!
//! Text and pictures go both ways. Files go both ways too: copy files here
//! and paste them into the remote, or copy them there and paste them here,
//! or drop them on the desktop. A phone has no file clipboard, so it is
//! text only there. A session may turn files off, or the whole clipboard
//! (see [`ClipboardMode`]).
//!
//! Pictures: offered to the remote as CF_DIB, CF_DIBV5 and PNG, encoded
//! once when offered; taken from the remote as PNG when it has it (alpha
//! survives), else as a DIB. A copy that carries text as well (a range of
//! cells, say) comes over as its text.
//!
//! Transfers report to the webview on `rdp-clipboard-{id}` (see
//! [`ClipEvent`]): progress both ways, and a remote copy over
//! [`ASK_ABOVE`] waits for the user to say download rather than coming
//! down on the chance of a paste.
//!
//! Every IronRDP callback arrives on the session thread, which is also the
//! thread that decodes the picture, so nothing may block there. Each callback
//! is one message to a worker thread of the session's own, which owns the OS
//! clipboard and the open files and answers through the session's clipboard
//! channel — unbounded and separate from input, so a large transfer can never
//! backpressure the keyboard.
//!
//! Speed: a file going out is read through a large read-ahead buffer that
//! stays open across the server's requests, so each request is a memcpy, not
//! an open-seek-read. A file coming in is asked for in 1 MiB pieces, one in
//! flight, and written as it lands. The wire is the limit either way.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ironrdp_client::rdp::RdpInputSender;
use ironrdp_cliprdr::backend::{ClipboardMessage, ClipboardMessageProxy, CliprdrBackend, CliprdrBackendFactory};
use ironrdp_cliprdr::pdu::{
    ClipboardFileAttributes, ClipboardFormat, ClipboardFormatId, ClipboardFormatName, ClipboardGeneralCapabilityFlags,
    FileContentsFlags, FileContentsRequest, FileContentsResponse, FileDescriptor, FormatDataRequest, FormatDataResponse,
    LockDataId, OwnedFormatDataResponse,
};
use ironrdp_pdu::ironrdp_core::{impl_as_any, IntoOwned};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// How much of an outgoing file is read ahead of the server's requests.
const READ_AHEAD: usize = 2 * 1024 * 1024;

/// How much of an incoming file is asked for at a time.
const FETCH_CHUNK: u32 = 1024 * 1024;

/// The most a remote copy may pull down without being asked. The remote side
/// announces file sizes before a byte moves; a copy larger than this waits
/// for the user to ask for it rather than coming down on the chance of a
/// paste.
pub const ASK_ABOVE: u64 = 100 * 1024 * 1024;
/// The most a session keeps of what the remote copied unasked, all copies
/// together. A copy the user asked for is not counted against it.
const MAX_SESSION_FETCH_TOTAL: u64 = 8 * 1024 * 1024 * 1024;

/// The largest picture taken either way, as RGBA bytes (a 8K screen is
/// 132 MB). Beyond it a copy is left where it is.
const MAX_PICTURE_BYTES: usize = 256 * 1024 * 1024;

/// How often progress is reported while bytes move.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// The id this side gives the PNG format it offers. Registered formats are
/// numbered by whoever lists them, from 0xC000; IronRDP takes 0xC0FE and
/// 0xC0FD for the file list, so this stays clear of both.
const PNG_FORMAT_ID: u32 = 0xC0A1;
/// The names the PNG format goes by: Windows apps register "PNG", the
/// Linux servers pass the MIME type through.
const PNG_NAMES: [&str; 2] = ["PNG", "image/png"];

/// What a session lets through the clipboard. Saved with the session as
/// `rdp_clipboard`: unset is `All`, `"text"` is `NoFiles`, `"off"` is off
/// (no channel at all, decided in `build_config`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardMode {
    All,
    NoFiles,
}

/// What the clipboard tells the webview, as `rdp-clipboard-{id}`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ClipEvent {
    /// Bytes are moving. `incoming` is remote to here.
    Progress { incoming: bool, done: u64, total: u64, files: usize },
    /// Everything arrived (incoming: and is on the local clipboard).
    Finished { incoming: bool, files: usize },
    /// The user stopped it.
    Cancelled { incoming: bool },
    /// It broke off: the remote stopped sending, or a file could not be
    /// read or written.
    Failed { incoming: bool, reason: String },
    /// The remote copied files too large to fetch unasked; `rdp_clipboard_fetch`
    /// takes them.
    Held { files: usize, total: u64 },
    /// The held copy is gone: the remote copied something else.
    HeldGone,
}

/// What the session thread hands the worker. Each is one IronRDP callback,
/// or one request from the app.
enum Cmd {
    Ready,
    Advertise { force: bool },
    FormatData(FormatDataRequest),
    FileContents(FileContentsRequest),
    RemoteCopy(Vec<ClipboardFormat>),
    RemoteData(OwnedFormatDataResponse),
    RemoteFiles(Vec<FileDescriptor>, Option<u32>),
    RemoteFileData(FileContentsResponse<'static>),
    Lock(u32),
    Unlock(u32),
    /// Files dropped on the desktop: offer exactly these.
    SendFiles(Vec<PathBuf>),
    /// Take the remote copy that is waiting for a yes.
    FetchHeld,
    /// Stop what is moving, either way.
    Cancel,
}

/// The app's end: ask the worker to look at the local clipboard again.
/// Dropping it — with the backend and the factory — ends the worker.
pub struct ClipboardHandle {
    tx: Sender<Cmd>,
}

impl ClipboardHandle {
    /// The local clipboard may have changed: offer it to the remote if so.
    /// Called when the desktop gains focus, which is when a paste can follow.
    pub fn sync(&self) {
        let _ = self.tx.send(Cmd::Advertise { force: false });
    }

    /// Put these files on the remote's clipboard (and this one's), as if
    /// they had been copied here.
    pub fn send_files(&self, paths: Vec<PathBuf>) {
        let _ = self.tx.send(Cmd::SendFiles(paths));
    }

    /// Download the remote copy that was too large to take unasked.
    pub fn fetch_held(&self) {
        let _ = self.tx.send(Cmd::FetchHeld);
    }

    /// Stop the transfer in progress.
    pub fn cancel(&self) {
        let _ = self.tx.send(Cmd::Cancel);
    }
}

/// Start the worker for one session. Returns the app's handle and the
/// factory IronRDP builds its backend from.
pub fn spawn(
    id: &str,
    input: RdpInputSender,
    app: AppHandle,
    mode: ClipboardMode,
) -> std::io::Result<(ClipboardHandle, Factory)> {
    // Under the app's own private folder, not the system temp folder that
    // every user of a Linux machine shares (see `private_tmp_dir`).
    let temp_dir = match crate::private_tmp_dir("rdp-clipboard") {
        Ok(base) => base.join(id),
        Err(e) => {
            tracing::warn!("RDP {id}: no private folder for clipboard files: {e}");
            crate::app_data_dir().join("tmp").join("rdp-clipboard").join(id)
        }
    };
    if let Err(e) = fs::create_dir_all(&temp_dir) {
        tracing::warn!("RDP {id}: no clipboard temp dir at {}: {e}", temp_dir.display());
    }

    let (tx, rx) = mpsc::channel();
    let worker = Worker {
        id: id.to_string(),
        proxy: Proxy(input),
        events: Events { app: app.clone(), name: format!("rdp-clipboard-{id}") },
        files: mode == ClipboardMode::All,
        local: LocalClipboard::new(app),
        temp_dir: temp_dir.clone(),
        ready: false,
        fingerprint: None,
        offered: Vec::new(),
        offered_picture: None,
        outgoing: Outgoing::default(),
        held: None,
        locked: HashMap::new(),
        readers: HashMap::new(),
        pending: None,
        fetch: None,
        fetches: 0,
        fetched_total: 0,
        next_stream: 1,
    };
    std::thread::Builder::new()
        .name(format!("rdp-clip-{id}"))
        .spawn(move || worker.run(rx))?;

    let factory = Factory {
        tx: tx.clone(),
        temp_dir: temp_dir.to_string_lossy().into_owned(),
        files: mode == ClipboardMode::All,
    };
    Ok((ClipboardHandle { tx }, factory))
}

/// Answers from the worker go to the session through its clipboard channel.
#[derive(Clone, Debug)]
struct Proxy(RdpInputSender);

impl ClipboardMessageProxy for Proxy {
    fn send_clipboard_message(&self, message: ClipboardMessage) {
        if self.0.send_clipboard(message).is_err() {
            tracing::debug!("RDP clipboard: session gone, message dropped");
        }
    }
}

/// IronRDP asks for a backend per channel initialisation; every one it gets
/// talks to the same worker.
pub struct Factory {
    tx: Sender<Cmd>,
    temp_dir: String,
    files: bool,
}

impl CliprdrBackendFactory for Factory {
    fn build_cliprdr_backend(&self) -> Box<dyn CliprdrBackend> {
        Box::new(Backend {
            tx: self.tx.clone(),
            temp_dir: self.temp_dir.clone(),
            files: self.files,
        })
    }
}

/// What IronRDP calls, on the session thread. Every method is a send.
#[derive(Debug)]
struct Backend {
    tx: Sender<Cmd>,
    temp_dir: String,
    /// Files may cross; when not, the capabilities say so and the server
    /// offers none.
    files: bool,
}

impl_as_any!(Backend);

impl Backend {
    fn send(&self, cmd: Cmd) {
        if self.tx.send(cmd).is_err() {
            tracing::debug!("RDP clipboard: worker gone");
        }
    }
}

impl CliprdrBackend for Backend {
    fn temporary_directory(&self) -> &str {
        &self.temp_dir
    }

    fn client_capabilities(&self) -> ClipboardGeneralCapabilityFlags {
        if !self.files {
            return ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES;
        }
        // Files as streams with no local paths on the wire, locked lists so
        // a copy that follows a copy does not corrupt a paste in flight, and
        // 64-bit offsets so the size of a file is not a limit.
        ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES
            | ClipboardGeneralCapabilityFlags::STREAM_FILECLIP_ENABLED
            | ClipboardGeneralCapabilityFlags::FILECLIP_NO_FILE_PATHS
            | ClipboardGeneralCapabilityFlags::CAN_LOCK_CLIPDATA
            | ClipboardGeneralCapabilityFlags::HUGE_FILE_SUPPORT_ENABLED
    }

    fn on_ready(&mut self) {
        self.send(Cmd::Ready);
    }

    fn on_request_format_list(&mut self) {
        self.send(Cmd::Advertise { force: true });
    }

    fn on_process_negotiated_capabilities(&mut self, capabilities: ClipboardGeneralCapabilityFlags) {
        tracing::debug!("RDP clipboard: negotiated {capabilities:?}");
    }

    fn on_remote_copy(&mut self, available_formats: &[ClipboardFormat]) {
        self.send(Cmd::RemoteCopy(available_formats.to_vec()));
    }

    fn on_format_data_request(&mut self, request: FormatDataRequest) {
        self.send(Cmd::FormatData(request));
    }

    fn on_format_data_response(&mut self, response: FormatDataResponse<'_>) {
        self.send(Cmd::RemoteData(response.into_owned()));
    }

    fn on_file_contents_request(&mut self, request: FileContentsRequest) {
        self.send(Cmd::FileContents(request));
    }

    fn on_file_contents_response(&mut self, response: FileContentsResponse<'_>) {
        self.send(Cmd::RemoteFileData(response.into_owned()));
    }

    fn on_lock(&mut self, data_id: LockDataId) {
        self.send(Cmd::Lock(data_id.0));
    }

    fn on_unlock(&mut self, data_id: LockDataId) {
        self.send(Cmd::Unlock(data_id.0));
    }

    fn on_remote_file_list(&mut self, files: &[FileDescriptor], clip_data_id: Option<u32>) {
        self.send(Cmd::RemoteFiles(files.to_vec(), clip_data_id));
    }
}

/// What is on the local clipboard, as far as the remote is concerned.
enum Local {
    #[cfg_attr(mobile, expect(dead_code, reason = "a phone's clipboard carries text only"))]
    Files(Vec<PathBuf>),
    Text(String),
    #[cfg_attr(mobile, expect(dead_code, reason = "a phone's clipboard carries text only"))]
    Picture(Picture),
    Empty,
}

/// A picture as the clipboard libraries hand it over: straight RGBA, rows
/// top to bottom.
#[derive(Clone)]
pub(crate) struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Picture {
    fn fingerprint(&self) -> u64 {
        let mut h = DefaultHasher::new();
        (self.width, self.height).hash(&mut h);
        self.rgba.hash(&mut h);
        h.finish()
    }
}

/// Enough to know whether the clipboard changed since it was last offered,
/// without keeping the content.
#[derive(PartialEq, Eq)]
enum Fingerprint {
    Files(Vec<PathBuf>),
    Text(u64),
    Picture(u64),
    Empty,
}

impl Fingerprint {
    fn of(local: &Local) -> Self {
        match local {
            Local::Files(paths) => Fingerprint::Files(paths.clone()),
            Local::Text(text) => {
                let mut h = DefaultHasher::new();
                text.hash(&mut h);
                Fingerprint::Text(h.finish())
            }
            Local::Picture(picture) => Fingerprint::Picture(picture.fingerprint()),
            Local::Empty => Fingerprint::Empty,
        }
    }
}

/// What was asked of the remote and not yet answered. The protocol allows
/// one outstanding request, and the answer does not say what it answers.
#[derive(Clone, Copy)]
enum Pending {
    Text,
    Files,
    Picture(PictureFormat),
}

/// The forms a picture crosses in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PictureFormat {
    Png,
    Dib,
    DibV5,
}

/// What a picture offered to the remote has been made into, once, when it
/// was offered: a paste asks for one of them, often more than once.
struct OfferedPicture {
    png: Vec<u8>,
    dib: Option<Vec<u8>>,
    dibv5: Option<Vec<u8>>,
}

/// Bytes the remote has pulled of the current offer, for progress.
#[derive(Default)]
struct Outgoing {
    /// Bytes in the offer's files (directories count nothing).
    total: u64,
    files: usize,
    done: u64,
    /// The user stopped it: what is left of this offer is refused.
    cancelled: bool,
    finished: bool,
    last_report: Option<Instant>,
}

/// The tauri side of progress reporting.
struct Events {
    app: AppHandle,
    name: String,
}

impl Events {
    fn emit(&self, event: ClipEvent) {
        if let Err(e) = self.app.emit(&self.name, event) {
            tracing::debug!("RDP clipboard: event not delivered: {e}");
        }
    }
}

/// An outgoing file the server is reading, kept open between its requests.
struct Reader {
    file: BufReader<File>,
    pos: u64,
}

/// An incoming copy, written into the temp dir as it lands, one file at a
/// time, one request in flight.
struct Fetch {
    lock: Option<u32>,
    root: PathBuf,
    files: Vec<FileDescriptor>,
    /// Index into `files` of the one being received.
    index: usize,
    out: Option<BufWriter<File>>,
    offset: u64,
    size: u64,
    stream: u32,
    /// All the files together, and what has landed of them, for progress.
    total: u64,
    done: u64,
    last_report: Option<Instant>,
}

/// The local clipboard, owned by the worker thread.
#[cfg(not(mobile))]
struct LocalClipboard {
    inner: Option<arboard::Clipboard>,
}

#[cfg(not(mobile))]
impl LocalClipboard {
    fn new(_app: AppHandle) -> Self {
        Self { inner: None }
    }

    /// Opened on first use, on this thread, which is what the OS wants.
    fn get(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.inner.is_none() {
            match arboard::Clipboard::new() {
                Ok(cb) => self.inner = Some(cb),
                Err(e) => tracing::warn!("RDP clipboard: no local clipboard: {e}"),
            }
        }
        self.inner.as_mut()
    }

    fn read(&mut self) -> Local {
        let Some(cb) = self.get() else { return Local::Empty };
        if let Ok(paths) = cb.get().file_list() {
            if !paths.is_empty() {
                return Local::Files(paths);
            }
        }
        if let Ok(text) = cb.get().text() {
            if !text.is_empty() {
                return Local::Text(text);
            }
        }
        match cb.get().image() {
            Ok(image) if image.bytes.len() <= MAX_PICTURE_BYTES && image.width > 0 && image.height > 0 => {
                Local::Picture(Picture { width: image.width, height: image.height, rgba: image.bytes.into_owned() })
            }
            _ => Local::Empty,
        }
    }

    fn write_picture(&mut self, picture: &Picture) {
        if let Some(cb) = self.get() {
            let image = arboard::ImageData {
                width: picture.width,
                height: picture.height,
                bytes: std::borrow::Cow::Borrowed(&picture.rgba),
            };
            if let Err(e) = cb.set().image(image) {
                tracing::warn!("RDP clipboard: could not set local picture: {e}");
            }
        }
    }

    fn write_text(&mut self, text: &str) {
        if let Some(cb) = self.get() {
            if let Err(e) = cb.set().text(text) {
                tracing::warn!("RDP clipboard: could not set local text: {e}");
            }
        }
    }

    fn write_files(&mut self, paths: &[PathBuf]) {
        if let Some(cb) = self.get() {
            if let Err(e) = cb.set().file_list(paths) {
                tracing::warn!("RDP clipboard: could not set local files: {e}");
            }
        }
    }
}

/// A phone (Android, iOS): text through the clipboard plugin, no files.
#[cfg(mobile)]
struct LocalClipboard {
    app: AppHandle,
}

#[cfg(mobile)]
impl LocalClipboard {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn read(&mut self) -> Local {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        match self.app.clipboard().read_text() {
            Ok(text) if !text.is_empty() => Local::Text(text),
            _ => Local::Empty,
        }
    }

    fn write_text(&mut self, text: &str) {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        if let Err(e) = self.app.clipboard().write_text(text) {
            tracing::warn!("RDP clipboard: could not set local text: {e}");
        }
    }

    fn write_files(&mut self, _paths: &[PathBuf]) {}

    fn write_picture(&mut self, _picture: &Picture) {}
}

/// The session's clipboard thread. See the module docs for why it exists.
struct Worker {
    id: String,
    proxy: Proxy,
    events: Events,
    /// Files may cross (the session did not turn them off).
    files: bool,
    local: LocalClipboard,
    temp_dir: PathBuf,
    /// The channel is up and capabilities are negotiated; files may be offered.
    ready: bool,
    /// What was last offered to the remote, so the same content is not
    /// offered twice and what came *from* the remote is not offered back.
    fingerprint: Option<Fingerprint>,
    /// The files behind the last offer, by the index the server uses.
    offered: Vec<PathBuf>,
    /// The picture behind the last offer, ready in every form offered.
    offered_picture: Option<OfferedPicture>,
    /// How far the remote has got pulling the offered files.
    outgoing: Outgoing,
    /// A remote copy too large to take unasked, until the user says.
    held: Option<(Vec<FileDescriptor>, Option<u32>)>,
    /// Offers the server locked, by its lock id: a request that names one
    /// must read from that list even after a newer offer replaced it.
    locked: HashMap<u32, Vec<PathBuf>>,
    /// Open outgoing files, by (lock, index).
    readers: HashMap<(Option<u32>, i32), Reader>,
    pending: Option<Pending>,
    fetch: Option<Fetch>,
    /// Incoming copies so far, to give each its own directory.
    fetches: u32,
    /// Bytes of incoming copies accepted this session.
    fetched_total: u64,
    next_stream: u32,
}

impl Worker {
    fn run(mut self, rx: Receiver<Cmd>) {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                Cmd::Ready => {
                    self.ready = true;
                    self.advertise(true);
                }
                Cmd::Advertise { force } => self.advertise(force),
                Cmd::FormatData(request) => self.format_data(request),
                Cmd::FileContents(request) => self.file_contents(request),
                Cmd::RemoteCopy(formats) => self.remote_copy(&formats),
                Cmd::RemoteData(response) => self.remote_data(response),
                Cmd::RemoteFiles(files, lock) => self.remote_files(files, lock),
                Cmd::RemoteFileData(response) => self.remote_file_data(response),
                Cmd::Lock(id) => {
                    self.locked.insert(id, self.offered.clone());
                }
                Cmd::Unlock(id) => {
                    self.locked.remove(&id);
                    self.readers.retain(|(lock, _), _| *lock != Some(id));
                }
                Cmd::SendFiles(paths) => self.send_files(paths),
                Cmd::FetchHeld => self.fetch_held(),
                Cmd::Cancel => self.cancel(),
            }
        }
        // Every sender is gone: the session ended. What the remote copied
        // here lived in the temp dir, and the session it belonged to is over.
        if let Err(e) = fs::remove_dir_all(&self.temp_dir) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::debug!("RDP {}: clipboard temp dir left behind: {e}", self.id);
            }
        }
    }

    fn send(&self, message: ClipboardMessage) {
        self.proxy.send_clipboard_message(message);
    }

    fn stream_id(&mut self) -> u32 {
        let id = self.next_stream;
        self.next_stream = self.next_stream.wrapping_add(1).max(1);
        id
    }

    /// Tell the remote what this machine's clipboard holds, if that changed.
    fn advertise(&mut self, force: bool) {
        let local = self.local.read();
        self.offer(local, force);
    }

    /// Offer `local` to the remote, unless it is what was offered last.
    fn offer(&mut self, local: Local, force: bool) {
        let fingerprint = Fingerprint::of(&local);
        if !force && self.fingerprint.as_ref() == Some(&fingerprint) {
            return;
        }

        if !matches!(local, Local::Picture(_)) {
            self.offered_picture = None;
        }
        match local {
            // Files need the channel ready; before that, offer nothing and
            // the `Ready` that follows offers them properly.
            Local::Files(paths) if self.ready && self.files => {
                let (descriptors, files) = describe(&paths);
                // A new offer replaces the unlocked readers of the old one.
                self.readers.retain(|(lock, _), _| lock.is_some());
                self.outgoing = Outgoing {
                    total: files.iter().filter_map(|p| fs::metadata(p).ok()).filter(|m| m.is_file()).map(|m| m.len()).sum(),
                    files: paths.len(),
                    ..Outgoing::default()
                };
                self.offered = files;
                if descriptors.is_empty() {
                    self.send(ClipboardMessage::SendInitiateCopy(Vec::new()));
                } else {
                    tracing::info!("RDP {}: offering {} file(s) to the remote", self.id, descriptors.len());
                    self.send(ClipboardMessage::SendInitiateFileCopy(descriptors));
                }
            }
            Local::Files(_) => self.send(ClipboardMessage::SendInitiateCopy(Vec::new())),
            Local::Text(_) => self.send(ClipboardMessage::SendInitiateCopy(vec![ClipboardFormat::new(
                ClipboardFormatId::CF_UNICODETEXT,
            )])),
            Local::Picture(picture) => match prepare_picture(&picture) {
                Some(prepared) => {
                    let formats = picture_formats(&prepared);
                    self.offered_picture = Some(prepared);
                    self.send(ClipboardMessage::SendInitiateCopy(formats));
                }
                None => {
                    self.offered_picture = None;
                    self.send(ClipboardMessage::SendInitiateCopy(Vec::new()));
                }
            },
            Local::Empty => self.send(ClipboardMessage::SendInitiateCopy(Vec::new())),
        }
        self.fingerprint = Some(fingerprint);
    }

    /// Files dropped on the desktop: they become this machine's clipboard
    /// too, so the next focus does not offer something else over them, and
    /// are offered whatever was offered before.
    fn send_files(&mut self, paths: Vec<PathBuf>) {
        let paths: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
        if paths.is_empty() || !self.files {
            return;
        }
        self.local.write_files(&paths);
        self.offer(Local::Files(paths), true);
    }

    /// The remote is pasting our text.
    fn format_data(&mut self, request: FormatDataRequest) {
        // A picture is answered from what was made of it when offered;
        // reading and encoding it again for each request would be slow.
        if let Some(picture) = self.offered_picture.as_ref() {
            let bytes = match request.format {
                f if f == ClipboardFormatId::new(PNG_FORMAT_ID) => Some(&picture.png),
                ClipboardFormatId::CF_DIB => picture.dib.as_ref(),
                ClipboardFormatId::CF_DIBV5 => picture.dibv5.as_ref(),
                _ => None,
            };
            if let Some(bytes) = bytes {
                let response = FormatDataResponse::new_data(bytes.clone());
                self.send(ClipboardMessage::SendFormatData(response));
                return;
            }
        }
        let response = match (request.format, self.local.read()) {
            (ClipboardFormatId::CF_UNICODETEXT, Local::Text(text)) => {
                FormatDataResponse::new_unicode_string(&text).into_owned()
            }
            _ => FormatDataResponse::new_error(),
        };
        self.send(ClipboardMessage::SendFormatData(response));
    }

    /// The remote is pasting our files: a size, or a range of one of them.
    fn file_contents(&mut self, request: FileContentsRequest) {
        let stream = request.stream_id;
        // Stopped by the user: the rest of this offer is refused, and the
        // remote's own copy dialog says the paste failed.
        if self.outgoing.cancelled && !request.flags.contains(FileContentsFlags::SIZE) {
            self.send(ClipboardMessage::SendFileContentsResponse(FileContentsResponse::new_error(stream)));
            return;
        }
        let response = match self.read_file_contents(&request) {
            Ok(response) => response,
            Err(e) => {
                tracing::warn!("RDP {}: file contents request {request:?} failed: {e}", self.id);
                FileContentsResponse::new_error(stream)
            }
        };
        if !request.flags.contains(FileContentsFlags::SIZE) && !response.is_error() {
            let sent = response.data().len() as u64;
            self.outgoing_progress(sent);
        }
        self.send(ClipboardMessage::SendFileContentsResponse(response));
    }

    /// The remote pulled `sent` more bytes of the offer.
    fn outgoing_progress(&mut self, sent: u64) {
        let out = &mut self.outgoing;
        if out.finished || out.total == 0 {
            return;
        }
        out.done = out.done.saturating_add(sent);
        if out.done >= out.total {
            out.finished = true;
            let files = out.files;
            self.events.emit(ClipEvent::Finished { incoming: false, files });
            return;
        }
        if out.last_report.is_none_or(|t| t.elapsed() >= PROGRESS_EVERY) {
            out.last_report = Some(Instant::now());
            let event = ClipEvent::Progress { incoming: false, done: out.done, total: out.total, files: out.files };
            self.events.emit(event);
        }
    }

    /// The user pressed Cancel: stop what is coming in, refuse what is left
    /// of what is going out.
    fn cancel(&mut self) {
        if self.fetch.is_some() {
            self.end_fetch(Some(ClipEvent::Cancelled { incoming: true }));
        }
        let out = &mut self.outgoing;
        if out.done > 0 && !out.finished && !out.cancelled {
            out.cancelled = true;
            self.readers.clear();
            self.events.emit(ClipEvent::Cancelled { incoming: false });
        }
    }

    fn read_file_contents(&mut self, request: &FileContentsRequest) -> std::io::Result<FileContentsResponse<'static>> {
        let list = match request.data_id {
            Some(lock) => self.locked.get(&lock),
            None => Some(&self.offered),
        };
        let path = usize::try_from(request.index)
            .ok()
            .and_then(|i| list.and_then(|l| l.get(i)))
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no such offered file"))?;

        if request.flags.contains(FileContentsFlags::SIZE) {
            let size = fs::metadata(&path)?.len();
            return Ok(FileContentsResponse::new_size_response(request.stream_id, size));
        }

        let reader = match self.readers.entry((request.data_id, request.index)) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(Reader {
                file: BufReader::with_capacity(READ_AHEAD, File::open(&path)?),
                pos: 0,
            }),
        };
        if reader.pos != request.position {
            reader.file.seek(SeekFrom::Start(request.position))?;
            reader.pos = request.position;
        }

        let mut data = vec![0u8; request.requested_size as usize];
        let mut filled = 0;
        while filled < data.len() {
            let n = reader.file.read(&mut data[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        data.truncate(filled);
        reader.pos += filled as u64;
        Ok(FileContentsResponse::new_data_response(request.stream_id, data))
    }

    /// The remote copied something. Ask for it now: files by their list
    /// (IronRDP parses it and calls back with the descriptors), else text.
    fn remote_copy(&mut self, formats: &[ClipboardFormat]) {
        let files = formats
            .iter()
            .find(|f| f.name.as_ref().is_some_and(|n| n.value() == ClipboardFormatName::FILE_LIST.value()))
            .map(|f| f.id())
            .filter(|_| self.files);
        let text = formats.iter().any(|f| f.id() == ClipboardFormatId::CF_UNICODETEXT);

        // A copy on the remote supersedes a transfer from the previous one,
        // and a copy that was waiting for a yes.
        if self.fetch.is_some() {
            self.end_fetch(Some(ClipEvent::Cancelled { incoming: true }));
        }
        if self.held.take().is_some() {
            self.events.emit(ClipEvent::HeldGone);
        }

        if let Some(id) = files {
            self.pending = Some(Pending::Files);
            self.send(ClipboardMessage::SendInitiatePaste(id));
        } else if text {
            self.pending = Some(Pending::Text);
            self.send(ClipboardMessage::SendInitiatePaste(ClipboardFormatId::CF_UNICODETEXT));
        } else if let Some((id, format)) = pick_picture(formats) {
            self.pending = Some(Pending::Picture(format));
            self.send(ClipboardMessage::SendInitiatePaste(id));
        } else {
            self.pending = None;
        }
    }

    /// The remote's text arrived: it becomes the local clipboard.
    fn remote_data(&mut self, response: OwnedFormatDataResponse) {
        let pending = self.pending.take();
        if response.is_error() {
            return;
        }
        match pending {
            Some(Pending::Text) => match response.to_unicode_string() {
                Ok(text) => {
                    self.local.write_text(&text);
                    // Ours now, but not to be offered back where it came from.
                    self.fingerprint = Some(Fingerprint::of(&Local::Text(text)));
                }
                Err(e) => tracing::debug!("RDP {}: remote text unreadable: {e}", self.id),
            },
            Some(Pending::Picture(format)) => match decode_picture(format, response.data()) {
                Ok(picture) => {
                    self.local.write_picture(&picture);
                    // The local clipboard may hand back slightly different
                    // bytes (Windows stores a DIB); read what it now holds,
                    // so a focus does not offer the picture straight back.
                    let fingerprint = match self.local.read() {
                        local @ Local::Picture(_) => Fingerprint::of(&local),
                        _ => Fingerprint::Picture(picture.fingerprint()),
                    };
                    self.offered_picture = None;
                    self.fingerprint = Some(fingerprint);
                }
                Err(e) => tracing::debug!("RDP {}: remote picture ({format:?}) unreadable: {e}", self.id),
            },
            _ => {}
        }
    }

    /// The remote's file list arrived: fetch the lot into the temp dir, then
    /// put the paths on the local clipboard.
    fn remote_files(&mut self, files: Vec<FileDescriptor>, lock: Option<u32>) {
        self.pending = None;
        let total: u64 = files.iter().filter_map(|f| f.file_size).sum();
        // Each copy is fetched without asking and kept until the session
        // ends, so a server that keeps copying could otherwise fill the disk:
        // past either limit, the user is asked.
        if total > ASK_ABOVE || self.fetched_total.saturating_add(total) > MAX_SESSION_FETCH_TOTAL {
            tracing::info!("RDP {}: remote copied {total} bytes of files; waiting to be asked", self.id);
            let count = top_level(&files).count();
            self.held = Some((files, lock));
            self.events.emit(ClipEvent::Held { files: count, total });
            return;
        }
        self.fetched_total += total;
        self.start_fetch(files, lock, total);
    }

    /// The user said download: take the held copy, whatever its size.
    fn fetch_held(&mut self) {
        let Some((files, lock)) = self.held.take() else { return };
        if self.fetch.is_some() {
            self.end_fetch(Some(ClipEvent::Cancelled { incoming: true }));
        }
        let total: u64 = files.iter().filter_map(|f| f.file_size).sum();
        self.start_fetch(files, lock, total);
    }

    fn start_fetch(&mut self, files: Vec<FileDescriptor>, lock: Option<u32>, total: u64) {
        self.fetches += 1;
        let root = self.temp_dir.join(self.fetches.to_string());
        if let Err(e) = fs::create_dir_all(&root) {
            tracing::warn!("RDP {}: cannot receive files: {e}", self.id);
            self.events.emit(ClipEvent::Failed { incoming: true, reason: e.to_string() });
            return;
        }
        tracing::info!("RDP {}: receiving {} file(s), {total} bytes", self.id, files.len());
        self.fetch = Some(Fetch {
            lock,
            root,
            files,
            index: 0,
            out: None,
            offset: 0,
            size: 0,
            stream: 0,
            total,
            done: 0,
            last_report: None,
        });
        self.fetch_step();
    }

    /// Open the next file to receive and ask for its first piece, or finish.
    fn fetch_step(&mut self) {
        loop {
            let Some(fetch) = self.fetch.as_mut() else { return };
            if fetch.out.is_some() {
                break;
            }
            let Some(desc) = fetch.files.get(fetch.index) else {
                self.finish_fetch();
                return;
            };
            let target = local_path(&fetch.root, desc);
            let is_dir = desc
                .attributes
                .is_some_and(|a| a.contains(ClipboardFileAttributes::DIRECTORY));
            let opened = if is_dir {
                fs::create_dir_all(&target).map(|_| None)
            } else {
                target
                    .parent()
                    .map(fs::create_dir_all)
                    .unwrap_or(Ok(()))
                    .and_then(|_| File::create(&target))
                    .map(|f| Some(BufWriter::with_capacity(READ_AHEAD, f)))
            };
            match opened {
                Ok(None) => fetch.index += 1,
                Ok(Some(out)) => {
                    fetch.out = Some(out);
                    fetch.offset = 0;
                    fetch.size = desc.file_size.unwrap_or(0);
                    if fetch.size == 0 {
                        // Empty, or of unknown size: ask and see.
                        fetch.out = None;
                        fetch.index += 1;
                        continue;
                    }
                    break;
                }
                Err(e) => {
                    tracing::warn!("RDP {}: cannot write {}: {e}", self.id, target.display());
                    self.end_fetch(Some(ClipEvent::Failed { incoming: true, reason: e.to_string() }));
                    return;
                }
            }
        }
        self.fetch_request();
    }

    fn fetch_request(&mut self) {
        let stream = self.stream_id();
        let Some(fetch) = self.fetch.as_mut() else { return };
        fetch.stream = stream;
        let remaining = fetch.size - fetch.offset;
        let request = FileContentsRequest {
            stream_id: stream,
            index: fetch.index as i32,
            flags: FileContentsFlags::RANGE,
            position: fetch.offset,
            requested_size: u32::try_from(remaining).unwrap_or(FETCH_CHUNK).min(FETCH_CHUNK),
            data_id: fetch.lock,
        };
        self.send(ClipboardMessage::SendFileContentsRequest(request));
    }

    /// A piece of an incoming file.
    fn remote_file_data(&mut self, response: FileContentsResponse<'static>) {
        let Some(fetch) = self.fetch.as_mut() else { return };
        if response.stream_id() != fetch.stream {
            return;
        }
        let data = response.data();
        if response.is_error() || data.is_empty() {
            tracing::warn!("RDP {}: remote stopped sending file {}", self.id, fetch.index);
            let reason = "the remote stopped sending".to_string();
            self.end_fetch(Some(ClipEvent::Failed { incoming: true, reason }));
            return;
        }
        let Some(out) = fetch.out.as_mut() else { return };
        if let Err(e) = out.write_all(data) {
            tracing::warn!("RDP {}: cannot write received file: {e}", self.id);
            self.end_fetch(Some(ClipEvent::Failed { incoming: true, reason: e.to_string() }));
            return;
        }
        fetch.offset += data.len() as u64;
        fetch.done += data.len() as u64;
        if fetch.last_report.is_none_or(|t| t.elapsed() >= PROGRESS_EVERY) {
            fetch.last_report = Some(Instant::now());
            let event = ClipEvent::Progress {
                incoming: true,
                done: fetch.done,
                total: fetch.total,
                files: top_level(&fetch.files).count(),
            };
            self.events.emit(event);
        }
        let Some(fetch) = self.fetch.as_mut() else { return };
        if fetch.offset >= fetch.size {
            let done = fetch.out.take();
            if let Some(mut out) = done {
                let _ = out.flush();
            }
            fetch.index += 1;
            self.fetch_step();
        } else {
            self.fetch_request();
        }
    }

    fn finish_fetch(&mut self) {
        let Some(fetch) = self.fetch.take() else { return };
        let paths: Vec<PathBuf> = top_level(&fetch.files).map(|f| fetch.root.join(&f.name)).collect();
        tracing::info!("RDP {}: received {} item(s) into {}", self.id, paths.len(), fetch.root.display());
        self.local.write_files(&paths);
        // Only worth a word when it took long enough to have been shown.
        if fetch.last_report.is_some() {
            self.events.emit(ClipEvent::Finished { incoming: true, files: paths.len() });
        }
        self.fingerprint = Some(Fingerprint::Files(paths));
    }

    /// Drop the transfer coming in and what it wrote, saying why if asked.
    fn end_fetch(&mut self, outcome: Option<ClipEvent>) {
        if let Some(fetch) = self.fetch.take() {
            drop(fetch.out);
            let _ = fs::remove_dir_all(&fetch.root);
            if let Some(event) = outcome {
                self.events.emit(event);
            }
        }
    }
}

/// The descriptors at the top of a copy: what was selected, not what is
/// inside the folders selected.
fn top_level(files: &[FileDescriptor]) -> impl Iterator<Item = &FileDescriptor> {
    files.iter().filter(|f| f.relative_path.as_deref().is_none_or(str::is_empty))
}

/// Make a local picture into the forms it is offered in. PNG always; the
/// DIBs from it, which is how IronRDP's converters go.
fn prepare_picture(picture: &Picture) -> Option<OfferedPicture> {
    let png = encode_png(picture).map_err(|e| tracing::debug!("RDP clipboard: picture not encoded: {e}")).ok()?;
    let dib = ironrdp_cliprdr_format::bitmap::png_to_cf_dib(&png).ok();
    let dibv5 = ironrdp_cliprdr_format::bitmap::png_to_cf_dibv5(&png).ok();
    Some(OfferedPicture { png, dib, dibv5 })
}

/// The format list for a prepared picture, best first.
fn picture_formats(picture: &OfferedPicture) -> Vec<ClipboardFormat> {
    let mut formats = Vec::new();
    if picture.dibv5.is_some() {
        formats.push(ClipboardFormat::new(ClipboardFormatId::CF_DIBV5));
    }
    if picture.dib.is_some() {
        formats.push(ClipboardFormat::new(ClipboardFormatId::CF_DIB));
    }
    formats.push(
        ClipboardFormat::new(ClipboardFormatId::new(PNG_FORMAT_ID)).with_name(ClipboardFormatName::new_static("PNG")),
    );
    formats
}

/// The picture to ask the remote for, if it copied one: PNG keeps alpha,
/// a DIBV5 may, a plain DIB does not.
fn pick_picture(formats: &[ClipboardFormat]) -> Option<(ClipboardFormatId, PictureFormat)> {
    let named_png = formats
        .iter()
        .find(|f| f.name().is_some_and(|n| PNG_NAMES.iter().any(|p| n.value().eq_ignore_ascii_case(p))));
    if let Some(f) = named_png {
        return Some((f.id(), PictureFormat::Png));
    }
    let has = |id: ClipboardFormatId| formats.iter().any(|f| f.id() == id);
    if has(ClipboardFormatId::CF_DIBV5) {
        return Some((ClipboardFormatId::CF_DIBV5, PictureFormat::DibV5));
    }
    if has(ClipboardFormatId::CF_DIB) {
        return Some((ClipboardFormatId::CF_DIB, PictureFormat::Dib));
    }
    None
}

fn encode_png(picture: &Picture) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let (w, h) = (u32::try_from(picture.width).map_err(|e| e.to_string())?, u32::try_from(picture.height).map_err(|e| e.to_string())?);
    if picture.rgba.len() != picture.width * picture.height * 4 {
        return Err("picture size does not match its bytes".into());
    }
    let mut png = Vec::new();
    // Fast: a screenshot goes out in tens of milliseconds, a little larger.
    image::codecs::png::PngEncoder::new_with_quality(
        &mut png,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(&picture.rgba, w, h, image::ExtendedColorType::Rgba8)
    .map_err(|e| e.to_string())?;
    Ok(png)
}

/// A picture from the remote, in the form it was asked for, as RGBA.
fn decode_picture(format: PictureFormat, data: &[u8]) -> Result<Picture, String> {
    let png;
    let png_bytes = match format {
        PictureFormat::Png => data,
        PictureFormat::Dib => {
            png = ironrdp_cliprdr_format::bitmap::dib_to_png(data).map_err(|e| e.to_string())?;
            &png
        }
        PictureFormat::DibV5 => {
            png = ironrdp_cliprdr_format::bitmap::dibv5_to_png(data).map_err(|e| e.to_string())?;
            &png
        }
    };
    let mut decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(png_bytes)).map_err(|e| e.to_string())?;
    {
        use image::ImageDecoder;
        let (w, h) = decoder.dimensions();
        if (w as usize).saturating_mul(h as usize).saturating_mul(4) > MAX_PICTURE_BYTES {
            return Err(format!("{w}x{h} is too large"));
        }
        decoder.set_limits(image::Limits::default()).map_err(|e| e.to_string())?;
    }
    let image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?.into_rgba8();
    Ok(Picture { width: image.width() as usize, height: image.height() as usize, rgba: image.into_raw() })
}

/// Where a received descriptor lands under `root`. IronRDP has already
/// rejected names that escape; this only turns the wire's `\` into a path.
fn local_path(root: &Path, desc: &FileDescriptor) -> PathBuf {
    let mut path = root.to_path_buf();
    if let Some(rel) = desc.relative_path.as_deref().filter(|r| !r.is_empty()) {
        for part in rel.split('\\').filter(|p| !p.is_empty()) {
            path.push(part);
        }
    }
    path.push(&desc.name);
    path
}

/// Describe local paths for the remote, directories walked, and the local
/// path behind each descriptor in the same order — that order is the index
/// the server uses when it asks for contents.
fn describe(paths: &[PathBuf]) -> (Vec<FileDescriptor>, Vec<PathBuf>) {
    let mut descriptors = Vec::new();
    let mut files = Vec::new();
    for path in paths {
        walk(path, None, &mut descriptors, &mut files);
    }
    (descriptors, files)
}

fn walk(path: &Path, relative: Option<&str>, descriptors: &mut Vec<FileDescriptor>, files: &mut Vec<PathBuf>) {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else { return };
    let Ok(meta) = fs::metadata(path) else { return };

    let mut desc = FileDescriptor::new(name).with_last_write_time(filetime(meta.modified().ok()));
    if let Some(rel) = relative {
        desc = desc.with_relative_path(rel);
    }

    if meta.is_dir() {
        descriptors.push(desc.with_attributes(ClipboardFileAttributes::DIRECTORY).with_file_size(0));
        files.push(path.to_path_buf());
        let below = match relative {
            Some(rel) => format!("{rel}\\{name}"),
            None => name.to_string(),
        };
        if let Ok(entries) = fs::read_dir(path) {
            let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            entries.sort();
            for entry in entries {
                walk(&entry, Some(&below), descriptors, files);
            }
        }
    } else {
        descriptors.push(
            desc.with_attributes(ClipboardFileAttributes::NORMAL)
                .with_file_size(meta.len()),
        );
        files.push(path.to_path_buf());
    }
}

/// Windows FILETIME: hundreds of nanoseconds since 1601.
fn filetime(time: Option<SystemTime>) -> u64 {
    const EPOCH_GAP_SECS: u64 = 11_644_473_600;
    let since_unix = time
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    (since_unix.as_secs() + EPOCH_GAP_SECS) * 10_000_000 + u64::from(since_unix.subsec_nanos() / 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(width: usize, height: usize) -> Picture {
        let mut rgba = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let on = (x + y) % 2 == 0;
                rgba.extend_from_slice(if on { &[255, 0, 0, 255] } else { &[0, 0, 255, 128] });
            }
        }
        Picture { width, height, rgba }
    }

    #[test]
    fn a_picture_survives_png_both_ways() {
        let picture = checker(7, 5);
        let prepared = prepare_picture(&picture).expect("prepared");
        let back = decode_picture(PictureFormat::Png, &prepared.png).unwrap();
        assert_eq!((back.width, back.height), (7, 5));
        assert_eq!(back.rgba, picture.rgba, "PNG keeps every pixel, alpha included");
    }

    #[test]
    fn a_picture_survives_a_dib_with_its_colours() {
        let picture = checker(6, 4);
        let prepared = prepare_picture(&picture).expect("prepared");
        for (format, bytes) in [
            (PictureFormat::Dib, prepared.dib.as_ref().expect("dib")),
            (PictureFormat::DibV5, prepared.dibv5.as_ref().expect("dibv5")),
        ] {
            let back = decode_picture(format, bytes).unwrap();
            assert_eq!((back.width, back.height), (6, 4), "{format:?}");
            // Rows are bottom-up in a DIB; the first pixel must still be the
            // top-left one, and red.
            assert_eq!(&back.rgba[0..3], &[255, 0, 0], "{format:?}");
            assert_eq!(&back.rgba[4..7], &[0, 0, 255], "{format:?}");
        }
    }

    #[test]
    fn an_offered_picture_lists_dibs_and_a_named_png() {
        let prepared = prepare_picture(&checker(2, 2)).unwrap();
        let formats = picture_formats(&prepared);
        let ids: Vec<u32> = formats.iter().map(|f| f.id().value()).collect();
        assert_eq!(ids, vec![17, 8, PNG_FORMAT_ID]);
        assert_eq!(formats[2].name().map(|n| n.value()), Some("PNG"));
    }

    #[test]
    fn a_remote_picture_is_taken_as_png_first() {
        let png = ClipboardFormat::new(ClipboardFormatId::new(0xC123)).with_name(ClipboardFormatName::new_static("image/png"));
        let dib = ClipboardFormat::new(ClipboardFormatId::CF_DIB);
        let dibv5 = ClipboardFormat::new(ClipboardFormatId::CF_DIBV5);
        let text = ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT);
        assert_eq!(pick_picture(&[dib.clone(), png.clone()]), Some((ClipboardFormatId::new(0xC123), PictureFormat::Png)));
        assert_eq!(pick_picture(&[dib.clone(), dibv5]), Some((ClipboardFormatId::CF_DIBV5, PictureFormat::DibV5)));
        assert_eq!(pick_picture(&[dib]), Some((ClipboardFormatId::CF_DIB, PictureFormat::Dib)));
        assert_eq!(pick_picture(&[text]), None);
    }

    #[test]
    fn rubbish_is_not_a_picture() {
        assert!(decode_picture(PictureFormat::Png, b"not a png").is_err());
        assert!(decode_picture(PictureFormat::Dib, &[0u8; 10]).is_err());
    }

    #[test]
    fn only_the_top_of_a_copy_is_counted() {
        let files = vec![
            FileDescriptor::new("dir"),
            FileDescriptor::new("inner.txt").with_relative_path("dir"),
            FileDescriptor::new("top.txt"),
        ];
        let names: Vec<&str> = top_level(&files).map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["dir", "top.txt"]);
    }

    #[test]
    fn clip_events_reach_the_webview_in_camel_case() {
        let json = serde_json::to_value(ClipEvent::Progress { incoming: true, done: 1, total: 2, files: 3 }).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "progress", "incoming": true, "done": 1, "total": 2, "files": 3 }));
        let json = serde_json::to_value(ClipEvent::HeldGone).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "heldGone" }));
    }

    #[test]
    fn a_folder_is_described_with_its_contents_in_index_order() {
        let dir = std::env::temp_dir().join(format!("reach-clip-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), b"hello").unwrap();
        fs::write(dir.join("sub").join("b.bin"), [0u8; 3]).unwrap();

        let (descriptors, files) = describe(std::slice::from_ref(&dir));
        assert_eq!(descriptors.len(), files.len());
        let names: Vec<(String, Option<String>, Option<u64>)> = descriptors
            .iter()
            .map(|d| (d.name.clone(), d.relative_path.clone(), d.file_size))
            .collect();
        let top = dir.file_name().unwrap().to_str().unwrap().to_string();
        assert_eq!(names[0], (top.clone(), None, Some(0)));
        assert_eq!(names[1], ("a.txt".into(), Some(top.clone()), Some(5)));
        assert_eq!(names[2], ("sub".into(), Some(top.clone()), Some(0)));
        assert_eq!(names[3], ("b.bin".into(), Some(format!("{top}\\sub")), Some(3)));
        assert!(descriptors[0].attributes.unwrap().contains(ClipboardFileAttributes::DIRECTORY));
        assert_eq!(files[3], dir.join("sub").join("b.bin"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn received_paths_follow_the_wire_separator() {
        let d = FileDescriptor::new("b.bin").with_relative_path("top\\sub");
        assert_eq!(local_path(Path::new("/r"), &d), Path::new("/r").join("top").join("sub").join("b.bin"));
    }

    #[test]
    fn filetime_of_the_unix_epoch_is_the_documented_constant() {
        assert_eq!(filetime(Some(UNIX_EPOCH)), 116_444_736_000_000_000);
        assert_eq!(filetime(None), 116_444_736_000_000_000);
    }
}
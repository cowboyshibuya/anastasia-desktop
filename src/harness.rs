//! The desktop's only agent boundary. The engine owns sessions and tool policy.

use std::{path::PathBuf, sync::atomic::{AtomicU64, Ordering}, time::Duration};

use anastasia_harness_api::{
    API_VERSION_MAJOR, ApiEvent, ApiRequest, ClientFrame, ServerFrame, api_socket_path,
};
use anastasia_transport::Stream;
use smol::channel;
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, sync::mpsc};

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub enum HarnessEvent {
    Connected,
    Frame(ServerFrame),
    Disconnected(String),
}

pub struct Harness {
    commands: mpsc::UnboundedSender<ClientFrame>,
    next_id: AtomicU64,
    pub events: channel::Receiver<HarnessEvent>,
}

impl Harness {
    pub fn start() -> Self {
        let (commands, receiver) = mpsc::unbounded_channel();
        let (events_tx, events) = channel::unbounded();
        std::thread::Builder::new()
            .name("anastasia-harness-client".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("harness client runtime");
                runtime.block_on(run(receiver, events_tx));
            })
            .expect("harness client thread");
        Self { commands, next_id: AtomicU64::new(1), events }
    }

    pub fn send(&self, request: ApiRequest) -> Result<u64, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.commands.send(ClientFrame::new(id, request))
            .map_err(|_| "Harness connection closed".to_string())?;
        Ok(id)
    }
}

async fn run(mut commands: mpsc::UnboundedReceiver<ClientFrame>, events: channel::Sender<HarnessEvent>) {
    let mut launched = false;
    let mut failed_connects = 0u32;
    let mut reported_unavailable = false;
    loop {
        let path = api_socket_path();
        let stream = match Stream::connect(&path).await {
            Ok(stream) => stream,
            Err(error) => {
                failed_connects = failed_connects.saturating_add(1);
                if !launched {
                    launched = true;
                    if let Err(start_error) = launch_engine() {
                        let _ = events.send(HarnessEvent::Disconnected(format!(
                            "Cannot connect to the Anastasia engine: {error}. {start_error}"
                        ))).await;
                    }
                }
                if failed_connects >= 10 && !reported_unavailable {
                    reported_unavailable = true;
                    let _ = events.send(HarnessEvent::Disconnected(format!(
                        "Engine is unavailable at {}: {error}", path.display()
                    ))).await;
                }
                while commands.try_recv().is_ok() {} // never replay a possibly stale user action
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        failed_connects = 0;
        reported_unavailable = false;
        let (read, mut write) = stream.into_split();
        let mut read = BufReader::new(read);
        let hello = ClientFrame::new(0, ApiRequest::Hello {
            min_version: API_VERSION_MAJOR,
            max_version: API_VERSION_MAJOR,
            client: "anastasia-desktop/0.1".into(),
        });
        let handshake = async {
            write.write_all(serde_json::to_string(&hello)?.as_bytes()).await?;
            write.write_all(b"\n").await?;
            let mut bytes = Vec::new();
            read_frame(&mut read, &mut bytes).await?;
            Ok::<ServerFrame, anyhow::Error>(serde_json::from_slice(&bytes)?)
        };
        let handshake = tokio::time::timeout(Duration::from_secs(5), handshake).await;
        let accepted = matches!(handshake, Ok(Ok(ServerFrame {
            reply_to: Some(0), event: ApiEvent::HelloOk { version: API_VERSION_MAJOR, .. }, ..
        })));
        if !accepted {
            let _ = events.send(HarnessEvent::Disconnected(format!(
                "The engine did not accept the desktop API handshake: {handshake:?}"
            ))).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        if events.send(HarnessEvent::Connected).await.is_err() { break; }
        let mut bytes = Vec::new();
        let ended = loop {
            tokio::select! {
                maybe = commands.recv() => {
                    let Some(frame) = maybe else { return; };
                    match serde_json::to_vec(&frame) {
                        Ok(mut line) => {
                            line.push(b'\n');
                            if let Err(error) = write.write_all(&line).await {
                                break error.to_string();
                            }
                        }
                        Err(error) => break error.to_string(),
                    }
                }
                result = read_frame(&mut read, &mut bytes) => {
                    match result {
                        Ok(()) => match serde_json::from_slice::<ServerFrame>(&bytes) {
                            Ok(frame) if frame.v == API_VERSION_MAJOR => {
                                if !matches!(frame.event, ApiEvent::Unknown)
                                    && events.send(HarnessEvent::Frame(frame)).await.is_err() {
                                    return;
                                }
                                bytes.clear();
                            }
                            Ok(_) => break "Engine API version changed".into(),
                            Err(error) => break format!("Invalid engine event: {error}"),
                        }
                        Err(error) => break error.to_string(),
                    }
                }
            }
        };
        while commands.try_recv().is_ok() {} // reconnection needs a fresh UI snapshot
        if events.send(HarnessEvent::Disconnected(ended)).await.is_err() { break; }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn read_frame<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R, bytes: &mut Vec<u8>) -> std::io::Result<()> {
    let remaining = MAX_FRAME_BYTES.saturating_sub(bytes.len());
    let mut limited = tokio::io::AsyncReadExt::take(reader, remaining as u64);
    limited.read_until(b'\n', bytes).await?;
    if bytes.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "engine closed the stream"));
    }
    if bytes.len() == MAX_FRAME_BYTES && !bytes.ends_with(b"\n") {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "engine frame too large"));
    }
    if !bytes.ends_with(b"\n") {
        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "incomplete engine frame"));
    }
    Ok(())
}

fn launch_engine() -> Result<(), String> {
    let binary = engine_binary().ok_or_else(||
        "Set ANASTASIA_ENGINE_BIN to a matching engine binary, or install a bundled release.".to_string()
    )?;
    std::process::Command::new(&binary)
        .arg("--no-update")
        .arg("api-bridge")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not start {}: {error}", binary.display()))
}

fn engine_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ANASTASIA_ENGINE_BIN") {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    let directory = exe.parent()?;
    let name = if cfg!(windows) { "anastasia-engine.exe" } else { "anastasia-engine" };
    [directory.join(name), directory.join("../Resources").join(name)]
        .into_iter()
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cut_or_oversized_frames_never_become_events() {
        for input in [b"{\"v\":1".as_slice(), &vec![b'x'; MAX_FRAME_BYTES][..]] {
            let mut reader = BufReader::new(input);
            assert!(read_frame(&mut reader, &mut Vec::new()).await.is_err());
        }
    }
}

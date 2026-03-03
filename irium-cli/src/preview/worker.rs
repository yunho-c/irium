use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
};

use image::DynamicImage;

use crate::preview::{PreviewBuildResult, PreviewSourceMeta, build_preview};

#[derive(Debug)]
pub enum PreviewWorkerCommand {
    GeneratePreview { request_id: u64, path: PathBuf },
    Shutdown,
}

#[derive(Debug)]
pub enum PreviewWorkerEvent {
    PreviewReady {
        request_id: u64,
        path: PathBuf,
        image: DynamicImage,
        meta: PreviewSourceMeta,
    },
    PreviewUnsupported {
        request_id: u64,
        path: PathBuf,
        reason: String,
    },
    PreviewError {
        request_id: u64,
        path: PathBuf,
        message: String,
    },
}

pub fn spawn_worker() -> Result<(Sender<PreviewWorkerCommand>, Receiver<PreviewWorkerEvent>), String>
{
    let (cmd_tx, cmd_rx) = mpsc::channel::<PreviewWorkerCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<PreviewWorkerEvent>();

    let _ = std::thread::Builder::new()
        .name("irium-preview-worker".to_string())
        .spawn(move || worker_loop(cmd_rx, evt_tx))
        .map_err(|error| format!("Failed to spawn preview worker thread: {error}"))?;

    Ok((cmd_tx, evt_rx))
}

fn worker_loop(cmd_rx: Receiver<PreviewWorkerCommand>, evt_tx: Sender<PreviewWorkerEvent>) {
    while let Ok(command) = cmd_rx.recv() {
        match command {
            PreviewWorkerCommand::Shutdown => break,
            PreviewWorkerCommand::GeneratePreview { request_id, path } => {
                let (request_id, path) = drain_to_latest_generate(&cmd_rx, request_id, path);
                match build_preview(&path) {
                    Ok(PreviewBuildResult::Ready { image, meta }) => {
                        let _ = evt_tx.send(PreviewWorkerEvent::PreviewReady {
                            request_id,
                            path,
                            image,
                            meta,
                        });
                    }
                    Ok(PreviewBuildResult::Unsupported { reason }) => {
                        let _ = evt_tx.send(PreviewWorkerEvent::PreviewUnsupported {
                            request_id,
                            path,
                            reason,
                        });
                    }
                    Err(message) => {
                        let _ = evt_tx.send(PreviewWorkerEvent::PreviewError {
                            request_id,
                            path,
                            message,
                        });
                    }
                }
            }
        }
    }
}

fn drain_to_latest_generate(
    cmd_rx: &Receiver<PreviewWorkerCommand>,
    mut request_id: u64,
    mut path: PathBuf,
) -> (u64, PathBuf) {
    loop {
        match cmd_rx.try_recv() {
            Ok(PreviewWorkerCommand::GeneratePreview {
                request_id: next_id,
                path: next_path,
            }) => {
                request_id = next_id;
                path = next_path;
            }
            Ok(PreviewWorkerCommand::Shutdown) => break,
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
        }
    }

    (request_id, path)
}

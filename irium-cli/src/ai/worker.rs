use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::ai::{
    ModelListItem, analyze::analyze_files, models::discover_models,
    suggest::generate_filename_suggestions,
};

#[derive(Debug, Clone)]
pub enum AiWorkerCommand {
    GenerateSuggestions {
        request_id: u64,
        api_key: String,
        model_id: String,
        paths: Vec<PathBuf>,
        user_prompt: Option<String>,
    },
    DiscoverModels {
        request_id: u64,
        api_key: Option<String>,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerProgressStage {
    DiscoveringModels,
    AnalyzingFiles,
    Generating,
}

#[derive(Debug, Clone)]
pub enum AiWorkerEvent {
    ProgressUpdate {
        request_id: u64,
        stage: WorkerProgressStage,
    },
    ModelsDiscovered {
        request_id: u64,
        models: Vec<ModelListItem>,
    },
    SuggestionsReady {
        request_id: u64,
        source_model: String,
        per_path_options: HashMap<PathBuf, [String; 3]>,
        warnings: Vec<String>,
        skipped_files: usize,
    },
    AiError {
        request_id: u64,
        message: String,
    },
}

pub fn spawn_worker() -> Result<(Sender<AiWorkerCommand>, Receiver<AiWorkerEvent>), String> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<AiWorkerCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<AiWorkerEvent>();

    let _ = std::thread::Builder::new()
        .name("irium-ai-worker".to_string())
        .spawn(move || worker_loop(cmd_rx, evt_tx))
        .map_err(|e| format!("Failed to spawn AI worker thread: {e}"))?;

    Ok((cmd_tx, evt_rx))
}

fn worker_loop(cmd_rx: Receiver<AiWorkerCommand>, evt_tx: Sender<AiWorkerEvent>) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(error) => {
            let _ = evt_tx.send(AiWorkerEvent::AiError {
                request_id: 0,
                message: format!("Failed to initialize async runtime: {error}"),
            });
            return;
        }
    };

    while let Ok(command) = cmd_rx.recv() {
        match command {
            AiWorkerCommand::Shutdown => break,
            AiWorkerCommand::DiscoverModels {
                request_id,
                api_key,
            } => {
                let _ = evt_tx.send(AiWorkerEvent::ProgressUpdate {
                    request_id,
                    stage: WorkerProgressStage::DiscoveringModels,
                });
                match runtime.block_on(discover_models(api_key.as_deref())) {
                    Ok(models) => {
                        let _ = evt_tx.send(AiWorkerEvent::ModelsDiscovered { request_id, models });
                    }
                    Err(error) => {
                        let _ = evt_tx.send(AiWorkerEvent::AiError {
                            request_id,
                            message: error,
                        });
                    }
                }
            }
            AiWorkerCommand::GenerateSuggestions {
                request_id,
                api_key,
                model_id,
                paths,
                user_prompt,
            } => {
                let _ = evt_tx.send(AiWorkerEvent::ProgressUpdate {
                    request_id,
                    stage: WorkerProgressStage::AnalyzingFiles,
                });
                let analyzed = runtime.block_on(analyze_files(paths));
                let mut warnings = analyzed.warnings;
                for file in &analyzed.files {
                    warnings.extend(file.warnings.clone());
                }

                let _ = evt_tx.send(AiWorkerEvent::ProgressUpdate {
                    request_id,
                    stage: WorkerProgressStage::Generating,
                });
                match runtime.block_on(generate_filename_suggestions(
                    &api_key,
                    &model_id,
                    &analyzed.files,
                    user_prompt.as_deref(),
                )) {
                    Ok(per_path_options) => {
                        let _ = evt_tx.send(AiWorkerEvent::SuggestionsReady {
                            request_id,
                            source_model: model_id,
                            per_path_options,
                            warnings,
                            skipped_files: analyzed.skipped_count,
                        });
                    }
                    Err(error) => {
                        let _ = evt_tx.send(AiWorkerEvent::AiError {
                            request_id,
                            message: error,
                        });
                    }
                }
            }
        }
    }
}

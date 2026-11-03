//! Messages to the user (`window/showMessage`, `window/logMessage`), shared by the message loop
//! and the analysis thread so that one broken config is one message, whoever notices it.

use core_incremental::{Failure, Stage};
use crossbeam_channel::Sender;
use lsp_server::{Message, Notification};
use lsp_types::notification::{LogMessage, Notification as LspNotification, ShowMessage};
use lsp_types::{LogMessageParams, MessageType, ShowMessageParams, Uri};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct Notifier {
    sender: Sender<Message>,
    /// Failures already shown to the user.
    shown: Arc<Mutex<HashSet<String>>>,
}

impl Notifier {
    pub(crate) fn new(sender: Sender<Message>) -> Self {
        Notifier {
            sender,
            shown: Default::default(),
        }
    }

    /// Forgets what was shown, so that a reloaded config reports its problems again.
    pub(crate) fn reset(&self) {
        self.shown.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    fn first_time(&self, message: &str) -> bool {
        self.shown
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(message.to_string())
    }

    fn send<N: LspNotification>(&self, params: N::Params)
    where
        N::Params: serde::Serialize,
    {
        let _ = self
            .sender
            .send(Notification::new(N::METHOD.to_string(), params).into());
    }

    fn show(&self, typ: MessageType, message: String) {
        self.send::<ShowMessage>(ShowMessageParams { typ, message });
    }

    fn log(&self, typ: MessageType, message: String) {
        self.send::<LogMessage>(LogMessageParams { typ, message });
    }

    /// The config could not be loaded. The message is the one every file's failure carries, so it
    /// counts as shown for them.
    pub(crate) fn config_failed(&self, error: String) {
        let message = format!("Sudsy: {error}");
        if self.first_time(&message) {
            self.show(MessageType::ERROR, message);
        }
    }

    /// Tells the user why a file could not be processed: once per distinct reason when it is
    /// something they can fix (config, worker), in the log when it is about one file's content.
    pub(crate) fn failed(&self, uri: &Uri, failure: &Failure) {
        let message = format!("Sudsy: {}", failure.message);
        match failure.stage {
            Stage::Unsupported => {}
            Stage::Format => self.log(
                MessageType::WARNING,
                format!("{}: {}", uri.as_str(), failure.message),
            ),
            Stage::Config | Stage::Options | Stage::Parse | Stage::Semantics => {
                if self.first_time(&message) {
                    let level = if matches!(failure.stage, Stage::Parse | Stage::Semantics) {
                        MessageType::WARNING
                    } else {
                        MessageType::ERROR
                    };
                    self.show(level, message);
                }
            }
        }
    }
}

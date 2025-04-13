use std::collections::HashSet;

use futures::channel::oneshot;
use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};

use super::{EventLoop, FileResponse};

#[derive(Debug)]
pub(crate) enum Command {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), anyhow::Error>>,
    },
    RegisterName {
        username: String,
    },
    FindUser {
        username: String,
        sender: oneshot::Sender<Result<PeerId, anyhow::Error>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), anyhow::Error>>,
    },
    StartProviding {
        file_name: String,
        sender: oneshot::Sender<()>,
    },
    GetProviders {
        file_name: String,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    RequestFile {
        file_name: String,
        peer: PeerId,
        sender: oneshot::Sender<Result<Vec<u8>, anyhow::Error>>,
    },
    RespondFile {
        file: Vec<u8>,
        channel: ResponseChannel<FileResponse>,
    },
    SendMessage {
        message: String,
    },
    DirectMessage {
        peer_id: PeerId,
        message: String,
        sender: oneshot::Sender<Result<(), anyhow::Error>>,
    },
}

impl EventLoop {
    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { addr, sender } => self.handle_start_listening(addr, sender),
            Command::RegisterName { username } => self.handle_register_name(username),
            Command::FindUser { username, sender } => self.handle_find_user(username, sender),
            Command::Dial {
                peer_id,
                peer_addr,
                sender,
            } => self.handle_dial(peer_id, peer_addr, sender),
            Command::StartProviding { file_name, sender } => {
                self.handle_start_providing(file_name, sender);
            }
            Command::GetProviders { file_name, sender } => {
                self.handle_get_providers(file_name, sender);
            }
            Command::RequestFile {
                file_name,
                peer,
                sender,
            } => self.handle_request_file(file_name, peer, sender),
            Command::RespondFile { file, channel } => self.handle_respond_file(file, channel),
            Command::SendMessage { message } => self.handle_send_message(&message),
            Command::DirectMessage {
                peer_id,
                message,
                sender,
            } => {
                self.handle_direct_message(&peer_id, message, sender);
            }
        }
    }
}

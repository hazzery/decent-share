use std::{collections::HashSet, error::Error};

use futures::channel::oneshot;
use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};

use super::{EventLoop, FileResponse};

#[derive(Debug)]
pub(crate) enum Command {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    RegisterName {
        username: String,
    },
    FindUser {
        username: String,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
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
        sender: oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
    },
    RespondFile {
        file: Vec<u8>,
        channel: ResponseChannel<FileResponse>,
    },
    SendMessage {
        message: String,
    },
    DirectMessage {
        username: String,
        message: String,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
}

impl EventLoop {
    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { addr, sender } => self.handle_start_listening(addr, sender),
            Command::RegisterName { username } => self.handle_register_name(username),
            Command::FindUser { username } => self.handle_find_user(username),
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
                username,
                message,
                sender,
            } => {
                self.handle_direct_message(&username, message, sender);
            }
        }
    }
}

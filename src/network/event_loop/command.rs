use std::path::PathBuf;

use futures::channel::oneshot;
use libp2p::{kad, PeerId};

use super::EventLoop;

#[derive(Debug)]
pub(crate) enum Command {
    RegisterName {
        username: String,
        status_sender: oneshot::Sender<Result<(), kad::PutRecordError>>,
    },
    FindPeerId {
        username: String,
        peer_id_sender: oneshot::Sender<Option<PeerId>>,
    },
    FindPeerUsername {
        peer_id: PeerId,
        username_sender: oneshot::Sender<Result<String, anyhow::Error>>,
    },
    MakeOffer {
        offered_file_name: String,
        offered_file_bytes: Vec<u8>,
        peer_id: PeerId,
        requested_file_name: String,
        requested_file_path: PathBuf,
        error_sender: oneshot::Sender<Result<(), anyhow::Error>>,
    },
    RespondTrade {
        peer_id: PeerId,
        requested_file_name: String,
        offered_file_name: String,
        requested_file_bytes: Option<Vec<u8>>,
        offered_bytes_sender: Option<oneshot::Sender<Result<Option<Vec<u8>>, anyhow::Error>>>,
    },
    SendMessage {
        message: String,
    },
    DirectMessage {
        peer_id: PeerId,
        message: String,
        error_sender: oneshot::Sender<Result<(), anyhow::Error>>,
    },
}

impl EventLoop {
    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::RegisterName {
                username,
                status_sender,
            } => self.handle_register_name(&username, status_sender),
            Command::FindPeerId {
                username,
                peer_id_sender,
            } => self.handle_find_peer_id(&username, peer_id_sender),
            Command::FindPeerUsername {
                peer_id,
                username_sender,
            } => self.handle_find_peer_username(peer_id, username_sender),
            Command::MakeOffer {
                offered_file_name,
                offered_file_bytes,
                peer_id,
                requested_file_name,
                requested_file_path,
                error_sender,
            } => self.handle_make_offer(
                offered_file_name,
                offered_file_bytes,
                peer_id,
                requested_file_name,
                requested_file_path,
                error_sender,
            ),
            Command::RespondTrade {
                peer_id,
                requested_file_name,
                offered_file_name,
                requested_file_bytes,
                offered_bytes_sender,
            } => self.handle_respond_trade(
                peer_id,
                requested_file_name,
                offered_file_name,
                requested_file_bytes,
                offered_bytes_sender,
            ),
            Command::SendMessage { message } => self.handle_send_message(&message),
            Command::DirectMessage {
                peer_id,
                message,
                error_sender,
            } => {
                self.handle_direct_message(&peer_id, message, error_sender);
            }
        }
    }
}

use std::path::PathBuf;

use futures::channel::oneshot;
use libp2p::PeerId;

use super::EventLoop;

#[derive(Debug)]
pub(crate) enum Command {
    RegisterName {
        username: String,
    },
    FindUser {
        username: String,
        sender: oneshot::Sender<Result<PeerId, anyhow::Error>>,
    },
    MakeOffer {
        offered_file_name: String,
        offered_file_bytes: Vec<u8>,
        peer_id: PeerId,
        requested_file_name: String,
        requested_file_path: PathBuf,
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
        sender: oneshot::Sender<Result<(), anyhow::Error>>,
    },
}

impl EventLoop {
    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::RegisterName { username } => self.handle_register_name(&username),
            Command::FindUser { username, sender } => self.handle_find_user(&username, sender),
            Command::MakeOffer {
                offered_file_name,
                offered_file_bytes,
                peer_id,
                requested_file_name,
                requested_file_path,
            } => self.handle_make_offer(
                offered_file_name,
                offered_file_bytes,
                peer_id,
                requested_file_name,
                requested_file_path,
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
                sender,
            } => {
                self.handle_direct_message(&peer_id, message, sender);
            }
        }
    }
}

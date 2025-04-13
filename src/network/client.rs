use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use libp2p::{core::Multiaddr, request_response::ResponseChannel, PeerId};

use super::FileResponse;
use super::{event_loop::Command, username_store::UsernameStore};

#[derive(Clone)]
pub(crate) struct Client {
    pub(super) sender: mpsc::Sender<Command>,
    username_store: Arc<Mutex<UsernameStore>>,
}

impl Client {
    /// Listen for incoming connections on the given address.
    pub(crate) async fn start_listening(&mut self, addr: Multiaddr) -> Result<(), anyhow::Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartListening { addr, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Dial the given peer at the given address.
    pub(crate) async fn dial(
        &mut self,
        peer_id: PeerId,
        peer_addr: Multiaddr,
    ) -> Result<(), anyhow::Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::Dial {
                peer_id,
                peer_addr,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Advertise the local node as the provider of the given file on the DHT.
    pub(crate) async fn start_providing(&mut self, file_name: String) {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartProviding { file_name, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.");
    }

    /// Find the providers for the given file on the DHT.
    pub(crate) async fn get_providers(&mut self, file_name: String) -> HashSet<PeerId> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::GetProviders { file_name, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Request the content of the given file from the given peer.
    pub(crate) async fn request_file(
        &mut self,
        peer: PeerId,
        file_name: String,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::RequestFile {
                file_name,
                peer,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not be dropped.")
    }

    /// Respond with the provided file content to the given request.
    pub(crate) async fn respond_file(
        &mut self,
        file: Vec<u8>,
        channel: ResponseChannel<FileResponse>,
    ) {
        self.sender
            .send(Command::RespondFile { file, channel })
            .await
            .expect("Command receiver not to be dropped.");
    }

    pub(crate) async fn register_username(&mut self, username: String) {
        self.sender
            .send(Command::RegisterName { username })
            .await
            .expect("Name to register successfully");
    }

    pub(crate) async fn find_user(&mut self, username: String) -> Option<PeerId> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::FindUser {
                username: username.clone(),
                sender,
            })
            .await
            .expect("Name to be successfully found");

        let peer_id = match receiver.await.expect("Sender not be dropped.") {
            Ok(peer_id) => peer_id,
            Err(error) => {
                eprintln!("{error:?}");
                return None;
            }
        };
        self.username_store
            .lock()
            .unwrap()
            .insert(username, peer_id);
        Some(peer_id)
    }

    pub(crate) async fn send_message(&mut self, message: String) {
        self.sender
            .send(Command::SendMessage { message })
            .await
            .expect("Message to send successfully");
    }

    pub(crate) async fn direct_message(
        &mut self,
        username: String,
        message: String,
    ) -> Result<(), anyhow::Error> {
        let (sender, receiver) = oneshot::channel();

        let peer_id = match self.username_store.lock().unwrap().get_peer_id(&username) {
            Some(peer_id) => peer_id.clone(),
            None => match self.find_user(username).await {
                Some(peer_id) => peer_id,
                None => return Err(anyhow!("The given username does not exist")),
            },
        };

        self.sender
            .send(Command::DirectMessage {
                peer_id,
                message,
                sender,
            })
            .await
            .expect("Direct message to send successfully");
        receiver.await.expect("Command receiver to not be dropped")
    }
}

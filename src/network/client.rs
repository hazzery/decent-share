use std::{
    borrow::ToOwned,
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use libp2p::{core::Multiaddr, request_response::ResponseChannel, PeerId};

use super::{event_loop::Command, FileResponse, TradeResponse};

#[derive(Clone)]
pub(crate) struct Client {
    pub(super) sender: mpsc::Sender<Command>,
    pub(super) username_peer_id_map: Arc<Mutex<HashMap<String, PeerId>>>,
}

impl Client {
    async fn get_peer_id(&mut self, username: String) -> Result<PeerId, anyhow::Error> {
        let peer_id = self
            .username_peer_id_map
            .lock()
            .unwrap()
            .get(&username)
            .map(ToOwned::to_owned);

        match peer_id {
            Some(peer_id) => Ok(peer_id),
            None => self.find_user(username).await,
        }
    }
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

    pub(crate) async fn offer_trade(
        &mut self,
        offered_file_name: String,
        recipient_username: String,
        requested_file_name: String,
    ) -> Result<Option<Vec<u8>>, anyhow::Error> {
        let peer_id = self.get_peer_id(recipient_username).await?;

        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::MakeOffer {
                offered_file_name,
                peer_id,
                requested_file_name,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped")
    }

    pub(crate) async fn respond_to_trade(
        &mut self,
        response: Option<Vec<u8>>,
        channel: ResponseChannel<TradeResponse>,
    ) {
        self.sender
            .send(Command::RespondTrade { response, channel })
            .await
            .expect("Command receiver to not be dropped");
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

    pub(crate) async fn find_user(&mut self, username: String) -> Result<PeerId, anyhow::Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::FindUser {
                username: username.clone(),
                sender,
            })
            .await
            .expect("Error sending FindUser command");

        let peer_id = receiver.await.expect("Sender not be dropped.");

        if let Ok(peer_id) = peer_id {
            self.username_peer_id_map
                .lock()
                .unwrap()
                .insert(username, peer_id);
        }

        peer_id
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

        let peer_id = self.get_peer_id(username).await?;

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

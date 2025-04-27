use std::{
    borrow::ToOwned,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, bail};
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use libp2p::{gossipsub, kad, PeerId};

use super::{event_loop::Command, username_store::UsernameStore};

#[derive(Clone)]
pub(crate) struct Client {
    pub(super) command_sender: mpsc::Sender<Command>,
    pub(super) username_store: Arc<Mutex<UsernameStore>>,
}

impl Client {
    /// Search the DHT for the peer ID associated with a given username if we
    /// don't already have it cached.
    pub(crate) async fn get_peer_id(&mut self, username: String) -> Option<PeerId> {
        let mut peer_id = self
            .username_store
            .lock()
            .unwrap()
            .get_peer_id(&username)
            .map(ToOwned::to_owned);

        if peer_id.is_none() {
            peer_id = self.find_user(username).await;
        }
        peer_id
    }

    /// Search the DHT for the username associated with a given peeer ID if we
    /// don't already have it cached.
    pub(crate) async fn get_username(&mut self, peer_id: PeerId) -> Result<String, anyhow::Error> {
        let username = self
            .username_store
            .lock()
            .unwrap()
            .get_username(&peer_id)
            .map(ToOwned::to_owned);

        match username {
            Some(username) => Ok(username),
            None => self.find_peer_username(peer_id).await,
        }
    }
}

/// Send messages to the network thread in the form of `Command` enum values
/// This allows the main thread to remain responsive to the user interface
/// while the network thread handles networking.
impl Client {
    pub(crate) async fn offer_trade(
        &mut self,
        offered_file_name: String,
        offered_file_bytes: Vec<u8>,
        recipient_username: String,
        requested_file_name: String,
        requested_file_path: PathBuf,
    ) -> Result<(), anyhow::Error> {
        let Some(peer_id) = self.get_peer_id(recipient_username.clone()).await else {
            bail!("'{recipient_username}' is not a registered user");
        };

        let (error_sender, error_receiver) = oneshot::channel();

        self.command_sender
            .send(Command::MakeTradeOffer {
                offered_file_name,
                offered_file_bytes,
                peer_id,
                requested_file_name,
                requested_file_path,
                error_sender,
            })
            .await
            .expect("Command receiver was dropped");

        error_receiver.await.expect("Error receiver was dropped")
    }

    pub(crate) async fn accept_trade(
        &mut self,
        username: String,
        requested_file_name: String,
        offered_file_name: String,
        requested_file_bytes: Vec<u8>,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let Some(peer_id) = self.get_peer_id(username.clone()).await else {
            bail!("'{username}' is not a register user");
        };

        let (offered_bytes_sender, offered_bytes_receiver) = oneshot::channel();

        self.command_sender
            .send(Command::RespondTrade {
                peer_id,
                requested_file_name,
                offered_file_name,
                requested_file_bytes: Some(requested_file_bytes),
                offered_bytes_sender: Some(offered_bytes_sender),
            })
            .await
            .expect("Command receiver was dropped");

        match offered_bytes_receiver
            .await
            .expect("Offered bytes was dropped")
        {
            Ok(Some(bytes)) => Ok(bytes),
            Ok(None) => Err(anyhow!("No bytes were received!")),
            Err(error) => Err(error),
        }
    }

    pub(crate) async fn decline_trade(
        &mut self,
        username: String,
        offered_file_name: String,
        requested_file_name: String,
    ) -> Result<(), anyhow::Error> {
        let Some(peer_id) = self.get_peer_id(username.clone()).await else {
            bail!("'{username}' is not a registered user");
        };

        self.command_sender
            .send(Command::RespondTrade {
                peer_id,
                requested_file_name,
                offered_file_name,
                requested_file_bytes: None,
                offered_bytes_sender: None,
            })
            .await
            .expect("Command receiver was dropped");

        Ok(())
    }

    pub(crate) async fn register_username(
        &mut self,
        username: String,
    ) -> Result<(), kad::PutRecordError> {
        let (status_sender, status_receiver) = oneshot::channel();

        self.command_sender
            .send(Command::RegisterUsername {
                username,
                status_sender,
            })
            .await
            .expect("Command receiver was dropped");

        status_receiver.await.expect("Status sender was dropped")
    }

    async fn find_user(&mut self, username: String) -> Option<PeerId> {
        let (peer_id_sender, peer_id_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::FindPeerId {
                username: username.clone(),
                peer_id_sender,
            })
            .await
            .expect("Command receiver was dropped");

        let peer_id = peer_id_receiver
            .await
            .expect("Peer ID sender not be dropped.");

        if let Some(peer_id) = peer_id {
            self.username_store
                .lock()
                .unwrap()
                .insert(username, peer_id);
        }

        peer_id
    }

    async fn find_peer_username(&mut self, peer_id: PeerId) -> Result<String, anyhow::Error> {
        let (username_sender, username_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::FindPeerUsername {
                peer_id,
                username_sender,
            })
            .await
            .expect("Command receiver was dropped");

        let username = username_receiver
            .await
            .expect("Username sender was dropped");

        if let Ok(ref username) = username {
            self.username_store
                .lock()
                .unwrap()
                .insert(username.to_owned(), peer_id);
        }

        username
    }

    pub(crate) async fn send_message(
        &mut self,
        message: String,
    ) -> Result<(), gossipsub::PublishError> {
        let (status_sender, status_receiver) = oneshot::channel();

        self.command_sender
            .send(Command::SendChatMessage {
                message,
                status_sender,
            })
            .await
            .expect("Command receiver was dropped");

        status_receiver.await.expect("Status sender was dropped")
    }

    pub(crate) async fn direct_message(
        &mut self,
        username: String,
        message: String,
    ) -> Result<(), anyhow::Error> {
        let Some(peer_id) = self.get_peer_id(username.clone()).await else {
            bail!("'{username}' is not a registered user");
        };

        let (error_sender, error_receiver) = oneshot::channel();

        self.command_sender
            .send(Command::DirectMessage {
                peer_id,
                message,
                error_sender,
            })
            .await
            .expect("Command receiver was dropped");

        error_receiver.await.expect("Error sender was dropped")
    }
}

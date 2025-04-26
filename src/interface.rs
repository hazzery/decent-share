use libp2p::gossipsub;

use crate::{
    action::{handle_accept_trade, handle_send, handle_trade},
    network::{Client, Event},
};

const TRADE_USAGE: &str = "Usage: trade <name_of_offered_file> <path_to_offered_file> <recipient_username> <name_of_requested_file> <path_to_put_requested_file>";
const SEND_USAGE: &str = "Usage: send <message_to_broadcast>";
const DM_USAGE: &str = "Usage: dm <username> <message>";
const ACCEPT_USAGE: &str = "Usage: accept <offerer_username> <name_of_offered_file> <path_to_place_received_file> <name_of_requested_file> <path_to_source_requested_file>";
const DECLINE_USAGE: &str =
    "Usage: decline <offerer_username> <name_of_offered_file> <name_of_requested_file>";

#[allow(clippy::too_many_lines)]
pub(crate) async fn handle_std_in(
    command: Result<Option<String>, std::io::Error>,
    network_client: &mut Client,
) {
    let command = match command {
        Ok(Some(command)) => command,
        Ok(None) => return,
        Err(error) => {
            eprintln!("Error reading command: {error:?}");
            return;
        }
    };

    let arguments = split_string(&command);
    let Some(action) = arguments.first() else {
        println!("No action specified");
        return;
    };

    match action.to_lowercase().as_str() {
        "send" => {
            let Some(message) = arguments.get(1) else {
                println!("{SEND_USAGE}");
                return;
            };
            if let Err(error) = handle_send(message, network_client).await {
                match error {
                    gossipsub::PublishError::InsufficientPeers => {
                        eprintln!("No peers are connected, unable to publish chat!");
                    }
                    gossipsub::PublishError::MessageTooLarge => {
                        eprintln!("Message was too large. Please use less characters");
                    }
                    _ => eprintln!("Error sending chat: {error:?}"),
                }
            }
        }
        "trade" => {
            let Some(offered_file_name) = arguments.get(1) else {
                println!("{TRADE_USAGE}");
                return;
            };
            let Some(offered_file_path) = arguments.get(2) else {
                println!("{TRADE_USAGE}");
                return;
            };
            let Some(username) = arguments.get(3) else {
                println!("{TRADE_USAGE}");
                return;
            };
            let Some(requested_file_name) = arguments.get(4) else {
                println!("{TRADE_USAGE}");
                return;
            };
            let Some(requested_file_path) = arguments.get(5) else {
                println!("{TRADE_USAGE}");
                return;
            };

            if let Err(error) = handle_trade(
                offered_file_name,
                offered_file_path,
                username,
                requested_file_name,
                requested_file_path,
                network_client,
            )
            .await
            {
                eprintln!("Error offering trade: {error:?}");
            }
        }
        "dm" => {
            let Some(username) = arguments.get(1) else {
                println!("{DM_USAGE}");
                return;
            };
            let Some(message) = arguments.get(2) else {
                println!("{DM_USAGE}");
                return;
            };
            if let Err(error) = network_client
                .direct_message(username.to_owned(), message.to_owned())
                .await
            {
                eprintln!("Error sending direct message: {error:?}");
            }
        }
        "accept" => {
            let Some(username) = arguments.get(1) else {
                println!("{ACCEPT_USAGE}");
                return;
            };
            let Some(offered_file_name) = arguments.get(2) else {
                println!("{ACCEPT_USAGE}");
                return;
            };
            let Some(offered_file_path) = arguments.get(3) else {
                println!("{ACCEPT_USAGE}");
                return;
            };
            let Some(requested_file_name) = arguments.get(4) else {
                println!("{ACCEPT_USAGE}");
                return;
            };
            let Some(requested_file_path) = arguments.get(5) else {
                println!("{ACCEPT_USAGE}");
                return;
            };
            if let Err(error) = handle_accept_trade(
                username,
                offered_file_name,
                offered_file_path,
                requested_file_name,
                requested_file_path,
                network_client,
            )
            .await
            {
                eprintln!("Error accepting trade: {error:?}");
            }
        }
        "decline" => {
            let Some(username) = arguments.get(1) else {
                println!("{DECLINE_USAGE}");
                return;
            };
            let Some(offered_file_name) = arguments.get(2) else {
                println!("{DECLINE_USAGE}");
                return;
            };
            let Some(requested_file_name) = arguments.get(3) else {
                println!("{DECLINE_USAGE}");
                return;
            };
            if let Err(error) = network_client
                .decline_trade(
                    username.to_owned(),
                    offered_file_name.to_owned(),
                    requested_file_name.to_owned(),
                )
                .await
            {
                eprintln!("Error declining trade: {error:?}");
            }
        }

        action => println!("Unknown action '{action}'"),
    }
}

pub async fn handle_network_event(event: Option<Event>, network_client: &mut Client) {
    let event = event.expect("Network event sender was dropped!");

    match event {
        Event::InboundTradeOffer {
            offered_file_name: offered_file,
            peer_id,
            requested_file_name: requested_file,
        } => {
            println!("You have received a trade offer!",);
            match network_client.get_username(peer_id).await {
                Ok(username) => println!("From: {username}"),
                Err(error) => println!("Error fetching username: {error:?}"),
            }
            println!("Receive: {offered_file}, Provide: {requested_file}");
        }
        Event::InboundTradeResponse {
            peer_id,
            offered_file_name: offered_file,
            requested_file_name: requested_file,
            was_accepted,
        } => {
            let response_message = if was_accepted { "accepted" } else { "declined" };
            let username = match network_client.get_username(peer_id).await {
                Ok(username) => username,
                Err(error) => error.to_string(),
            };
            println!("{username} has {response_message} your trade for {offered_file}.");
            if was_accepted {
                println!("{requested_file} is now available at the path you specified");
            }
        }
        Event::InboundDirectMessage { peer_id, message } => {
            println!("You have received a direct message!");
            match network_client.get_username(peer_id).await {
                Ok(username) => println!("From {username}:"),
                Err(error) => println!("Error fetching username: {error:?}"),
            }
            println!("{message}");
        }
        Event::InboundChat { peer_id, message } => {
            println!("Received new global chat!");
            match network_client.get_username(peer_id).await {
                Ok(username) => println!("From {username}:"),
                Err(error) => println!("Error fetching username: {error:?}"),
            }
            println!("{message}");
        }
        Event::RegistrationRequest { username } => {
            if let Err(error) = network_client.register_username(username.clone()).await {
                println!("Failed to register username: {error:?}");
            } else {
                println!("successfully registered as {username}");
            }
        }
    }
}

fn split_string(input: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""([^"]*)"|\S+"#).unwrap();
    re.captures_iter(input)
        .map(|cap| cap.get(0).unwrap().as_str().to_string())
        .collect()
}

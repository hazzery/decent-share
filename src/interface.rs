use crate::{
    action::{handle_accept_trade, handle_send, handle_trade},
    network::{Client, Event},
};

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
                println!("Usage: send <message_to_broadcast>");
                return;
            };
            handle_send(message, network_client).await;
        }
        "trade" => {
            let trade_usage = "Usage: trade <name_of_offered_file> <path_to_offered_file> <recipient_username> <name_of_requested_file> <path_to_put_requested_file>";
            let Some(offered_file_name) = arguments.get(1) else {
                println!("{trade_usage}");
                return;
            };
            let Some(offered_file_path) = arguments.get(2) else {
                println!("{trade_usage}");
                return;
            };
            let Some(username) = arguments.get(3) else {
                println!("{trade_usage}");
                return;
            };
            let Some(requested_file_name) = arguments.get(4) else {
                println!("{trade_usage}");
                return;
            };
            let Some(requested_file_path) = arguments.get(5) else {
                println!("{trade_usage}");
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
        "register" => {
            let Some(username) = arguments.get(1) else {
                println!("Missing username");
                return;
            };

            network_client.register_username(username.clone()).await;
        }
        "find" => {
            let Some(username) = arguments.get(1) else {
                println!("Missing username");
                return;
            };
            match network_client.find_user(username.clone()).await {
                Ok(_peer_id) => println!("Found Peer ID of {username}!"),
                Err(error) => eprintln!("Error finding user: {error:?}"),
            }
        }
        "dm" => {
            let dm_usage = "Usage: dm <username> <message>";
            let Some(username) = arguments.get(1) else {
                println!("{dm_usage}");
                return;
            };
            let Some(message) = arguments.get(2) else {
                println!("{dm_usage}");
                return;
            };
            if let Err(error) = network_client
                .direct_message(username.clone(), message.clone())
                .await
            {
                eprintln!("Error sending direct message: {error:?}");
            }
        }
        "accept" => {
            let accept_usage = "Usage: accept <offerer_username> <name_of_offered_file> <path_to_place_received_file> <name_of_requested_file> <path_to_source_requested_file>";

            let Some(username) = arguments.get(1) else {
                println!("{accept_usage}");
                return;
            };
            let Some(offered_file_name) = arguments.get(2) else {
                println!("{accept_usage}");
                return;
            };
            let Some(offered_file_path) = arguments.get(3) else {
                println!("{accept_usage}");
                return;
            };
            let Some(requested_file_name) = arguments.get(4) else {
                println!("{accept_usage}");
                return;
            };
            let Some(requested_file_path) = arguments.get(5) else {
                println!("{accept_usage}");
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
            let decline_usage =
                "Usage: decline <offerer_username> <name_of_offered_file> <name_of_requested_file>";
            let Some(username) = arguments.get(1) else {
                println!("{decline_usage}");
                return;
            };
            let Some(offered_file_name) = arguments.get(2) else {
                println!("{decline_usage}");
                return;
            };
            let Some(requested_file_name) = arguments.get(3) else {
                println!("{decline_usage}");
                return;
            };
            if let Err(error) = network_client
                .decline_trade(
                    username.to_string(),
                    offered_file_name.to_string(),
                    requested_file_name.to_string(),
                )
                .await
            {
                eprintln!("Error declining trade: {error:?}");
            }
        }

        action => println!("Unknown action {action}"),
    }
}

pub async fn handle_network_event(event: Option<Event>) {
    let Some(event) = event else {
        println!("Received empty network event");
        return;
    };

    match event {
        Event::InboundTradeOffer {
            offered_file_name: offered_file,
            username,
            requested_file_name: requested_file,
        } => {
            println!("You have received a trade offer!",);
            match username {
                Ok(username) => println!("From: {username}"),
                Err(error) => println!("Error fetching username: {error:?}"),
            }
            println!("Receive: {offered_file}, Provide: {requested_file}");
        }
        Event::InboundTradeResponse {
            username,
            offered_file_name: offered_file,
            requested_file_name: requested_file,
            was_accepted,
        } => {
            let response_message = if was_accepted { "accepted" } else { "declined" };
            let username = match username {
                Ok(username) => username,
                Err(error) => error.to_string(),
            };
            println!("{username} has {response_message} your trade for {offered_file}.");
            if was_accepted {
                println!("{requested_file} is now available at the path you specified");
            }
        }
        Event::InboundDirectMessage { username, message } => {
            println!("You have received a direct message!");
            match username {
                Ok(username) => println!("From {username}:"),
                Err(error) => println!("Error fetching username: {error:?}"),
            }
            println!("{message}");
        }
        Event::InboundChat { username, message } => {
            println!("Received new global chat!");
            match username {
                Ok(username) => println!("From {username}:"),
                Err(error) => println!("Error fetching username: {error:?}"),
            }
            println!("{message}");
        }
    }
}

fn split_string(input: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""([^"]*)"|\S+"#).unwrap();
    re.captures_iter(input)
        .map(|cap| cap.get(0).unwrap().as_str().to_string())
        .collect()
}

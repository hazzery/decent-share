use std::{path::PathBuf, str::FromStr};

use crate::{
    action::{handle_get, handle_send, handle_trade},
    network::{Client, Event},
};

pub(crate) async fn handle_std_in(
    command: Result<Option<String>, std::io::Error>,
    network_client: &mut Client,
) -> Result<(), anyhow::Error> {
    let Some(command) = command? else {
        return Ok(());
    };

    let arguments = split_string(&command);
    let Some(action) = arguments.first() else {
        println!("No action specified");
        return Ok(());
    };

    match action.as_str() {
        "Send" => {
            let Some(message) = arguments.get(1) else {
                println!("Missing message content");
                return Ok(());
            };
            handle_send(message, network_client).await;
        }
        "Trade" => {
            let Some(offered_file_name) = arguments.get(1) else {
                println!("Usage: Offer <name_of_offered_file> <path_to_offered_file> <recipient_username> <name_of_requested_file> <path_to_put_requested_file>");
                return Ok(());
            };
            let Some(offered_file_path) = arguments.get(2) else {
                println!("Usage: Offer <name_of_offered_file> <path_to_offered_file> <recipient_username> <name_of_requested_file> <path_to_put_requested_file>");
                return Ok(());
            };
            let Some(username) = arguments.get(3) else {
                println!("Usage: Offer <name_of_offered_file> <path_to_offered_file> <recipient_username> <name_of_requested_file> <path_to_put_requested_file>");
                return Ok(());
            };
            let Some(requested_file_name) = arguments.get(4) else {
                println!("Usage: Offer <name_of_offered_file> <path_to_offered_file> <recipient_username> <name_of_requested_file> <path_to_put_requested_file>");
                return Ok(());
            };
            let Some(requested_file_path) = arguments.get(5) else {
                println!("Usage: Offer <name_of_offered_file> <path_to_offered_file> <recipient_username> <name_of_requested_file> <path_to_put_requested_file>");
                return Ok(());
            };

            handle_trade(
                offered_file_name,
                offered_file_path,
                username,
                requested_file_name,
                requested_file_path,
                network_client,
            )
            .await?;
        }
        "Get" => {
            let Some(name) = arguments.get(1) else {
                println!("Missing name for file to recieve");
                return Ok(());
            };
            handle_get(name, network_client).await?;
        }
        "Register" => {
            let Some(username) = arguments.get(1) else {
                println!("Missing username");
                return Ok(());
            };

            network_client.register_username(username.clone()).await;
        }
        "Find" => {
            let Some(username) = arguments.get(1) else {
                println!("Missing username");
                return Ok(());
            };
            let _ = network_client.find_user(username.clone()).await;
        }
        "Dm" => {
            let Some(username) = arguments.get(1) else {
                println!("Missing username and message");
                return Ok(());
            };
            let Some(message) = arguments.get(2) else {
                println!("Missing message");
                return Ok(());
            };
            network_client
                .direct_message(username.clone(), message.clone())
                .await
                .expect("Direct message failed");
        }

        action => println!("Unknown action {action}"),
    };

    Ok(())
}

pub async fn handle_network_event(event: Option<Event>, network_client: &mut Client) {
    let Some(event) = event else {
        println!("Received empty network event");
        return;
    };

    match event {
        Event::InboundTradeOffer {
            offered_file,
            username,
            requested_file,
            channel,
        } => {
            println!("You have received a trade offer!",);
            if let Some(username) = username {
                println!("From: {username}");
            }
            println!("Receive: {offered_file}, Provide: {requested_file}");
            println!("To accept this trade, enter the path to {requested_file}. Any empty line or an invalid path will decline the trade");

            let mut decision = String::new();
            let _ = std::io::stdin().read_line(&mut decision);
            let path = PathBuf::from_str(decision.trim_end());

            let response = match path {
                Ok(path) => match std::fs::read(path) {
                    Ok(bytes) => {
                        println!("yay!");
                        Some(bytes)
                    }
                    Err(error) => {
                        eprintln!("{error:?}");
                        None
                    }
                },
                Err(error) => {
                    eprintln!("{error:?}");
                    None
                }
            };
            network_client.respond_to_trade(response, channel).await;
        }
        Event::InboundRequest { request, channel } => {}
    }
}

fn split_string(input: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""([^"]*)"|\S+"#).unwrap();
    re.captures_iter(input)
        .map(|cap| cap.get(0).unwrap().as_str().to_string())
        .collect()
}

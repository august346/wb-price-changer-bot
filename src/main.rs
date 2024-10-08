mod utils;

use std::net::IpAddr;
use reqwest::header;
use serde_json::json;
use tgbot::api::Client;
use tgbot::handler::{LongPoll, UpdateHandler, WebhookServer};
use tgbot::types::{ChatPeerId, DeleteWebhook, LabeledPrice, MessageData, ParseMode, SendInvoice, SendMessage, SetWebhook, SuccessfulPayment, Update, UpdateType};
use tracing::{error, info, Level};

struct Handler {
    client: Client,
}

impl UpdateHandler for Handler {
    async fn handle(&self, update: Update) {
        if let Err(err) = handle_update(&self.client, update).await {
            error!("Failed handle_update: {:?}", err);
        }
    }
}

async fn send_invoice(client: &Client, chat_id: ChatPeerId) -> Result<(), String> {
    let cmd = SendInvoice::new(
        chat_id,
        "API KEY",
        "WB API KEY for repricing chrome extension",
        "1",
        "XTR",
        vec![LabeledPrice::new(123, "20")],
    );

    client
        .execute(cmd)
        .await
        .map_err(|err| utils::make_err(Box::new(err), "send invoice"))?;

    Ok(())
}

async fn get_api_key(user_id: &str) -> Result<String, String> {
    let api_key = utils::get_env("SUPER_API_KEY")?;
    let api_url = utils::get_env("API_URL")?;

    let client = reqwest::Client::builder()
        .build().map_err(|err| utils::make_err(Box::new(err), "build client"))?;

    let mut headers = header::HeaderMap::new();
    headers.insert("Authorization", api_key
        .parse()
        .map_err(|err| utils::make_err(Box::new(err), "parse api key"))?);
    headers.insert("Content-Type", "application/json".parse()
        .map_err(|err| utils::make_err(Box::new(err), "parse content type"))?);

    // Create the JSON body with user_id
    let body = json!({
        "user_id": user_id
    });

    let request = client
        .post(api_url)
        .headers(headers)
        .json(&body);

    let response = request
        .send()
        .await
        .map_err(|err| utils::make_err(Box::new(err), "get response"))?;
    let body = response
        .text()
        .await
        .map_err(|err| utils::make_err(Box::new(err), "get response body"))?;

    Ok(body)
}

async fn send_start(
    client: &Client, chat_id: ChatPeerId,
) -> Result<(), String> {

    let cmd = SendMessage::new(chat_id, "hi");

    client
        .execute(cmd)
        .await
        .map_err(|err| utils::make_err(Box::new(err), "send start"))?;

    Ok(())
}

async fn send_api_key(
    client: &Client, chat_id: ChatPeerId, _: SuccessfulPayment,
) -> Result<(), String> {
    let api_key = get_api_key("1").await?;
    let msg = format!("Your API KEY:\n\n`{}`", api_key);

    let cmd = SendMessage::new(
        chat_id,
        msg,
    ).with_parse_mode(ParseMode::Markdown);

    client
        .execute(cmd)
        .await
        .map_err(|err| utils::make_err(Box::new(err), "send api key"))?;

    Ok(())
}

async fn test_buy(client: &Client, chat_id: ChatPeerId) -> Result<(), String> {
    let api_key = get_api_key("1").await?;
    let msg = format!("Your API KEY:\n\n`{}`", api_key);

    let cmd = SendMessage::new(
        chat_id,
        msg,
    ).with_parse_mode(ParseMode::Markdown);

    client
        .execute(cmd)
        .await
        .map_err(|err| utils::make_err(Box::new(err), "test buy"))?;

    Ok(())
}

async fn handle_update(client: &Client, update: Update) -> Result<(), String> {
    match update.update_type {
        UpdateType::Message(message) => {
            let chat_id = message.chat.get_id();
            let superuser = message
                .chat
                .get_username()
                .is_some_and(
                    |un| utils::get_env("S_USERNAME")
                        .is_ok_and(|env| un.to_string().to_lowercase() == env));
            match message.data {
                MessageData::Text(text) => {
                    if let Some(commands) = text.get_bot_commands() {
                        let command = &commands[0];
                        match command.command.as_str() {
                            "/start" => { send_start(client, chat_id).await?; }
                            "/buy" => { send_invoice(client, chat_id).await?; }
                            "/test_buy" if superuser => { test_buy(client, chat_id).await?; }
                            _ => {}
                        }
                    }
                }
                MessageData::SuccessfulPayment(sp) => send_api_key(
                    client, chat_id, sp,
                ).await?,
                _ => {}
            };
        }
        _ => {}
    };

    Ok(())
}

async fn run_bot() -> Result<(), String> {
    let token = utils::get_env("TGBOT_TOKEN")?;
    let client = Client::new(token).expect("Failed to create API");

    let handler = Handler { client: client.clone() };

    let webhook_feature = utils::get_env_feature_turned_on("WEBHOOK_FEATURE");

    info!("The bot will be started as {}", if webhook_feature { "WEBHOOK" } else { "LONG_POOL" } );

    if webhook_feature {
        let webhook_address = utils::get_env("WEBHOOK_ADDRESS")?;
        client
            .execute(SetWebhook::new(webhook_address).with_drop_pending_updates(true))
            .await
            .map_err(|err| utils::make_err(Box::new(err), "set webhook"))?;

        let host = utils::get_env("HOST")?
            .parse::<IpAddr>()
            .map_err(|err| utils::make_err(Box::new(err), "invalid host"))?;
        let port = utils::get_env("PORT")?
            .parse::<u16>()
            .map_err(|err| utils::make_err(Box::new(err), "invalid port number"))?;

        WebhookServer::new("/", handler)
            .run((host, port))
            .await
            .map_err(|err| utils::make_err(Box::new(err), "create webhook"))?;
    } else {
        client
            .execute(DeleteWebhook::default().with_drop_pending_updates(true))
            .await
            .map_err(|err| utils::make_err(Box::new(err), "delete webhook"))?;
        LongPoll::new(handler.client.clone(), handler).run().await;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    utils::get_env("TGBOT_TOKEN")?;
    utils::get_env("SUPER_API_KEY")?;
    utils::get_env("API_URL")?;
    utils::get_env("S_USERNAME")?;

    tracing_subscriber::fmt().json()
        .with_max_level(Level::ERROR)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    run_bot().await
}

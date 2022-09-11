mod api;
mod storage;

use anyhow::Ok;
use api::RpcRequest;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::sync::Arc;
use std::time::Duration;
use storage::*;
use teloxide::prelude::*;
use tokio::task::JoinError;
use tokio::{task, time};

use serde_json::{Map, Value};

#[derive(Debug, Deserialize)]
pub struct ApiConfig {
    pub url: String,
    pub password: String,
    pub update_key: String,
    pub login_method: String,
    pub update_method: String,
}
#[derive(Debug, Deserialize)]
struct TelegramConfig {
    token: String,
    chatid: i64,
}
#[derive(Debug, Deserialize)]
struct Config {
    api: ApiConfig,
    telegram: TelegramConfig,
    storage: String,
    interval: u64,
}

fn read_config(config_fp: String) -> Result<Config, anyhow::Error> {
    let f = File::open(config_fp)?;
    let c = serde_yaml::from_reader::<File, Config>(f)?;
    Ok(c)
}

async fn send_notification(chat_id: ChatId, bot: &AutoSend<Bot>, message: String) {
    match bot.send_message(chat_id, message).await {
        Result::Ok(_) => (),
        // Debug error since it may not interrupt the logic (may be time out e.g.)
        Err(e) => {
            dbg!(e);
        }
    }
}


async fn update(
    url: &String,
    login_request: &RpcRequest,
    update_request: &RpcRequest,
    update_key: &String,
    storage_file: &String,
    chat_id: ChatId,
    bot: &AutoSend<Bot>,
) {
    match api::get_current_state(url, login_request, update_request, update_key).await {
        Result::Ok(s) => {
            if let Result::Ok(previous) = load_current(storage_file) {
                if previous.ip != s.ip {
                    send_notification(
                        chat_id,
                        bot,
                        format!("IP changed from {} to {}", &previous.ip, &s.ip),
                    )
                    .await
                }

                for (k, v) in &s.queue {
                    let previous_progress = previous.queue.get(k);
                    if let Some(p) = previous_progress {
                        if v == &100.0 && p < &100.0 {
                            send_notification(chat_id, bot, format!("Done with {}", &k)).await
                        }
                    }
                    if let Result::Err(e) = save_current(storage_file, &s) {
                        // Exit program bc initial storage file cannot be created => unable to do the logic
                        let msg = format!("Could not save current state {:?}", &e);
                        send_notification(chat_id, bot, msg).await;
                        panic!("Could not save current state {:?}", &e);
                    }
                }
            } else {
                match init_current(storage_file, &s) {
                    Result::Ok(_f) => (),
                    // Exit program bc initial storage file cannot be created => unable to do the logic
                    Err(e) => {
                        send_notification(chat_id, bot, format!("Unable to create init storage file {:?}", &e)).await;
                        panic!("Unable to create init storage file {:?}", &e)
                    },
                }
            }
        }
        Err(e) => {
            // Debug error since it may not interrupt the logic (may be time out e.g.)
            send_notification(chat_id, bot, format!("Unabel to get current state {:?}", &e)).await;
            dbg!(e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), JoinError> {
    let mut args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: api-telegram <path/to/config>")
    }

    let config = match read_config(args.pop().unwrap()) {
        Result::Ok(c) => c,
        Err(e) => panic!("Unable to read config file! : {:?}", e),
    };

    let login_request = Arc::new(RpcRequest {
        id: 1,
        method: config.api.login_method,
        params: vec![Value::String(config.api.password)],
    });
    let update_request = Arc::new(RpcRequest {
        id: 1,
        method: config.api.update_method,
        params: vec![
            Value::Array(vec![
                Value::String("name".to_string()),
                Value::String("progress".to_string()),
            ]),
            Value::Object(Map::new()),
        ],
    });
    let url = Arc::new(config.api.url);
    let update_key = Arc::new(config.api.update_key);
    let bot = Bot::new(&config.telegram.token).auto_send();
    let task = task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(config.interval));
        loop {
            interval.tick().await;
            update(
                url.clone().as_ref(),
                login_request.clone().as_ref(),
                update_request.clone().as_ref(),
                update_key.clone().as_ref(),
                &config.storage,
                ChatId(config.telegram.chatid),
                &bot,
            )
            .await;
        }
    });
    task.await
}

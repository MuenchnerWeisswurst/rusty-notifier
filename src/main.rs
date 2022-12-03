extern crate pretty_env_logger;
#[macro_use]
extern crate log;

mod api;
mod notification_channels;
mod storage;

use anyhow::Ok;
use api::RpcRequest;
use log::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::sync::Arc;
use std::time::Duration;
use storage::*;
use teloxide::prelude::*;
use tokio::task::JoinError;
use tokio::{task, time};

use serde_json::{Map, Value};

use notification_channels::send_notfication_until;

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
    retries: u8,
    interval: u8,
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

async fn update(
    config: &Config,
    login_request: &RpcRequest,
    update_request: &RpcRequest,
    chat_id: ChatId,
    bot: &AutoSend<Bot>,
) {
    match api::get_current_state(
        &config.api.url,
        login_request,
        update_request,
        &config.api.update_key,
    )
    .await
    {
        Result::Ok(s) => {
            trace!("Got new state: {:?}", s);
            if let Result::Ok(previous) = load_current(&config.storage) {
                if previous.ip != s.ip {
                    send_notfication_until(
                        chat_id,
                        bot,
                        config.telegram.retries,
                        config.telegram.interval,
                        format!("IP changed from {} to {}", &previous.ip, &s.ip),
                    )
                    .await;
                }

                for (k, v) in &s.queue {
                    let previous_progress = previous.queue.get(k);
                    if let Some(p) = previous_progress {
                        if v == &100.0 && p < &100.0 {
                            send_notfication_until(
                                chat_id,
                                bot,
                                config.telegram.retries,
                                config.telegram.interval,
                                format!("Done with {}", &k),
                            )
                            .await;
                        }
                    }
                    if let Result::Err(e) = save_current(&config.storage, &s) {
                        // Exit program bc initial storage file cannot be created => unable to do the logic
                        let msg = format!("Could not save current state {:?}", &e);
                        send_notfication_until(
                            chat_id,
                            bot,
                            config.telegram.retries,
                            config.telegram.interval,
                            msg,
                        )
                        .await;
                        error!("Could not save current state {:?}", &e);
                    }
                }
            } else {
                match init_current(&config.storage, &s) {
                    Result::Ok(_f) => (),
                    // Exit program bc initial storage file cannot be created => unable to do the logic
                    Err(e) => {
                        send_notfication_until(
                            chat_id,
                            bot,
                            config.telegram.retries,
                            config.telegram.interval,
                            format!("Unable to create init storage file {:?}", &e),
                        )
                        .await;
                        error!("Unable to create init storage file {:?}", &e)
                    }
                }
            }
        }
        Err(e) => {
            // Debug error since it may not interrupt the logic (may be time out e.g.)
            send_notfication_until(
                chat_id,
                bot,
                config.telegram.retries,
                config.telegram.interval,
                format!("Unabel to get current state {:?}", &e),
            )
            .await;
            debug!("{}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), JoinError> {
    pretty_env_logger::init();
    let mut args: Vec<String> = env::args().collect();
    let config = if args.len() != 2 {
        let keys = vec![
            "API_URL",
            "API_PASSWORD",
            "API_UPDATE_KEY",
            "API_LOGIN_METHOD",
            "API_UPDATE_METHOD",
            "TELEGRAM_TOKEN",
            "TELEGRAM_CHAT_ID",
            "TELEGRAM_RETRIES",
            "TELEGRAM_INTERVAL",
            "STORAGE",
            "POLL_INTERVAL",
        ];
        let values = keys
            .iter()
            .map(|k| {
                if let Result::Ok(v) = env::var(k) {
                    (String::from(*k), v)
                } else {
                    panic!("{k} must be set!")
                }
            })
            .collect::<HashMap<String, String>>();
        let api_config = ApiConfig {
            url: values.get("API_URL").unwrap().to_owned(),
            password: values.get("API_PASSWORD").unwrap().to_owned(),
            update_key: values.get("API_UPDATE_KEY").unwrap().to_owned(),
            login_method: values.get("API_LOGIN_METHOD").unwrap().to_owned(),
            update_method: values.get("API_UPDATE_METHOD").unwrap().to_owned(),
        };
        let telegram_config = TelegramConfig {
            token: values.get("TELEGRAM_TOKEN").unwrap().to_owned(),
            chatid: values
                .get("TELEGRAM_CHAT_ID")
                .and_then(|id| id.parse::<i64>().ok())
                .unwrap(),
            retries: values
                .get("TELEGRAM_RETRIES")
                .and_then(|r| r.parse::<u8>().ok())
                .unwrap(),
            interval: values
                .get("TELEGRAM_INTERVAL")
                .and_then(|i| i.parse::<u8>().ok())
                .unwrap(),
        };
        Arc::new(Config {
            api: api_config,
            telegram: telegram_config,
            storage: values.get("STORAGE").unwrap().to_owned(),
            interval: values
                .get("POLL_INTERVAL")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap(),
        })
    } else {
        Arc::new(match read_config(args.pop().unwrap()) {
            Result::Ok(c) => c,
            Err(e) => panic!("Unable to read config file! : {:?}", e),
        })
    };

    let login_request = Arc::new(RpcRequest {
        id: 1,
        method: config.api.login_method.clone(),
        params: vec![Value::String(config.api.password.clone())],
    });
    let update_request = Arc::new(RpcRequest {
        id: 1,
        method: config.api.update_method.clone(),
        params: vec![
            Value::Array(vec![
                Value::String("name".to_string()),
                Value::String("progress".to_string()),
            ]),
            Value::Object(Map::new()),
        ],
    });
    let bot = Bot::new(&config.telegram.token).auto_send();
    let task = task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(config.interval));
        loop {
            interval.tick().await;
            update(
                config.clone().as_ref(),
                login_request.clone().as_ref(),
                update_request.clone().as_ref(),
                ChatId(config.telegram.chatid),
                &bot,
            )
            .await;
        }
    });
    task.await
}

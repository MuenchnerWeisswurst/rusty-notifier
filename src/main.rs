mod api;

use anyhow::Ok;
use api::CurrentState;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::BufWriter;
use std::time::Duration;
use std::{fs::File, io::BufReader};
use teloxide::prelude::*;
use tokio::task::JoinError;
use tokio::{task, time};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiConfig {
    pub url: String,
    pub password: String,
    pub update_key: String,
    pub login_method: String,
    pub update_method: String,
}
#[derive(Debug, Deserialize, Serialize)]
struct TelegramConfig {
    token: String,
    chatid: i64,
}
#[derive(Debug, Deserialize, Serialize)]
struct Config {
    api: ApiConfig,
    telegram: TelegramConfig,
    storage: String,
    interval: u64,
}

fn load_current(file_path: &String) -> Result<CurrentState, anyhow::Error> {
    let f = File::open(file_path)?;
    let reader = BufReader::new(f);
    let u = serde_json::from_reader::<BufReader<File>, CurrentState>(reader)?;
    Ok(u)
}
fn save_current(file_path: &String, current: &CurrentState) -> Result<(), anyhow::Error> {
    let f = File::options().write(true).open(file_path)?;
    let writer = BufWriter::new(f);
    let u = serde_json::to_writer_pretty(writer, current)?;
    Ok(u)
}

fn init_current(file_path: &String, current: &CurrentState) -> Result<(), anyhow::Error> {
    let file = File::create(file_path)?;
    serde_json::to_writer_pretty(file, current)?;
    Ok(())
}

fn read_config(config_fp: String) -> Result<Config, anyhow::Error> {
    let f = File::open(config_fp)?;
    let c = serde_yaml::from_reader::<File, Config>(f)?;
    Ok(c)
}

async fn send_notification(config: &TelegramConfig, bot: &AutoSend<Bot>, message: String) {
    match bot.send_message(ChatId(config.chatid), message).await {
        Result::Ok(_) => (),
        Err(e) => panic!("Unabel to send message: {:?}", e),
    }
}

async fn update(config: &Config, bot: &AutoSend<Bot>) {
    match api::get_current_state(config.api.clone()) {
        Result::Ok(s) => {
            if let Result::Ok(previous) = load_current(&config.storage) {
                let ip = s.ip.clone();
                if previous.ip != ip {
                    send_notification(
                        &config.telegram,
                        bot,
                        format!("IP changed from {} to {}", &previous.ip, &ip),
                    )
                    .await
                }

                for (k, v) in &s.queue {
                    let previous_progress = previous.queue.get(k);
                    if let Some(p) = previous_progress {
                        if v == &100.0 && p < &100.0 {
                            // send notification
                            send_notification(&config.telegram, bot, format!("Done with {}", &k))
                                .await
                        }
                    }
                    if let Result::Err(e) = save_current(&config.storage, &s) {
                        panic!("Could not save current state {:?}", e);
                    }
                }
            } else {
                match init_current(&config.storage, &s) {
                    Result::Ok(_f) => (),
                    Err(e) => panic!("{:?}", e),
                }
            }
        }
        Err(e) => {
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

    let bot = Bot::new(&config.telegram.token).auto_send();
    let task = task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(config.interval));
        loop {
            interval.tick().await;
            update(&config, &bot).await;
        }
    });
    task.await
}

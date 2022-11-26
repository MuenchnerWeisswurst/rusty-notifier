use teloxide::prelude::*;
use tokio::time;

async fn send_notification(chat_id: ChatId, bot: &AutoSend<Bot>, message: &String) -> bool {
    match bot.send_message(chat_id, message).await {
        Result::Ok(_) => false,
        Err(e) => match e {
            teloxide::RequestError::Network(err) => {
                if err.is_timeout() {
                    true
                } else {
                    debug!("{}", err);
                    false
                }
            }
            // Debug error since it may not interrupt the logic
            _ => {
                debug!("{}", e);
                false
            }
        },
    }
}

pub(crate) async fn send_notfication_until(
    chat_id: ChatId,
    bot: &AutoSend<Bot>,
    retries: u8,
    interval: u8,
    message: String,
) {
    let mut tried = 0;
    while tried < retries && send_notification(chat_id, bot, &message).await {
        tried += 1;
        debug!("Retry {}", &tried);
        let _ = time::sleep(time::Duration::from_secs(interval as u64));
    }
}

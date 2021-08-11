#[macro_use]
mod utils;
mod beancount;
mod git;
mod handler;

use std::convert::TryInto;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use log::{debug, error, info};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use tbot::contexts::methods::ChatMethods;
use tbot::proxy::{Intercept, Proxy};
use tbot::types::callback::Origin;
use tbot::types::User;
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
struct Bot {
    token: String,
    secret: String,
    #[serde(default = "state_default")]
    state_file: String,
}

fn state_default() -> String {
    String::from("state.json")
}

#[derive(Debug, Deserialize)]
struct Beancount {
    root: String,
    default_currency: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    bot: Bot,
    beancount: Beancount,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Database {
    #[serde(default)]
    auth_users: Vec<i64>,
}

static CONFIG: OnceCell<Config> = OnceCell::new();

fn get_config() -> &'static Config {
    CONFIG.get().expect("Config hasn't been initialized")
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let config: Config = toml::from_str(&read_to_string("bot.toml")?)?;
    CONFIG.set(config).unwrap();
    run().await
}

fn init_proxy() -> Option<Proxy> {
    std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .map(|uri| {
            let uri = uri
                .try_into()
                .unwrap_or_else(|e| panic!("Illegal HTTPS_PROXY: {}", e));
            Proxy::new(Intercept::All, uri)
        })
        .ok()
}

async fn run() -> Result<()> {
    let state_file = &get_config().bot.state_file;
    let database: Database = if PathBuf::from(state_file).exists() {
        serde_json::from_str(&read_to_string(state_file)?)?
    } else {
        Default::default()
    };
    let mut bot = if let Some(proxy) = init_proxy() {
        tbot::Bot::with_proxy(get_config().bot.token.clone(), proxy)
    } else {
        tbot::Bot::new(get_config().bot.token.clone())
    }
    .stateful_event_loop(RwLock::new(database));

    bot.command("auth", |context, state| async {
        if let Err(e) = handler::auth(context, state).await {
            debug!("{:?}", e);
        }
    });

    bot.text_if(
        |context, state| async move {
            if let Some(User { id: user_id, .. }) = context.from {
                // ignore messages that are 3 minutes or older
                utils::elapsed(context.date) <= 180
                    && state.read().await.auth_users.contains(&user_id.0)
            } else {
                false
            }
        },
        |context, state| async move {
            if let Err(e) = handler::command(Arc::clone(&context), state).await {
                let r = context
                    .send_message_in_reply(&format!("{:?}", e))
                    .call()
                    .await;
                if let Err(e) = r {
                    error!("Send back error message failed: {:?}", e);
                } else {
                    debug!("{:?}", e);
                }
            }
        },
    );

    bot.data_callback_if(
        |context, state| async move {
            let user_id = context.from.id.0;
            state.read().await.auth_users.contains(&user_id)
        },
        |context, state| async move {
            if let Err(e) = handler::confirm(Arc::clone(&context), state).await {
                if let Origin::Message(ref msg) = context.origin {
                    let r = context
                        .bot
                        .send_message(msg.chat.id, &format!("{:?}", e))
                        .call()
                        .await;
                    if let Err(e) = r {
                        error!("Send back error message failed: {:?}", e);
                    } else {
                        debug!("{:?}", e);
                    }
                }
            }
        },
    );

    info!("Bot starting");
    bot.polling().start().await.expect("Bot start failed");
    Ok(())
}

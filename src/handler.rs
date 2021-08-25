use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use log::info;

use tbot::contexts::methods::ChatMethods;
use tbot::contexts::{Command, DataCallback, Text};
use tbot::types::callback::Origin;
use tbot::types::keyboard::inline::{Button, ButtonKind};
use tbot::types::message::Kind;
use tokio::sync::RwLock;

use crate::beancount::{append_to_file, get_accounts, Transaction};
use crate::git::{check_repo, commit_file};
use crate::utils::command_split;
use crate::{get_config, Database};

/// Handler for command `/auth`
pub async fn auth(context: Arc<Command<Text>>, state: Arc<RwLock<Database>>) -> Result<()> {
    let state_file = &get_config().bot.state_file;
    if let Some(ref user) = context.from {
        if !state.read().await.auth_users.contains(&user.id.0)
            && context.text.value == get_config().bot.secret
        {
            let mut guard = state.write().await;
            if log::log_enabled!(log::Level::Info) {
                let username = user
                    .username
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("<noname>");
                info!("Authorizing user {} (@{})", user.id.0, username);
            }
            guard.auth_users.push(user.id.0);
            serde_json::to_writer(File::create(state_file)?, &*guard)?;
            context.send_message("Authorized!").call().await?;
            context.delete_this_message().call().await?;
        }
    }
    Ok(())
}

/// Handler for command `/accounts`
pub async fn accounts(context: Arc<Command<Text>>, _state: Arc<RwLock<Database>>) -> Result<()> {
    check_repo(&get_config().beancount.root).context("Check repo failed")?;
    let mut accounts = get_accounts(&get_config().beancount.root).context("get accounts failed")?;
    let query = context.text.value.to_lowercase();
    let query: Vec<_> = query.split_ascii_whitespace().collect();
    let accs: Vec<_> = if query.is_empty() {
        accounts
    } else {
        accounts
            .drain(..)
            .filter(|ac| query.iter().all(|q| ac.to_lowercase().contains(q)))
            .collect()
    };
    context.send_message(&accs.join(" ")).call().await?;
    Ok(())
}

/// Handler for messages
pub async fn command(context: Arc<Text>, _state: Arc<RwLock<Database>>) -> Result<()> {
    let accounts = get_accounts(&get_config().beancount.root).context("get accounts failed")?;
    let cmd_split = command_split(&context.text.value)
        .ok_or_else(|| anyhow!("Invalid command {}", context.text.value))?;
    let txn = Transaction::today_from_command(
        &cmd_split,
        &accounts,
        &get_config().beancount.default_currency,
    )?;
    let keyboard = vec![
        Button::new("提交", ButtonKind::CallbackData("commit")),
        Button::new("取消", ButtonKind::CallbackData("cancel")),
    ];

    context
        .send_message_in_reply(&format!("{}", txn))
        .reply_markup(&[keyboard.as_slice()][..])
        .call()
        .await?;
    Ok(())
}

/// Handler for commit confirmation
pub async fn confirm(context: Arc<DataCallback>, _state: Arc<RwLock<Database>>) -> Result<()> {
    let root = &get_config().beancount.root;
    if let Origin::Message(ref origin) = context.origin {
        if let Kind::Text(ref txt) = origin.kind {
            let msg = match context.data.as_str() {
                "commit" => {
                    check_repo(root).context("Check repo failed")?;
                    // start of txt.value is YYYY-MM-DD.
                    // filename = {root}/txs/{year}/{month}.bean
                    let filename = PathBuf::from(root)
                        .join("txs")
                        .join(&txt.value[..4])
                        .join(format!("{}.bean", &txt.value[5..7]));
                    append_to_file(&txt.value, &filename).context("Append to file failed")?;
                    let orig_cmd =
                        if let Some(Kind::Text(t)) = origin.reply_to.as_ref().map(|rt| &rt.kind) {
                            Some(t.value.as_str())
                        } else {
                            None
                        };
                    commit_file(root, &filename, orig_cmd).context("Commit file failed")?;
                    "已提交✅"
                }
                "cancel" => "已取消❌",
                s => unreachable!("undefined message: {}", s),
            };
            context
                .bot
                .edit_message_text(
                    origin.chat.id,
                    origin.id,
                    &format!("{}\n\n{}", txt.value, msg),
                )
                .call()
                .await?;
        }
    }
    Ok(())
}

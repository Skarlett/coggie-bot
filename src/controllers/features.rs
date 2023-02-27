use crate::{REPO, pkglib::{CoggiebotError}};
use serenity::model::prelude::Message;
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};

use std::{path::PathBuf, io::BufRead};
use serenity::prelude::*;

pub const FEATURES_FILE_ENV: &'static str = "COGGIEBOT_FEATURES";

struct FileFinder {
    name: String,
    paths: Vec<PathBuf>,
    env: Vec<String>,
}

impl FileFinder {
    pub fn find_file(&self) -> Result<PathBuf, CoggiebotError> {
        let mut paths = self.paths.clone();
        for env_var in &self.env {
            if let Ok(path) = std::env::var(env_var) {
                paths.push(PathBuf::from(path));
            }
        }
        for path in paths.iter().rev() {
            let file = path.join(&self.name);
            if file.exists() {
                return Ok(file);
            }
        }

        Err(CoggiebotError::UserMessage(format!(
            "Could not find file {} in paths {:?}",
            self.name, self.paths
        )))
    }
}

pub fn feature_list() -> anyhow::Result<Vec<Result<(String, bool), CoggiebotError>>>
{
    const STATIC_PATHS: [&'static str; 4] = [
        ".",
        "./share",
        "/usr/share/coggiebot/",
        "/usr/local/share/coggiebot"
    ];

    let finder = FileFinder {
        name: "coggiebot-features.lst".to_string(),
        paths: STATIC_PATHS.iter().map(|s| PathBuf::from(*s)).collect(),
        env: vec![FEATURES_FILE_ENV.to_string()],
    };

    let file = std::fs::File::open(finder.find_file()?)?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::with_capacity(512);

    let mut features = Vec::new();
    while let Ok(nread) = reader.read_line(&mut line)
    {
        line.clear();
        if nread == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        features.push(
            match line.split_once(":") {
                Some((name, enabled_int)) =>
                match enabled_int.parse::<usize>()
                {
                    Ok(1) => Ok((name.to_owned(), true)),
                    Ok(0) => Ok((name.to_owned(), false)),
                    Ok(n) => Err(CoggiebotError::UserMessage(format!("Invalid value for feature {}: {}", name, n))),
                    Err(_) => Err(CoggiebotError::UserMessage(format!("Expected integer, got: {:?}", enabled_int))),
                }

                None => Err(CoggiebotError::UserMessage(format!("Invalid line in enabled-features: {}", line)))
            }
        );
    }
    Ok(features)
}


#[group]
#[commands(features)]
pub struct Features;

#[command("enabled-features")]
async fn features(ctx: &Context, msg: &Message) -> CommandResult {
    let features = feature_list()?;

    msg.channel_id
       .send_message(&ctx.http, |m|
           m.add_embed(|e|
                e.title("Coggie Bot")
                 .description("Coggie Bot is an open source \"Discord\" (discord.com) bot.")
                 .url(REPO)
                 .fields(features
                    .iter().map(|r| match r {
                        Ok((name, true)) => (name, "enabled", true),
                        Ok((name, false)) => (name, "disabled", true),
                        Err(CoggiebotError::UserMessage(msg)) =>(msg, "error", true),
                        _ => unreachable!(),
                    })
                 )
            )
       ).await?;
    Ok(())
}

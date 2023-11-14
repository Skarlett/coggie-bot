use crate::{REPO, CoggieProc};

use serenity::{model::prelude::Message, framework::standard::Args};
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};

use tokio::sync::oneshot;

use std::{path::PathBuf, io::BufRead};
use serenity::prelude::*;
use tokio::process::Command;
use std::process::Stdio;
use std::time::Duration;
use tokio::time::timeout;

use std::os::fd::{FromRawFd, AsRawFd};

#[group]
#[commands(prerelease)]
pub struct PreRelease;

/// activate: "@botname prerelease {repo}/(branch-name)"
/// This relies on the branch containing an up to date flake.nix file which exports
/// `packages.<system>.coggiebot-pre-release` as a derivation.
/// which only contains feature flags for the newly spawned running instance.
///
/// this feature can be used by users to test new features before they are released.
///
/// if the user specifies the origin of the branch else where than the default
/// then the bot will first check meta-data for the repo owner and verify that
/// the user is marked as a maintainer inside coggiebot's metadata.
///
/// before spawning the new instance the bot will check that none of its features,
/// and the others are overlapping. if they are overlapping then the bot will
/// refuse to spawn the new instance.
///
/// Example: cargo build --release \
///             --features "prerelease-feature-name" \
///             --no-default-features
#[command]
#[aliases("pre-release")]
async fn prerelease(ctx: &Context, msg: &Message, args: Args) -> CommandResult
{
    let prelease_match = std::env::var("COGBOT_PRERELEASE_MATCH")
        .unwrap_or("github:Skarlett/coggie-bot".to_string());

    let uri = args.rest();

    if !uri.contains(prelease_match.as_str()) {
        msg.reply(ctx, "ERROR: not implemented yet").await?;
    }

    Command::new("nix")
        .arg("--extra-experimental-features")
        .arg("\"nix-command flakes\"")
        .arg("build")
        .arg("--no-link")
        .arg(uri)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("failed to execute process");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    
    let mut child = Command::new("nix")
        .arg("run")
        .arg(uri)
        .stdin(unsafe { Stdio::from_raw_fd(stdin.as_raw_fd()) })
        .stdout(unsafe { Stdio::from_raw_fd(stdout.as_raw_fd()) })
        .stderr(unsafe { Stdio::from_raw_fd(stderr.as_raw_fd()) })
        .spawn()
        .expect("failed to execute process");
    
    if let Err(_) = timeout(Duration::from_secs(10), child.wait()).await {
        tracing::info!("Child Died");
    }    

    let (stx, srx) = oneshot::channel();

    tokio::spawn(async move {
        let _ = child.wait().await;
        stx.send(()).unwrap();
    });
    
    let tx = {
        let mut glob = ctx.data.write().await;
        glob.remove::<crate::ProcCtlKey>()
            .expect("procctlkey not found")
    };
    
    let _ = tx.send(crate::CoggieProc::UntilSignal(srx));
    Ok(())
}
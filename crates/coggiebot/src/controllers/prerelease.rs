use crate::{REPO, pkglib::{CoggiebotError}};
use serenity::model::prelude::Message;
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};

use std::{path::PathBuf, io::BufRead};
use serenity::prelude::*;



#[group]
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
async fn prerelease(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "ERROR: not implemented yet").await?;
    Ok(())
}

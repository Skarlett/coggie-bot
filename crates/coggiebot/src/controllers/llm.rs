use reqwest::{Error, header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE}};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::env;
use rand::{Rng};
use serenity::model::id::{UserId, ChannelId};
use serenity::prelude::TypeMapKey;
use serenity::client::Context;
use serenity::model::prelude::Message;
use serenity::client::ClientBuilder;
use serenity::framework::StandardFramework;
use serenity::framework::standard::{
    Args, CommandResult,
    macros::{command, group, hook},
};

#[group]
#[commands(set_model, get_model, cost, set_system_prompt)]
pub struct LLMCommands;

const CONTEXT_SZ: usize = 10;

#[derive(Deserialize, Debug)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: ChatMessage,
    index: u32,
    finish_reason: String,
}

#[derive(Deserialize, Debug)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct NanoGpt {
    cost: f64,
    input_tokens: u64,
    output_tokens: u64,
    payment_source: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
    nanoGPT: Option<NanoGpt>
}

struct UserQuota {
    daily_limit: u32,
    daily_used: u32,
    daily_reset: Instant,

    // Rate limiting bucket
    bucket_limit: u32,    // Max requests per bucket
    bucket_tokens: u32,   // Current tokens available
    bucket_last_refill: Instant,
    bucket_refill_rate: Duration, // How often we add tokens,
    notified: bool // was the user notified of daily rate overuse
}

struct QuotaManager {
    quotas: HashMap<UserId, UserQuota>,
    default_daily_limit: u32,
    default_bucket_limit: u32,
    default_refill_rate: Duration,
}

struct QuotaManagerKey;
impl TypeMapKey for QuotaManagerKey {
    type Value = Arc<Mutex<QuotaManager>>;
}

// UserId
const QUOTA_WHITELIST: &'static [ UserId ] = &[
    UserId(1270692556979830786)
];

impl QuotaManager {
    fn new(daily_limit: u32, bucket_limit: u32, refill_rate: Duration) -> Self {
        QuotaManager {
            quotas: HashMap::new(),
            default_daily_limit: daily_limit,
            default_bucket_limit: bucket_limit,
            default_refill_rate: refill_rate,
        }
    }

    fn check_quota(&mut self, user_id: UserId) -> bool {
        if QUOTA_WHITELIST.contains(&user_id) {
            return true;
        }

        let now = Instant::now();

        let quota = self.quotas.entry(user_id).or_insert_with(|| UserQuota {
            daily_limit: self.default_daily_limit,
            daily_used: 0,
            daily_reset: now + Duration::from_secs(3600 * 24),
            bucket_limit: self.default_bucket_limit,
            bucket_tokens: self.default_bucket_limit,
            bucket_last_refill: now,
            bucket_refill_rate: self.default_refill_rate,
            notified: false
        });

        // Reset daily quota if needed
        if now >= quota.daily_reset {
            quota.daily_used = 0;
            quota.daily_reset = now + Duration::from_secs(3600 * 24);
        }

        // Refill bucket tokens
        let elapsed = now.duration_since(quota.bucket_last_refill);
        let refills = (elapsed.as_millis() / quota.bucket_refill_rate.as_millis()) as u32;

        if refills > 0 {
            quota.bucket_tokens = (quota.bucket_tokens + refills).min(quota.bucket_limit);
            quota.bucket_last_refill += quota.bucket_refill_rate * refills;
        }

        // Check if request can be processed
        if quota.daily_used < quota.daily_limit && quota.bucket_tokens > 0 {
            quota.daily_used += 1;
            quota.bucket_tokens -= 1;
            true
        } else {
            false
        }
    }

    fn get_remaining_daily(&self, user_id: &UserId) -> u32 {
        if let Some(quota) = self.quotas.get(user_id) {
            quota.daily_limit - quota.daily_used
        } else {
            self.default_daily_limit
        }
    }

    fn get_remaining_bucket(&self, user_id: &UserId) -> u32 {
        if let Some(quota) = self.quotas.get(user_id) {
            quota.bucket_tokens
        } else {
            self.default_bucket_limit
        }
    }
}


#[derive(Serialize, Debug)]
pub struct LLMessage {
    pub role: String,
    pub content: String
}

#[derive(Serialize, Debug)]
pub struct LLMRequest {
    pub model: String,
    pub messages: Vec<LLMessage>
}

impl LLMRequest {
    fn new(model: String, messages: Vec<LLMessage>) -> Self {
        Self {
            model,
            messages
        }
    }
}

pub struct LLMState {
    model: String,
    system_prompt: String,
    apikey: String,
    cost_meter: f64,
}

struct LLMStateKey;
impl TypeMapKey for LLMStateKey {
    type Value = Arc<Mutex<LLMState>>;
}

impl LLMState {
    fn new(model: String, apikey: String) -> Self {
        Self {
            apikey,
            model: model.to_string(),
            system_prompt: include_str!("system-prompt.txt").to_string(),
            cost_meter: 0.0,
        }
    }

    async fn send(&self, aictx: &LLMRequest) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        // Set up headers
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.apikey.clone()))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Create HTTP client
        let client = reqwest::Client::new();

        // Simple chat completion
        let response = client
            .post("https://nano-gpt.com/api/v1/chat/completions")
            .headers(headers)
            .json(aictx)
            .send()
            .await?;

        // let response_data: ChatResponse = response.json().await?;
        // Try adding this to debug
        let text = response.text().await?;
        // println!("Raw JSON: {}", text);

        // Then parse separately
        let response_data: ChatResponse = serde_json::from_str(&text)?;
        return Ok(response_data)
    }

    async fn gather_context(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        limit: usize
    ) -> Result<LLMRequest, serenity::Error> {

        let mut aictx = LLMRequest::new(
            self.model.clone(),
            Vec::with_capacity(CONTEXT_SZ + 1)
        );

        aictx.messages.push(
            LLMessage {
                role: "system".to_string(),
                content: self.system_prompt.clone()
            }
        );

        // Get messages (up to limit)
        let messages = channel_id.messages(
            &ctx.http,
            |retriever| retriever.limit(limit as u64)
        ).await?;

        // Process messages
        for message in messages.iter().rev() {
            let nickname =
                message.author_nick(&ctx.http).await
                   .unwrap_or_else(|| message.author.name.clone());

            aictx.messages.push(LLMessage {
              role:
                if message.author.id == ctx.cache.current_user().id {
                    "assistant".to_string()
                }
                else { "user".to_string() },

              content: message.content.clone()
            });
        }
        Ok(aictx)
    }
}


const CHANNEL_WHITELIST: &'static [ ChannelId ] = &[
    ChannelId(943224671183204374)
];

pub async fn on_message(ctx: Context, msg: Message) -> Option<String> {
    // Ignore messages from bots including self
    if msg.author.bot {
        return None;
    }

    // Check if channel is whitelisted
    if !CHANNEL_WHITELIST.contains(&msg.channel_id) {
        return None;
    }

    // Check word count limit (200 words)
    let word_count = msg.content.split_whitespace().count();
    if !QUOTA_WHITELIST.contains(&msg.author.id) && word_count >= 200 {
        let _ = msg.reply(&ctx.http, "Message exceeds the 200 word limit.").await;
        return None;
    }

    // Get data lock
    let data = ctx.data.read().await;
    let quota_manager: Arc<Mutex<QuotaManager>> = data.get::<QuotaManagerKey>().unwrap().clone();
    let llm_manager: Arc<Mutex<LLMState>> = data.get::<LLMStateKey>().unwrap().clone();

    let bot_id = ctx.cache.current_user().id;

    // Check if bot is mentioned or random chance (1/64)
    let random_trigger = rand::thread_rng().gen_range(0..8) == 5;
    let is_link_only = msg.content.trim().starts_with("http")
        && !msg.content.trim().contains(" ");

    let bot_prefix = ['!', '.', '?', '$', '^', '~', '%', '&', '*'].contains(
        &msg.content.trim().chars().next().unwrap()
    );

    if is_link_only || bot_prefix {
        return None;
    }

    let is_mentioned = [
        "coggie",
        "coggers",
        "coggerz",
        "cogz",
        "coger",
        "cogger",
        "cugs",
        "cuggie",
        "terminator",
        "sentient",
        "jr",
        "luni",
        "lunarix",
        "dad"
    ].iter().any(|x| msg.content.contains(x)) || msg.mentions_user_id(bot_id);

    if is_mentioned || random_trigger {
        // For mentions, check quota
        if is_mentioned {
            let mut quota_manager = quota_manager.lock().await;
            let user_id = msg.author.id;

            if !quota_manager.check_quota(user_id.clone()) {
                return None;
            };
        };

        {
            let mut llm_manager = llm_manager.lock().await;

            if let Ok(aictx) = llm_manager.gather_context(ctx.clone(), msg.channel_id, CONTEXT_SZ).await {
                return match llm_manager.send(&aictx).await {
                    Ok(response) => {
                        if let Some(cost) = &response.nanoGPT {
                            llm_manager.cost_meter += cost.cost;
                        }

                        if !response.choices.is_empty() {
                            Some(response.choices[0].message.content.clone())
                        } else { None }
                    },

                    Err(e) => None, //{ let _ = msg.channel_id.say(&ctx.http, format!("{:?}", e)).await; }
                }
            }
        }
    }
    return None
}

#[hook]
async fn normal_message(ctx: &Context, msg: &Message) {
    let typing = msg.channel_id.start_typing(&ctx.http).unwrap();
    if let Some(response) = on_message(ctx.clone(), msg.clone()).await {
        let _ = msg.channel_id.say(&ctx, response).await;
        typing.stop();
    };
}

pub fn setup_framework(mut cfg: StandardFramework) -> StandardFramework {
    match (env::var("NANOGPT_API_KEY"), env::var("NANOGPT_MODEL")) {
       (Ok(_), Ok(_)) => return cfg.normal_message(normal_message),
       _ => {
           tracing::warn!("Skipping LLM due to missing NANOGPT_API_KEY or NANOGPT_MODEL env var");
           return cfg;
       }
    };
}

pub async fn init(mut cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("LLM initializing...");

    let key = match env::var("NANOGPT_API_KEY") {
       Ok(data) => data.to_string(),
       Err(e) => {
           tracing::warn!("Skipping LLM due to missing NANOGPT_API_KEY env var");
           return cfg;
       }
    };

    let model = match env::var("NANOGPT_MODEL") {
       Ok(data) => data.to_string(),
       Err(e) => {
           tracing::warn!("Skipping LLM due to missing NANOGPT_MODEL env var");
           return cfg;
       }
    };

    cfg =
        cfg
        .type_map_insert::<LLMStateKey>(Arc::new(Mutex::new(LLMState::new(model, key))))
        .type_map_insert::<QuotaManagerKey>(Arc::new(Mutex::new(QuotaManager::new(
            30, /* daily limit */
            2,  /* bucket limit */
            Duration::from_millis(15000) /* bucket refill */
        ))))
    ;

    cfg
}

#[command]
async fn set_model(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Check if user is whitelisted
    if !QUOTA_WHITELIST.contains(&msg.author.id) {
        msg.reply(ctx, "You don't have permission to use this command.").await?;
        return Ok(());
    }

    let model = args.rest().trim();
    if model.is_empty() {
        msg.reply(ctx, "Please provide a model name.").await?;
        return Ok(());
    }

    // Get data lock and update model
    let data = ctx.data.read().await;
    let llm_manager = data.get::<LLMStateKey>().unwrap().clone();
    {
        let mut llm_state = llm_manager.lock().await;
        llm_state.model = model.to_string();
    }

    msg.reply(ctx, &format!("Model set to: {}", model)).await?;
    Ok(())
}

#[command]
async fn get_model(ctx: &Context, msg: &Message) -> CommandResult {
    // Get data lock and retrieve model
    let data = ctx.data.read().await;
    let llm_manager = data.get::<LLMStateKey>().unwrap().clone();
    let model;
    {
        let llm_state = llm_manager.lock().await;
        model = llm_state.model.clone();
    }

    msg.reply(ctx, &format!("Current model: {}", model)).await?;
    Ok(())
}


#[command]
pub async fn cost(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let llm_manager = data.get::<LLMStateKey>().unwrap().clone();
    let cost;
    {
        let llm_state = llm_manager.lock().await;
        cost = llm_state.cost_meter.clone();
    }

    msg.reply(ctx, format!("{}", cost)).await?;
    Ok(())
}

#[command]
pub async fn set_system_prompt(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if !CHANNEL_WHITELIST.contains(&msg.channel_id) {
        return Ok(());
    }

    let llm_manager = ctx.data.read().await.get::<LLMStateKey>().unwrap().clone();
    let prompt = args.rest().trim();
    {
        let mut llm_state = llm_manager.lock().await;
        llm_state.system_prompt = prompt.to_string();
    }
    msg.reply(ctx, "New System Prompt in place.");
    Ok(())
}

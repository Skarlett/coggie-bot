use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::{cookie::Jar, Url};
use tokio::process::Command;
use tokio::io::AsyncWriteExt;

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/112.0";

#[derive(Debug)]
pub enum ARLError {
    ParseError(serde_json::Error),
    HTTP(reqwest::Error),
}

impl From<serde_json::Error> for ARLError {
    fn from(e: serde_json::Error) -> Self {
        Self::ParseError(e)
    }
}

impl From<reqwest::Error> for ARLError {
    fn from(e: reqwest::Error) -> Self {
        Self::HTTP(e)
    }
}

impl std::fmt::Display for ARLError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError(e) => write!(f, "Parse error: {}", e),
            Self::HTTP(e) => write!(f, "HTTP error: {}", e),
        }
    }
}

impl std::error::Error for ARLError {}

#[derive(Debug)]
pub struct ExtractChecks {
    pub name: String,
    pub email: String,
    pub explicit: String,
    pub offer_name: String,
    pub offer_id: i64,
    pub country: String,
    pub expiration: i64,
    pub inscription: String,
    pub default_sound_quality: String,
    pub lossless: bool,
    pub mobile_sq: SoundQuality,
    pub tablet_sq: SoundQuality,
    pub web_sq: SoundQuality,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SoundQuality {
    pub high: bool,
    pub lossless: bool,
    pub low: bool,
    pub reality: bool,
    pub standard: bool,
}

impl ExtractChecks {
    #[inline(always)]
    pub fn premium(&self) -> bool {
        self.offer_id > 100_000
    }

    #[inline(always)]
    pub fn explicit(&self) -> bool {
        self.explicit.to_lowercase() == "explicit_display"
    }

    #[inline(always)]
    pub fn lossless(&self) -> bool {
        self.lossless
        // self.mobile_sq.lossless || self.tablet_sq.lossless || self.web_sq.lossless
    }

    pub async fn tabulize(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut child = Command::new("column")
            .arg("-t")
            .arg("-s")
            .arg(",")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn command");

        fn f(x: bool) -> char {
            x.then(|| '✓').unwrap_or('✗')
        }

        child.stdin
            .as_mut()
            .expect("failed to get stdin")
            .write_all(
                format!(
                    "Quality,Mobile,Tablet,Web\nReality,{},{},{}\nLossless,{},{},{}\nHigh,{},{},{}\nStandard,{},{},{}\nLow,{},{},{}\n",

                    f(self.mobile_sq.reality),
                    f(self.tablet_sq.reality),
                    f(self.web_sq.reality),

                    f(self.mobile_sq.lossless),
                    f(self.tablet_sq.lossless),
                    f(self.web_sq.lossless),

                    f(self.mobile_sq.high),
                    f(self.tablet_sq.high),
                    f(self.web_sq.high),

                    f(self.mobile_sq.standard),
                    f(self.tablet_sq.standard),
                    f(self.web_sq.standard),

                    f(self.mobile_sq.low),
                    f(self.tablet_sq.low),
                    f(self.web_sq.low),
               ).as_bytes()
            )
            .await
            .expect("failed to write to stdin");
        return Ok(String::from_utf8(child.wait_with_output().await?.stdout)?);
    }

    #[inline(always)]
    pub fn dank(&self) -> bool {
        self.premium()
            && self.explicit()
            && self.lossless()
    }
}

pub async fn get_arl_data(arl: &str) -> Result<Value, reqwest::Error> {
    let url = "https://www.deezer.com".parse::<Url>().unwrap();
    let jar = Jar::default();
    jar.add_cookie_str(&format!("arl={}; Domain=deezer.com", arl), &url);
    let resp = reqwest::Client::builder()
        .cookie_store(true)
        .cookie_provider(std::sync::Arc::new(jar))
        .build()?
        .post("https://www.deezer.com/ajax/gw-light.php?method=deezer.getUserData&input=3&api_version=1.0&api_token=&cid=433085605")
        .header("Origin", "https://www.deezer.com")
        .header("Referer", "https://www.deezer.com/us/")
        .header("User-Agent", USER_AGENT)
        .body("{}")
        .send()
        .await?;

    Ok(resp.json().await?)
}


pub async fn check_arl(arl: &str) -> Result<ExtractChecks, ARLError> {
    let data = get_arl_data(arl).await?;

    let mut name: String = String::from("Anonymous");
    let firstname = data["results"]["USER"]["FIRSTNAME"].to_string().trim().to_string();
    let blogname = data["results"]["USER"]["BLOG_NAME"].to_string().trim().to_string();

    if !firstname.is_empty() {
        name = firstname.to_string();
    }
    else if !blogname.is_empty() {
        name = blogname.to_string();
    }

    Ok(ExtractChecks {
        name,
        explicit: data["results"]["USER"]["EXPLICIT_CONTENT_LEVEL"].as_str().unwrap().to_string(),
        offer_name: data["results"]["OFFER_NAME"].to_string(),
        offer_id: data["results"]["OFFER_ID"].as_i64().unwrap(),
        email: data["results"]["USER"]["EMAIL"].to_string(),
        country: data["results"]["USER"]["OPTIONS"]["license_country"].as_str().unwrap().to_string(),
        default_sound_quality: data["results"]["USER"]["OPTIONS"]["audio_quality_default_preset"].to_string(),
        expiration: data["results"]["USER"]["OPTIONS"]["expiration_timestamp"].as_i64().unwrap(),
        inscription: data["results"]["USER"]["INSCRIPTION_DATE"].to_string(),
        lossless: data["results"]["USER"]["OPTIONS"]["mobile_lossless"].as_bool().unwrap(),
        mobile_sq: dbg!(serde_json::from_value::<SoundQuality>(dbg!(data["results"]["USER"]["OPTIONS"]["mobile_sound_quality"].clone())))?,
        tablet_sq: serde_json::from_value::<SoundQuality>(data["results"]["USER"]["OPTIONS"]["tablet_sound_quality"].clone())?,
        web_sq: dbg!(serde_json::from_value::<SoundQuality>(dbg!(data["results"]["USER"]["OPTIONS"]["web_sound_quality"].clone())))?,
      })
}

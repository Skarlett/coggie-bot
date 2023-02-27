/// ARL tokens are used for deezer API access
struct ArlToken;
impl TypeMapKey for ArlToken {
    type Value = String;
}


struct DeezerConfig
{
    pub arl_token: String,
}

fn play_deezer() {}



#[group]
struct Demix;

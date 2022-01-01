use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct AylaLoginResponse {
    pub access_token: String,
    pub refresh_token: String,
}

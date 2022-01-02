use crate::ayla::RequestType;
use crate::region::Region;
use reqwest::{Method, Response, StatusCode};
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use serde_repr::*;

mod ayla;
pub mod error;
mod models;
pub mod region;
pub use error::SharkError;

pub type Result<T> = std::result::Result<T, error::SharkError>;

#[derive(Deserialize)]
struct GetDevicesResponse {
    device: SharkDevice,
}

#[derive(Serialize_repr)]
#[repr(u8)]
pub enum OperatingMode {
    Stop = 0,
    Pause = 1,
    Start = 2,
    Return = 3,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct SharkDevice {
    pub dsn: String,
    pub model: String,
    pub oem_model: String,
    pub mac: String,
    pub product_name: String,
    pub key: u64,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub struct SharkClientBuilder {
    email: String,
    password: String,
    region: Region,
}

impl SharkClientBuilder {
    fn new(email: String, password: String) -> Self {
        Self {
            email,
            password,
            region: Region::Us,
        }
    }

    pub fn region(mut self, region: Region) -> Self {
        self.region = region;
        self
    }

    /// Given the provided credentials attempt to login to the Shark API service
    pub async fn build(self) -> Result<SharkClient> {
        SharkClient::from_creds(self.region, self.email, self.password).await
    }
}

pub struct SharkClient {
    ayla: ayla::AylaClient,
}

impl SharkClient {
    pub fn builder<E, P>(email: E, password: P) -> SharkClientBuilder
    where
        E: Into<String>,
        P: Into<String>,
    {
        SharkClientBuilder::new(email.into(), password.into())
    }

    async fn from_creds(region: Region, email: String, password: String) -> Result<Self> {
        let ayla = ayla::AylaClient::new(region, email, password);

        let mut sharkvac = Self { ayla };
        sharkvac.ayla.sign_in().await?;

        Ok(sharkvac)
    }

    pub async fn get_devices(&self) -> Result<Vec<SharkDevice>> {
        let req = self.ayla.request(
            RequestType::Device,
            Method::GET,
            "/apiv1/devices",
            None::<()>,
        )?;

        let res = self.ayla.execute(req).await?;
        Ok(get_api_response::<Vec<GetDevicesResponse>>(res)
            .await?
            .into_iter()
            .map(|v| v.device)
            .collect())
    }

    pub async fn get_device_properties(&self, dsn: &str) -> Result<()> {
        let req = self.ayla.request(
            RequestType::Device,
            Method::GET,
            format!("/apiv1/dsns/{}/properties", dsn),
            None::<()>,
        )?;

        let res = self.ayla.execute(req).await?;
        let properties = get_api_response::<serde_json::Value>(res).await?;
        println!("{:#?}", properties);

        Ok(())
    }

    pub async fn set_device_operating_mode(&self, dsn: &str, mode: OperatingMode) -> Result<()> {
        let body = json!({ "datapoint": { "value": mode }});
        let req = self.ayla.request(
            RequestType::Device,
            Method::POST,
            format!(
                "/apiv1/dsns/{}/properties/SET_Operating_Mode/datapoints",
                dsn
            ),
            Some(body),
        )?;

        let res = self.ayla.execute(req).await?;
        let _ = get_api_response(res).await?;
        Ok(())
    }

    /// Refresh API token (expires after 24h)
    pub async fn refresh_token(&mut self) -> Result<()> {
        self.ayla.refresh_token().await?;
        Ok(())
    }

    /// Sign out of the Shark API
    pub async fn sign_out(self) -> Result<()> {
        self.ayla.sign_out().await?;
        Ok(())
    }
}

/// Check for a successful Shark API response or return a SharkError
async fn get_api_response<T>(r: Response) -> Result<T>
where
    T: DeserializeOwned,
{
    match r.status() {
        StatusCode::OK => Ok(r.json::<T>().await?),
        sc => Err(error::SharkError::ApiError(sc, r.text().await.unwrap())),
    }
}

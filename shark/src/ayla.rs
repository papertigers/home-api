use crate::models::AylaLoginResponse;
use crate::{error, Region};
use reqwest::{header, Client, Method, Request, Response, StatusCode, Url};
use serde::Serialize;
use serde_json::json;

type Result<T> = std::result::Result<T, error::AylaError>;

pub(crate) struct AylaClient {
    client: Client,
    region: Region,
    email: String,
    password: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    auth_expiration: Option<String>,
    is_authenticated: bool,
}

pub(crate) enum RequestType {
    Device,
    User,
}

impl AylaClient {
    pub(crate) fn new(region: Region, email: String, password: String) -> Self {
        let mut headers = header::HeaderMap::new();
        headers.append(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.append(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );
        let client = Client::builder().default_headers(headers).build().unwrap();
        Self {
            client,
            region,
            email,
            password,
            access_token: None,
            refresh_token: None,
            auth_expiration: None,
            is_authenticated: false,
        }
    }

    pub(crate) fn request<P, B>(
        &self,
        req_type: RequestType,
        method: Method,
        path: P,
        body: Option<B>,
    ) -> Result<Request>
    where
        P: AsRef<str>,
        B: Serialize,
    {
        let base = match req_type {
            RequestType::Device => Url::parse(self.region.device_url()).unwrap(),
            RequestType::User => Url::parse(self.region.user_url()).unwrap(),
        };

        let url = base.join(path.as_ref()).unwrap();
        let mut rb = self.client.request(method, url);

        if self.is_authenticated {
            if let Some(token) = &self.access_token {
                let mut headers = header::HeaderMap::new();
                headers.append(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(token).unwrap(),
                );
                rb = rb.headers(headers);
            }
        }

        if let Some(b) = body {
            rb = rb.json(&b);
        }

        Ok(rb.build()?)
    }

    pub(crate) async fn execute(&self, request: Request) -> Result<Response> {
        let res = self.client.execute(request).await?;
        Ok(res)
    }

    pub(crate) async fn sign_in(&mut self) -> Result<()> {
        if !self.is_authenticated {
            let body = json!({
                "user": {
                    "email": self.email,
                    "password": self.password,
                    "application": {
                        "app_id": self.region.app_id(),
                        "app_secret": self.region.app_secret(),
                    }
                }
            });

            let req = self.request(
                RequestType::User,
                Method::POST,
                "/users/sign_in",
                Some(body),
            )?;

            let res = self.client.execute(req).await?;
            match res.status() {
                StatusCode::OK => {
                    let alr: AylaLoginResponse = res.json().await?;
                    self.access_token = Some(alr.access_token);
                    self.refresh_token = Some(alr.refresh_token);
                    self.auth_expiration = None;
                    self.is_authenticated = true;
                }
                sc => {
                    return Err(error::AylaError::LoginError(sc, res.text().await.unwrap()));
                }
            };
        }

        Ok(())
    }

    pub(crate) async fn refresh_token(&mut self) -> Result<()> {
        let body = json!({"user": {"refresh_token": self.refresh_token }});
        let req = self.request(
            RequestType::User,
            Method::POST,
            "/users/refresh_token",
            Some(body),
        )?;

        let res = self.client.execute(req).await?;
        match res.status() {
            StatusCode::OK => {
                let alr: AylaLoginResponse = res.json().await?;
                self.access_token = Some(alr.access_token);
                self.refresh_token = Some(alr.refresh_token);
                self.auth_expiration = None;
                self.is_authenticated = true;
            }
            sc => {
                return Err(error::AylaError::RefreshTokenError(
                    sc,
                    res.text().await.unwrap(),
                ));
            }
        };

        Ok(())
    }

    pub(crate) async fn sign_out(self) -> Result<()> {
        if self.is_authenticated {
            let body = json!({"user": {"access_token": self.access_token }});
            let req = self.request(
                RequestType::User,
                Method::POST,
                "/users/sign_out",
                Some(body),
            )?;
            let res = self.client.execute(req).await?;
            match res.status() {
                StatusCode::OK => (),
                sc => {
                    return Err(error::AylaError::LogoutError(sc, res.text().await.unwrap()));
                }
            };
        }

        Ok(())
    }
}

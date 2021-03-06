use crate::with_anyhow;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::url::Url;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, TokenResponse,
    TokenUrl,
};
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::reject;
use warp::reject::{InvalidHeader, MissingCookie, Reject};
use warp::reply;
use warp::Filter;

#[derive(Debug)]
pub struct OAuth {
    pub url: Url,
    client: BasicClient,
    user_table: Mutex<HashMap<String, String>>,

    #[allow(unused)]
    csrf_token: CsrfToken, // TODO
}

impl OAuth {
    pub fn new() -> anyhow::Result<Self> {
        let client = BasicClient::new(
            ClientId::new(env::var("CS5223FET_CLIENT_ID")?),
            Some(ClientSecret::new(env::var("CS5223FET_SECRET")?)),
            AuthUrl::new("https://github.com/login/oauth/authorize".to_string())?,
            Some(TokenUrl::new(
                "https://github.com/login/oauth/access_token".to_string(),
            )?),
        )
        .set_redirect_uri(RedirectUrl::new(format!(
            "{}/redirect",
            env::var("CS5223FET_URL")?
        ))?);
        let (auth_url, csrf_token) = client.authorize_url(CsrfToken::new_random).url();
        Ok(Self {
            client,
            user_table: Mutex::new(HashMap::new()),
            url: auth_url,
            csrf_token,
        })
    }
}

#[derive(Debug)]
struct Expired;
impl Reject for Expired {}

#[derive(Deserialize)]
struct User {
    login: String,
}

impl OAuth {
    pub fn user_id(
        self: &Arc<Self>,
    ) -> impl Filter<Extract = (String,), Error = warp::Rejection> + Clone {
        let oauth = self.clone();
        warp::cookie::<String>("token").and_then(move |token| {
            let oauth = oauth.clone();
            async move {
                let user_table = &oauth.user_table;
                let id = user_table.lock().await.get(&token).cloned();
                let id = if let Some(id) = id {
                    id.clone()
                } else {
                    let id: anyhow::Result<_> = async {
                        let resp = reqwest::Client::new()
                            .get("https://api.github.com/user")
                            .header("Authorization", format!("token {}", token))
                            .header("Accept", "application/vnd.github.v3+json")
                            .header("User-Agent", "Foo") // https://stackoverflow.com/a/21979251
                            .send()
                            .await?;
                        Ok(resp.json::<User>().await?.login.clone())
                    }
                    .await;
                    let id = if let Ok(id) = id {
                        id
                    } else {
                        return Err(reject::custom(Expired));
                    };
                    user_table.lock().await.insert(token, id.clone());
                    id
                };
                Ok(id)
            }
        })
    }
}

#[derive(Deserialize)]
struct RedirectQuery {
    code: String,
}

impl OAuth {
    pub fn redirect(
        self: &Arc<Self>,
        home_prompt: String,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let oauth = self.clone();
        warp::path("redirect")
            .and(warp::query())
            .and_then(move |query: RedirectQuery| {
                let oauth = oauth.clone();
                let home_prompt = home_prompt.clone();
                with_anyhow(async move {
                    let token_resp = oauth
                        .client
                        .exchange_code(AuthorizationCode::new(query.code))
                        .request_async(async_http_client)
                        .await?;
                    let token = token_resp.access_token().secret();
                    Ok(reply::with_header(
                        reply::html(home_prompt),
                        "Set-Cookie",
                        format!("token={}", token),
                    ))
                })
            })
    }

    pub fn recover(
        route: impl Clone + Filter<Extract = impl warp::Reply, Error = warp::Rejection>,
        login_prompt: String,
    ) -> impl Filter<Extract = impl warp::Reply> + Clone {
        route.recover(move |rejection: warp::Rejection| {
            let login_prompt = Ok(reply::html(login_prompt.clone()));
            async move {
                if let Some(Expired) = rejection.find() {
                    return login_prompt;
                }
                if let Some(_) = rejection.find::<InvalidHeader>() {
                    return login_prompt;
                }
                if let Some(_) = rejection.find::<MissingCookie>() {
                    return login_prompt;
                }
                Err(rejection)
            }
        })
    }
}

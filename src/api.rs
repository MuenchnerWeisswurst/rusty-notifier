use std::collections::HashMap;

use anyhow::anyhow;
use reqwest::header::HeaderMap;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ACCEPT: &str = "Accept";
const CONTENT: &str = "Content-Type";
const COOKIE: &str = "set-cookie";
const SENT_COOKIE: &str = "cookie";
const ACCEPT_TYPE: &str = "application/json";

#[derive(Debug, Deserialize)]
struct RpcResponse {
    error: Option<Value>,
    #[serde(alias = "id")]
    _id: i32,
    result: Value,
}

#[derive(Debug, Serialize)]
pub struct RpcRequest {
    pub method: String,
    pub params: Vec<Value>,
    pub id: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CurrentState {
    pub queue: HashMap<String, f64>,
    pub ip: String,
}

fn get_headermap(cookie: Option<String>) -> HeaderMap {
    let mut header_map = HeaderMap::new();
    header_map.insert(ACCEPT, ACCEPT_TYPE.parse().unwrap());
    header_map.insert(CONTENT, ACCEPT_TYPE.parse().unwrap());
    if let Some(c) = cookie {
        header_map.insert(SENT_COOKIE, c.parse().unwrap());
    }
    header_map
}

pub async fn get_current_state(
    url: &String,
    login_request: &RpcRequest,
    update_request: &RpcRequest,
    update_key: &String,
) -> Result<CurrentState, anyhow::Error> {
        let client = Client::new();
        if let Ok(cookie) = login(url, &client, login_request).await {
            return get_state(url, &client, update_request, update_key, cookie).await;
        }
        Err(anyhow!("Unable to do stuff"))
}

async fn send_request(
    url: &String,
    client: &Client,
    request: &RpcRequest,
    cookie: Option<String>,
) -> reqwest::Result<Response> {
    client
        .post(url)
        .json(request)
        .headers(get_headermap(cookie))
        .send()
        .await
}

async fn login(url: &String, client: &Client, request: &RpcRequest) -> Result<String, anyhow::Error> {
    let res = send_request(url, client, request, None).await?;
    let headers = res.headers().to_owned();
    let ok = res.json::<RpcResponse>().await.map(|t| match t.result {
        Value::Bool(a) => a && t.error.is_none(),
        _ => false,
    });
    match ok {
        Ok(b) => {
            if !b {
                return Err(anyhow!("Login failed!"));
            }
        }
        Err(e) => return Err(anyhow!(e)),
    }
    headers
        .get(COOKIE)
        .map(|c| c.to_str().map_err(|e| anyhow!(e)))
        .unwrap_or_else(|| Err(anyhow!("set-cookie header not set!")))
        .map(String::from)
}

async fn get_state(
    url: &String,
    client: &Client,
    request: &RpcRequest,
    update_key: &String,
    cookie: String,
) -> Result<CurrentState, anyhow::Error> {
    let res = send_request(url, client, request, Some(cookie)).await?.json::<RpcResponse>().await?;

    match res.result {
        Value::Object(ref map) => {
            let ip = map
                .get("stats")
                .and_then(|obj| match obj {
                    Value::Object(stats_map) => stats_map.get("external_ip").map(|ip| ip.as_str()),
                    _ => None,
                })
                .flatten()
                .map(String::from);
            let res_map = map.get(update_key).and_then(|imap| match imap {
                Value::Object(tmap) => {
                    let res_map: HashMap<String, f64> = tmap
                        .iter()
                        .filter_map(|(_t_id, v)| match v {
                            Value::Object(data) => {
                                let name =
                                    data.get("name").and_then(|v| v.as_str()).map(String::from);
                                let progress = data.get("progress").and_then(|v| v.as_f64());
                                match (name, progress) {
                                    (Some(n), Some(p)) => Some((n, p)),
                                    _ => None,
                                }
                            }
                            _ => None,
                        })
                        .collect();
                    Some(res_map)
                }
                _ => None,
            });
            match (ip, res_map) {
                (Some(iip), Some(map)) => Ok(CurrentState {
                    ip: iip,
                    queue: map,
                }),
                _ => Err(anyhow!("Weird response, received : {:?}", &res)),
            }
        }
        _ => Err(anyhow!("Weird respone, received : {:?}", &res)),
    }
}

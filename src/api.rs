use std::collections::HashMap;

use anyhow::anyhow;
use reqwest::{blocking::*, header::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::thread;

use crate::ApiConfig;
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
struct RpcRequest {
    method: String,
    params: Vec<Value>,
    id: i32,
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

pub fn get_current_state(config: ApiConfig) -> Result<CurrentState, anyhow::Error> {
    thread::spawn(move || {
        let client = Client::new();
        if let Ok(cookie) = login(&config, &client) {
            return get_state(&config, &client, cookie);
        }
        Err(anyhow!("Unable to do stuff"))
    })
    .join()
    .expect("getting the current status thread failed")
}

fn send_request(
    config: &ApiConfig,
    client: &Client,
    request: RpcRequest,
    cookie: Option<String>,
) -> reqwest::Result<Response> {
    client
        .post(&config.url)
        .json(&request)
        .headers(get_headermap(cookie))
        .send()
}

fn login(config: &ApiConfig, client: &Client) -> Result<String, anyhow::Error> {
    let res = send_request(
        config,
        client,
        RpcRequest {
            method: config.login_method.clone(),
            params: vec![Value::String(config.password.clone())],
            id: 1,
        },
        None,
    )?;
    let headers = res.headers().clone();
    let ok = res.json::<RpcResponse>().map(|t| match t.result {
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

fn get_state(
    config: &ApiConfig,
    client: &Client,
    cookie: String,
) -> Result<CurrentState, anyhow::Error> {
    let res = send_request(
        config,
        client,
        RpcRequest {
            method: config.update_method.clone(),
            params: vec![
                Value::Array(vec![
                    Value::String("name".to_string()),
                    Value::String("progress".to_string()),
                ]),
                Value::Object(Map::new()),
            ],
            id: 1,
        },
        Some(cookie),
    )?
    .json::<RpcResponse>()?;

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
            let res_map = map
                // TODO: Make this key configurable for legal reasons
                .get(&config.update_key)
                .and_then(|imap| match imap {
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

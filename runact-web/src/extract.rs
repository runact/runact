use std::collections::HashMap;

use crate::request::Request;

#[derive(Debug, Clone)]
pub struct Path {
    params: Vec<(String, String)>,
}

impl Path {
    pub fn extract(req: &Request) -> Result<Self, String> {
        let params = req
            .params_ref()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(Self { params })
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Query {
    pairs: HashMap<String, String>,
}

impl Query {
    pub fn extract(req: &Request) -> Result<Self, String> {
        let qs = req.query_string().unwrap_or("");
        let pairs = parse_query_string(qs);
        Ok(Self { pairs })
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.pairs.get(name).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Json {
    data: Vec<u8>,
}

impl Json {
    pub fn extract(req: &Request) -> Result<Self, String> {
        Ok(Self {
            data: req.body.clone(),
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.data)
    }
}

#[derive(Debug, Clone)]
pub struct Header {
    value: String,
}

impl Header {
    pub fn extract(req: &Request, name: &str) -> Result<Self, String> {
        req.headers
            .get(name)
            .map(|v| Self {
                value: v.to_string(),
            })
            .ok_or_else(|| format!("missing header: {name}"))
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

fn parse_query_string(qs: &str) -> HashMap<String, String> {
    qs.split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?.to_string();
            let value = parts.next().unwrap_or("").to_string();
            Some((key, value))
        })
        .collect()
}

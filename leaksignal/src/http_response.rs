use std::{
    collections::HashMap,
    io::Write,
    pin::Pin,
    str::FromStr,
    sync::Arc,
    task::Poll,
    time::{Duration, SystemTime},
};

use anyhow::{bail, Result};
use fancy_regex::Regex;
use flate2::write::GzDecoder;
use futures::{task::waker, Future, FutureExt};
use log::{error, warn};
use prost::Message;
use proxy_wasm::{
    hostcalls,
    traits::{Context, HttpContext},
    types::{Action, MetricType},
};
use rand::{thread_rng, Rng};

use crate::{
    config::{upstream, UpstreamConfigHandle, LEAKSIGNAL_SERVICE_NAME},
    metric::Metric,
    parsers::{html::parse_html, json::parse_json, ParseResponse},
    pipe::{pipe, DummyWaker, PipeReader, PipeWriter},
    policy::{policy, PathPolicy, TokenExtractionConfig, TokenExtractionSite},
    proto::{Header, MatchDataRequest},
    GIT_COMMIT,
};

const MATCH_PUSH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, PartialEq, Debug)]
enum ContentType {
    Html,
    Json,
    Jpeg,
    Unknown,
}

impl Default for ContentType {
    fn default() -> Self {
        ContentType::Unknown
    }
}

impl FromStr for ContentType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "text/html" => ContentType::Html,
            "image/jpg" | "image/jpeg" => ContentType::Jpeg,
            "application/json" => ContentType::Json,
            _ => ContentType::Unknown,
        })
    }
}

impl Default for ContentEncoding {
    fn default() -> Self {
        ContentEncoding::None
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ContentEncoding {
    Gzip,
    None,
    Unknown,
}

impl FromStr for ContentEncoding {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "gzip" => ContentEncoding::Gzip,
            _ => ContentEncoding::Unknown,
        })
    }
}

pub struct HttpResponseContext {
    //todo: remove when on_http_response_headers fixed (see internal comment)
    has_response_started: bool,
    data: Option<ResponseData>,
    content_encoding: ContentEncoding,
    decompressor: GzDecoder<Vec<u8>>,
    response_writer: Option<PipeWriter>,
    response_read_task: Option<Pin<Box<dyn Future<Output = Option<ResponseOutputData>>>>>,
}

impl Default for HttpResponseContext {
    fn default() -> Self {
        Self {
            has_response_started: false,
            content_encoding: Default::default(),
            data: Some(Default::default()),
            response_writer: None,
            response_read_task: None,
            decompressor: GzDecoder::new(vec![]),
        }
    }
}

#[derive(Default)]
struct ResponseData {
    request_start: u64,
    response_start: u64,
    response_body_start: u64,
    request_headers: Vec<Header>,
    response_headers: Vec<Header>,
    content_type: ContentType,
    content_encoding: ContentEncoding,
    path: String,
    token: Option<String>,
    policy: Option<PathPolicy>,
    ip: String,
}

struct ResponseOutputData {
    response: ParseResponse,
    packet: MatchDataRequest,
    upstream: Option<UpstreamConfigHandle>,
}

fn timestamp() -> u64 {
    hostcalls::get_current_time()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

async fn response_body_task(
    mut data: ResponseData,
    mut reader: PipeReader,
) -> Option<ResponseOutputData> {
    let policy = match policy() {
        Some(policy) => policy,
        None => {
            warn!("processing response, but no policy loaded");
            return None;
        }
    };
    // todo: cache this?
    let path_policy = data.policy.expect("missing policy");

    let mut matches = vec![];

    let response = match data.content_type {
        ContentType::Html => {
            match parse_html(
                &*policy,
                &mut reader,
                &path_policy.configuration,
                &mut matches,
            )
            .await
            {
                Ok(x) => x,
                Err(e) => {
                    error!("failed to read html: {:?}", e);
                    return None;
                }
            }
        }
        ContentType::Json => {
            match parse_json(
                &*policy,
                &mut reader,
                &path_policy.configuration,
                &mut matches,
            )
            .await
            {
                Ok(x) => x,
                Err(e) => {
                    error!("failed to read json: {:?}", e);
                    return None;
                }
            }
        }
        ContentType::Jpeg => unimplemented!(), // parse_jpeg(&*body, &configuration),
        ContentType::Unknown => unreachable!(),
    };

    let body_size = reader.total_read() as u64;
    let body = if policy.body_collection_rate <= 0.0 {
        None
    } else {
        let chance: f64 = thread_rng().gen();
        if chance < policy.body_collection_rate {
            reader.fetch_full_content()
        } else {
            None
        }
    };

    let response_body_end = timestamp();

    let upstream = upstream();

    let mut match_counts: HashMap<&str, i64> = HashMap::new();
    for matching in &matches {
        *match_counts.entry(&*matching.category_name).or_default() += 1;
    }
    let policy_path = path_policy.policy_path;
    for (category_name, count) in match_counts {
        let metric = Metric::lookup_or_define(
            format!("ls.{policy_path}.{category_name}.count"),
            MetricType::Counter,
        );
        metric.increment(count);
    }

    let packet = MatchDataRequest {
        api_key: upstream.as_ref().map(|x| x.api_key.clone()).flatten(),
        deployment_name: upstream
            .as_ref()
            .map(|x| x.deployment_name.clone())
            .unwrap_or_default(),
        policy_id: policy.policy_id().to_string(),
        highest_action_taken: crate::proto::Action::None as i32,
        time_request_start: data.request_start,
        time_response_start: data.response_start,
        time_response_body_start: data.response_body_start,
        time_response_body_end: response_body_end,
        time_response_parse_end: response_body_end,
        request_headers: std::mem::take(&mut data.request_headers),
        response_headers: std::mem::take(&mut data.response_headers),
        matches,
        body_size,
        body,
        policy_path,
        commit: GIT_COMMIT.to_string(),
        token: data.token.unwrap_or_default(),
        ip: data.ip,
    };

    Some(ResponseOutputData {
        response,
        packet,
        upstream,
    })
}

impl Context for HttpResponseContext {
    fn on_grpc_call_response(&mut self, _token_id: u32, status_code: u32, _response_size: usize) {
        if status_code != 0 {
            warn!("MatchData upload failed with status_code {status_code}");
        }
    }
}

impl HttpResponseContext {
    fn process_data(&mut self, body_size: usize) -> Result<Vec<u8>> {
        let body = match self.get_http_response_body(0, body_size) {
            Some(x) => x,
            None => {
                bail!("missing body for response");
            }
        };
        match self.content_encoding {
            ContentEncoding::Gzip => {
                self.decompressor.write_all(&body[..])?;
                Ok(self.decompressor.get_mut().drain(..).collect::<Vec<_>>())
            }
            ContentEncoding::None | ContentEncoding::Unknown => Ok(body),
        }
    }

    fn data(&mut self) -> &mut ResponseData {
        self.data.as_mut().unwrap()
    }
}

fn extract_token_regex(value: &str, regex: &Regex) -> Option<String> {
    let captures = regex.captures(value).ok()??;
    if let Some(captured) = captures.get(1) {
        Some(captured.as_str().to_string())
    } else {
        Some(captures.get(0)?.as_str().to_string())
    }
}

impl HttpContext for HttpResponseContext {
    fn on_http_request_headers(&mut self, _num_headers: usize, end_of_stream: bool) -> Action {
        if !end_of_stream {
            //TODO: this doesn't work for some reason
            // return Action::Pause;
        }
        let policy = match policy() {
            Some(policy) => policy,
            None => {
                warn!("processing request headers, but no policy loaded");
                return Action::Continue;
            }
        };

        if self.data().request_start == 0 {
            let mut ip = String::from_utf8_lossy(
                &self
                    .get_property(vec!["source", "address"])
                    .unwrap_or_default()[..],
            )
            .into_owned();
            // remove port number
            if let Some(last_colon) = ip.rfind(':') {
                ip.truncate(last_colon);
            }

            let path = self
                .get_http_request_header_bytes(":path")
                .map(|x| String::from_utf8_lossy(&x).into_owned())
                .unwrap_or_else(|| "/".to_string());
            let hostname = self
                .get_http_request_header_bytes(":authority")
                .map(|x| String::from_utf8_lossy(&x).into_owned())
                .unwrap_or_else(String::new);
            let data = self.data();
            data.ip = ip;
            data.request_start = timestamp();
            data.path = path;
            let full_path = format!("{}{}", hostname, data.path);
            data.policy = Some(policy.get_path_config(&*full_path));
        } else if self.data().policy.is_none() {
            return Action::Continue;
        }

        for (name, value) in self.get_http_request_headers_bytes() {
            let value = match String::from_utf8(value) {
                Ok(x) => x,
                Err(_) => continue,
            };

            let data = self.data();

            if let Some(policy) = &data.policy {
                match policy.token_extractor.as_deref() {
                    Some(TokenExtractionConfig {
                        location: TokenExtractionSite::Request,
                        header,
                        regex,
                    }) if &name == header => {
                        data.token = extract_token_regex(&*value, &regex.0);
                    }
                    Some(TokenExtractionConfig {
                        location: TokenExtractionSite::RequestCookie,
                        header,
                        regex,
                    }) if name == "cookie" => {
                        for value in value.split("; ") {
                            let (name, value) = match value.split_once('=') {
                                Some(x) => x,
                                None => continue,
                            };
                            if name == header {
                                data.token = extract_token_regex(value, &regex.0);
                                break;
                            }
                        }
                    }
                    _ => (),
                }
            }

            let value = if policy.collected_request_headers.contains(&name) {
                Some(value)
            } else {
                None
            };

            data.request_headers.push(Header { name, value });
        }
        Action::Continue
    }

    fn on_http_response_headers(&mut self, _: usize, end_of_stream: bool) -> Action {
        if !end_of_stream {
            //TODO: this doesn't work for some reason
            // return Action::Pause;
        }

        if self.data().policy.is_none() {
            return Action::Continue;
        }

        if !self.has_response_started {
            self.has_response_started = true;
            self.data.as_mut().unwrap().response_start = timestamp();
        }
        self.set_http_response_header("content-length", None);
        if let Some(policy) = policy() {
            for (name, value) in self.get_http_response_headers_bytes() {
                let value = String::from_utf8_lossy(&value).into_owned();

                let data = self.data();
                if let Some(policy) = &data.policy {
                    match policy.token_extractor.as_deref() {
                        Some(TokenExtractionConfig {
                            location: TokenExtractionSite::Response,
                            header,
                            regex,
                        }) if &name == header => {
                            data.token = extract_token_regex(&*value, &regex.0);
                        }
                        _ => (),
                    }
                }

                let value = if policy.collected_response_headers.contains(&name) {
                    Some(value)
                } else {
                    None
                };

                data.response_headers.push(Header { name, value });
            }
        } else {
            warn!("processing response headers, but no policy loaded");
        }

        //todo: we might need to cover multiple content-type headers here
        let content_type = self.get_http_response_header_bytes("content-type");
        self.data.as_mut().unwrap().content_type =
            match content_type.map(|x| String::from_utf8_lossy(&x).into_owned()) {
                Some(value) => {
                    if let Some((init, _)) = value.split_once(';') {
                        init.trim()
                            .parse()
                            .expect("content-type parse failed (impossible)")
                    } else {
                        value
                            .trim()
                            .parse()
                            .expect("content-type parse failed (impossible)")
                    }
                }
                None => ContentType::Unknown,
            };
        let content_encoding = self.get_http_response_header_bytes("content-encoding");
        self.data.as_mut().unwrap().content_encoding =
            match content_encoding.map(|x| String::from_utf8_lossy(&x).into_owned()) {
                Some(value) => value
                    .trim()
                    .parse()
                    .expect("content-type parse failed (impossible)"),
                None => ContentEncoding::None,
            };
        self.content_encoding = self.data.as_ref().unwrap().content_encoding;
        Action::Continue
    }

    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        if let Some(data) = &mut self.data {
            if data.policy.is_none() {
                return Action::Continue;
            }

            if data.content_type == ContentType::Unknown || data.content_type == ContentType::Jpeg {
                return Action::Continue;
            }
            data.response_body_start = timestamp();
            let (reader, writer) = pipe(0);
            self.response_writer = Some(writer);
            self.response_read_task = Some(Box::pin(response_body_task(
                self.data.take().unwrap(),
                reader,
            )));
        }
        // cleared when we fail to do anything (i.e. no policy).
        // when there is more chunks of that original response, continue skipping here.
        if self.response_read_task.is_none() {
            return Action::Continue;
        }

        if body_size > 0 {
            let body = match self.process_data(body_size) {
                Err(e) => {
                    error!("failed to read body: {:?}", e);
                    return Action::Continue;
                }
                Ok(x) => x,
            };
            if !body.is_empty() {
                self.response_writer
                    .as_mut()
                    .expect("receives data after end_of_stream")
                    .append(body);
            }
        }
        if end_of_stream {
            if matches!(self.content_encoding, ContentEncoding::Gzip) {
                let body = match std::mem::replace(&mut self.decompressor, GzDecoder::new(vec![]))
                    .finish()
                {
                    Err(e) => {
                        error!("failed to read body: {:?}", e);
                        return Action::Continue;
                    }
                    Ok(x) => x,
                };
                if !body.is_empty() {
                    self.response_writer
                        .as_mut()
                        .expect("receives data after end_of_stream")
                        .append(body);
                }
            }
            self.response_writer.take().unwrap();
        } else if body_size == 0 {
            return Action::Continue;
        }

        let waker = waker(Arc::new(DummyWaker));
        let mut context = std::task::Context::from_waker(&waker);
        match self
            .response_read_task
            .as_mut()
            .unwrap()
            .poll_unpin(&mut context)
        {
            Poll::Ready(None) => {
                self.response_read_task.take();
                Action::Continue
            }
            Poll::Ready(Some(data)) => {
                self.response_read_task.take();
                if let Some(upstream) = data.upstream {
                    let emitted_packet = data.packet.encode_to_vec();

                    if let Err(e) = self.dispatch_grpc_call(
                        unsafe { std::str::from_utf8_unchecked(&upstream.service_definition[..]) },
                        LEAKSIGNAL_SERVICE_NAME,
                        "MatchData",
                        vec![],
                        Some(&emitted_packet[..]),
                        MATCH_PUSH_TIMEOUT,
                    ) {
                        error!("failed to upstream match information: {:?}", e);
                    }
                }
                match data.response {
                    ParseResponse::Block => {
                        warn!("blocking request for '{}'", data.packet.policy_path);
                        self.set_http_response_body(0, body_size, &[]);
                        Action::Continue
                    }
                    ParseResponse::Continue => Action::Continue,
                }
            }
            Poll::Pending => Action::Continue,
        }
    }
}

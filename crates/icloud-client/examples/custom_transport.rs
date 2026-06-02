use std::{env, error::Error, fmt, fs};

use icloud_client::{
    AsyncTransport, BlockingICloudClient, BlockingTransport, RestoredBlockingClient,
};
use icloud_protocol::{
    HeaderMap, HeaderName, HeaderValue, HeaderValueError, RequestSpec, ResponseParts, StatusCode,
    header,
};
use icloud_types::{ClientContext, ClientStateSnapshot, FindMyRefreshOptions, FindMySoundCommand};

#[derive(Debug, Clone)]
struct HostTransport<H> {
    host: H,
}

trait HostHttp {
    fn send(&self, request: HostRequest) -> Result<HostResponse, HostError>;
}

#[derive(Debug)]
struct HostRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct HostResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct HostError(String);

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Error for HostError {}

impl<H> BlockingTransport for HostTransport<H>
where
    H: HostHttp + fmt::Debug + Clone,
{
    type Error = HostError;

    fn execute(&self, request: RequestSpec) -> Result<ResponseParts, Self::Error> {
        execute_via_host(&self.host, request)
    }
}

impl<H> AsyncTransport for HostTransport<H>
where
    H: HostHttp + fmt::Debug + Clone,
{
    type Error = HostError;

    async fn execute(&self, request: RequestSpec) -> Result<ResponseParts, Self::Error> {
        execute_via_host(&self.host, request)
    }
}

fn execute_via_host<H>(host: &H, request: RequestSpec) -> Result<ResponseParts, HostError>
where
    H: HostHttp,
{
    let host_request = HostRequest {
        method: request.method.as_str().to_owned(),
        url: request.url.to_string(),
        headers: request
            .headers
            .iter()
            .map(|(name, value)| Ok((name.as_str().to_owned(), value.to_str()?.to_owned())))
            .collect::<Result<_, HeaderValueError>>()
            .map_err(|error| HostError(error.to_string()))?,
        body: request.body.as_bytes().unwrap_or_default().to_vec(),
    };

    let host_response = host.send(host_request)?;
    let mut headers = HeaderMap::new();
    for (name, value) in host_response.headers {
        let Some(name) = known_response_header(name.as_str()) else {
            continue;
        };
        let value =
            HeaderValue::parse(value.as_str()).map_err(|error| HostError(error.to_string()))?;
        headers.append(name, value);
    }

    Ok(ResponseParts::new(
        StatusCode::from_u16(host_response.status),
        headers,
        host_response.body,
    ))
}

fn known_response_header(name: &str) -> Option<HeaderName> {
    match name.to_ascii_lowercase().as_str() {
        "set-cookie" => Some(header::SET_COOKIE),
        "scnt" => Some(HeaderName::from_static("scnt")),
        "x-apple-id-session-id" => Some(HeaderName::from_static("x-apple-id-session-id")),
        "x-apple-session-token" => Some(HeaderName::from_static("x-apple-session-token")),
        "x-apple-id-account-country" => Some(HeaderName::from_static("x-apple-id-account-country")),
        "x-apple-twosv-trust-token" => Some(HeaderName::from_static("x-apple-twosv-trust-token")),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct ExampleHost;

impl HostHttp for ExampleHost {
    fn send(&self, request: HostRequest) -> Result<HostResponse, HostError> {
        // Replace this with the embedding environment's HTTP call. Preserve repeated
        // headers as repeated entries: multiple Set-Cookie values must be appended
        // separately so CookieJar can ingest every cookie.
        Err(HostError(format!(
            "host HTTP adapter is not wired: {} {} ({} headers, {} bytes)",
            request.method,
            request.url,
            request.headers.len(),
            request.body.len()
        )))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let widget_key = env::var("ICLOUD_WIDGET_KEY").unwrap_or_else(|_| {
        "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d".to_owned()
    });
    let snapshot_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "icloud-session.snapshot.json".to_owned());
    let snapshot: ClientStateSnapshot = serde_json::from_slice(&fs::read(snapshot_path)?)?;

    let context = ClientContext::production(widget_key)?;
    let transport = HostTransport { host: ExampleHost };
    let mut client = match BlockingICloudClient::restore(context, transport, snapshot) {
        RestoredBlockingClient::Authenticated(client) => client,
        _ => return Err("snapshot is not bootstrapped/authenticated".into()),
    };

    let devices = client.refresh_findmy_devices(&FindMyRefreshOptions::default())?;
    if let Some(device) = devices.devices.first() {
        client.play_findmy_sound(&FindMySoundCommand::new(device.id.as_str(), "Find My"))?;
    }

    Ok(())
}

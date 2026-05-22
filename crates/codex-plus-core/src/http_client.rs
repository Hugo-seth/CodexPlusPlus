use std::sync::OnceLock;
use std::time::Duration;

static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static LOOPBACK_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

const LOOPBACK_TIMEOUT: Duration = Duration::from_secs(3);

pub fn shared_client() -> &'static reqwest::Client {
    SHARED_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .build()
            .expect("default reqwest client should build")
    })
}

pub fn shared_loopback_client() -> &'static reqwest::Client {
    LOOPBACK_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(LOOPBACK_TIMEOUT)
            .build()
            .expect("loopback reqwest client should build")
    })
}

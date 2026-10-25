use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// The HTTP client every outbound request goes through, built on first use so connections to
/// the same host are reused. It speaks HTTPS only and gives up after 10 seconds.
pub fn client() -> Result<&'static reqwest::Client, reqwest::Error> {
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .https_only(true)
        .build()?;
    Ok(CLIENT.get_or_init(|| client))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_is_built_once_and_shared() {
        let first = client().unwrap();
        let second = client().unwrap();

        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn client_refuses_plain_http() {
        let err = tauri::async_runtime::block_on(
            client().unwrap().get("http://localhost/latest.json").send(),
        )
        .unwrap_err();

        assert!(err.is_builder(), "{err}");
    }
}

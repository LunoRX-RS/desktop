//! Reads `lunorx://share/{token}` links. Pure logic: no Tauri types, so it is tested without the app.

/// The token is 43 characters of URL-safe base64, the same shape the backend issues.
fn token_ok(token: &str) -> bool {
    token.len() == 43
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The share token from a `lunorx://share/{token}` link, or `None` for any other URL.
pub fn token_from_url(raw: &str) -> Option<String> {
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "lunorx" || url.host_str() != Some("share") {
        return None;
    }
    let mut segments = url.path_segments()?;
    let token = segments.next()?;
    (segments.next().is_none() && token_ok(token)).then(|| token.to_string())
}

#[cfg(test)]
mod tests {
    use super::token_from_url;

    const TOKEN: &str = "SIB1x3ILmH3J9aRSASqqls3ZR3BPfYhh2JC6OGu7UjU";

    #[test]
    fn reads_a_share_link() {
        assert_eq!(
            token_from_url(&format!("lunorx://share/{TOKEN}")).as_deref(),
            Some(TOKEN)
        );
    }

    #[test]
    fn rejects_other_schemes_hosts_and_paths() {
        assert_eq!(token_from_url(&format!("https://share/{TOKEN}")), None);
        assert_eq!(token_from_url(&format!("lunorx://playlist/{TOKEN}")), None);
        assert_eq!(
            token_from_url(&format!("lunorx://share/{TOKEN}/extra")),
            None
        );
        assert_eq!(token_from_url("lunorx://share/"), None);
    }

    #[test]
    fn rejects_tokens_of_the_wrong_shape() {
        assert_eq!(token_from_url("lunorx://share/short"), None);
        assert_eq!(
            token_from_url("lunorx://share/SIB1x3ILmH3J9aRSASqqls3ZR3BPfYhh2JC6OGu7Uj%2F"),
            None
        );
        assert_eq!(token_from_url("not a url"), None);
    }
}

//! Where an agent's bridge may be, and what an agent connects to (spec 103
//! D1): every shell checks the same.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlProblem {
    Invalid,
    /// `ws:` to another machine: the token would cross the network in the clear.
    Insecure,
}

/// Why a bridge URL cannot be used, or `None`. Encrypted (`wss:`) everywhere;
/// plain `ws:` only to this machine — the exception the service endpoints make
/// for a developer's own server.
pub fn bridge_url_problem(url: &str) -> Option<UrlProblem> {
    let Ok(parsed) = url::Url::parse(url.trim()) else {
        return Some(UrlProblem::Invalid);
    };
    match parsed.scheme() {
        "wss" if parsed.host().is_some() => None,
        "ws" if matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")) => None,
        "ws" => Some(UrlProblem::Insecure),
        _ => Some(UrlProblem::Invalid),
    }
}

/// The MCP endpoint an agent connects to, derived from the bridge's client
/// endpoint (`wss://host/connect` → `https://host/mcp`), or `None`.
pub fn mcp_endpoint(url: &str) -> Option<String> {
    if bridge_url_problem(url).is_some() {
        return None;
    }
    let mut parsed = url::Url::parse(url.trim()).ok()?;
    let secure = parsed.scheme() == "wss";
    parsed
        .set_scheme(if secure { "https" } else { "http" })
        .ok()?;
    let path = parsed.path().trim_end_matches('/');
    let base = path.strip_suffix("/connect").unwrap_or(path).to_owned();
    parsed.set_path(&format!("{base}/mcp"));
    parsed.set_query(None);
    Some(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bridge_is_encrypted_unless_it_is_this_machine() {
        assert_eq!(
            bridge_url_problem("wss://bridge.clientmcp.dev/connect"),
            None
        );
        assert_eq!(bridge_url_problem("ws://127.0.0.1:8080/connect"), None);
        assert_eq!(bridge_url_problem("ws://localhost:8080/connect"), None);
        assert_eq!(
            bridge_url_problem("ws://192.168.1.4:8080/connect"),
            Some(UrlProblem::Insecure)
        );
        assert_eq!(
            bridge_url_problem("https://bridge.clientmcp.dev"),
            Some(UrlProblem::Invalid)
        );
        assert_eq!(bridge_url_problem("not a url"), Some(UrlProblem::Invalid));
    }

    #[test]
    fn the_agents_endpoint_is_derived_from_the_bridges() {
        assert_eq!(
            mcp_endpoint("wss://bridge.clientmcp.dev/connect").as_deref(),
            Some("https://bridge.clientmcp.dev/mcp")
        );
        assert_eq!(
            mcp_endpoint("ws://127.0.0.1:8080/connect?x=1").as_deref(),
            Some("http://127.0.0.1:8080/mcp")
        );
        assert_eq!(
            mcp_endpoint("wss://example.com/").as_deref(),
            Some("https://example.com/mcp")
        );
        assert_eq!(mcp_endpoint("ws://10.0.0.1/connect"), None);
    }
}

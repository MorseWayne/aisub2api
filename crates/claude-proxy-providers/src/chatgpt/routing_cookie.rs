//! Per-client storage for the Codex backend's infrastructure routing cookie.

use reqwest::Url;
use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{COOKIE, HeaderMap, HeaderValue, SET_COOKIE};

#[derive(Default)]
pub(super) struct RoutingCookieJar {
    jar: Jar,
}

impl RoutingCookieJar {
    pub(super) fn websocket_request_headers(&self, url: &Url, headers: &mut HeaderMap) {
        if let Some(url) = websocket_cookie_url(url)
            && let Some(cookie) = self.cookies(&url)
        {
            // Explicit provider cookies take precedence over the infrastructure jar.
            headers.entry(COOKIE).or_insert(cookie);
        }
    }

    pub(super) fn store_websocket_response(&self, url: &Url, headers: &HeaderMap) {
        if let Some(url) = websocket_cookie_url(url) {
            self.set_cookies(&mut headers.get_all(SET_COOKIE).iter(), &url);
        }
    }
}

fn websocket_cookie_url(url: &Url) -> Option<Url> {
    if url.scheme() != "wss" {
        return None;
    }
    let mut url = url.clone();
    url.set_scheme("https").ok()?;
    Some(url)
}

fn allowed_url(url: &Url) -> bool {
    url.scheme() == "https" && matches!(url.host_str(), Some("chatgpt.com" | "chat.openai.com"))
}

impl CookieStore for RoutingCookieJar {
    fn set_cookies(&self, headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        if allowed_url(url) {
            let mut routing = headers.filter(|header| {
                header
                    .to_str()
                    .ok()
                    .and_then(|value| value.split_once('='))
                    .is_some_and(|(name, _)| name.trim() == "__oailb")
            });
            // Jar enforces domain, path, Secure and expiration semantics.
            self.jar.set_cookies(&mut routing, url);
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        if !allowed_url(url) {
            return None;
        }
        self.jar.cookies(url).map(|mut header| {
            header.set_sensitive(true);
            header
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_and_http_share_only_scoped_routing_cookies() {
        let jar = RoutingCookieJar::default();
        let http = Url::parse("https://chatgpt.com/backend-api/codex/models").unwrap();
        let websocket = Url::parse("wss://chatgpt.com/backend-api/codex/responses").unwrap();
        let cookie = HeaderValue::from_static("__oailb=http; Path=/backend-api; Secure");
        jar.set_cookies(&mut std::iter::once(&cookie), &http);
        let mut request = HeaderMap::new();
        jar.websocket_request_headers(&websocket, &mut request);
        assert_eq!(request[COOKIE], "__oailb=http");
        assert!(request[COOKIE].is_sensitive());
        let mut response = HeaderMap::new();
        response.append(
            SET_COOKIE,
            HeaderValue::from_static("__oailb=websocket; Path=/backend-api; Secure"),
        );
        response.append(
            SET_COOKIE,
            HeaderValue::from_static("session=secret; Path=/"),
        );
        jar.store_websocket_response(&websocket, &response);
        assert_eq!(jar.cookies(&http).unwrap(), "__oailb=websocket");
        request.insert(COOKIE, HeaderValue::from_static("explicit=provider"));
        jar.websocket_request_headers(&websocket, &mut request);
        assert_eq!(request[COOKIE], "explicit=provider");
        for url in [
            "ws://chatgpt.com/backend-api/codex/responses",
            "wss://attacker.example/",
            "wss://chatgpt.com/other",
        ] {
            let url = Url::parse(url).unwrap();
            let mut request = HeaderMap::new();
            jar.websocket_request_headers(&url, &mut request);
            assert!(!request.contains_key(COOKIE));
        }
        response.insert(
            SET_COOKIE,
            HeaderValue::from_static("__oailb=; Max-Age=0; Path=/backend-api; Secure"),
        );
        jar.store_websocket_response(&websocket, &response);
        assert!(jar.cookies(&http).is_none());
    }

    #[test]
    fn routing_cookie_respects_scope_expiration_and_client_isolation() {
        let jar = RoutingCookieJar::default();
        let url = Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let cookies = [
            HeaderValue::from_static(
                "__oailb=route; Path=/backend-api; Max-Age=3600; Secure; HttpOnly",
            ),
            HeaderValue::from_static("session=secret; Path=/"),
            HeaderValue::from_static("__oailb_other=secret; Path=/"),
        ];
        jar.set_cookies(&mut cookies.iter(), &url);
        let followup = Url::parse("https://chatgpt.com/backend-api/codex/models").unwrap();
        assert_eq!(jar.cookies(&followup).unwrap(), "__oailb=route");
        assert!(jar.cookies(&followup).unwrap().is_sensitive());
        assert!(RoutingCookieJar::default().cookies(&followup).is_none());
        for outside in [
            "http://chatgpt.com/backend-api/codex/responses",
            "https://chatgpt.com/",
            "https://other.chatgpt.com/backend-api/codex/responses",
            "https://chat.openai.com/backend-api/codex/responses",
            "https://api.openai.com/backend-api/codex/responses",
            "https://chatgpt.com.attacker.example/backend-api/codex/responses",
        ] {
            assert!(
                jar.cookies(&Url::parse(outside).unwrap()).is_none(),
                "{outside}"
            );
        }
        let expired = HeaderValue::from_static("__oailb=; Path=/backend-api; Max-Age=0; Secure");
        jar.set_cookies(&mut std::iter::once(&expired), &url);
        assert!(jar.cookies(&url).is_none());
    }

    #[test]
    fn routing_cookie_rejects_untrusted_sources_and_invalid_domains() {
        let jar = RoutingCookieJar::default();
        let cookie = HeaderValue::from_static("__oailb=route; Domain=chatgpt.com; Path=/");
        for source in [
            "http://chatgpt.com/",
            "https://other.chatgpt.com/",
            "https://attacker.example/",
        ] {
            jar.set_cookies(&mut std::iter::once(&cookie), &Url::parse(source).unwrap());
        }
        let url = Url::parse("https://chatgpt.com/").unwrap();
        assert!(jar.cookies(&url).is_none());
        let invalid_domain = HeaderValue::from_static("__oailb=route; Domain=example.com; Path=/");
        jar.set_cookies(&mut std::iter::once(&invalid_domain), &url);
        assert!(jar.cookies(&url).is_none());
    }
}

//! "Sign in with your browser": Peak opens Microsoft's sign-in page and
//! catches the result on a short-lived listener on this computer's loopback
//! address (the way desktop apps are meant to do it, RFC 8252). A one-time
//! PKCE secret and a random `state` value protect the exchange, and nothing
//! outside this computer can reach the listener.

use crate::error::{Error, Result};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use chacha20poly1305::aead::OsRng;
use chacha20poly1305::aead::rand_core::RngCore;
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const AUTHORIZE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";

fn random_token(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    OsRng.fill_bytes(&mut b);
    B64URL.encode(b)
}

/// The PKCE challenge Microsoft checks the verifier against (method S256).
pub fn challenge_for(verifier: &str) -> String {
    B64URL.encode(Sha256::digest(verifier.as_bytes()))
}

/// A sign-in waiting for the browser to come back.
pub struct Pending {
    listener: TcpListener,
    state: String,
    /// Sent when swapping the code for tokens; proves this is the same app that began the sign-in.
    pub verifier: String,
    /// Where Microsoft sends the browser. Must be what the Azure app allows (`http://localhost`, any port).
    pub redirect_uri: String,
}

impl Pending {
    /// Start listening on a free port on this computer only.
    pub async fn new() -> Result<Pending> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| Error::msg(format!("Couldn't open a local port for browser sign-in: {e}")))?;
        let port = listener.local_addr()?.port();
        Ok(Pending {
            listener,
            state: random_token(24),
            // 48 random bytes make a 64-character verifier (the spec allows 43 to 128).
            verifier: random_token(48),
            redirect_uri: format!("http://localhost:{port}"),
        })
    }

    /// The Microsoft page to open in the browser.
    pub fn authorize_url(&self, client_id: &str, scope: &str) -> String {
        let mut url = reqwest::Url::parse(AUTHORIZE).expect("valid base URL");
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("response_mode", "query")
            .append_pair("scope", scope)
            .append_pair("code_challenge", &challenge_for(&self.verifier))
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &self.state)
            .append_pair("prompt", "select_account");
        url.to_string()
    }

    /// Wait for the browser to come back with the result. `cancelled` is
    /// checked several times a second so Cancel takes effect straight away.
    pub async fn wait_for_code(&self, cancelled: impl Fn() -> bool) -> Result<String> {
        loop {
            if cancelled() {
                return Err(Error::msg("Sign-in cancelled."));
            }
            let accepted = tokio::time::timeout(Duration::from_millis(250), self.listener.accept()).await;
            let Ok(Ok((stream, _))) = accepted else { continue };
            if let Some(outcome) = self.serve(stream).await {
                return outcome;
            }
        }
    }

    /// Answer one browser request. Returns the result if this was the real callback.
    async fn serve(&self, mut stream: TcpStream) -> Option<Result<String>> {
        let mut buf = vec![0u8; 8192];
        // Browsers sometimes open a connection and send nothing; don't wait on those.
        let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf)).await.ok()?.ok()?;
        let head = String::from_utf8_lossy(&buf[..n]);
        let reply = interpret(head.lines().next().unwrap_or(""), &self.state);
        let response = format!(
            "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
            reply.status,
            reply.html.len(),
            reply.html
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
        reply.outcome
    }
}

struct Reply {
    status: &'static str,
    html: String,
    outcome: Option<Result<String>>,
}

fn page(title: &str, message: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{title}</title>\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><style>\
         body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#0b0c0e;color:#e7e9ee;\
         font:16px/1.5 system-ui,Segoe UI,sans-serif}}main{{max-width:420px;padding:32px;text-align:center}}\
         h1{{font-size:22px;margin:0 0 8px}}p{{margin:0;color:#9aa1b2}}</style></head>\
         <body><main><h1>{title}</h1><p>{message}</p></main></body></html>"
    )
}

/// Decide what to say to one request and whether it carries the sign-in result.
fn interpret(request_line: &str, expected_state: &str) -> Reply {
    let not_found = || Reply { status: "404 Not Found", html: page("Not found", "Nothing here."), outcome: None };
    let bad = || Reply {
        status: "400 Bad Request",
        html: page("That didn't work", "This page doesn't match a sign-in that Peak started. Go back to Peak and try again."),
        outcome: None,
    };

    let mut parts = request_line.split_whitespace();
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else { return bad() };
    let Ok(url) = reqwest::Url::parse(&format!("http://localhost{target}")) else { return bad() };
    if url.path() != "/" {
        return not_found(); // favicon and the like
    }
    let get = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    // Anything that doesn't carry our `state` is ignored, so a stray page or program can't end the sign-in.
    if get("state").as_deref() != Some(expected_state) {
        return bad();
    }
    if let Some(error) = get("error") {
        let detail = get("error_description").unwrap_or_else(|| error.clone());
        let message = if error == "access_denied" {
            "Sign-in was cancelled.".to_string()
        } else {
            format!("Microsoft sign-in failed: {}", detail.lines().next().unwrap_or(&error))
        };
        return Reply {
            status: "200 OK",
            html: page("Sign-in didn't finish", "You can close this tab and try again from Peak."),
            outcome: Some(Err(Error::msg(message))),
        };
    }
    match get("code") {
        Some(code) if !code.is_empty() => Reply {
            status: "200 OK",
            html: page("You're signed in", "You can close this tab and go back to Peak."),
            outcome: Some(Ok(code)),
        },
        _ => bad(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(reply: &Reply) -> Option<&str> {
        match &reply.outcome {
            Some(Ok(c)) => Some(c.as_str()),
            _ => None,
        }
    }

    #[test]
    fn pkce_challenge_matches_the_rfc_example() {
        // RFC 7636, appendix B.
        assert_eq!(challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn only_the_real_callback_counts() {
        let reply = interpret("GET /?code=abc123&state=S3 HTTP/1.1", "S3");
        assert_eq!(ok(&reply), Some("abc123"));
        assert!(reply.html.contains("signed in"));
        // Wrong or missing state, other paths, odd methods: ignored.
        assert!(interpret("GET /?code=abc123&state=nope HTTP/1.1", "S3").outcome.is_none());
        assert!(interpret("GET /?code=abc123 HTTP/1.1", "S3").outcome.is_none());
        assert!(interpret("GET /favicon.ico HTTP/1.1", "S3").outcome.is_none());
        assert!(interpret("POST /?code=abc123&state=S3 HTTP/1.1", "S3").outcome.is_none());
        assert!(interpret("garbage", "S3").outcome.is_none());
        assert!(interpret("GET /?state=S3 HTTP/1.1", "S3").outcome.is_none(), "no code, no result");
    }

    #[test]
    fn reports_cancelled_and_failed_sign_ins() {
        let r = interpret("GET /?error=access_denied&error_description=x&state=S3 HTTP/1.1", "S3");
        assert_eq!(r.outcome.unwrap().unwrap_err().to_string(), "Sign-in was cancelled.");
        let r = interpret("GET /?error=server_error&error_description=Something%20broke%0D%0ATrace&state=S3 HTTP/1.1", "S3");
        assert_eq!(r.outcome.unwrap().unwrap_err().to_string(), "Microsoft sign-in failed: Something broke");
        // An error without our state is ignored too.
        assert!(interpret("GET /?error=access_denied&state=other HTTP/1.1", "S3").outcome.is_none());
    }

    #[tokio::test]
    async fn builds_the_authorize_url() {
        let p = Pending::new().await.unwrap();
        assert!(p.redirect_uri.starts_with("http://localhost:"));
        assert!(p.verifier.len() >= 43 && p.verifier.len() <= 128);
        let url = reqwest::Url::parse(&p.authorize_url("client-1", "XboxLive.signin offline_access")).unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("login.microsoftonline.com"));
        assert_eq!(q["client_id"], "client-1");
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["redirect_uri"], p.redirect_uri);
        assert_eq!(q["scope"], "XboxLive.signin offline_access");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], challenge_for(&p.verifier));
        assert_eq!(q["state"], p.state);
        assert!(!url.as_str().contains(&p.verifier), "the verifier itself is never sent in the URL");
    }

    /// Talk to the listener over a real connection, like a browser would.
    async fn browse(port: u16, target: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn catches_the_code_over_a_real_connection() {
        let p = std::sync::Arc::new(Pending::new().await.unwrap());
        let port: u16 = p.redirect_uri.rsplit(':').next().unwrap().parse().unwrap();
        let state = p.state.clone();
        let waiting = tokio::spawn({
            let p = p.clone();
            async move { p.wait_for_code(|| false).await }
        });
        // Noise first: a favicon request and a callback with the wrong state.
        assert!(browse(port, "/favicon.ico").await.starts_with("HTTP/1.1 404"));
        assert!(browse(port, "/?code=evil&state=wrong").await.starts_with("HTTP/1.1 400"));
        // Then the real one.
        let page = browse(port, &format!("/?code=the-code&state={state}")).await;
        assert!(page.starts_with("HTTP/1.1 200") && page.contains("signed in"));
        assert_eq!(waiting.await.unwrap().unwrap(), "the-code");
    }

    #[tokio::test]
    async fn cancel_stops_waiting() {
        let p = Pending::new().await.unwrap();
        let flag = std::sync::atomic::AtomicBool::new(false);
        let waiting = p.wait_for_code(|| flag.load(std::sync::atomic::Ordering::SeqCst));
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(400)).await;
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        };
        let (result, _) = tokio::join!(waiting, stopper);
        assert_eq!(result.unwrap_err().to_string(), "Sign-in cancelled.");
    }
}

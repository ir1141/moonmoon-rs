use axum::body::Body;
use axum::http::{HeaderValue, Request, header};
use axum::middleware::Next;
use axum::response::Response;
use rand::Rng;
use rand::distr::Alphanumeric;

#[derive(Clone)]
pub struct CspNonce(pub String);

const CSP_TEMPLATE: &str = "default-src 'self'; \
img-src 'self' https://static-cdn.jtvnw.net https://i.ytimg.com https://cdn.7tv.app https://cdn.betterttv.net https://cdn.frankerfacez.com; \
style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
font-src 'self' https://fonts.gstatic.com; \
script-src 'self' 'nonce-{NONCE}' https://unpkg.com https://www.youtube.com; \
connect-src 'self' https://7tv.io https://api.betterttv.net https://api.frankerfacez.com; \
frame-src https://www.youtube.com https://player.twitch.tv; \
frame-ancestors 'none'";

pub async fn csp_nonce(mut req: Request<Body>, next: Next) -> Response {
    let nonce: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(22)
        .map(char::from)
        .collect();

    req.extensions_mut().insert(CspNonce(nonce.clone()));

    let mut response = next.run(req).await;

    let headers = response.headers_mut();
    headers.remove(header::CONTENT_SECURITY_POLICY);
    let csp = CSP_TEMPLATE.replace("{NONCE}", &nonce);
    if let Ok(value) = HeaderValue::from_str(&csp) {
        headers.insert(header::CONTENT_SECURITY_POLICY, value);
    }
    response
}

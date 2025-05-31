use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn verify_slack_request(
    slack_signing_secret: &str,
    request_body: &str,
    timestamp: &str,
    signature: &str,
) -> bool {
    if let Ok(ts) = timestamp.parse::<i64>() {
        let now = chrono::Utc::now().timestamp();
        if (now - ts).abs() > 300 {
            return false;
        }
    } else {
        return false;
    }

    let sig_basestring = format!("v0:{}:{}", timestamp, request_body);

    let mut mac = match HmacSha256::new_from_slice(slack_signing_secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => return false,
    };

    mac.update(sig_basestring.as_bytes());
    let result = mac.finalize();
    let expected_signature = format!("v0={}", hex::encode(result.into_bytes()));

    expected_signature == signature
}

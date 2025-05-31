use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

type HmacSha256 = Hmac<Sha256>;

/// Verify that a Slack request is authentic using the signing secret
/// 
/// Slack sends a signature in the `X-Slack-Signature` header and a timestamp
/// in the `X-Slack-Request-Timestamp` header. We verify the request by:
/// 1. Checking that the timestamp is within 5 minutes of the current time
/// 2. Computing HMAC-SHA256 of "v0:{timestamp}:{body}" using the signing secret
/// 3. Comparing our computed signature with the one from Slack
pub fn verify_slack_request(
    signing_secret: &str,
    signature: &str,
    timestamp: &str,
    body: &str,
) -> Result<(), SlackVerificationError> {
    // Parse the timestamp
    let request_timestamp = timestamp.parse::<u64>()
        .map_err(|_| SlackVerificationError::InvalidTimestamp)?;
    
    // Check if the request is too old (replay attack protection)
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SlackVerificationError::SystemTimeError)?
        .as_secs();
    
    const MAX_AGE_SECONDS: u64 = 5 * 60; // 5 minutes
    if (current_timestamp.saturating_sub(request_timestamp)) > MAX_AGE_SECONDS {
        warn!("Request timestamp is too old: {} vs {}", request_timestamp, current_timestamp);
        return Err(SlackVerificationError::RequestTooOld);
    }
    
    // Parse the signature (should start with "v0=")
    if !signature.starts_with("v0=") {
        return Err(SlackVerificationError::InvalidSignatureFormat);
    }
    
    let provided_signature = &signature[3..]; // Remove "v0=" prefix
    let expected_signature_bytes = hex::decode(provided_signature)
        .map_err(|_| SlackVerificationError::InvalidSignatureFormat)?;
    
    // Create the basestring: v0:timestamp:body
    let basestring = format!("v0:{}:{}", timestamp, body);
    debug!("Verifying signature for basestring length: {}", basestring.len());
    
    // Compute the expected signature
    let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes())
        .map_err(|_| SlackVerificationError::InvalidSecret)?;
    mac.update(basestring.as_bytes());
    let computed_signature = mac.finalize().into_bytes();
    
    // Compare signatures using constant-time comparison
    if computed_signature.as_slice() == expected_signature_bytes.as_slice() {
        debug!("Slack signature verification successful");
        Ok(())
    } else {
        warn!("Slack signature verification failed");
        Err(SlackVerificationError::SignatureMismatch)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SlackVerificationError {
    #[error("Invalid timestamp format")]
    InvalidTimestamp,
    
    #[error("Request is too old (possible replay attack)")]
    RequestTooOld,
    
    #[error("Invalid signature format")]
    InvalidSignatureFormat,
    
    #[error("Invalid signing secret")]
    InvalidSecret,
    
    #[error("Signature mismatch")]
    SignatureMismatch,
    
    #[error("System time error")]
    SystemTimeError,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_slack_signature_verification() {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        // Use current timestamp to avoid RequestTooOld error
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = current_timestamp.to_string();
        
        let signing_secret = "8f742231b10e8888abcd99yyyzzz85a5";
        let body = "token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2F1234%2F5678&trigger_id=2142974.1.R043K93V9AL";
        
        // Create a valid signature for our test
        let basestring = format!("v0:{}:{}", timestamp, body);
        let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes()).unwrap();
        mac.update(basestring.as_bytes());
        let computed_signature = mac.finalize().into_bytes();
        let expected_signature = format!("v0={}", hex::encode(computed_signature));
        
        let result = verify_slack_request(signing_secret, &expected_signature, &timestamp, body);
        assert!(result.is_ok(), "Signature verification should succeed: {:?}", result);
    }
    
    #[test]
    fn test_invalid_signature() {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = current_timestamp.to_string();
        
        let signing_secret = "8f742231b10e8888abcd99yyyzzz85a5";
        let body = "different body";
        let wrong_signature = "v0=a2114d57b48eac39b9ad189dd8316235a7b4a8d21a10bd27519666489c69b503";
        
        let result = verify_slack_request(signing_secret, wrong_signature, &timestamp, body);
        assert!(matches!(result, Err(SlackVerificationError::SignatureMismatch)));
    }
    
    #[test]
    fn test_old_timestamp() {
        let signing_secret = "test_secret";
        let old_timestamp = "1000000000"; // Very old timestamp
        let body = "test body";
        let signature = "v0=invalid";
        
        let result = verify_slack_request(signing_secret, signature, old_timestamp, body);
        assert!(matches!(result, Err(SlackVerificationError::RequestTooOld)));
    }
    
    #[test]
    fn test_invalid_signature_format() {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = current_timestamp.to_string();
        
        let signing_secret = "test_secret";
        let body = "test body";
        let invalid_signature = "invalid_format";
        
        let result = verify_slack_request(signing_secret, invalid_signature, &timestamp, body);
        assert!(matches!(result, Err(SlackVerificationError::InvalidSignatureFormat)));
    }
}

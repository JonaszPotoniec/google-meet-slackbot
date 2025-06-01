use aes_gcm::{aead::OsRng, Aes256Gcm, KeyInit};
use base64::{engine::general_purpose, Engine as _};

fn main() {
    let key = Aes256Gcm::generate_key(OsRng);
    let key_b64 = general_purpose::STANDARD.encode(key);

    println!("Generated TOKEN_ENCRYPTION_KEY:");
    println!("{}", key_b64);
    println!();
    println!("This is a 32-byte AES-256 key encoded in base64.");
    println!("Keep this secret and secure!");
}

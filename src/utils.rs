#[cfg(feature = "encryption_layer")]
/// Generates a secret that can be passed to the options of the encryption layer and for creating
/// a public key
pub fn generate_secret() -> x25519_dalek::StaticSecret {
    let mut rng = rand::thread_rng();
    use rand_core::RngCore;
    let mut secret = [0u8; 32];
    rng.fill_bytes(&mut secret);

    x25519_dalek::StaticSecret::from(secret)
}

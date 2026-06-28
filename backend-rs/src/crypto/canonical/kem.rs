//src/crypto/canonical/kem.rs

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use fips203::ml_kem_768;
use fips203::traits::{Decaps, Encaps, KeyGen, SerDes};

#[derive(Debug, Clone)]
pub struct MlKemKeypair {
    pub pk_b64: String,
    pub sk_b64: String,
}

/// Generate ML-KEM-768 keypair (base64, URL-safe, no padding)
pub fn mlkem_generate_keypair_b64() -> MlKemKeypair {
    let (ek, dk) = ml_kem_768::KG::try_keygen()
        .expect("ML-KEM key generation should succeed with the default RNG");

    MlKemKeypair {
        pk_b64: URL_SAFE_NO_PAD.encode(ek.into_bytes()),
        sk_b64: URL_SAFE_NO_PAD.encode(dk.into_bytes()),
    }
}

/// Encapsulate to recipient public key (base64)
pub fn mlkem_encapsulate_b64(recipient_pk_b64: &str) -> Result<(String, Vec<u8>), String> {
    let pk_bytes = URL_SAFE_NO_PAD
        .decode(recipient_pk_b64)
        .map_err(|e| format!("pk decode failed: {e}"))?;

    let pk_array: [u8; ml_kem_768::EK_LEN] = pk_bytes
        .try_into()
        .map_err(|_| "invalid mlkem public key length".to_string())?;

    let ek = ml_kem_768::EncapsKey::try_from_bytes(pk_array)
        .map_err(|_| "invalid mlkem public key bytes".to_string())?;

    let (ss, ct) = ek
        .try_encaps()
        .map_err(|_| "mlkem encapsulate failed".to_string())?;

    Ok((
        URL_SAFE_NO_PAD.encode(ct.into_bytes()),
        ss.into_bytes().to_vec(),
    ))
}

/// Decapsulate using owner secret key (base64)
pub fn mlkem_decapsulate_b64(owner_sk_b64: &str, ct_b64: &str) -> Result<Vec<u8>, String> {
    let sk_bytes = URL_SAFE_NO_PAD
        .decode(owner_sk_b64)
        .map_err(|e| format!("sk decode failed: {e}"))?;

    let ct_bytes = URL_SAFE_NO_PAD
        .decode(ct_b64)
        .map_err(|e| format!("ct decode failed: {e}"))?;

    let sk_array: [u8; ml_kem_768::DK_LEN] = sk_bytes
        .try_into()
        .map_err(|_| "invalid mlkem secret key length".to_string())?;

    let ct_array: [u8; ml_kem_768::CT_LEN] = ct_bytes.try_into().map_err(|_| {
        format!(
            "decapsulate: ciphertext len mismatch (expected {})",
            ml_kem_768::CT_LEN
        )
    })?;

    let dk = ml_kem_768::DecapsKey::try_from_bytes(sk_array)
        .map_err(|_| "invalid mlkem secret key bytes".to_string())?;

    let ct = ml_kem_768::CipherText::try_from_bytes(ct_array)
        .map_err(|_| "invalid mlkem ciphertext bytes".to_string())?;

    let ss = dk
        .try_decaps(&ct)
        .map_err(|_| "mlkem decapsulate failed".to_string())?;

    Ok(ss.into_bytes().to_vec())
}

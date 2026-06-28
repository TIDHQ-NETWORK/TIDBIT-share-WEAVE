// src/pqc/kyber.rs

use fips203::ml_kem_1024;
use fips203::traits::{KeyGen, SerDes};

#[derive(Debug, Clone)]
pub struct KyberKeypair {
    pub public_key: Vec<u8>,
    pub secret_key: Vec<u8>,
}

pub fn generate_keypair() -> KyberKeypair {
    let (ek, dk) = ml_kem_1024::KG::try_keygen()
        .expect("ML-KEM key generation should succeed with the default RNG");
    KyberKeypair {
        public_key: ek.into_bytes().to_vec(),
        secret_key: dk.into_bytes().to_vec(),
    }
}

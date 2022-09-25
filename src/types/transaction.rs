use serde::{Serialize, Deserialize};
use ring::signature::{Ed25519KeyPair, Signature, KeyPair, VerificationAlgorithm};
use rand::Rng;
use crate::types::address::{self, Address};
use ring::error::Unspecified;
use crate::types::key_pair::random;
use crate::types::key_pair;
use crate::types::hash::{self, Hashable, H256};

#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct UTXOInput {
    pub tx_hash: H256,
    pub index: u8,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct UTXOOutput {
    pub recipient_address: Address,
    pub value: u32,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Transaction {
    pub input: Vec<UTXOInput>,
    pub output: Vec<UTXOOutput>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SignedTransaction {
    pub transaction: Transaction,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl Hashable for Transaction {
    fn hash(&self) -> H256 {
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        let msg = &encoded[..];
        ring::digest::digest(&ring::digest::SHA256, msg).into()
    }
}

impl Hashable for SignedTransaction {
    fn hash(&self) -> H256 {
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        let msg = &encoded[..];
        ring::digest::digest(&ring::digest::SHA256, msg).into()
    }
}

/// Create digital signature of a transaction
pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Signature {
    // serialize transaction struct to byte slice
    let encoded: Vec<u8> = bincode::serialize(&t).unwrap();
    let msg = &encoded[..];
    // use key to sign the message
    key.sign(msg)
}

/// Verify digital signature of a transaction, using public key instead of secret key
pub fn verify(t: &Transaction, public_key: &[u8], signature: &[u8]) -> bool {
    // serialize transaction struct to byte slice
    let encoded: Vec<u8> = bincode::serialize(&t).unwrap();
    let msg = &encoded[..];
    // parse public key bytes to public key
    let key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key);
    // verify the message
    match key.verify(msg, signature) {
        Ok(_) => true,
        Err(_) => false,
    }
}

// #[cfg(any(test, test_utilities))]
pub fn generate_random_transaction() -> Transaction {
    let input = vec![UTXOInput { tx_hash: hash::generate_random_hash(), index: 0 }];
    let output = vec![UTXOOutput { recipient_address: address::generate_random_address(), value: 0 }];
    Transaction { input, output }
}

pub fn generate_random_signed_transaction() -> SignedTransaction {
    let transaction = generate_random_transaction();
    let key = key_pair::random();
    let signature = sign(&transaction, &key);
    SignedTransaction {
        transaction,
        signature: signature.as_ref().to_vec(),
        public_key: key.public_key().as_ref().to_vec(),
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::key_pair;
    use ring::signature::KeyPair;


    #[test]
    fn sign_verify() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        assert!(verify(&t, key.public_key().as_ref(), signature.as_ref()));
    }

    #[test]
    fn sign_verify_two() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        let key_2 = key_pair::random();
        let t_2 = generate_random_transaction();
        assert!(!verify(&t_2, key.public_key().as_ref(), signature.as_ref()));
        assert!(!verify(&t, key_2.public_key().as_ref(), signature.as_ref()));
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
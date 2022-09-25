use super::hash::{Hashable, H256};
use ring::digest::{digest, SHA256};
use futures::StreamExt;

/// A Merkle tree.
#[derive(Debug, Default)]
pub struct MerkleTree {
    hashes: Vec<H256>,
    // start from leaves, go to upper level util reach root
    root: H256,
    height: usize,
    count: usize, // count of leaves
}

impl MerkleTree {
    pub fn new<T>(data: &[T]) -> Self where T: Hashable, {
        // empty data, return an empty merkle tree
        if data.is_empty() {
            return MerkleTree {
                hashes: vec![],
                root: Default::default(),
                height: 0,
                count: 0,
            };
        }
        let mut count = data.len();
        let mut height = 0;
        let mut queue: Vec<H256> = Vec::new(); // data vector
        let mut hashes: Vec<H256> = Vec::new();

        for v in data {
            queue.push(v.hash());
        }
        // odd number of data, duplicate one at the end
        if count % 2 != 0 {
            count += 1;
            queue.push(data[data.len() - 1].hash());
        }
        // create tree level by level from the bottom
        while !queue.is_empty() {
            let mut size = queue.len();
            height += 1;
            // size == 1, reach the root, end loop
            if size == 1 {
                hashes.push(queue.remove(0));
                break;
            }
            // odd size, duplicate data at the end of queue
            if size % 2 != 0 {
                let last = queue.remove(queue.len() - 1);
                queue.push(last);
                queue.push(last);
                size += 1;
            }
            // pop first two data out of queue, concat their slice together, generate a new H256, push into queue
            for _ in 0..size / 2 {
                // pop first two data and add to hashes
                let left = queue.remove(0);
                let right = queue.remove(0);
                hashes.push(left);
                hashes.push(right);
                // get the slices and concat together
                let left_slice = <[u8; 32]>::from(left);
                let right_slice = <[u8; 32]>::from(right);
                let concat = [&left_slice[..], &right_slice[..]].concat();
                // get parent node and push into queue
                let digest = digest(&SHA256, &concat);
                let h256 = H256::from(digest);
                queue.push(h256);
            }
        }
        // root node is at the end of hashes
        let root = hashes[hashes.len() - 1];
        MerkleTree {
            hashes,
            root,
            height,
            count,
        }
    }

    pub fn root(&self) -> H256 {
        self.root
    }

    /// Returns the Merkle Proof of data at index i
    pub fn proof(&self, index: usize) -> Vec<H256> {
        let mut proof: Vec<H256> = Vec::new();
        let mut i = index; // index in hashes vector
        let mut level_len = self.count;
        let mut start = 0;
        let mut div = 2;
        // loop from leaves to next level of root
        for _ in 0..self.height - 1 {
            // add neighbor hash to proof
            if i % 2 == 0 {
                proof.push(self.hashes[i + 1]);
            } else {
                proof.push(self.hashes[i - 1]);
            }
            // start at next level
            start = start + level_len;
            i = start + index / div;
            level_len /= 2;
            // if next level has odd length, fill up one
            if level_len % 2 != 0 && level_len != 1 { level_len += 1; }
            div *= 2;
        }
        proof
    }
}

/// Verify that the datum hash with a vector of proofs will produce the Merkle root. Also need the
/// index of datum and `leaf_size`, the total number of leaves.
pub fn verify(root: &H256, datum: &H256, proof: &[H256], index: usize, _leaf_size: usize) -> bool {
    let mut index = index;
    let mut hash = *datum; // hash along the path to root
    for i in 0..proof.len() {
        let left_slice: [u8; 32];
        let right_slice: [u8; 32];
        // if i is even, it is on the left side, right hash is in the proof vector; vice versa
        if index % 2 == 0 {
            left_slice = <[u8; 32]>::from(hash);
            right_slice = <[u8; 32]>::from(proof[i]);
        } else {
            right_slice = <[u8; 32]>::from(hash);
            left_slice = <[u8; 32]>::from(proof[i]);
        }
        // concat and get parent hash
        let concat = [&left_slice[..], &right_slice[..]].concat();
        let digest = digest(&SHA256, &concat);
        hash = H256::from(digest);
        index /= 2;
    }
    if hash != *root {
        return false;
    }
    true
}
// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use crate::types::hash::H256;
    use super::*;

    macro_rules! gen_merkle_tree_data {
        () => {{
            vec![
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010202")).into(),
            ]
        }};
    }

    #[test]
    fn merkle_root() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        println!("{:#?}", merkle_tree);
        let root = merkle_tree.root();
        assert_eq!(
            root,
            (hex!("6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920")).into()
        );
        // "b69566be6e1720872f73651d1851a0eae0060a132cf0f64a0ffaea248de6cba0" is the hash of
        // "0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d"
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
        // "6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920" is the hash of
        // the concatenation of these two hashes "b69..." and "965..."
        // notice that the order of these two matters
    }

    #[test]
    fn merkle_proof() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert_eq!(proof,
                   vec![hex!("965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into()]
        );
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
    }

    #[test]
    fn merkle_verifying() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert!(verify(&merkle_tree.root(), &input_data[0].hash(), &proof, 0, input_data.len()));
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
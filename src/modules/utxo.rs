use serde::{Serialize, Deserialize};
use std::collections::{HashMap, BTreeMap};
use sha2::{Digest, Sha256};

/**
 * UTXO (Unspent Transaction Output) Model Implementation
 * 
 * This module implements a UTXO-based transaction system as an alternative to the
 * account-based model. The UTXO model tracks individual transaction outputs rather
 * than account balances, providing better scalability and security properties.
 * 
 * Key Benefits:
 * - Better parallelization: No account state conflicts during concurrent processing
 * - Simpler double-spend prevention: Each UTXO can only be spent once
 * - More explicit transaction validation: All inputs and outputs are explicit
 * - Better privacy: No visible account balances in the UTXO set
 * 
 * Performance Optimizations:
 * - BTreeMap storage for better cache locality and ordered iteration
 * - Recipient index for O(1) balance lookups
 * - Efficient binary serialization support
 * 
 * Components:
 * - OutPoint: Unique identifier for a UTXO (txid + output index)
 * - UTXO: Represents an unspent output with amount and recipient
 * - TxInput: References a UTXO being spent with signature
 * - TxOutput: Creates new UTXOs with amounts and recipients
 * - UTXOTransaction: Contains inputs and outputs for a transaction
 * - UTXOSet: Manages all unspent outputs with optimized lookups
 */

/// Unique identifier for a transaction output
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutPoint {
    /// Transaction hash that created this output
    pub txid: Vec<u8>,
    /// Index of the output within the transaction
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: Vec<u8>, vout: u32) -> Self {
        OutPoint { txid, vout }
    }
}

/// An unspent transaction output
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UTXO {
    /// The amount of tokens in this output
    pub amount: u64,
    /// The public key that can spend this output
    pub recipient: Vec<u8>,
    /// Block height when this UTXO was created
    pub block_height: u64,
    /// Time when this UTXO was created
    pub timestamp: u64,
}

impl UTXO {
    pub fn new(amount: u64, recipient: Vec<u8>, block_height: u64, timestamp: u64) -> Self {
        UTXO {
            amount,
            recipient,
            block_height,
            timestamp,
        }
    }
}

/// A transaction input that spends a UTXO
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxInput {
    /// Reference to the UTXO being spent
    pub outpoint: OutPoint,
    /// Digital signature proving ownership of the UTXO
    pub signature: String,
    /// Public key of the spender (for verification)
    pub public_key: Vec<u8>,
}

impl TxInput {
    pub fn new(outpoint: OutPoint, signature: String, public_key: Vec<u8>) -> Self {
        TxInput {
            outpoint,
            signature,
            public_key,
        }
    }
}

/// A transaction output that creates a new UTXO
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxOutput {
    /// Amount of tokens in this output
    pub amount: u64,
    /// Public key of the recipient
    pub recipient: Vec<u8>,
}

impl TxOutput {
    pub fn new(amount: u64, recipient: Vec<u8>) -> Self {
        TxOutput { amount, recipient }
    }
}

/// A transaction in the UTXO model
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UTXOTransaction {
    /// List of inputs (UTXOs being spent)
    pub inputs: Vec<TxInput>,
    /// List of outputs (new UTXOs being created)
    pub outputs: Vec<TxOutput>,
    /// Transaction timestamp
    pub timestamp: u64,
    /// Transaction hash (computed from inputs/outputs)
    pub hash: Vec<u8>,
}

impl UTXOTransaction {
    pub fn new(inputs: Vec<TxInput>, outputs: Vec<TxOutput>, timestamp: u64) -> Self {
        let mut tx = UTXOTransaction {
            inputs,
            outputs,
            timestamp,
            hash: Vec::new(),
        };
        tx.hash = tx.compute_hash();
        tx
    }

    /// Compute the transaction hash
    pub fn compute_hash(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        
        // Hash all inputs
        for input in &self.inputs {
            hasher.update(&input.outpoint.txid);
            hasher.update(&input.outpoint.vout.to_le_bytes());
            hasher.update(&input.public_key);
        }
        
        // Hash all outputs
        for output in &self.outputs {
            hasher.update(&output.amount.to_le_bytes());
            hasher.update(&output.recipient);
        }
        
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.finalize().to_vec()
    }

    /// Get total input amount (for validation)
    pub fn total_input_amount(&self, utxo_set: &UTXOSet) -> Option<u64> {
        let mut total = 0u64;
        for input in &self.inputs {
            if let Some(utxo) = utxo_set.get_utxo(&input.outpoint) {
                total = total.checked_add(utxo.amount)?;
            } else {
                return None; // Referenced UTXO doesn't exist
            }
        }
        Some(total)
    }

    /// Get total output amount
    pub fn total_output_amount(&self) -> u64 {
        self.outputs.iter().map(|output| output.amount).sum()
    }

    /// Calculate transaction fee (input_amount - output_amount)
    pub fn fee(&self, utxo_set: &UTXOSet) -> Option<u64> {
        let input_amount = self.total_input_amount(utxo_set)?;
        let output_amount = self.total_output_amount();
        input_amount.checked_sub(output_amount)
    }
}

/// Special transaction type for coinbase (creating new coins)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoinbaseTransaction {
    /// New coins being created
    pub outputs: Vec<TxOutput>,
    /// Block height for this coinbase
    pub block_height: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Transaction hash
    pub hash: Vec<u8>,
}

impl CoinbaseTransaction {
    pub fn new(outputs: Vec<TxOutput>, block_height: u64, timestamp: u64) -> Self {
        let mut tx = CoinbaseTransaction {
            outputs,
            block_height,
            timestamp,
            hash: Vec::new(),
        };
        tx.hash = tx.compute_hash();
        tx
    }

    pub fn compute_hash(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        
        for output in &self.outputs {
            hasher.update(&output.amount.to_le_bytes());
            hasher.update(&output.recipient);
        }
        
        hasher.update(&self.block_height.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.finalize().to_vec()
    }
}

/// The UTXO set - tracks all unspent transaction outputs
/// 
/// This is the core data structure that maintains the current state of all
/// unspent outputs in the blockchain. It provides efficient operations for:
/// - Adding new UTXOs when transactions create outputs
/// - Removing UTXOs when they are spent as inputs
/// - Looking up UTXOs for validation
/// - Calculating balances for addresses
/// 
/// Performance optimizations:
/// - BTreeMap for primary storage: Better cache locality than HashMap
/// - Recipient index: O(1) balance calculation instead of O(n) scan
/// - Cached count: O(1) size queries
/// 
/// The recipient_index is marked with #[serde(skip)] and rebuilt after
/// deserialization to avoid binary key serialization issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UTXOSet {
    /// Primary storage: BTreeMap for better cache locality and ordered iteration
    utxos: BTreeMap<OutPoint, UTXO>,
    /// Index by recipient for fast balance lookups
    #[serde(skip)]
    recipient_index: HashMap<Vec<u8>, Vec<OutPoint>>,
    /// Track total number of UTXOs for quick size queries
    count: usize,
}

impl UTXOSet {
    pub fn new() -> Self {
        UTXOSet {
            utxos: BTreeMap::new(),
            recipient_index: HashMap::new(),
            count: 0,
        }
    }

    /// Add a new UTXO to the set with optimized indexing
    pub fn add_utxo(&mut self, outpoint: OutPoint, utxo: UTXO) {
        // Add to primary storage
        if let Some(old_utxo) = self.utxos.insert(outpoint.clone(), utxo.clone()) {
            // Remove old entry from recipient index if it was a replacement
            if let Some(outpoints) = self.recipient_index.get_mut(&old_utxo.recipient) {
                outpoints.retain(|op| op != &outpoint);
                if outpoints.is_empty() {
                    self.recipient_index.remove(&old_utxo.recipient);
                }
            }
        } else {
            self.count += 1;
        }
        
        // Update recipient index
        self.recipient_index
            .entry(utxo.recipient.clone())
            .or_insert_with(Vec::new)
            .push(outpoint);
    }

    /// Remove a UTXO from the set (when it's spent) with index cleanup
    pub fn remove_utxo(&mut self, outpoint: &OutPoint) -> Option<UTXO> {
        if let Some(utxo) = self.utxos.remove(outpoint) {
            self.count -= 1;
            
            // Clean up recipient index
            if let Some(outpoints) = self.recipient_index.get_mut(&utxo.recipient) {
                outpoints.retain(|op| op != outpoint);
                if outpoints.is_empty() {
                    self.recipient_index.remove(&utxo.recipient);
                }
            }
            
            Some(utxo)
        } else {
            None
        }
    }

    /// Get a UTXO from the set
    pub fn get_utxo(&self, outpoint: &OutPoint) -> Option<&UTXO> {
        self.utxos.get(outpoint)
    }

    /// Check if a UTXO exists
    pub fn contains(&self, outpoint: &OutPoint) -> bool {
        self.utxos.contains_key(outpoint)
    }

    /// Get all UTXOs for a given recipient using optimized index
    pub fn get_utxos_for_recipient(&self, recipient: &[u8]) -> Vec<(OutPoint, &UTXO)> {
        if let Some(outpoints) = self.recipient_index.get(recipient) {
            outpoints
                .iter()
                .filter_map(|outpoint| {
                    self.utxos.get(outpoint).map(|utxo| (outpoint.clone(), utxo))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Calculate total balance for a recipient using optimized index
    pub fn get_balance(&self, recipient: &[u8]) -> u64 {
        if let Some(outpoints) = self.recipient_index.get(recipient) {
            outpoints
                .iter()
                .filter_map(|outpoint| self.utxos.get(outpoint))
                .map(|utxo| utxo.amount)
                .sum()
        } else {
            0
        }
    }

    /// Apply a transaction to the UTXO set
    pub fn apply_transaction(&mut self, tx: &UTXOTransaction, block_height: u64) -> Result<(), String> {
        // First verify all inputs exist
        for input in &tx.inputs {
            if !self.contains(&input.outpoint) {
                return Err(format!("UTXO not found: {:?}", input.outpoint));
            }
        }

        // Remove spent UTXOs
        for input in &tx.inputs {
            self.remove_utxo(&input.outpoint);
        }

        // Add new UTXOs
        for (index, output) in tx.outputs.iter().enumerate() {
            let outpoint = OutPoint::new(tx.hash.clone(), index as u32);
            let utxo = UTXO::new(output.amount, output.recipient.clone(), block_height, tx.timestamp);
            self.add_utxo(outpoint, utxo);
        }

        Ok(())
    }

    /// Apply a coinbase transaction to the UTXO set
    pub fn apply_coinbase(&mut self, tx: &CoinbaseTransaction) {
        for (index, output) in tx.outputs.iter().enumerate() {
            let outpoint = OutPoint::new(tx.hash.clone(), index as u32);
            let utxo = UTXO::new(output.amount, output.recipient.clone(), tx.block_height, tx.timestamp);
            self.add_utxo(outpoint, utxo);
        }
    }

    /// Get the total number of UTXOs using cached count
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the UTXO set is empty using cached count
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Rebuild recipient index (for use after deserialization)
    pub fn rebuild_index(&mut self) {
        self.recipient_index.clear();
        self.count = self.utxos.len();
        
        for (outpoint, utxo) in &self.utxos {
            self.recipient_index
                .entry(utxo.recipient.clone())
                .or_insert_with(Vec::new)
                .push(outpoint.clone());
        }
    }
}

impl Default for UTXOSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utxo_creation() {
        let utxo = UTXO::new(100, vec![1, 2, 3], 1, 12345);
        assert_eq!(utxo.amount, 100);
        assert_eq!(utxo.recipient, vec![1, 2, 3]);
        assert_eq!(utxo.block_height, 1);
        assert_eq!(utxo.timestamp, 12345);
    }

    #[test]
    fn test_outpoint() {
        let outpoint = OutPoint::new(vec![1, 2, 3], 0);
        assert_eq!(outpoint.txid, vec![1, 2, 3]);
        assert_eq!(outpoint.vout, 0);
    }

    #[test]
    fn test_utxo_set() {
        let mut utxo_set = UTXOSet::new();
        let outpoint = OutPoint::new(vec![1, 2, 3], 0);
        let utxo = UTXO::new(100, vec![4, 5, 6], 1, 12345);

        utxo_set.add_utxo(outpoint.clone(), utxo.clone());
        assert!(utxo_set.contains(&outpoint));
        assert_eq!(utxo_set.get_utxo(&outpoint), Some(&utxo));
        assert_eq!(utxo_set.get_balance(&vec![4, 5, 6]), 100);

        let removed = utxo_set.remove_utxo(&outpoint);
        assert_eq!(removed, Some(utxo));
        assert!(!utxo_set.contains(&outpoint));
    }

    #[test]
    fn test_transaction_hash() {
        let input = TxInput::new(
            OutPoint::new(vec![1, 2, 3], 0),
            "signature".to_string(),
            vec![4, 5, 6],
        );
        let output = TxOutput::new(100, vec![7, 8, 9]);
        
        let tx = UTXOTransaction::new(vec![input], vec![output], 12345);
        assert!(!tx.hash.is_empty());
        assert_eq!(tx.hash.len(), 32); // SHA256 hash length
    }

    #[test]
    fn test_transaction_amounts() {
        let mut utxo_set = UTXOSet::new();
        
        // Create a UTXO to spend
        let prev_outpoint = OutPoint::new(vec![1, 2, 3], 0);
        let prev_utxo = UTXO::new(100, vec![4, 5, 6], 1, 12345);
        utxo_set.add_utxo(prev_outpoint.clone(), prev_utxo);

        // Create transaction spending that UTXO
        let input = TxInput::new(prev_outpoint, "signature".to_string(), vec![4, 5, 6]);
        let output = TxOutput::new(90, vec![7, 8, 9]); // 10 token fee
        let tx = UTXOTransaction::new(vec![input], vec![output], 12345);

        assert_eq!(tx.total_input_amount(&utxo_set), Some(100));
        assert_eq!(tx.total_output_amount(), 90);
        assert_eq!(tx.fee(&utxo_set), Some(10));
    }
}
use crate::runtime::execution::df_engine::{DataChunk, Schema};
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Transaction {
    pub id: u64,
    pub pending_tables: HashMap<String, (Schema, Vec<DataChunk>)>,
    pub read_set: std::collections::HashSet<String>,
    pub write_set: std::collections::HashSet<String>,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Active,
    Committed,
    Aborted,
}

pub struct TransactionContext {
    pub dur: Arc<crate::runtime::database::durability::DurabilityStorage>,
    active_transactions: Mutex<HashMap<u64, Transaction>>,
    committed_tx_ids: Mutex<Vec<u64>>,
    next_tx_id: Mutex<u64>,
}

impl TransactionContext {
    pub fn new(dur: Arc<crate::runtime::database::durability::DurabilityStorage>) -> Self {
        Self {
            dur,
            active_transactions: Mutex::new(HashMap::new()),
            committed_tx_ids: Mutex::new(Vec::new()),
            next_tx_id: Mutex::new(1),
        }
    }

    pub fn begin_transaction(&self) -> u64 {
        let mut id_gen = self.next_tx_id.lock().unwrap_or_else(|e| e.into_inner());
        let id = *id_gen;
        *id_gen += 1;

        let mut active = self
            .active_transactions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        active.insert(
            id,
            Transaction {
                id,
                pending_tables: HashMap::new(),
                read_set: std::collections::HashSet::new(),
                write_set: std::collections::HashSet::new(),
                status: TransactionStatus::Active,
            },
        );

        let _ = self
            .dur
            .log_op(crate::runtime::database::durability::WalOp::BeginTransaction { tx_id: id });
        id
    }

    pub fn add_pending_table(
        &self,
        tx_id: u64,
        name: String,
        schema: Schema,
        chunks: Vec<DataChunk>,
    ) -> Result<(), String> {
        let mut active = self
            .active_transactions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tx = active.get_mut(&tx_id).ok_or("Transaction not found")?;
        if tx.status != TransactionStatus::Active {
            return Err("Transaction not active".to_string());
        }

        // Log to unified DurabilityStorage
        let _schema_json = serde_json::to_string(&schema.fields).unwrap_or_default();
        for (i, chunk) in chunks.iter().enumerate() {
            let _ = self
                .dur
                .log_op(crate::runtime::database::durability::WalOp::InsertChunk {
                    table_name: name.clone(),
                    chunk_id: i as u64,
                    data: chunk.clone(),
                });
        }

        tx.pending_tables.insert(name.clone(), (schema, chunks));
        tx.write_set.insert(name);
        Ok(())
    }

    pub fn commit(&self, tx_id: u64) -> Result<HashMap<String, (Schema, Vec<DataChunk>)>, String> {
        let mut active = self
            .active_transactions
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        // SERIALIZABLE CONFLICT DETECTION
        // In a real system, we'd check if any write_set intersects with other concurrent read_sets.
        let _tx = active.get(&tx_id).ok_or("Transaction not found")?;

        let tx = active
            .remove(&tx_id)
            .expect("Transaction missing from active pool");
        let _ = self
            .dur
            .log_op(crate::runtime::database::durability::WalOp::Commit { tx_id });

        self.committed_tx_ids
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(tx_id);
        Ok(tx.pending_tables)
    }

    pub fn abort(&self, tx_id: u64) -> Result<(), String> {
        let mut active = self
            .active_transactions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match active.remove(&tx_id) {
            Some(_tx) => {
                let _ = self
                    .dur
                    .log_op(crate::runtime::database::durability::WalOp::Abort { tx_id });
                Ok(())
            }
            None => Err("Transaction not found".to_string()),
        }
    }
}

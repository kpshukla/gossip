use crate::error::Error;
use crate::storage::{RawDatabase, Storage, MAX_LMDB_KEY};
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::{PublicKey, RelayUrl};
use std::sync::Mutex;

// PublicKey:Url -> () (presence in the table means the relay is pinned for that person)
//   key: key!(pubkey.as_bytes + url.as_str().as_bytes)
//   val: empty

static PERSON_PINNED_RELAYS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON_PINNED_RELAYS1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_person_pinned_relays1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PERSON_PINNED_RELAYS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON_PINNED_RELAYS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON_PINNED_RELAYS1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<Bytes, Bytes>()
                    // no .flags needed
                    .name("person_pinned_relays1")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_PINNED_RELAYS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    fn person_pinned_relay_key(pubkey: PublicKey, url: &RelayUrl) -> Vec<u8> {
        let mut key = pubkey.to_bytes();
        key.extend(url.as_str().as_bytes());
        key.truncate(MAX_LMDB_KEY);
        key
    }

    /// Pin a relay for a person, guaranteeing gossip always considers it for
    /// them even if it's missing from their published relay list.
    pub fn pin_person_relay<'a>(
        &'a self,
        pubkey: PublicKey,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key = Self::person_pinned_relay_key(pubkey, url);

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_person_pinned_relays1()?.put(txn, &key, &[])?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    /// Unpin a relay for a person
    pub fn unpin_person_relay<'a>(
        &'a self,
        pubkey: PublicKey,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key = Self::person_pinned_relay_key(pubkey, url);

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_person_pinned_relays1()?.delete(txn, &key)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    /// Is this relay pinned for this person?
    pub fn is_person_relay_pinned(
        &self,
        pubkey: PublicKey,
        url: &RelayUrl,
    ) -> Result<bool, Error> {
        let key = Self::person_pinned_relay_key(pubkey, url);
        let txn = self.env.read_txn()?;
        Ok(self.db_person_pinned_relays1()?.get(&txn, &key)?.is_some())
    }

    /// Get all relays pinned for a person
    pub fn get_person_pinned_relays(&self, pubkey: PublicKey) -> Result<Vec<RelayUrl>, Error> {
        let start_key = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        let iter = self
            .db_person_pinned_relays1()?
            .prefix_iter(&txn, &start_key)?;
        let mut output: Vec<RelayUrl> = Vec::new();
        for result in iter {
            let (key, _val) = result?;
            let url_bytes = &key[start_key.len()..];
            if let Ok(url_str) = std::str::from_utf8(url_bytes) {
                if let Ok(url) = RelayUrl::try_from_str(url_str) {
                    output.push(url);
                }
            }
        }
        Ok(output)
    }
}

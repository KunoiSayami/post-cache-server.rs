use std::collections::HashMap;

use kstool_helper_generator::Helper;
use rusty_leveldb::{LdbIterator, DB};

type KeyType = u64;
const KEY_LENGTH: usize = 8;

#[derive(Helper)]
pub enum PersistentStorageEvent {
    Put(KeyType, Vec<u8>),
    Delete(KeyType),
    Exit,
}

pub struct LevelDB {
    handle: std::thread::JoinHandle<anyhow::Result<()>>,
}

impl LevelDB {
    pub fn run(
        level_db_path: &str,
    ) -> rusty_leveldb::Result<(HashMap<KeyType, Vec<u8>>, PersistentStorageHelper, Self)> {
        let mut db = rusty_leveldb::DB::open(level_db_path, Default::default())?;
        let (s, r) = PersistentStorageHelper::new(8);

        let mut iter = db.new_iter()?;
        let mut ret = HashMap::new();
        let mut buf = [0u8; KEY_LENGTH];

        let mut pending_remove = vec![];

        while let Some((key, value)) = iter.next() {
            if key.len() < KEY_LENGTH {
                pending_remove.push(key);
                continue;
            }
            buf.copy_from_slice(&key[..KEY_LENGTH]);
            ret.insert(KeyType::from_be_bytes(buf), value);
        }

        Ok((
            ret,
            s,
            Self {
                handle: std::thread::spawn(move || Self::staff(pending_remove, db, r)),
            },
        ))
    }

    fn staff(
        pending_remove: Vec<Vec<u8>>,
        mut db: DB,
        mut receiver: PersistentStorageEventReceiver,
    ) -> anyhow::Result<()> {
        for key in pending_remove {
            db.delete(&key)?;
        }

        while let Some(event) = receiver.blocking_recv() {
            if let PersistentStorageEvent::Exit = event {
                break;
            }
            Self::handle_event(&mut db, event)
                .inspect_err(|e| log::error!("LevelDB Error: {e:?}"))
                .ok();
        }

        db.close()?;

        Ok(())
    }

    fn handle_event(db: &mut DB, event: PersistentStorageEvent) -> rusty_leveldb::Result<()> {
        match event {
            PersistentStorageEvent::Put(key, value) => {
                db.put(&key.to_be_bytes(), &value)?;
            }
            PersistentStorageEvent::Delete(key) => {
                db.delete(&key.to_be_bytes())?;
            }
            PersistentStorageEvent::Exit => unreachable!(),
        }
        Ok(())
    }

    pub async fn join(self) -> anyhow::Result<()> {
        tokio::task::spawn_blocking(move || self.handle.join())
            .await?
            .unwrap()
    }
}

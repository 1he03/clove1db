use itertools::Itertools;

use crate::{
    dto::{InputDto, OutputDto},
    entity::Entity,
    event_emitter::EventEmitter,
    repository::{BackupRecordRepository, Repository},
    units::Result,
};
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Domain<E: Entity> {
    repository: Repository<E>,
    emitter: EventEmitter,
    _marker: PhantomData<E>,
}

impl<E: Entity> Domain<E> {
    pub fn new(repository: Repository<E>, emitter: EventEmitter) -> Self {
        Self {
            repository,
            emitter,
            _marker: PhantomData,
        }
    }

    pub async fn create<I, O>(&self, input: I) -> Result<O>
    where
        I: InputDto<E>,
        O: OutputDto<E>,
    {
        input.validate()?;
        let entity = input.into_entity()?;
        self.repository.set(entity.entity_id(), &entity).await?;
        self.emitter
            .info(format!("Created → {}", entity.entity_id()));
        Ok(O::from_entity(entity))
    }

    pub async fn get<O: OutputDto<E>>(&self, id: &str) -> Result<O> {
        let entity = self.repository.get(id).await?;
        self.emitter.info(format!("Get → {}", id));
        Ok(O::from_entity(entity))
    }

    pub async fn list<O: OutputDto<E>>(&self) -> Result<Vec<O>> {
        let entities = self.repository.list().await?;
        self.emitter
            .info(format!("List → {} item(s)", entities.len()));
        Ok(O::from_entities(entities))
    }

    pub async fn update<I, O>(&self, id: &str, input: I) -> Result<O>
    where
        I: InputDto<E>,
        O: OutputDto<E>,
    {
        input.validate()?;
        let entity = input.into_entity()?;
        self.repository.set(id, &entity).await?;
        self.emitter.info(format!("Updated → {}", id));
        Ok(O::from_entity(entity))
    }

    pub async fn update_bulk<I, O>(&self, inputs: Vec<(String, I)>) -> Result<(Vec<O>, String)>
    where
        I: InputDto<E>,
        O: OutputDto<E>,
    {
        for (_, input) in &inputs {
            input.validate()?;
        }

        let mut results = Vec::new();
        let mut bulk_ids = Vec::new();

        for (id, input) in inputs {
            let current_version = self.repository.current_version(&id).await.unwrap_or(0);
            let entity = input.into_entity()?;
            self.repository.set(&id, &entity).await?;
            self.emitter.info(format!("Bulk updated → {}", id));
            bulk_ids.push((id, current_version));
            results.push(O::from_entity(entity));
        }

        let bulk_id = self.repository.set_bulk(bulk_ids).await?;
        self.emitter
            .info(format!("Bulk snapshot saved → {}", bulk_id));

        Ok((results, bulk_id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.repository.delete(id).await?;
        self.emitter.info(format!("Deleted → {}", id));
        Ok(())
    }

    pub async fn restore_by_version(&self, id: &str, version: u64) -> Result<()> {
        self.repository.restore_by_version(id, version).await?;
        self.emitter
            .info(format!("Restored → {} from v{}", id, version));
        Ok(())
    }

    pub async fn get_by_version<O: OutputDto<E>>(
        &self,
        id: &str,
        version: u64,
    ) -> Result<BackupRecordRepository<O>> {
        let entity = self.repository.get_by_version(id, version).await?;
        self.emitter.info(format!("Get → {} from v{}", id, version));

        let data = if let Some(data) = entity.data {
            let value = O::from_entity(data);
            Some(value)
        } else {
            None
        };

        Ok(BackupRecordRepository::<O> {
            version: entity.version,
            timestamp: entity.timestamp,
            date: entity.date,
            operation: entity.operation,
            table: entity.table,
            key: entity.key,
            data: data,
            restored_version: entity.restored_version,
            bulk_id: entity.bulk_id,
        })
    }

    pub async fn restore_at(&self, id: &str, timestamp: i64) -> Result<()> {
        self.repository.restore_at(id, timestamp).await?;
        self.emitter
            .info(format!("Restored → {} at ts {}", id, timestamp));
        Ok(())
    }

    pub async fn get_version_by_at<O: OutputDto<E>>(
        &self,
        id: &str,
        timestamp: i64,
    ) -> Result<BackupRecordRepository<O>> {
        let entity = self.repository.get_version_by_at(id, timestamp).await?;
        self.emitter.info(format!(
            "Get → Id {} Version {} from timestamp {}",
            id, entity.version, timestamp
        ));

        let data = if let Some(data) = entity.data {
            let value = O::from_entity(data);
            Some(value)
        } else {
            None
        };

        Ok(BackupRecordRepository::<O> {
            version: entity.version,
            timestamp: entity.timestamp,
            date: entity.date,
            operation: entity.operation,
            table: entity.table,
            key: entity.key,
            data: data,
            restored_version: entity.restored_version,
            bulk_id: entity.bulk_id,
        })
    }

    pub async fn restore_bulk(&self, bulk_id: &str) -> Result<()> {
        self.repository.restore_bulk(bulk_id).await?;
        self.emitter.info(format!("Bulk restored → {}", bulk_id));
        Ok(())
    }

    pub async fn history<O: OutputDto<E>>(
        &self,
        id: &str,
    ) -> Result<Vec<BackupRecordRepository<O>>> {
        let entities = self.repository.history(id).await?;
        self.emitter.info(format!(
            "Get history → id {} - {} item(s)",
            id,
            entities.len()
        ));

        let history = entities
            .into_iter()
            .map(|record| {
                let data = if let Some(data) = record.data {
                    let value = O::from_entity(data);
                    Some(value)
                } else {
                    None
                };

                BackupRecordRepository::<O> {
                    version: record.version,
                    timestamp: record.timestamp,
                    date: record.date,
                    operation: record.operation,
                    table: record.table,
                    key: record.key,
                    data: data,
                    restored_version: record.restored_version,
                    bulk_id: record.bulk_id,
                }
            })
            .collect_vec();

        Ok(history)
    }

    pub fn repo(&self) -> &Repository<E> {
        &self.repository
    }
    pub fn emitter(&self) -> &EventEmitter {
        &self.emitter
    }
}

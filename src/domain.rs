use crate::{entity::Entity, repository::Repository, event_emitter::EventEmitter, dto::{InputDto, OutputDto}, units::Result};
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Domain<E: Entity> {
    repository: Repository<E>,
    emitter: EventEmitter,
    _marker: PhantomData<E>,
}

impl<E: Entity> Domain<E> {
    pub fn new(repository: Repository<E>, emitter: EventEmitter) -> Self {
        Self { repository, emitter, _marker: PhantomData }
    }

    pub async fn create<I, O>(&self, input: I) -> Result<O>
    where I: InputDto<E>, O: OutputDto<E>
    {
        input.validate()?;
        let entity = input.into_entity()?;
        self.repository.set(entity.entity_id(), &entity).await?;
        self.emitter.info(format!("created → {}", entity.entity_id()));
        Ok(O::from_entity(entity))
    }

    pub async fn get<O: OutputDto<E>>(&self, id: &str) -> Result<O> {
        let entity = self.repository.get(id).await?;
        self.emitter.info(format!("get → {}", id));
        Ok(O::from_entity(entity))
    }

    pub async fn list<O: OutputDto<E>>(&self) -> Result<Vec<O>> {
        let entities = self.repository.list().await?;
        self.emitter.info(format!("list → {} item(s)", entities.len()));
        Ok(O::from_entities(entities))
    }

    pub async fn update<I, O>(&self, id: &str, input: I) -> Result<O>
    where I: InputDto<E>, O: OutputDto<E>
    {
        input.validate()?;
        let entity = input.into_entity()?;
        self.repository.set(id, &entity).await?;
        self.emitter.info(format!("updated → {}", id));
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
    
        let mut results  = Vec::new();
        let mut bulk_ids = Vec::new();
    
        for (id, input) in inputs {
            let current_version = self.repository.current_version(&id).await.unwrap_or(0);
            let entity = input.into_entity()?;
            self.repository.set(&id, &entity).await?;
            self.emitter.info(format!("bulk updated → {}", id));
            bulk_ids.push((id, current_version));
            results.push(O::from_entity(entity));
        }
    
        let bulk_id = self.repository.set_bulk(bulk_ids).await?;
        self.emitter.info(format!("bulk snapshot saved → {}", bulk_id));
    
        Ok((results, bulk_id))
    }
    

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.repository.delete(id).await?;
        self.emitter.info(format!("deleted → {}", id));
        Ok(())
    }

    pub async fn restore_by_version(&self, id: &str, version: u64) -> Result<()> {
        self.repository.restore_by_version(id, version).await?;
        self.emitter.info(format!("restored → {} from v{}", id, version));
        Ok(())
    }

    pub async fn restore_at(&self, id: &str, timestamp: i64) -> Result<()> {
        self.repository.restore_at(id, timestamp).await?;
        self.emitter.info(format!("restored → {} at ts {}", id, timestamp));
        Ok(())
    }
    
    pub async fn restore_bulk(&self, bulk_id: &str) -> Result<()> {
        self.repository.restore_bulk(bulk_id).await?;
        self.emitter.info(format!("bulk restored → {}", bulk_id));
        Ok(())
    }

    pub fn repo(&self) -> &Repository<E> { &self.repository }
    pub fn emitter(&self) -> &EventEmitter { &self.emitter }
}

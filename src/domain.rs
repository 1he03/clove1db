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
        Ok(O::from_entity(entity))
    }

    pub async fn list<O: OutputDto<E>>(&self) -> Result<Vec<O>> {
        let entities = self.repository.list().await?;
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

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.repository.delete(id).await?;
        self.emitter.info(format!("deleted → {}", id));
        Ok(())
    }

    pub fn repo(&self) -> &Repository<E> { &self.repository }
    pub fn emitter(&self) -> &EventEmitter { &self.emitter }
}

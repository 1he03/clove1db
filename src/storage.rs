// storage.rs
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::env;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    repository::DatabaseManager,
    domain::Domain,
    entity::Entity,
    event_emitter::EventEmitter,
    event_handler::EventHandler,
    repository::Repository,
    units::Result,
};

const DEFAULT_CACHE_CAPACITY: u64 = 10_000;
const DEFAULT_CACHE_TTL: u64      = 300;
const DEFAULT_CACHE_IDLE: u64     = 60;
const LOG_CHANNEL_CAPACITY: usize = 1024;

// ── Inner ──────────────────────────────────────────────────────────────────────

struct StorageInner {
    emitter:          Arc<EventEmitter>,
    domains:          HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    // نحتفظ بهم للـ maintenance (backup update، إلخ)
    database_managers: HashMap<String, DatabaseManager>,
}

// ── Storage ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Storage(Arc<StorageInner>);

impl Storage {
    pub fn builder(config: StorageConfig) -> StorageBuilder {
        StorageBuilder::new(config)
    }

    /// استدعاء Domain<E>
    pub fn domain<E: Entity>(&self) -> &Domain<E> {
        self.0
            .domains
            .get(&TypeId::of::<E>())
            .and_then(|b| b.downcast_ref::<Domain<E>>())
            .unwrap_or_else(|| panic!(
                "[Storage] Domain<{}> غير مسجّل",
                std::any::type_name::<E>()
            ))
    }

    /// وصول لـ DatabaseManager معيّن بالاسم
    pub fn db_manager(&self, name: &str) -> &DatabaseManager {
        self.0
            .database_managers
            .get(name)
            .unwrap_or_else(|| panic!(
                "[Storage] DatabaseManager '{}' غير موجود",
                name
            ))
    }

    pub fn db_list(&self) -> Vec<&DatabaseManager> {
        self.0
            .database_managers
            .values()
            .collect()
    }

    pub fn db_list_names(&self) -> Vec<String> {
        self.0
            .database_managers
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    pub fn event_handler<H: EventHandler>(&self, handler: H) -> &Self {
        self.0.emitter.event_handler(handler);
        self
    }

    pub fn emitter(&self) -> &EventEmitter {
        &self.0.emitter
    }
}

// ── DomainFactory ──────────────────────────────────────────────────────────────

trait DomainFactory: Send + Sync {
    fn table_name(&self) -> &'static str;
    fn build(
        &self,
        database_manager: &DatabaseManager,
        emitter: &EventEmitter,
    ) -> (TypeId, Box<dyn Any + Send + Sync>);
}

struct TypedFactory<E: Entity> {
    table: &'static str,
    _marker: PhantomData<fn() -> E>,
}

impl<E: Entity> DomainFactory for TypedFactory<E> {
    fn table_name(&self) -> &'static str {
        self.table
    }

    fn build(
        &self,
        database_manager: &DatabaseManager,
        emitter: &EventEmitter,
    ) -> (TypeId, Box<dyn Any + Send + Sync>) {
        let repo   = Repository::<E>::new(self.table, database_manager.clone());
        let child  = emitter.child(self.table);
        let domain = Domain::new(repo, child);
        (TypeId::of::<E>(), Box::new(domain))
    }
}

// ── DatabaseConfig — builder لكل DatabaseManager ──────────────────────────────

pub struct DatabaseConfig {
    dir_path:        PathBuf,
    backup_dir_path: Option<PathBuf>,
    dir_name:        String,
    db_name:         String,
    cache_capacity:  u64,
    cache_ttl:       u64,
    cache_idle:      u64,
    factories:       Vec<Box<dyn DomainFactory>>,
    backup_enabled:  bool,
}

impl DatabaseConfig {
    pub fn new(
        dir_name: &str,
        db_name: &str,
    ) -> Self {

        Self {
            dir_path: PathBuf::from(""),
            backup_dir_path: None,
            dir_name:   dir_name.to_string(),
            db_name:    db_name.to_string(),
            cache_capacity: DEFAULT_CACHE_CAPACITY,
            cache_ttl:      DEFAULT_CACHE_TTL,
            cache_idle:     DEFAULT_CACHE_IDLE,
            factories:  Vec::new(),
            backup_enabled: false,
        }
    }

    pub fn dir_path(mut self, path: PathBuf) -> Self {
        self.dir_path = path;
        self
    }

    pub fn backup_dir(mut self, path: PathBuf) -> Self {
        self.backup_enabled = true;
        self.backup_dir_path = Some(path);
        self
    }

    pub fn backup_enabled(mut self, enabled: bool) -> Self {
        self.backup_enabled = enabled;
        self
    }

    pub fn cache(mut self, capacity: u64, ttl_secs: u64, idle_secs: u64) -> Self {
        self.cache_capacity = capacity;
        self.cache_ttl      = ttl_secs;
        self.cache_idle     = idle_secs;
        self
    }

    /// سجّل Repository<E> تحت هذا الـ DatabaseManager
    pub fn register<E: Entity>(mut self, table: &'static str) -> Self {
        self.factories.push(Box::new(TypedFactory::<E> {
            table,
            _marker: PhantomData,
        }));
        self
    }
}

#[derive(Clone)]
pub struct StorageConfig {
    log_channel_capacity: usize,
    name: String,
    dir_path: PathBuf,
}

impl StorageConfig {
    pub fn default() -> Self {
        let dir_path = env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        Self { log_channel_capacity: LOG_CHANNEL_CAPACITY, name: "storage".to_string(), dir_path }
    }

    pub fn change_log_channel_capacity(mut self, capacity: usize) -> Self {
        self.log_channel_capacity = capacity;
        self
    }

    pub fn change_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn change_dir_path(mut self, path: PathBuf) -> Self {
        self.dir_path = path;
        self
    }
}

// ── StorageBuilder ─────────────────────────────────────────────────────────────

pub struct StorageBuilder {
    emitter: Arc<EventEmitter>,
    database_configs: Vec<DatabaseConfig>,
    storage_config: StorageConfig,
}

impl StorageBuilder {
    fn new(config: StorageConfig) -> Self {
        Self {
            emitter: Arc::new(EventEmitter::new(config.name.clone(), config.log_channel_capacity)),
            database_configs: Vec::new(),
            storage_config: config,
        }
    }

    /// event_handler مرة واحدة يغطي كل DatabaseManagers
    pub fn event_handler<H: EventHandler>(self, handler: H) -> Self {
        self.emitter.event_handler(handler);
        self
    }

    /// أضف DatabaseManager مع repositories الخاصة فيه
    pub fn add_database(mut self, config: DatabaseConfig) -> Self {
        let mut config = config;
        if (config.dir_path.to_str().is_some() && config.dir_path.to_str().unwrap().is_empty()) || config.dir_path.to_str().is_none() {
            config.dir_path = self.storage_config.dir_path.clone();
        }
        if config.backup_enabled && config.backup_dir_path.is_none() {
            config.backup_dir_path = Some(config.dir_path.clone());
        }
        self.database_configs.push(config);
        self
    }

    pub async fn build(self) -> Result<Storage> {
        let mut domains: HashMap<TypeId, Box<dyn Any + Send + Sync>> = HashMap::new();
        let mut database_managers: HashMap<String, DatabaseManager> = HashMap::new();

        for config in self.database_configs {
            // اجمع أسماء الجداول لهذا الـ DatabaseManager
            let tables: Vec<String> = config.factories
                .iter()
                .map(|f| f.table_name().to_string())
                .collect();

            // أنشئ DatabaseManager مستقل لكل config
            let db_manager = DatabaseManager::new(
                &config.dir_path,
                config.backup_dir_path.as_ref(),
                &config.dir_name,
                &config.db_name,
                tables,
                &self.emitter,
                config.cache_capacity,
                config.cache_ttl,
                config.cache_idle,
            ).await?;

            // أنشئ Domain لكل factory تحت هذا الـ DatabaseManager
            for factory in &config.factories {
                let (type_id, domain) = factory.build(&db_manager, &self.emitter);
                domains.insert(type_id, domain);
            }

            // احتفظ بالـ DatabaseManager للـ maintenance
            database_managers.insert(config.db_name.clone(), db_manager);
        }

        self.emitter.info(format!(
            "✅ Storage ready — {} db(s) | {} domain(s)",
            database_managers.len(),
            domains.len()
        ));

        Ok(Storage(Arc::new(StorageInner {
            emitter: self.emitter,
            domains,
            database_managers,
        })))
    }
}

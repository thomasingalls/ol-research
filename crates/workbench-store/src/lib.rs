//! Application persistence, independent of HTTP, model providers and runners.
//!
//! Connect, explicitly migrate, then use the storage API. Native checkpoints
//! retain exact bytes and a versioned envelope; they are not rendered messages.

mod records;
pub use records::{Checkpoint, Error, MAX_CHECKPOINT_BYTES, NativeState, Project, Session};

use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool};

/// Embedded, forward-only application migrations. Connection does not run them.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// A cloneable handle to the application's Postgres connection pool.
#[derive(Debug, Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    /// Connect with a bounded pool and finite connection/statement deadlines.
    /// TLS and authentication options come from the URL; it is never logged.
    ///
    /// # Errors
    /// Returns the `SQLx` connection/configuration error.
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let options = database_url
            .parse::<PgConnectOptions>()?
            .application_name("ol-rht-workbench")
            .disable_statement_logging();
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    sqlx::raw_sql(
                        "SET statement_timeout = '30s'; \
                         SET idle_in_transaction_session_timeout = '30s'",
                    )
                    .execute(connection)
                    .await?;
                    Ok(())
                })
            })
            .connect_with(options)
            .await?;
        Ok(Self { pool })
    }

    /// Adopt an existing pool, retaining the caller's connection settings.
    #[must_use]
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Apply pending migrations. Call from an explicit deployment/dev operation.
    ///
    /// # Errors
    /// Reports migration failures, including changed checksums.
    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        MIGRATOR.run(&self.pool).await
    }

    /// Check connectivity without modifying schema or application records.
    ///
    /// # Errors
    /// Returns the `SQLx` query/connection error.
    pub async fn health(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Close the pool after outstanding operations release their connections.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

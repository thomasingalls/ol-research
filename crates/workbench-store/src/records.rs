use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::Store;

/// Maximum inline checkpoint size. Large artifact bytes belong outside this API.
pub const MAX_CHECKPOINT_BYTES: usize = 8 * 1024 * 1024;

/// Stable research-project identity, independent of any runner or conversation UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

/// A session's current storage revision; native branches remain inside its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
}

/// The envelope needed to select a reader for native session bytes.
/// This crate neither parses the payload nor executes anything it contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeState {
    pub runner: String,
    pub runner_version: String,
    pub format: String,
    pub format_version: String,
    pub native_session_id: String,
    pub payload: Vec<u8>,
}

/// An immutable snapshot. Revisions order storage commits, not agent turns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub id: Uuid,
    pub session_id: Uuid,
    pub revision: i64,
    pub runner: String,
    pub runner_version: String,
    pub format: String,
    pub format_version: String,
    pub native_session_id: String,
    pub payload: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

/// Storage conflicts are distinct from database failures and absent records.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    InvalidInput(&'static str),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("the supplied ID already identifies different data")]
    IdConflict,
    #[error("session revision changed: expected {expected}, found {actual}")]
    RevisionConflict { expected: i64, actual: i64 },
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

impl NativeState {
    fn validate(&self) -> Result<(), Error> {
        for field in [
            &self.runner,
            &self.runner_version,
            &self.format,
            &self.format_version,
            &self.native_session_id,
        ] {
            validate_text(field, 256)?;
        }
        if self.payload.is_empty() || self.payload.len() > MAX_CHECKPOINT_BYTES {
            return Err(Error::InvalidInput(
                "checkpoint payload must contain between 1 byte and 8 MiB",
            ));
        }
        Ok(())
    }
}

impl Checkpoint {
    fn matches(&self, state: &NativeState) -> bool {
        self.runner == state.runner
            && self.runner_version == state.runner_version
            && self.format == state.format
            && self.format_version == state.format_version
            && self.native_session_id == state.native_session_id
            && self.payload == state.payload
    }
}

fn validate_text(value: &str, max_chars: usize) -> Result<(), Error> {
    if value.trim().is_empty() || value.chars().count() > max_chars || value.contains('\0') {
        return Err(Error::InvalidInput(
            "text must be nonempty, bounded and contain no NUL",
        ));
    }
    Ok(())
}

impl Store {
    /// Create with a caller-chosen stable ID. An identical retry returns the row.
    ///
    /// # Errors
    /// Invalid title, reused ID with different data, or a database failure.
    pub async fn create_project(&self, id: Uuid, title: &str) -> Result<Project, Error> {
        validate_text(title, 512)?;
        sqlx::query!(
            "INSERT INTO workbench_projects (id, title) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
            id,
            title,
        )
        .execute(&self.pool)
        .await?;
        let project = self.project(id).await?.ok_or(Error::NotFound("project"))?;
        if project.title != title {
            return Err(Error::IdConflict);
        }
        Ok(project)
    }

    /// Read a project. Authorization belongs to the calling application.
    ///
    /// # Errors
    /// Returns a database failure; an absent ID returns `None`.
    pub async fn project(&self, id: Uuid) -> Result<Option<Project>, Error> {
        Ok(sqlx::query_as!(
            Project,
            "SELECT id, title, created_at FROM workbench_projects WHERE id = $1",
            id,
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Create a session in an existing project; identical retries are idempotent.
    ///
    /// # Errors
    /// Invalid title, missing parent, conflicting ID, or a database failure.
    pub async fn create_session(
        &self,
        project_id: Uuid,
        id: Uuid,
        title: &str,
    ) -> Result<Session, Error> {
        validate_text(title, 512)?;
        let result = sqlx::query!(
            "INSERT INTO workbench_sessions (id, project_id, title) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO NOTHING",
            id,
            project_id,
            title,
        )
        .execute(&self.pool)
        .await;
        if let Err(error) = result {
            if error
                .as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_foreign_key_violation)
            {
                return Err(Error::NotFound("project"));
            }
            return Err(error.into());
        }
        let session = self
            .session(project_id, id)
            .await?
            .ok_or(Error::IdConflict)?;
        if session.title != title {
            return Err(Error::IdConflict);
        }
        Ok(session)
    }

    /// Read only within the supplied project. IDs alone are not authorization.
    ///
    /// # Errors
    /// Returns a database failure; an absent or differently scoped ID is `None`.
    pub async fn session(&self, project_id: Uuid, id: Uuid) -> Result<Option<Session>, Error> {
        Ok(sqlx::query_as!(
            Session,
            "SELECT id, project_id, title, revision, created_at
             FROM workbench_sessions WHERE project_id = $1 AND id = $2",
            project_id,
            id,
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Page sessions by UUID, without loading any checkpoint payloads.
    ///
    /// # Errors
    /// Limit must be 1..=100; database errors are propagated.
    pub async fn sessions(
        &self,
        project_id: Uuid,
        after: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<Session>, Error> {
        if !(1..=100).contains(&limit) {
            return Err(Error::InvalidInput("page size must be 1..=100"));
        }
        Ok(sqlx::query_as!(
            Session,
            "SELECT id, project_id, title, revision, created_at FROM workbench_sessions
             WHERE project_id = $1 AND ($2::uuid IS NULL OR id > $2) ORDER BY id LIMIT $3",
            project_id,
            after,
            i64::from(limit),
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Read the latest committed native snapshot in this project's session.
    ///
    /// # Errors
    /// Returns a database failure; no visible checkpoint returns `None`.
    pub async fn latest_checkpoint(
        &self,
        project_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<Checkpoint>, Error> {
        Ok(sqlx::query_as!(
            Checkpoint,
            "SELECT c.id, c.session_id, c.revision, c.runner, c.runner_version,
                    c.format, c.format_version, c.native_session_id, c.payload, c.created_at
             FROM workbench_checkpoints c JOIN workbench_sessions s ON s.id = c.session_id
             WHERE s.project_id = $1 AND s.id = $2 ORDER BY c.revision DESC LIMIT 1",
            project_id,
            session_id,
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Read an earlier checkpoint without changing the session's current revision.
    ///
    /// # Errors
    /// Returns a database failure; an absent or differently scoped ID is `None`.
    pub async fn checkpoint(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Checkpoint>, Error> {
        Ok(sqlx::query_as!(
            Checkpoint,
            "SELECT c.id, c.session_id, c.revision, c.runner, c.runner_version,
                    c.format, c.format_version, c.native_session_id, c.payload, c.created_at
             FROM workbench_checkpoints c JOIN workbench_sessions s ON s.id = c.session_id
             WHERE s.project_id = $1 AND s.id = $2 AND c.id = $3",
            project_id,
            session_id,
            id,
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Append exact native bytes and advance the storage revision atomically.
    ///
    /// Reuse `id`, `expected_revision` and identical state after a lost response.
    /// That retry returns the original checkpoint, even after newer commits.
    /// Competing writers at one revision cannot both commit. This is storage
    /// concurrency control, not an execution lease or permission to run tools.
    ///
    /// # Errors
    /// Invalid state, absent/scoped-out session, ID/revision conflict or database
    /// failure. Failed appends do not advance the session revision.
    pub async fn save_checkpoint(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        id: Uuid,
        expected_revision: i64,
        state: &NativeState,
    ) -> Result<Checkpoint, Error> {
        state.validate()?;
        let revision = expected_revision
            .checked_add(1)
            .filter(|_| expected_revision >= 0)
            .ok_or(Error::InvalidInput("invalid expected revision"))?;
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_scalar!(
            "SELECT revision FROM workbench_sessions WHERE project_id = $1 AND id = $2 FOR UPDATE",
            project_id,
            session_id,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound("session"))?;

        let existing = sqlx::query_as!(
            Checkpoint,
            "SELECT id, session_id, revision, runner, runner_version, format,
                    format_version, native_session_id, payload, created_at
             FROM workbench_checkpoints WHERE id = $1",
            id,
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            if existing.session_id != session_id
                || existing.revision != revision
                || !existing.matches(state)
            {
                return Err(Error::IdConflict);
            }
            tx.commit().await?;
            return Ok(existing);
        }
        if current != expected_revision {
            return Err(Error::RevisionConflict {
                expected: expected_revision,
                actual: current,
            });
        }
        sqlx::query!(
            "UPDATE workbench_sessions SET revision = $1 WHERE id = $2",
            revision,
            session_id,
        )
        .execute(&mut *tx)
        .await?;
        let inserted = sqlx::query_as!(
            Checkpoint,
            "INSERT INTO workbench_checkpoints
                (id, session_id, revision, runner, runner_version, format, format_version, native_session_id, payload)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING id, session_id, revision, runner, runner_version, format,
                       format_version, native_session_id, payload, created_at",
            id,
            session_id,
            revision,
            state.runner,
            state.runner_version,
            state.format,
            state.format_version,
            state.native_session_id,
            state.payload,
        )
        .fetch_one(&mut *tx)
        .await;
        let checkpoint = inserted.map_err(|error| {
            if error
                .as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
            {
                Error::IdConflict
            } else {
                error.into()
            }
        })?;
        tx.commit().await?;
        Ok(checkpoint)
    }
}

//! Real Postgres tests, explicitly run by `just db-test`. Each `SQLx` test gets
//! its own database; ordinary workspace tests compile these but do not need PG.

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use sqlx::PgPool;
use uuid::Uuid;
use workbench_store::{Error, NativeState, Store};

fn state(payload: Vec<u8>) -> NativeState {
    NativeState {
        runner: "fixture-runner".into(),
        runner_version: "test-revision-1".into(),
        format: "opaque-test".into(),
        format_version: "1".into(),
        native_session_id: "native-session-1".into(),
        payload,
    }
}

async fn setup(pool: PgPool) -> (Store, Uuid, Uuid) {
    let store = Store::from_pool(pool);
    let project = Uuid::new_v4();
    let session = Uuid::new_v4();
    store
        .create_project(project, "Research project")
        .await
        .unwrap();
    store
        .create_session(project, session, "Research session")
        .await
        .unwrap();
    (store, project, session)
}

#[sqlx::test(migrations = false)]
#[ignore = "requires local Postgres; run just db-test"]
async fn migrations_are_explicit_and_repeatable(pool: PgPool) {
    let store = Store::from_pool(pool.clone());
    store.health().await.unwrap();
    let absent: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('workbench_projects')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(absent, None);
    store.migrate().await.unwrap();
    let id = Uuid::new_v4();
    let project = store
        .create_project(id, "Survives migration retry")
        .await
        .unwrap();
    store.migrate().await.unwrap();
    assert_eq!(store.project(id).await.unwrap(), Some(project));
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn creation_scoping_and_pagination(pool: PgPool) {
    let (store, project, session) = setup(pool).await;
    let original = store.project(project).await.unwrap().unwrap();
    assert_eq!(
        store
            .create_project(project, "Research project")
            .await
            .unwrap(),
        original
    );
    assert!(matches!(
        store.create_project(project, "Different").await,
        Err(Error::IdConflict)
    ));
    let original_session = store.session(project, session).await.unwrap().unwrap();
    assert_eq!(
        store
            .create_session(project, session, "Research session")
            .await
            .unwrap(),
        original_session
    );
    assert!(matches!(
        store.create_session(project, session, "Different").await,
        Err(Error::IdConflict)
    ));

    let other = Uuid::new_v4();
    store
        .create_project(other, "Another project")
        .await
        .unwrap();
    assert_eq!(store.session(other, session).await.unwrap(), None);
    assert!(matches!(
        store
            .create_session(other, session, "Research session")
            .await,
        Err(Error::IdConflict)
    ));
    assert!(matches!(
        store
            .create_session(Uuid::new_v4(), Uuid::new_v4(), "Missing parent")
            .await,
        Err(Error::NotFound("project"))
    ));

    let second = Uuid::new_v4();
    store
        .create_session(project, second, "Second session")
        .await
        .unwrap();
    let page = store.sessions(project, None, 1).await.unwrap();
    assert_eq!(page.len(), 1);
    let next = store.sessions(project, Some(page[0].id), 1).await.unwrap();
    assert_eq!(next.len(), 1);
    assert_ne!(page[0].id, next[0].id);
    assert!(
        store
            .sessions(project, Some(next[0].id), 1)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(store.sessions(other, None, 100).await.unwrap().is_empty());
    assert!(matches!(
        store.sessions(project, None, 0).await,
        Err(Error::InvalidInput(_))
    ));
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn retry_history_and_reconnect(pool: PgPool) {
    let options = pool.connect_options();
    let (store, project, session) = setup(pool).await;
    let id = Uuid::new_v4();
    // Deliberately retain JSONL whitespace, duplicate keys, and byte ordering.
    let first = state(b"{\"id\":\"a\",\"x\":1,\"x\":2}\n \n".to_vec());
    let saved = store
        .save_checkpoint(project, session, id, 0, &first)
        .await
        .unwrap();
    assert_eq!(saved.revision, 1);
    let mut newer = first.clone();
    newer.runner_version = "test-revision-2".into();
    newer.payload.extend_from_slice(b"{\"parentId\":\"a\"}\n");
    let second = store
        .save_checkpoint(project, session, Uuid::new_v4(), 1, &newer)
        .await
        .unwrap();
    // A delayed retry returns the original commit, without rewinding the head.
    assert_eq!(
        store
            .save_checkpoint(project, session, id, 0, &first)
            .await
            .unwrap(),
        saved
    );
    assert!(matches!(
        store.save_checkpoint(project, session, id, 0, &newer).await,
        Err(Error::IdConflict)
    ));
    let mut different_format = first.clone();
    different_format.format_version = "2".into();
    assert!(matches!(
        store
            .save_checkpoint(project, session, id, 0, &different_format)
            .await,
        Err(Error::IdConflict)
    ));
    assert!(matches!(
        store
            .save_checkpoint(project, session, Uuid::new_v4(), 0, &first)
            .await,
        Err(Error::RevisionConflict {
            expected: 0,
            actual: 2
        })
    ));

    store.close().await;
    let reopened = sqlx::postgres::PgPoolOptions::new()
        .connect_with((*options).clone())
        .await
        .unwrap();
    let reopened = Store::from_pool(reopened);
    assert_eq!(
        reopened.latest_checkpoint(project, session).await.unwrap(),
        Some(second)
    );
    assert_eq!(
        reopened.checkpoint(project, session, id).await.unwrap(),
        Some(saved)
    );
    assert_eq!(
        reopened
            .session(project, session)
            .await
            .unwrap()
            .unwrap()
            .revision,
        2
    );
    assert_eq!(
        reopened
            .latest_checkpoint(Uuid::new_v4(), session)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        reopened
            .checkpoint(project, Uuid::new_v4(), id)
            .await
            .unwrap(),
        None
    );
    assert!(matches!(
        reopened
            .save_checkpoint(Uuid::new_v4(), session, Uuid::new_v4(), 2, &first)
            .await,
        Err(Error::NotFound("session"))
    ));
    reopened.close().await;
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn competing_writers_and_duplicate_requests(pool: PgPool) {
    let (store, project, session) = setup(pool.clone()).await;
    let state = state(vec![1, 2, 3]);
    let (a, b) = tokio::join!(
        store.save_checkpoint(project, session, Uuid::new_v4(), 0, &state),
        store.save_checkpoint(project, session, Uuid::new_v4(), 0, &state),
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let failure = a.err().or_else(|| b.err()).unwrap();
    assert!(matches!(
        failure,
        Error::RevisionConflict {
            expected: 0,
            actual: 1
        }
    ));

    let retry_id = Uuid::new_v4();
    let (a, b) = tokio::join!(
        store.save_checkpoint(project, session, retry_id, 1, &state),
        store.save_checkpoint(project, session, retry_id, 1, &state),
    );
    assert_eq!(a.unwrap(), b.unwrap());
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM workbench_checkpoints WHERE session_id = $1")
            .bind(session)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn failed_insert_rolls_back_revision(pool: PgPool) {
    let (store, project, session) = setup(pool.clone()).await;
    // Force failure after the transactional head update, at the payload insert.
    sqlx::query("ALTER TABLE workbench_checkpoints ADD CHECK (format <> 'reject')")
        .execute(&pool)
        .await
        .unwrap();
    let mut rejected = state(vec![1]);
    rejected.format = "reject".into();
    assert!(matches!(
        store
            .save_checkpoint(project, session, Uuid::new_v4(), 0, &rejected)
            .await,
        Err(Error::Database(_))
    ));
    assert_eq!(
        store
            .session(project, session)
            .await
            .unwrap()
            .unwrap()
            .revision,
        0
    );
    assert_eq!(
        store.latest_checkpoint(project, session).await.unwrap(),
        None
    );
    let valid = state(vec![2]);
    assert_eq!(
        store
            .save_checkpoint(project, session, Uuid::new_v4(), 0, &valid)
            .await
            .unwrap()
            .revision,
        1
    );

    let other = Uuid::new_v4();
    store
        .create_session(project, other, "Other session")
        .await
        .unwrap();
    let used = store
        .latest_checkpoint(project, session)
        .await
        .unwrap()
        .unwrap()
        .id;
    assert!(matches!(
        store.save_checkpoint(project, other, used, 0, &valid).await,
        Err(Error::IdConflict)
    ));
    assert_eq!(
        store
            .session(project, other)
            .await
            .unwrap()
            .unwrap()
            .revision,
        0
    );
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn arbitrary_native_bytes_roundtrip(pool: PgPool) {
    let (store, project, session) = setup(pool).await;
    let runtime = tokio::runtime::Handle::current();
    // Run proptest's synchronous shrinker outside the async runtime thread.
    tokio::task::spawn_blocking(move || {
        let mut runner = TestRunner::new(Config {
            cases: 32,
            ..Config::default()
        });
        runner
            .run(&proptest::collection::vec(any::<u8>(), 1..4096), |bytes| {
                let actual = runtime.block_on(async {
                    let revision = store
                        .session(project, session)
                        .await
                        .unwrap()
                        .unwrap()
                        .revision;
                    store
                        .save_checkpoint(
                            project,
                            session,
                            Uuid::new_v4(),
                            revision,
                            &state(bytes.clone()),
                        )
                        .await
                        .unwrap();
                    store
                        .latest_checkpoint(project, session)
                        .await
                        .unwrap()
                        .unwrap()
                        .payload
                });
                prop_assert_eq!(actual, bytes);
                Ok(())
            })
            .unwrap();
    })
    .await
    .unwrap();
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn invalid_input_cannot_advance_the_head(pool: PgPool) {
    let (store, project, session) = setup(pool).await;
    let valid = state(vec![0, 255]);
    for revision in [-1, i64::MAX] {
        assert!(matches!(
            store
                .save_checkpoint(project, session, Uuid::new_v4(), revision, &valid)
                .await,
            Err(Error::InvalidInput(_))
        ));
    }
    let mut bad_version = valid.clone();
    bad_version.runner_version.clear();
    let mut nul = valid;
    nul.native_session_id.push('\0');
    for rejected in [
        state(vec![]),
        state(vec![0; workbench_store::MAX_CHECKPOINT_BYTES + 1]),
        bad_version,
        nul,
    ] {
        assert!(matches!(
            store
                .save_checkpoint(project, session, Uuid::new_v4(), 0, &rejected)
                .await,
            Err(Error::InvalidInput(_))
        ));
    }
    assert_eq!(
        store
            .session(project, session)
            .await
            .unwrap()
            .unwrap()
            .revision,
        0
    );
    assert_eq!(
        store.latest_checkpoint(project, session).await.unwrap(),
        None
    );
}

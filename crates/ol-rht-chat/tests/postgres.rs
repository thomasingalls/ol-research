//! HTTP + real Postgres, isolated per test. No model or corpus required.

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn readiness_checks_schema_and_db_but_liveness_stays_independent(pool: PgPool) {
    let store = Store::from_pool(pool.clone());
    let api = Api::start(store.clone(), Uuid::new_v4()).await;
    let base = api.url.trim_end_matches("/v1/chat/sessions");
    assert_eq!(
        api.client
            .get(format!("{base}/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE _sqlx_migrations SET checksum = decode('00', 'hex')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        api.client
            .get(format!("{base}/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    store.close().await;
    assert_eq!(
        api.client
            .get(format!("{base}/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        api.client
            .get(format!("{base}/health"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

use axum::http::StatusCode;
use ol_rht_chat::Config;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
use workbench_store::{NativeState, Store};

struct Api {
    url: String,
    client: reqwest::Client,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Api {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Api {
    async fn start(store: Store, project: Uuid) -> Self {
        let router = ol_rht_chat::router_with_storage(
            Config::local("http://127.0.0.1:8081/v1", "test").unwrap(),
            store,
            project,
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/chat/sessions", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            url,
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            task,
        }
    }

    async fn get(&self, suffix: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{suffix}", self.url))
            .send()
            .await
            .unwrap()
    }

    async fn put(&self, suffix: &str, body: &Value) -> reqwest::Response {
        self.client
            .put(format!("{}{suffix}", self.url))
            .json(body)
            .send()
            .await
            .unwrap()
    }
}

async fn setup(pool: PgPool) -> (Store, Uuid, Api, Uuid) {
    let store = Store::from_pool(pool);
    let project = Uuid::new_v4();
    store.create_project(project, "Chat tests").await.unwrap();
    let api = Api::start(store.clone(), project).await;
    let session = Uuid::new_v4();
    let response = api
        .put(
            &format!("/{session}"),
            &json!({"title":"Test conversation"}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    (store, project, api, session)
}

fn node(id: &str, parent: Option<&str>, role: &str, content: &Value, status: &Value) -> Value {
    json!({"parent_id":parent,"message":{"id":id,"role":role,"content":content},
        "created_at":"2026-09-26T12:00:00.000Z","status":status,"model_message_id":null,"part_ids":[]})
}

fn history() -> Value {
    json!({"head_id":"a1","messages":[
        node("u1", None, "user", &json!([{"type":"text","text":"Find λ papers"}]), &Value::Null),
        node("a1", Some("u1"), "assistant", &json!([
            {"type":"reasoning","text":"Search first"},
            {"type":"tool_call","tool_call_id":"search1","call":{"name":"search","args":{"query":"λ","limit":5}},
             "result":{"name":"search","data":{"status":"ok","query":"λ","total":0,"hits":[],"answered":{"epoch":3,"batch":2,"at":"2026-09-26","sources":["pubmed"],"attribution":["NLM"]}}}},
            {"type":"text","text":"No results"}
        ]), &json!({"type":"complete","reason":"stop"})),
        node("a2", Some("u1"), "assistant", &json!([
            {"type":"tool_call","tool_call_id":"paper1","call":{"name":"get_paper","args":{"id":"ol:paper"}},"result":null}
        ]), &json!({"type":"incomplete","reason":"cancelled","error":null})),
        node("edited", None, "user", &json!([{"type":"text","text":"A different question"}]), &Value::Null),
        node("failed", Some("edited"), "assistant", &json!([]), &json!({"type":"incomplete","reason":"error","error":"Model unavailable"}))
    ]})
}

fn save(revision: i64, history: &Value) -> Value {
    json!({"checkpoint_id":Uuid::new_v4().to_string(),"expected_revision":revision,"history":history})
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn round_trip_after_api_restart_preserves_all_branches(pool: PgPool) {
    let (store, project, api, session) = setup(pool).await;
    let path = format!("/{session}/history");
    let blank = api.get(&path).await;
    assert_eq!(blank.headers()["cache-control"], "no-store");
    assert_eq!(
        blank.json::<Value>().await.unwrap(),
        json!({"revision":0,"history":{"head_id":null,"messages":[]}})
    );
    let tree = history();
    assert_eq!(
        api.put(&path, &save(0, &tree.clone())).await.status(),
        StatusCode::OK
    );
    drop(api);
    let restarted = Api::start(store.clone(), project).await;
    let restored = restarted.get(&path).await.json::<Value>().await.unwrap();
    assert_eq!(restored, json!({"revision":1,"history":tree}));
    let checkpoint = store
        .latest_checkpoint(project, session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.format, "ol-rht-chat-history");
    assert_eq!(checkpoint.format_version, "1");
    assert_eq!(checkpoint.native_session_id, session.to_string());
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn competing_writers_and_delayed_retries_do_not_overwrite(pool: PgPool) {
    let (_, _, api, session) = setup(pool).await;
    let path = format!("/{session}/history");
    let first = save(0, &history());
    let mut other_tree = history();
    other_tree["head_id"] = json!("a2");
    let other = save(0, &other_tree);
    let (a, b) = tokio::join!(api.put(&path, &first), api.put(&path, &other));
    let winner = if a.status() == StatusCode::OK {
        assert_eq!(b.status(), StatusCode::CONFLICT);
        &first
    } else {
        assert_eq!(a.status(), StatusCode::CONFLICT);
        assert_eq!(b.status(), StatusCode::OK);
        &other
    };
    let newest = save(1, &history());
    assert_eq!(api.put(&path, &newest).await.status(), StatusCode::OK);
    let delayed = api.put(&path, winner).await;
    assert_eq!(delayed.status(), StatusCode::OK);
    assert_eq!(delayed.json::<Value>().await.unwrap()["revision"], 1);
    let loaded = api.get(&path).await.json::<Value>().await.unwrap();
    assert_eq!(loaded["revision"], 2);
    assert_eq!(loaded["history"], newest["history"]);
    let mut changed = winner.clone();
    changed["history"]["messages"][0]["message"]["content"][0]["text"] = json!("changed retry");
    assert_eq!(
        api.put(&path, &changed).await.status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn creation_pagination_missing_records_and_project_scoping(pool: PgPool) {
    let (store, project, api, session) = setup(pool).await;
    assert_eq!(
        api.put(
            &format!("/{session}"),
            &json!({"title":"Test conversation"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        api.put(&format!("/{session}"), &json!({"title":"different"}))
            .await
            .status(),
        StatusCode::CONFLICT
    );
    for _ in 0..51 {
        store
            .create_session(project, Uuid::new_v4(), "More chats")
            .await
            .unwrap();
    }
    let page = api.get("").await.json::<Value>().await.unwrap();
    assert_eq!(page["sessions"].as_array().unwrap().len(), 50);
    let cursor = page["next_cursor"].as_str().unwrap();
    let tail = api
        .get(&format!("?after={cursor}"))
        .await
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(tail["sessions"].as_array().unwrap().len(), 2);
    assert!(tail["next_cursor"].is_null());
    assert_eq!(
        api.get("?after=invalid").await.status(),
        StatusCode::BAD_REQUEST
    );
    let other_project = Uuid::new_v4();
    store
        .create_project(other_project, "Other project")
        .await
        .unwrap();
    let other = Api::start(store, other_project).await;
    let path = format!("/{session}/history");
    assert_eq!(other.get(&path).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        other.put(&path, &save(0, &history())).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        other.get("").await.json::<Value>().await.unwrap()["sessions"],
        json!([])
    );
    assert_eq!(
        api.get("/not-a-uuid/history").await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        api.get(&format!("/{}/history", Uuid::new_v4()))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn malformed_and_oversized_history_never_advances_revision(pool: PgPool) {
    let (_, _, api, session) = setup(pool).await;
    let path = format!("/{session}/history");
    let mut cases = Vec::new();
    for (pointer, value) in [
        ("/head_id", json!("u1")), // would prune descendants on UI import
        ("/head_id", json!("missing")),
        ("/messages/0/parent_id", json!("a1")),
        ("/messages/1/message/id", json!("u1")),
        ("/messages/1/status/type", json!("running")),
        ("/messages/1/status", Value::Null),
        ("/messages/0/created_at", json!("bad date")),
        (
            "/messages/1/message/content/1/result/name",
            json!("get_paper"),
        ),
        ("/messages/1/message/content/1/call/args/limit", json!(99)),
    ] {
        let mut tree = history();
        *tree.pointer_mut(pointer).unwrap() = value;
        cases.push(tree);
    }
    for tree in cases {
        assert!(
            api.put(&path, &save(0, &tree))
                .await
                .status()
                .is_client_error()
        );
    }
    // History has its own limit, independent of the smaller model-request limit.
    let mut large = history();
    large["messages"][0]["message"]["content"][0]["text"] = json!("a".repeat(300 * 1024));
    assert_eq!(
        api.put(&path, &save(0, &large)).await.status(),
        StatusCode::OK
    );
    let mut oversized = history();
    oversized["messages"][0]["message"]["content"][0]["text"] = json!("a".repeat(2 * 1024 * 1024));
    assert_eq!(
        api.put(&path, &save(1, &oversized)).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        api.get(&path).await.json::<Value>().await.unwrap()["revision"],
        1
    );
}

#[sqlx::test(migrator = "workbench_store::MIGRATOR")]
#[ignore = "requires local Postgres; run just db-test"]
async fn unknown_formats_and_unavailable_database_fail_explicitly(pool: PgPool) {
    let (store, project, api, session) = setup(pool).await;
    let state = NativeState {
        runner: "future-agent".into(),
        runner_version: "1".into(),
        format: "native-session".into(),
        format_version: "2".into(),
        native_session_id: "native".into(),
        payload: b"opaque future state".to_vec(),
    };
    store
        .save_checkpoint(project, session, Uuid::new_v4(), 0, &state)
        .await
        .unwrap();
    let response = api.get(&format!("/{session}/history")).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("unsupported history format")
    );
    assert_eq!(
        api.put(&format!("/{session}/history"), &save(1, &history()))
            .await
            .status(),
        StatusCode::CONFLICT
    );
    store.close().await;
    let unavailable = api.get("").await;
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    let text = unavailable.text().await.unwrap();
    assert!(text.contains("storage is unavailable"));
    assert!(!text.contains("postgres://"));
}

-- Application storage. Native agent sessions remain opaque and versioned;
-- this schema does not attempt to reconstruct them from UI messages.
CREATE TABLE workbench_projects (
    id uuid PRIMARY KEY,
    title text NOT NULL CHECK (length(btrim(title)) BETWEEN 1 AND 512),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE workbench_sessions (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES workbench_projects(id),
    title text NOT NULL CHECK (length(btrim(title)) BETWEEN 1 AND 512),
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX workbench_sessions_project ON workbench_sessions(project_id, id);

CREATE TABLE workbench_checkpoints (
    id uuid PRIMARY KEY,
    session_id uuid NOT NULL REFERENCES workbench_sessions(id),
    revision bigint NOT NULL CHECK (revision > 0),
    runner text NOT NULL CHECK (length(btrim(runner)) BETWEEN 1 AND 256),
    runner_version text NOT NULL CHECK (length(btrim(runner_version)) BETWEEN 1 AND 256),
    format text NOT NULL CHECK (length(btrim(format)) BETWEEN 1 AND 256),
    format_version text NOT NULL CHECK (length(btrim(format_version)) BETWEEN 1 AND 256),
    native_session_id text NOT NULL CHECK (length(btrim(native_session_id)) BETWEEN 1 AND 256),
    payload bytea NOT NULL CHECK (octet_length(payload) BETWEEN 1 AND 8388608),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (session_id, revision)
);

-- No cascading deletes: removal/retention needs an explicit product policy.
-- Checkpoints are append-only through the Rust API; no payload rewrites.

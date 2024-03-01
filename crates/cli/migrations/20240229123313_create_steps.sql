CREATE TABLE steps (
    id TEXT PRIMARY KEY,
    pipeline_id TEXT,
    name TEXT,
    state TEXT,
    status TEXT,
    arg TEXT,
    created_at TIMESTAMP WITHOUT TIME ZONE,
    updated_at TIMESTAMP WITHOUT TIME ZONE,
    finished_at TIMESTAMP WITHOUT TIME ZONE,
    FOREIGN KEY(pipeline_id) REFERENCES pipelines(id)
)
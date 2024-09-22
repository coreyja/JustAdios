-- Add migration script here
CREATE TABLE
  Jobs (
    job_id UUID PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    payload JSONB NOT NULL,
    priority INT NOT NULL,
    run_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
    locked_at TIMESTAMPTZ,
    locked_by TEXT,
    context TEXT NOT NULL
  );

CREATE TABLE
  Crons (
    cron_id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    last_run_at TIMESTAMP
    WITH
      TIME ZONE NOT NULL,
      created_at TIMESTAMP
    WITH
      TIME ZONE NOT NULL,
      updated_at TIMESTAMP
    WITH
      TIME ZONE NOT NULL
  );

CREATE UNIQUE INDEX idx_crons_name ON Crons (name);

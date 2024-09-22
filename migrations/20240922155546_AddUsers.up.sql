-- Add up migration script here
CREATE TABLE
  Users (
    user_id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
    zoom_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_at TIMESTAMP
    WITH
      TIME ZONE NOT NULL,
      created_at TIMESTAMP
    WITH
      TIME ZONE NOT NULL DEFAULT now (),
      updated_at TIMESTAMP
    WITH
      TIME ZONE NOT NULL DEFAULT now ()
  );

CREATE UNIQUE INDEX ON Users (zoom_id);

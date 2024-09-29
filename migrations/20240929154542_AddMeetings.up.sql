-- Add up migration script here
CREATE TABLE
  meetings (
    meeting_id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
    user_id UUID NOT NULL REFERENCES Users (user_id),
    zoom_id TEXT NOT NULL,
    zoom_uuid TEXT NOT NULL,
    start_time TIMESTAMP
    WITH
      TIME ZONE NOT NULL,
      end_time TIMESTAMP
    WITH
      TIME ZONE NULL,
      created_at TIMESTAMP
    WITH
      TIME ZONE NOT NULL DEFAULT now (),
      updated_at TIMESTAMP
    WITH
      TIME ZONE NOT NULL DEFAULT now ()
  );

CREATE UNIQUE INDEX ON meetings (zoom_uuid);

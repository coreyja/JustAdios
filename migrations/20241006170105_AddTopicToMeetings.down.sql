-- Add down migration script here
ALTER TABLE Meetings
DROP COLUMN topic;

-- Add up migration script here
ALTER TABLE Meetings
ADD COLUMN topic TEXT NULL;

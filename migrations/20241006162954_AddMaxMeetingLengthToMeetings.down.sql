-- Add down migration script here
ALTER TABLE Meetings
DROP COLUMN max_meeting_length_minutes;

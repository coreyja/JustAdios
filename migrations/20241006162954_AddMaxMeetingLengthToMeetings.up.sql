-- Add up migration script here
ALTER TABLE Meetings
ADD COLUMN max_meeting_length_minutes INTEGER;

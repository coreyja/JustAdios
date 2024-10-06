-- Add up migration script here
ALTER TABLE Users
ADD COLUMN default_meeting_length_minutes INTEGER NULL;

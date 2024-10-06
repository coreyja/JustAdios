-- Add down migration script here
ALTER TABLE Users
DROP COLUMN default_meeting_length_minutes;

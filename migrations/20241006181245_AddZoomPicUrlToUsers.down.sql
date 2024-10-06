-- Add down migration script here
ALTER TABLE Users
DROP COLUMN zoom_pic_url;

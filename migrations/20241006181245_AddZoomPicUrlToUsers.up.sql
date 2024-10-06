-- Add up migration script here
ALTER TABLE Users
ADD COLUMN zoom_pic_url TEXT NULL;

# Todos

- [ ] Encrypt the Access and Refresh Tokens
- [ ] Add webhooks for when participants enter a meeting and use that for the start time for non-immediate meetings
  - This means we will have to make desicsions about when a meeting is "started". Does a single person joining kick off the timer, or is it when the second person joins?
  - This also means we will need to store this state in the DB

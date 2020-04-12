CREATE TABLE IF NOT EXISTS 'templates' (
  id INTEGER PRIMARY KEY,
  channel TEXT NOT NULL,
  command TEXT NOT NULL,
  template TEXT NOT NULL,
  UNIQUE(channel, command)
);

CREATE TABLE IF NOT EXISTS 'templates' (
  id INTEGER PRIMARY KEY,
  channel TEXT NOT NULL,
  command TEXT NOT NULL,
  template TEXT NOT NULL,
  UNIQUE(channel, command)
);

CREATE TABLE IF NOT EXISTS 'points' (
  id INTEGER PRIMARY KEY,
  channel TEXT NOT NULL,
  user_id INTEGER NOT NULL,
  points INTEGER NOT NULL DEFAULT 0,
  UNIQUE(channel, user_id)
);

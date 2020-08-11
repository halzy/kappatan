INSERT INTO points (channel, user_id, points) 
VALUES (?, ?, ?) 
ON CONFLICT (channel, user_id) DO 
UPDATE SET points=points + excluded.points;

INSERT INTO templates (channel, command, template) VALUES (?, ?, ?) ON CONFLICT (channel, command) DO UPDATE SET template=excluded.template;

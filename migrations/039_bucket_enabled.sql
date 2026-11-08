ALTER TABLE account ADD COLUMN bucket_enabled INTEGER NOT NULL DEFAULT 1
  CHECK (bucket_enabled IN (0, 1) AND (bucket_enabled = 1 OR account_type = 'bucket'));

CREATE TABLE account_asset_link (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  asset_id   INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  UNIQUE (account_id, asset_id)
);

CREATE INDEX idx_account_asset_link_account ON account_asset_link (account_id);
CREATE INDEX idx_account_asset_link_asset ON account_asset_link (asset_id);

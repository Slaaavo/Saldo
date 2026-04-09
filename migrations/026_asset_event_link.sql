-- Phase 6: asset-to-event link and system-generated flag
ALTER TABLE event ADD COLUMN linked_asset_id INTEGER DEFAULT NULL REFERENCES account(id);
ALTER TABLE event ADD COLUMN is_system_generated INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_event_linked_asset ON event(linked_asset_id);

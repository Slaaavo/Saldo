-- Migration 035: Remember which eKasa receipt an imported entry came from.
--
-- event.ekasa_receipt_ref holds one canonical reference per receipt: the receipt id for an
-- online receipt, the offline QR code for an offline one. Every leg of an imported split
-- group carries the same reference. It lives on event, not event_data: edits insert a new
-- event_data row that copies only the payload columns, so a column there would be lost on
-- the first edit.
--
-- Not UNIQUE: the user may import a receipt twice on purpose. Entries imported before this
-- migration have NULL, so only later imports can be recognised as duplicates.

ALTER TABLE event ADD COLUMN ekasa_receipt_ref TEXT DEFAULT NULL;

CREATE INDEX idx_event_ekasa_receipt_ref ON event (ekasa_receipt_ref) WHERE ekasa_receipt_ref IS NOT NULL;

-- Migration 003: add account_type column to account table
ALTER TABLE account ADD COLUMN account_type TEXT NOT NULL DEFAULT 'account';

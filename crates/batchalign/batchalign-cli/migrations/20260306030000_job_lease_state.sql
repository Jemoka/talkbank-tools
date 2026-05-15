ALTER TABLE jobs
    ADD COLUMN leased_by_node TEXT;

ALTER TABLE jobs
    ADD COLUMN lease_expires_at REAL;

ALTER TABLE jobs
    ADD COLUMN lease_heartbeat_at REAL;

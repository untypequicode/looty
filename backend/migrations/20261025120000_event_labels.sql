-- Étiquettes de tableau sur les événements de son calendrier (synchronisées en CATEGORIES).
CREATE TABLE IF NOT EXISTS event_labels (
    event_id UUID NOT NULL REFERENCES workspace_events(id) ON DELETE CASCADE,
    label_id UUID NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (event_id, label_id)
);
CREATE INDEX IF NOT EXISTS idx_event_labels_label ON event_labels(label_id);

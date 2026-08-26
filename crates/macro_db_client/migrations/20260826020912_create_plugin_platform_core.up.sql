CREATE TABLE plugin_releases (
    id UUID PRIMARY KEY,
    plugin_id TEXT NOT NULL CHECK (char_length(plugin_id) BETWEEN 3 AND 253),
    version TEXT NOT NULL CHECK (char_length(version) BETWEEN 1 AND 128),
    manifest JSONB NOT NULL CHECK (jsonb_typeof(manifest) = 'object'),
    manifest_ref TEXT NOT NULL CHECK (char_length(manifest_ref) BETWEEN 1 AND 2048),
    integrity_ref TEXT NOT NULL CHECK (char_length(integrity_ref) BETWEEN 1 AND 2048),
    provenance_ref TEXT NOT NULL CHECK (char_length(provenance_ref) BETWEEN 1 AND 2048),
    release_root_sha256 TEXT NOT NULL CHECK (release_root_sha256 ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE (plugin_id, version),
    UNIQUE (id, plugin_id)
);

CREATE FUNCTION reject_plugin_release_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'plugin releases are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER plugin_releases_immutable
BEFORE UPDATE OR DELETE ON plugin_releases
FOR EACH ROW EXECUTE FUNCTION reject_plugin_release_mutation();

CREATE TABLE plugin_installations (
    id UUID PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES "Project" (id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL,
    release_id UUID NOT NULL,
    installed_by_user_id TEXT NOT NULL REFERENCES "User" (id) ON DELETE RESTRICT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    grant_version BIGINT NOT NULL DEFAULT 1 CHECK (grant_version > 0),
    source TEXT NOT NULL DEFAULT 'local_sideload' CHECK (source = 'local_sideload'),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (project_id, plugin_id),
    FOREIGN KEY (release_id, plugin_id)
        REFERENCES plugin_releases (id, plugin_id) ON DELETE RESTRICT
);

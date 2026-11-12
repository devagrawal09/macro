-- The marketplace: HTML artifacts a person publishes for their team to
-- install. A listing's content lives in its versions, and an install follows
-- the newest minor version of the major it is on.
SET LOCAL lock_timeout = '5s';

CREATE TABLE IF NOT EXISTS marketplace_listing (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('artifact')),
    -- Who published it.
    user_id TEXT NOT NULL,
    -- The team that sees it. NULL is everyone: a listing Macro provides.
    team_id UUID REFERENCES team (id) ON DELETE CASCADE,
    -- The document its newest version was published from. A plain pointer,
    -- not a foreign key: a listing outlives its source document.
    source_document_id UUID NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    -- Unlisted: hidden from browsing, still served to those who installed it.
    removed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A document is published as one listing at a time, so repeating a publish
-- can't list it twice.
CREATE UNIQUE INDEX IF NOT EXISTS marketplace_listing_source_document_idx
    ON marketplace_listing (source_document_id)
    WHERE removed_at IS NULL;

CREATE INDEX IF NOT EXISTS marketplace_listing_team_id_idx
    ON marketplace_listing (team_id)
    WHERE removed_at IS NULL;

-- One published snapshot. Rows are only ever inserted.
CREATE TABLE IF NOT EXISTS marketplace_listing_version (
    listing_id UUID NOT NULL REFERENCES marketplace_listing (id) ON DELETE CASCADE,
    major INT NOT NULL CHECK (major >= 1),
    minor INT NOT NULL CHECK (minor >= 0),
    -- SHA-256 of `html`, as a page build names its source.
    source_hash TEXT NOT NULL,
    html TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (listing_id, major, minor)
);

-- A person's install of a listing: the major version they are on.
CREATE TABLE IF NOT EXISTS marketplace_install (
    listing_id UUID NOT NULL REFERENCES marketplace_listing (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    major INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (listing_id, user_id)
);

CREATE INDEX IF NOT EXISTS marketplace_install_user_id_idx
    ON marketplace_install (user_id);

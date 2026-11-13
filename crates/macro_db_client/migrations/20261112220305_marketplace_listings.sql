-- The marketplace: HTML artifacts and routines a person publishes for their
-- team to install. A listing's content lives in its versions, and an install
-- follows the newest minor version of the major it is on. A routine install
-- is a routine of the person's own, which minor versions update until they
-- edit it.
--
-- Deleting a user deletes their installs, and deletes the listings they
-- published along with everyone's installs of them: a listing has no owner
-- left to publish its versions or answer for it.
SET LOCAL lock_timeout = '5s';

CREATE TABLE IF NOT EXISTS marketplace_listing (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('artifact', 'routine')),
    -- Who published it.
    user_id TEXT NOT NULL REFERENCES "User" (id) ON DELETE CASCADE,
    -- The team that sees it. NULL is everyone: a listing Macro provides.
    team_id UUID REFERENCES team (id) ON DELETE CASCADE,
    -- What its versions are published from: a document for an artifact, a
    -- routine for a routine. A plain pointer, not a foreign key: a listing
    -- outlives its source.
    source_id UUID NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    -- Unlisted: hidden from browsing, still served to those who installed it.
    removed_at TIMESTAMPTZ,
    -- The listing its source was copied from, when its source is a recorded
    -- marketplace copy. Cleared when that listing is deleted.
    derived_from_listing_id UUID REFERENCES marketplace_listing (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- The target of each version's (listing_id, kind) key.
    UNIQUE (id, kind)
);

-- A source is published as one listing at a time, so repeating a publish
-- can't list it twice.
CREATE UNIQUE INDEX IF NOT EXISTS marketplace_listing_source_idx
    ON marketplace_listing (kind, source_id)
    WHERE removed_at IS NULL;

CREATE INDEX IF NOT EXISTS marketplace_listing_team_id_idx
    ON marketplace_listing (team_id)
    WHERE removed_at IS NULL;

-- Deleting a listing clears the lineage of those derived from it.
CREATE INDEX IF NOT EXISTS marketplace_listing_derived_from_idx
    ON marketplace_listing (derived_from_listing_id)
    WHERE derived_from_listing_id IS NOT NULL;

-- One published snapshot. Rows are only ever inserted.
CREATE TABLE IF NOT EXISTS marketplace_listing_version (
    listing_id UUID NOT NULL,
    -- The listing's kind, which decides the one content column it sets.
    kind TEXT NOT NULL,
    major INT NOT NULL CHECK (major >= 1),
    minor INT NOT NULL CHECK (minor >= 0),
    -- SHA-256 of the content: `html` as a page build names its source, or
    -- `routine` as canonical JSON.
    source_hash TEXT NOT NULL,
    -- An artifact's page.
    html TEXT,
    -- A routine's snapshot: name, description, trigger, task and variables.
    routine JSONB,
    -- What the publisher says changed, for installers to read.
    changelog TEXT CHECK (char_length(changelog) <= 2000),
    -- A routine version the server made major because its interface changed,
    -- whatever bump was asked.
    forced_major BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (listing_id, major, minor),
    FOREIGN KEY (listing_id, kind)
        REFERENCES marketplace_listing (id, kind) ON DELETE CASCADE,
    CHECK (
        (kind = 'artifact' AND html IS NOT NULL AND routine IS NULL)
        OR (kind = 'routine' AND routine IS NOT NULL AND html IS NULL)
    )
);

-- A person's install of a listing: the major version they are on, and the
-- minor when they pinned it.
--
-- A routine install also keeps the routine it created, the version that
-- routine was last filled in from, the person's fills and the hash of its
-- task as saved; a task that no longer hashes alike was edited by its person,
-- and the install detaches. Deleting the install or the listing leaves the
-- routine: it is the person's own.
CREATE TABLE IF NOT EXISTS marketplace_install (
    listing_id UUID NOT NULL REFERENCES marketplace_listing (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES "User" (id) ON DELETE CASCADE,
    major INT NOT NULL,
    -- The routine it created. A plain pointer: the routine is the person's
    -- to delete.
    routine_id UUID,
    -- The minor version the routine was last filled in from.
    routine_minor INT,
    -- What the person chose: their time zone and each variable's fill.
    routine_fills JSONB,
    -- SHA-256 of the routine's task as installed or last updated.
    routine_task_hash TEXT,
    -- A first routine install claimed the row and is creating the routine:
    -- only the claimer creates it. A claim with no routine after a few
    -- minutes was abandoned, and the next install takes it over.
    routine_claimed_at TIMESTAMPTZ,
    -- The person edited the routine; updates no longer reach it.
    detached_at TIMESTAMPTZ,
    -- The last update failed; the next minor version tries again.
    update_failed_at TIMESTAMPTZ,
    -- The person pinned the install to this minor of its major: a page keeps
    -- showing it and a routine gets no automatic updates. Taking a version
    -- or unpinning clears it.
    pinned_minor INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (listing_id, user_id),
    CHECK (
        (routine_id IS NULL AND routine_minor IS NULL AND routine_fills IS NULL
            AND routine_task_hash IS NULL)
        OR (routine_id IS NOT NULL AND routine_minor IS NOT NULL
            AND routine_fills IS NOT NULL AND routine_task_hash IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS marketplace_install_user_id_idx
    ON marketplace_install (user_id);

-- A copy a person made of what they installed: a new document from an
-- artifact's version, or their installed routine made their own. Publishing
-- a copy records the listing it came from as the new listing's
-- `derived_from_listing_id`.
CREATE TABLE IF NOT EXISTS marketplace_copy (
    kind TEXT NOT NULL CHECK (kind IN ('artifact', 'routine')),
    -- The new document, or the routine made one's own. A plain pointer: it
    -- is the person's to delete.
    copy_id UUID NOT NULL,
    listing_id UUID NOT NULL REFERENCES marketplace_listing (id) ON DELETE CASCADE,
    -- The version copied.
    major INT NOT NULL,
    minor INT NOT NULL,
    user_id TEXT NOT NULL REFERENCES "User" (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (kind, copy_id)
);

CREATE INDEX IF NOT EXISTS marketplace_copy_listing_id_idx
    ON marketplace_copy (listing_id);

CREATE INDEX IF NOT EXISTS marketplace_copy_user_id_idx
    ON marketplace_copy (user_id);

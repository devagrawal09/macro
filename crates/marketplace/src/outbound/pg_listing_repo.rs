//! PostgreSQL implementation of the [`ListingRepo`] port.

#[cfg(test)]
mod test;

use std::str::FromStr;

use chrono::{DateTime, Utc};
use macro_user_id::user_id::MacroUserIdStr;
use sqlx::PgPool;
use uuid::Uuid;

use scheduled_action::domain::templates::fill::RoutineFills;

use crate::domain::models::{
    AddedVersion, FollowingInstall, InstallUpdate, InstalledPage, InstalledRoutine, Listing,
    ListingContent, ListingCursor, ListingId, ListingKind, ListingOrigin, ListingPage,
    ListingQuery, ListingScope, ListingVersion, ListingsShown, NewCopy, NewListing, RoutineInstall,
    RoutineVersion, TaskHash, Version, VersionBump, VersionNotes,
};
use crate::domain::ports::ListingRepo;

/// Postgres-backed marketplace repository.
#[derive(Debug, Clone)]
pub struct PgListingRepo {
    pool: PgPool,
}

impl PgListingRepo {
    /// Create a repository backed by the provided pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Errors produced by the Postgres marketplace repository.
#[derive(Debug, thiserror::Error)]
pub enum ListingRepoErr {
    /// Underlying database error.
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    /// A stored publisher is not a parseable macro user id.
    #[error("invalid user id stored for a listing")]
    InvalidUserId,
    /// A stored kind is not one this code knows.
    #[error("unknown listing kind stored: {0}")]
    UnknownKind(String),
    /// A stored routine snapshot or fills don't read.
    #[error("invalid routine stored for a listing: {0}")]
    InvalidRoutine(#[from] serde_json::Error),
}

struct ListingRow {
    id: Uuid,
    kind: String,
    user_id: String,
    team_id: Option<Uuid>,
    source_id: Uuid,
    title: String,
    description: String,
    removed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    latest_major: i32,
    latest_minor: i32,
    installed_major: Option<i32>,
    installed_minor: Option<i32>,
    pinned: Option<bool>,
    installed_routine_id: Option<Uuid>,
    based_on_id: Option<Uuid>,
    based_on_title: Option<String>,
    detached: Option<bool>,
    update_failed: Option<bool>,
    install_count: i64,
}

impl TryFrom<ListingRow> for Listing {
    type Error = ListingRepoErr;

    fn try_from(row: ListingRow) -> Result<Self, Self::Error> {
        let kind =
            ListingKind::from_str(&row.kind).map_err(|_| ListingRepoErr::UnknownKind(row.kind))?;
        let publisher =
            MacroUserIdStr::try_from(row.user_id).map_err(|_| ListingRepoErr::InvalidUserId)?;
        let scope = match row.team_id {
            Some(team_id) => ListingScope::Team { team_id },
            None => ListingScope::Global,
        };

        Ok(Self {
            id: ListingId::from_uuid(row.id),
            kind,
            scope,
            publisher,
            source_id: row.source_id,
            title: row.title,
            description: row.description,
            latest_version: Version {
                major: row.latest_major,
                minor: row.latest_minor,
            },
            installed_major: row.installed_major,
            installed_version: row
                .installed_major
                .zip(row.installed_minor)
                .map(|(major, minor)| Version { major, minor }),
            pinned: row.pinned.unwrap_or(false),
            based_on: row
                .based_on_id
                .zip(row.based_on_title)
                .map(|(id, title)| ListingOrigin {
                    id: ListingId::from_uuid(id),
                    title,
                }),
            installed_routine: row.installed_routine_id.map(|routine_id| InstalledRoutine {
                routine_id,
                detached: row.detached.unwrap_or(false),
                update_failed: row.update_failed.unwrap_or(false),
            }),
            install_count: row.install_count,
            removed: row.removed_at.is_some(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Which of the listings a caller may see one read returns.
struct Selection {
    only: Option<Uuid>,
    installed_only: bool,
    listed_only: bool,
    kind: Option<ListingKind>,
    source_id: Option<Uuid>,
    /// An `ILIKE` pattern for the title or description.
    pattern: Option<String>,
    after: Option<ListingCursor>,
    limit: i64,
}

/// `search` as an `ILIKE` pattern that matches it literally: its wildcards
/// and Postgres's default escape character are escaped.
fn escape_like(search: &str) -> String {
    let mut escaped = String::with_capacity(search.len());
    for character in search.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

impl PgListingRepo {
    /// The listings a member of `team` may see, as `user` sees them, newest
    /// first: their team's and everyone's, leaving out unlisted ones `user`
    /// hasn't installed, narrowed by `selection`.
    async fn select(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        selection: Selection,
    ) -> Result<Vec<Listing>, ListingRepoErr> {
        // Newest first, then by id: a cursor resumes after the listing it
        // names, among those created at the same instant too.
        let rows = sqlx::query_as!(
            ListingRow,
            r#"
            SELECT
                l.id,
                l.kind,
                l.user_id,
                l.team_id,
                l.source_id,
                l.title,
                l.description,
                l.removed_at,
                l.created_at,
                l.updated_at,
                v.major AS "latest_major!",
                v.minor AS "latest_minor!",
                i.major AS "installed_major?",
                -- A routine is on the version it was last filled in from; a
                -- page shows the minor it is pinned to, or the newest of its
                -- major.
                COALESCE(i.routine_minor, i.pinned_minor, iv.minor) AS "installed_minor?",
                (i.pinned_minor IS NOT NULL) AS "pinned?",
                i.routine_id AS "installed_routine_id?",
                o.id AS "based_on_id?",
                o.title AS "based_on_title?",
                (i.detached_at IS NOT NULL) AS "detached?",
                (i.update_failed_at IS NOT NULL) AS "update_failed?",
                (
                    SELECT count(*)
                    FROM marketplace_install c
                    WHERE c.listing_id = l.id
                      AND (c.routine_claimed_at IS NULL OR c.routine_id IS NOT NULL)
                ) AS "install_count!"
            FROM marketplace_listing l
            CROSS JOIN LATERAL (
                SELECT major, minor
                FROM marketplace_listing_version
                WHERE listing_id = l.id
                ORDER BY major DESC, minor DESC
                LIMIT 1
            ) v
            -- A claim still creating its routine isn't an install yet.
            LEFT JOIN marketplace_install i
                ON i.listing_id = l.id
               AND i.user_id = $1
               AND (i.routine_claimed_at IS NULL OR i.routine_id IS NOT NULL)
            LEFT JOIN LATERAL (
                SELECT max(minor) AS minor
                FROM marketplace_listing_version
                WHERE listing_id = l.id AND major = i.major
            ) iv ON true
            -- The listing it was copied from, only when $1 may see it: their
            -- team's or everyone's, and listed or installed by them.
            LEFT JOIN marketplace_listing o
                ON o.id = l.derived_from_listing_id
               AND (o.team_id IS NULL OR o.team_id = $2)
               AND (
                   o.removed_at IS NULL
                   OR EXISTS (
                       SELECT 1
                       FROM marketplace_install oi
                       WHERE oi.listing_id = o.id
                         AND oi.user_id = $1
                         AND (oi.routine_claimed_at IS NULL OR oi.routine_id IS NOT NULL)
                   )
               )
            WHERE (l.team_id IS NULL OR l.team_id = $2)
              AND (l.removed_at IS NULL OR i.user_id IS NOT NULL)
              AND ($3::uuid IS NULL OR l.id = $3)
              AND (NOT $4::bool OR i.user_id IS NOT NULL)
              AND (NOT $5::bool OR l.removed_at IS NULL)
              AND ($6::text IS NULL OR l.kind = $6)
              AND ($7::uuid IS NULL OR l.source_id = $7)
              AND ($8::text IS NULL OR l.title ILIKE $8 OR l.description ILIKE $8)
              AND (
                  $9::timestamptz IS NULL
                  OR l.created_at < $9
                  OR (l.created_at = $9 AND l.id > $10)
              )
            ORDER BY l.created_at DESC, l.id
            LIMIT $11
            "#,
            user.as_ref(),
            team,
            selection.only,
            selection.installed_only,
            selection.listed_only,
            selection.kind.map(|kind| kind.to_string()),
            selection.source_id,
            selection.pattern,
            selection.after.map(|cursor| cursor.created_at),
            selection.after.map(|cursor| cursor.id.as_uuid()),
            selection.limit,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(Listing::try_from).collect()
    }
}

impl ListingRepo for PgListingRepo {
    type Err = ListingRepoErr;

    #[tracing::instrument(err, skip_all)]
    async fn create_listing(&self, listing: NewListing) -> Result<bool, Self::Err> {
        let mut transaction = self.pool.begin().await?;

        let inserted = sqlx::query!(
            r#"
            INSERT INTO marketplace_listing
                (id, kind, user_id, team_id, source_id, title, description,
                 derived_from_listing_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (kind, source_id) WHERE removed_at IS NULL DO NOTHING
            "#,
            listing.id.as_uuid(),
            listing.content.kind().to_string(),
            listing.publisher.as_ref(),
            listing.team_id,
            listing.source_id,
            listing.text.title,
            listing.text.description,
            listing.derived_from.map(|origin| origin.as_uuid()),
        )
        .execute(&mut *transaction)
        .await?;
        if inserted.rows_affected() == 0 {
            return Ok(false);
        }

        insert_version(
            &mut transaction,
            listing.id,
            Version::FIRST,
            &listing.content,
            &VersionNotes {
                changelog: listing.changelog,
                forced_major: false,
            },
        )
        .await?;

        transaction.commit().await?;
        Ok(true)
    }

    #[tracing::instrument(err, skip_all)]
    async fn listing(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        id: ListingId,
    ) -> Result<Option<Listing>, Self::Err> {
        let selection = Selection {
            only: Some(id.as_uuid()),
            installed_only: false,
            listed_only: false,
            kind: None,
            source_id: None,
            pattern: None,
            after: None,
            limit: 1,
        };
        let listings = self.select(user, team, selection).await?;
        Ok(listings.into_iter().next())
    }

    #[tracing::instrument(err, skip_all)]
    async fn listings(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        query: &ListingQuery,
    ) -> Result<ListingPage, Self::Err> {
        let limit = usize::from(query.limit);
        // One listing past the page says whether another page follows.
        let selection = Selection {
            only: None,
            installed_only: query.shown == ListingsShown::Installed,
            listed_only: query.shown == ListingsShown::Listed,
            kind: query.kind,
            source_id: query.source_id,
            pattern: query
                .search
                .as_deref()
                .map(|search| format!("%{}%", escape_like(search))),
            after: query.after,
            limit: i64::from(query.limit) + 1,
        };
        let mut listings = self.select(user, team, selection).await?;

        let next = if listings.len() > limit {
            listings.truncate(limit);
            listings.last().map(ListingCursor::after)
        } else {
            None
        };
        Ok(ListingPage { listings, next })
    }

    #[tracing::instrument(err, skip_all)]
    async fn add_version(
        &self,
        id: ListingId,
        bump: VersionBump,
        content: &ListingContent,
        notes: &VersionNotes,
    ) -> Result<AddedVersion, Self::Err> {
        let mut transaction = self.pool.begin().await?;

        // Versions of one listing are added one at a time.
        let listed = sqlx::query_scalar!(
            r#"
            SELECT id
            FROM marketplace_listing
            WHERE id = $1 AND removed_at IS NULL
            FOR UPDATE
            "#,
            id.as_uuid(),
        )
        .fetch_optional(&mut *transaction)
        .await?;
        if listed.is_none() {
            return Ok(AddedVersion::Missing);
        }

        let newest = sqlx::query!(
            r#"
            SELECT major, minor, source_hash
            FROM marketplace_listing_version
            WHERE listing_id = $1
            ORDER BY major DESC, minor DESC
            LIMIT 1
            "#,
            id.as_uuid(),
        )
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(newest) = newest else {
            return Ok(AddedVersion::Missing);
        };
        if newest.source_hash == content.source_hash().as_ref() {
            return Ok(AddedVersion::Unchanged);
        }

        let next = Version {
            major: newest.major,
            minor: newest.minor,
        }
        .next(bump);
        insert_version(&mut transaction, id, next, content, notes).await?;
        sqlx::query!(
            r#"UPDATE marketplace_listing SET updated_at = now() WHERE id = $1"#,
            id.as_uuid(),
        )
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(AddedVersion::Added(next))
    }

    #[tracing::instrument(err, skip_all)]
    async fn versions(&self, id: ListingId) -> Result<Vec<ListingVersion>, Self::Err> {
        let rows = sqlx::query!(
            r#"
            SELECT major, minor, changelog, forced_major, created_at
            FROM marketplace_listing_version
            WHERE listing_id = $1
            ORDER BY major DESC, minor DESC
            "#,
            id.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| ListingVersion {
                version: Version {
                    major: row.major,
                    minor: row.minor,
                },
                changelog: row.changelog,
                forced_major: row.forced_major,
                created_at: row.created_at,
            })
            .collect())
    }

    #[tracing::instrument(err, skip_all)]
    async fn routine_version(
        &self,
        id: ListingId,
        major: Option<i32>,
    ) -> Result<Option<RoutineVersion>, Self::Err> {
        let row = sqlx::query!(
            r#"
            SELECT major, minor, routine AS "routine!"
            FROM marketplace_listing_version
            WHERE listing_id = $1
              AND kind = 'routine'
              AND ($2::int IS NULL OR major = $2)
            ORDER BY major DESC, minor DESC
            LIMIT 1
            "#,
            id.as_uuid(),
            major,
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(RoutineVersion {
                version: Version {
                    major: row.major,
                    minor: row.minor,
                },
                snapshot: serde_json::from_value(row.routine)?,
            })
        })
        .transpose()
    }

    #[tracing::instrument(err, skip_all)]
    async fn routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> Result<Option<RoutineInstall>, Self::Err> {
        let row = sqlx::query!(
            r#"
            SELECT
                major,
                routine_minor AS "minor!",
                routine_id AS "routine_id!",
                routine_fills AS "fills!",
                routine_task_hash AS "task_hash!"
            FROM marketplace_install
            WHERE listing_id = $1 AND user_id = $2 AND routine_id IS NOT NULL
            "#,
            id.as_uuid(),
            user.as_ref(),
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(RoutineInstall {
                version: Version {
                    major: row.major,
                    minor: row.minor,
                },
                routine_id: row.routine_id,
                fills: serde_json::from_value(row.fills)?,
                task_hash: TaskHash(row.task_hash),
            })
        })
        .transpose()
    }

    #[tracing::instrument(err, skip_all)]
    async fn claim_routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        major: i32,
        stale_after_secs: u32,
    ) -> Result<bool, Self::Err> {
        // The row's insert decides between racing installs. A claim with no
        // routine past the timeout is taken over in the same statement, so
        // two late installs can't both take it.
        let claimed = sqlx::query_scalar!(
            r#"
            INSERT INTO marketplace_install (listing_id, user_id, major, routine_claimed_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (listing_id, user_id) DO UPDATE
            SET major = EXCLUDED.major,
                routine_claimed_at = now(),
                updated_at = now()
            WHERE marketplace_install.routine_id IS NULL
              AND marketplace_install.routine_claimed_at
                  < now() - make_interval(secs => $4::float8)
            RETURNING listing_id
            "#,
            id.as_uuid(),
            user.as_ref(),
            major,
            f64::from(stale_after_secs),
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(claimed.is_some())
    }

    #[tracing::instrument(err, skip_all)]
    async fn release_routine_claim(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> Result<(), Self::Err> {
        sqlx::query!(
            r#"
            DELETE FROM marketplace_install
            WHERE listing_id = $1 AND user_id = $2 AND routine_id IS NULL
              AND routine_claimed_at IS NOT NULL
            "#,
            id.as_uuid(),
            user.as_ref(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(err, skip_all)]
    async fn save_routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        install: &RoutineInstall,
    ) -> Result<(), Self::Err> {
        sqlx::query!(
            r#"
            INSERT INTO marketplace_install
                (listing_id, user_id, major, routine_id, routine_minor, routine_fills,
                 routine_task_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (listing_id, user_id) DO UPDATE
            SET major = EXCLUDED.major,
                routine_id = EXCLUDED.routine_id,
                routine_minor = EXCLUDED.routine_minor,
                routine_fills = EXCLUDED.routine_fills,
                routine_task_hash = EXCLUDED.routine_task_hash,
                routine_claimed_at = NULL,
                detached_at = NULL,
                update_failed_at = NULL,
                pinned_minor = NULL,
                updated_at = now()
            "#,
            id.as_uuid(),
            user.as_ref(),
            install.version.major,
            install.routine_id,
            install.version.minor,
            serde_json::to_value(&install.fills)?,
            install.task_hash.0,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(err, skip_all)]
    async fn installs_behind(
        &self,
        id: ListingId,
        version: Version,
    ) -> Result<Vec<FollowingInstall>, Self::Err> {
        let rows = sqlx::query!(
            r#"
            SELECT
                user_id,
                major,
                routine_minor AS "minor!",
                routine_id AS "routine_id!",
                routine_fills AS "fills!",
                routine_task_hash AS "task_hash!"
            FROM marketplace_install
            WHERE listing_id = $1
              AND major = $2
              AND routine_minor < $3
              AND routine_id IS NOT NULL
              AND detached_at IS NULL
              AND pinned_minor IS NULL
            "#,
            id.as_uuid(),
            version.major,
            version.minor,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let fills: RoutineFills = serde_json::from_value(row.fills)?;
                Ok(FollowingInstall {
                    user: MacroUserIdStr::try_from(row.user_id)
                        .map_err(|_| ListingRepoErr::InvalidUserId)?,
                    install: RoutineInstall {
                        version: Version {
                            major: row.major,
                            minor: row.minor,
                        },
                        routine_id: row.routine_id,
                        fills,
                        task_hash: TaskHash(row.task_hash),
                    },
                })
            })
            .collect()
    }

    #[tracing::instrument(err, skip_all)]
    async fn record_update(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        update: &InstallUpdate,
    ) -> Result<(), Self::Err> {
        match update {
            InstallUpdate::Updated { version, task_hash } => {
                sqlx::query!(
                    r#"
                    UPDATE marketplace_install
                    SET routine_minor = $3,
                        routine_task_hash = $4,
                        update_failed_at = NULL,
                        updated_at = now()
                    WHERE listing_id = $1 AND user_id = $2 AND major = $5
                    "#,
                    id.as_uuid(),
                    user.as_ref(),
                    version.minor,
                    task_hash.0,
                    version.major,
                )
                .execute(&self.pool)
                .await?;
            }
            InstallUpdate::Detached => {
                sqlx::query!(
                    r#"
                    UPDATE marketplace_install
                    SET detached_at = now(), update_failed_at = NULL, updated_at = now()
                    WHERE listing_id = $1 AND user_id = $2 AND detached_at IS NULL
                    "#,
                    id.as_uuid(),
                    user.as_ref(),
                )
                .execute(&self.pool)
                .await?;
            }
            InstallUpdate::Failed => {
                sqlx::query!(
                    r#"
                    UPDATE marketplace_install
                    SET update_failed_at = now(), updated_at = now()
                    WHERE listing_id = $1 AND user_id = $2
                    "#,
                    id.as_uuid(),
                    user.as_ref(),
                )
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    #[tracing::instrument(err, skip_all)]
    async fn set_pinned(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        pinned: bool,
    ) -> Result<bool, Self::Err> {
        // A pin holds the version the install has: its routine's, or the
        // newest minor of a page's major. Pinning again keeps the pin.
        let found = sqlx::query_scalar!(
            r#"
            UPDATE marketplace_install i
            SET pinned_minor = CASE
                    WHEN $3 THEN COALESCE(
                        i.pinned_minor,
                        i.routine_minor,
                        (
                            SELECT max(v.minor)
                            FROM marketplace_listing_version v
                            WHERE v.listing_id = i.listing_id AND v.major = i.major
                        )
                    )
                END,
                updated_at = now()
            WHERE i.listing_id = $1
              AND i.user_id = $2
              -- A claim still creating its routine isn't an install yet.
              AND (i.routine_claimed_at IS NULL OR i.routine_id IS NOT NULL)
            RETURNING i.listing_id
            "#,
            id.as_uuid(),
            user.as_ref(),
            pinned,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.is_some())
    }

    #[tracing::instrument(err, skip_all)]
    async fn record_copy(&self, copy: &NewCopy) -> Result<(), Self::Err> {
        sqlx::query!(
            r#"
            INSERT INTO marketplace_copy (kind, copy_id, listing_id, major, minor, user_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (kind, copy_id) DO NOTHING
            "#,
            copy.kind.to_string(),
            copy.copy_id,
            copy.listing_id.as_uuid(),
            copy.version.major,
            copy.version.minor,
            copy.user.as_ref(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(err, skip_all)]
    async fn copied_from(
        &self,
        kind: ListingKind,
        copy_id: Uuid,
    ) -> Result<Option<ListingId>, Self::Err> {
        let listing = sqlx::query_scalar!(
            r#"SELECT listing_id FROM marketplace_copy WHERE kind = $1 AND copy_id = $2"#,
            kind.to_string(),
            copy_id,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(listing.map(ListingId::from_uuid))
    }

    #[tracing::instrument(err, skip_all)]
    async fn remove_listing(&self, id: ListingId) -> Result<(), Self::Err> {
        sqlx::query!(
            r#"
            UPDATE marketplace_listing
            SET removed_at = now(), updated_at = now()
            WHERE id = $1 AND removed_at IS NULL
            "#,
            id.as_uuid(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(err, skip_all)]
    async fn install_latest(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> Result<(), Self::Err> {
        sqlx::query!(
            r#"
            INSERT INTO marketplace_install (listing_id, user_id, major)
            SELECT $1, $2, max(major)
            FROM marketplace_listing_version
            WHERE listing_id = $1
            HAVING max(major) IS NOT NULL
            ON CONFLICT (listing_id, user_id)
            DO UPDATE SET major = EXCLUDED.major, pinned_minor = NULL, updated_at = now()
            "#,
            id.as_uuid(),
            user.as_ref(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(err, skip_all)]
    async fn uninstall(&self, id: ListingId, user: &MacroUserIdStr<'_>) -> Result<(), Self::Err> {
        sqlx::query!(
            r#"DELETE FROM marketplace_install WHERE listing_id = $1 AND user_id = $2"#,
            id.as_uuid(),
            user.as_ref(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(err, skip_all)]
    async fn installed_page(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
    ) -> Result<Option<InstalledPage>, Self::Err> {
        let row = sqlx::query!(
            r#"
            SELECT
                v.major AS "major!",
                v.minor AS "minor!",
                v.html AS "html!",
                (i.pinned_minor IS NOT NULL) AS "pinned!"
            FROM marketplace_install i
            JOIN marketplace_listing l ON l.id = i.listing_id
            CROSS JOIN LATERAL (
                SELECT major, minor, html
                FROM marketplace_listing_version
                WHERE listing_id = i.listing_id
                  AND major = i.major
                  AND (i.pinned_minor IS NULL OR minor = i.pinned_minor)
                ORDER BY minor DESC
                LIMIT 1
            ) v
            WHERE i.listing_id = $1
              AND i.user_id = $2
              AND (l.team_id IS NULL OR l.team_id = $3)
              AND l.kind = 'artifact'
            "#,
            id.as_uuid(),
            user.as_ref(),
            team,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| InstalledPage {
            version: Version {
                major: row.major,
                minor: row.minor,
            },
            html: row.html,
            pinned: row.pinned,
        }))
    }

    #[tracing::instrument(err, skip_all)]
    async fn installers(
        &self,
        id: ListingId,
        major: Option<i32>,
        limit: u16,
    ) -> Result<Vec<MacroUserIdStr<'static>>, Self::Err> {
        // Someone who left the listing's team keeps their install row but
        // can no longer open it, so they aren't told of its versions.
        let users = sqlx::query_scalar!(
            r#"
            SELECT i.user_id
            FROM marketplace_install i
            JOIN marketplace_listing l ON l.id = i.listing_id
            WHERE i.listing_id = $1
              AND ($2::int IS NULL OR i.major = $2)
              -- A claim still creating its routine isn't an install yet.
              AND (i.routine_claimed_at IS NULL OR i.routine_id IS NOT NULL)
              AND (
                  l.team_id IS NULL
                  OR EXISTS (
                      SELECT 1
                      FROM team_user t
                      WHERE t.team_id = l.team_id AND t.user_id = i.user_id
                  )
              )
            ORDER BY i.updated_at DESC, i.user_id
            LIMIT $3
            "#,
            id.as_uuid(),
            major,
            i64::from(limit),
        )
        .fetch_all(&self.pool)
        .await?;

        // One unreadable id never keeps the others from hearing of it.
        Ok(users
            .into_iter()
            .filter_map(|user| {
                MacroUserIdStr::try_from(user)
                    .inspect_err(|error| {
                        tracing::warn!(error = ?error, "skipped an installer with an invalid id");
                    })
                    .ok()
            })
            .collect())
    }
}

/// Store one version of a listing, its content in the column for its kind.
async fn insert_version(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: ListingId,
    version: Version,
    content: &ListingContent,
    notes: &VersionNotes,
) -> Result<(), ListingRepoErr> {
    let (html, routine) = match content {
        ListingContent::Page(page) => (Some(page.html.as_str()), None),
        ListingContent::Routine(routine) => (None, Some(serde_json::to_value(&routine.snapshot)?)),
    };

    sqlx::query!(
        r#"
        INSERT INTO marketplace_listing_version
            (listing_id, kind, major, minor, source_hash, html, routine, changelog,
             forced_major)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
        id.as_uuid(),
        content.kind().to_string(),
        version.major,
        version.minor,
        content.source_hash().as_ref(),
        html,
        routine,
        notes.changelog.as_deref(),
        notes.forced_major,
    )
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

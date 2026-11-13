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
    ListingContent, ListingId, ListingKind, ListingScope, NewListing, RoutineInstall,
    RoutineVersion, TaskHash, Version, VersionBump,
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
    installed_routine_id: Option<Uuid>,
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

impl ListingRepo for PgListingRepo {
    type Err = ListingRepoErr;

    #[tracing::instrument(err, skip_all)]
    async fn create_listing(&self, listing: NewListing) -> Result<bool, Self::Err> {
        let mut transaction = self.pool.begin().await?;

        let inserted = sqlx::query!(
            r#"
            INSERT INTO marketplace_listing
                (id, kind, user_id, team_id, source_id, title, description)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (kind, source_id) WHERE removed_at IS NULL DO NOTHING
            "#,
            listing.id.as_uuid(),
            listing.content.kind().to_string(),
            listing.publisher.as_ref(),
            listing.team_id,
            listing.source_id,
            listing.text.title,
            listing.text.description,
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
        )
        .await?;

        transaction.commit().await?;
        Ok(true)
    }

    #[tracing::instrument(err, skip_all)]
    async fn listings(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        only: Option<ListingId>,
    ) -> Result<Vec<Listing>, Self::Err> {
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
                i.routine_id AS "installed_routine_id?",
                (i.detached_at IS NOT NULL) AS "detached?",
                (i.update_failed_at IS NOT NULL) AS "update_failed?",
                (
                    SELECT count(*)
                    FROM marketplace_install c
                    WHERE c.listing_id = l.id
                ) AS "install_count!"
            FROM marketplace_listing l
            CROSS JOIN LATERAL (
                SELECT major, minor
                FROM marketplace_listing_version
                WHERE listing_id = l.id
                ORDER BY major DESC, minor DESC
                LIMIT 1
            ) v
            LEFT JOIN marketplace_install i
                ON i.listing_id = l.id AND i.user_id = $1
            WHERE (l.team_id IS NULL OR l.team_id = $2)
              AND (l.removed_at IS NULL OR i.user_id IS NOT NULL)
              AND ($3::uuid IS NULL OR l.id = $3)
            ORDER BY l.created_at DESC, l.id
            "#,
            user.as_ref(),
            team,
            only.map(|id| id.as_uuid()),
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(Listing::try_from).collect()
    }

    #[tracing::instrument(err, skip_all)]
    async fn add_version(
        &self,
        id: ListingId,
        bump: VersionBump,
        content: &ListingContent,
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
        insert_version(&mut transaction, id, next, content).await?;
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
                detached_at = NULL,
                update_failed_at = NULL,
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
            DO UPDATE SET major = EXCLUDED.major, updated_at = now()
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
            SELECT v.major AS "major!", v.minor AS "minor!", v.html AS "html!"
            FROM marketplace_install i
            JOIN marketplace_listing l ON l.id = i.listing_id
            CROSS JOIN LATERAL (
                SELECT major, minor, html
                FROM marketplace_listing_version
                WHERE listing_id = i.listing_id AND major = i.major
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
        }))
    }
}

/// Store one version of a listing, its content in the column for its kind.
async fn insert_version(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: ListingId,
    version: Version,
    content: &ListingContent,
) -> Result<(), ListingRepoErr> {
    let (html, routine) = match content {
        ListingContent::Page(page) => (Some(page.html.as_str()), None),
        ListingContent::Routine(routine) => {
            (None, Some(serde_json::to_value(&routine.snapshot)?))
        }
    };

    sqlx::query!(
        r#"
        INSERT INTO marketplace_listing_version
            (listing_id, kind, major, minor, source_hash, html, routine)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        id.as_uuid(),
        content.kind().to_string(),
        version.major,
        version.minor,
        content.source_hash().as_ref(),
        html,
        routine,
    )
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

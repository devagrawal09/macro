DROP TABLE IF EXISTS plugin_installations;
DROP TRIGGER IF EXISTS plugin_releases_immutable ON plugin_releases;
DROP FUNCTION IF EXISTS reject_plugin_release_mutation();
DROP TABLE IF EXISTS plugin_releases;

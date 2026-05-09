-- Postgres LISTEN/NOTIFY trigger for workspace_nodes changes.
-- Fires on INSERT, UPDATE, DELETE and publishes a JSON payload to the
-- 'workspace_changes' channel so the RealtimeService can fan-out to WebSocket clients.

CREATE OR REPLACE FUNCTION notify_workspace_change() RETURNS trigger AS $$
BEGIN
  PERFORM pg_notify(
    'workspace_changes',
    json_build_object(
      'op',        TG_OP,
      'tenant_id', COALESCE(NEW.tenant_id, OLD.tenant_id),
      'node_id',   COALESCE(NEW.id, OLD.id),
      'kind',      COALESCE(NEW.kind, OLD.kind)
    )::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS workspace_change_notify ON workspace_nodes;
CREATE TRIGGER workspace_change_notify
  AFTER INSERT OR UPDATE OR DELETE ON workspace_nodes
  FOR EACH ROW EXECUTE FUNCTION notify_workspace_change();

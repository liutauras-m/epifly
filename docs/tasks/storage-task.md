
New capability workspace-tree with list, read_file, edit_file (anchored), write_file (with expected_etag), create_file, create_folder, move_node, rename_node, delete_node. ~1 day.
Fix the rename/move S3 key issue (copy+delete) in move_node handler. ~2 h.
Add grep_workspace backed by tantivy index updated from the same StorageEvent stream Phase 5 introduces. ~1 day.
Add If-Match: <etag> precondition on PATCH content + propagate to edit_file/write_file. ~half day.
Frontend diff + approval UI in ui. ~1–2 days.
Audit-log all agent mutations with actor=agent. ~2 h.

Update gateway construction to use the factory
In state.rs, replace direct calls to RedbThreadProjectionStore::new(...) and InMemoryThreadProjectionStore::new() with the factory.

Add contract tests
This is the highest-value part. Write shared tests that verify any ThreadProjectionStore implementation behaves the same:

resolve_or_create preserves node_id
second resolve does not overwrite folder_path
set_status works
update_folder_path works
get returns None for missing projections
missing projection updates return an error
Keep concrete store exports for now
Don’t immediately remove InMemoryThreadProjectionStore / RedbThreadProjectionStore from lib.rs. Other crates/tests currently use them. Once call sites mostly depend on the factory/trait, you can decide whether to narrow the public API.

My suggested implementation order:

Add factory types/functions in thread_projection.rs.
Re-export the factory and backend enum from store/mod.rs and lib.rs.
Replace construction in state.rs.
Add contract tests for InMemory first.
Add Redb contract tests using a temp database if the crate already has tempdir/dev-dependency support.
That gives you the useful design patterns: Repository, Adapter, Factory, and Contract Tests. Clean, boring, hard to misuse.
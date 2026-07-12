//! Real-world repository integration tests (run with --ignored)

use crate::{DaemonHarness, CommitAssertions};
use crate::real_world::sources::RealWorldRepo;
use std::time::Duration;

/// Test: axios - add interceptor creates feat commit
#[tokio::test]
#[ignore]
async fn test_axios_add_interceptor() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Clone axios
    let repo = RealWorldRepo::axios();
    repo.clone_into(temp_dir.path()).await.unwrap();
    repo.init_fresh_git(temp_dir.path()).unwrap();
    
    // Setup AVCS
    let mut harness = DaemonHarness::new_from_path(temp_dir.path().to_path_buf()).await.unwrap();
    harness.init_avcs(None).await.unwrap();
    harness.start_daemon().await.unwrap();
    
    // Add a new interceptor
    harness.write_file("lib/core/InterceptorManager.ts", r#"
export class InterceptorManager {
  private interceptors: any[] = [];
  
  use(interceptor: any) {
    this.interceptors.push(interceptor);
    return this;
  }
  
  // New feature: intercept with condition
  useWhen(condition: Function, interceptor: any) {
    this.interceptors.push({ condition, interceptor });
    return this;
  }
}
"#).unwrap();
    
    // Add test
    harness.write_file("test/InterceptorManager.test.ts", r#"
import { InterceptorManager } from '../lib/core/InterceptorManager';

test('useWhen adds conditional interceptor', () => {
  const manager = new InterceptorManager();
  manager.useWhen(() => true, () => {});
  expect(manager['interceptors'].length).toBe(1);
});
"#).unwrap();
    
    // Wait for commit
    let commit = harness.wait_for_commit(Duration::from_secs(60)).await.unwrap();
    
    // Verify
    commit.assert_that()
        .has_type("feat")
        .includes_files(&["lib/core/InterceptorManager.ts"]);
    
    harness.stop_daemon().await.unwrap();
}

/// Test: fastify - fix route handling
#[tokio::test]
#[ignore]
async fn test_fastify_fix_route() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    let repo = RealWorldRepo::fastify();
    repo.clone_into(temp_dir.path()).await.unwrap();
    repo.init_fresh_git(temp_dir.path()).unwrap();
    
    let mut harness = DaemonHarness::new_from_path(temp_dir.path().to_path_buf()).await.unwrap();
    harness.init_avcs(None).await.unwrap();
    harness.start_daemon().await.unwrap();
    
    // Add a route fix
    harness.write_file("lib/router.ts", r#"
// Fix: handle async route handlers correctly
export async function handleRoute(req: any, res: any) {
  try {
    await req.handler(req, res);
  } catch (err) {
    res.status(500).send(err.message);
  }
}
"#).unwrap();
    
    let commit = harness.wait_for_commit(Duration::from_secs(60)).await.unwrap();
    
    commit.assert_that()
        .has_type("fix");
    
    harness.stop_daemon().await.unwrap();
}

/// Test: trpc - update documentation
#[tokio::test]
#[ignore]
async fn test_trpc_update_docs() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    let repo = RealWorldRepo::trpc();
    repo.clone_into(temp_dir.path()).await.unwrap();
    repo.init_fresh_git(temp_dir.path()).unwrap();
    
    let mut harness = DaemonHarness::new_from_path(temp_dir.path().to_path_buf()).await.unwrap();
    harness.init_avcs(None).await.unwrap();
    harness.start_daemon().await.unwrap();
    
    // Update docs
    harness.write_file("docs/api-reference.md", r#"
# API Reference

## createTRPCClient

Updated with new examples.
"#).unwrap();
    
    let commit = harness.wait_for_commit(Duration::from_secs(60)).await.unwrap();
    
    commit.assert_that()
        .has_type("docs")
        .includes_files(&["docs/api-reference.md"]);
    
    harness.stop_daemon().await.unwrap();
}
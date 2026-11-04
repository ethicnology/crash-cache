use crash_cache::shared::persistence::{establish_connection_pool,run_migrations,Repositories};
use std::sync::{Arc,Barrier};

#[test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
fn concurrent_issue_occurrences_keep_an_exact_count() {
    let url = std::env::var("DATABASE_URL").expect("isolated DATABASE_URL required");
    assert!(url.contains("crash_cache_test"));
    let pool = establish_connection_pool(&url, 8, 10);
    run_migrations(&pool);
    let repos = Repositories::new(pool.clone());
    let fingerprint = uuid::Uuid::new_v4().to_string();
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (pool, repos, fingerprint, barrier) = (
                pool.clone(),
                repos.clone(),
                fingerprint.clone(),
                barrier.clone(),
            );
            std::thread::spawn(move || {
                let mut conn = pool.get().unwrap();
                barrier.wait();
                repos
                    .issue
                    .get_or_create(&mut conn, &fingerprint, None, None)
                    .unwrap()
            })
        })
        .collect();
    let ids: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert!(ids.iter().all(|id| *id == ids[0]));
    let issue = repos
        .issue
        .find_by_fingerprint(&fingerprint)
        .unwrap()
        .unwrap();
    assert_eq!(issue.event_count, 8);
}

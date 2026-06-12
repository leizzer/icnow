use icnow::open_db_graph;
use std::thread;

#[test]
fn test_race() {
    let mut handles = vec![];
    for _ in 0..3 {
        handles.push(thread::spawn(|| {
            let _ = open_db_graph("/Users/cristian/projects/dgapp_bkp/knowledge.db");
        }));
    }
    for h in handles {
        let _ = h.join();
    }
}

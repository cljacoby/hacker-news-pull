use hacker_news::client::html_client::Client;
use hacker_news::model::Listing;
use hacker_news::model::Id;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use rusqlite::Transaction;
use rusqlite::TransactionBehavior;
use std::env;
use tokio::sync::Mutex;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time;
use std::sync::Arc;
use std::error::Error;


fn db_create(path: &Path) -> Connection {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .unwrap();
    conn.execute(
        r#"CREATE TABLE Listings (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            score INTEGER,
            user TEXT,
            url TEXT NOT NULL)"#,
        [],
    )
    .unwrap();

    conn
}

fn db_open(path: &Path) -> Connection {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();

    conn
}

fn db_insert(conn: &mut Connection, listings: &Vec<Listing>) {
    let tx = Transaction::new(conn, TransactionBehavior::Deferred).unwrap();
    for l in listings.iter() {
        tx.execute(
            r#"INSERT OR REPLACE INTO Listings (
                id,
                title,
                score,
                user,
                url)
            VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![l.id, l.title, l.score, l.user, l.url],
        )
        .unwrap();
    }

    tx.commit().unwrap();
}

fn db_query_ids(conn: &mut Connection) -> Result<Vec<Id>, Box<dyn Error>> {
    let mut stmt = conn.prepare("SELECT id FROM Listings")?;
    let mut rows = stmt.query([])?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next()? {
        ids.push(row.get(0)?);
    }

    Ok(ids)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>>{
    env_logger::init();
    
    let mut db_path = PathBuf::from(env::var("HOME").unwrap());
    db_path.push("hn.db");
    let db_path = Arc::new(db_path);
    let queue: Arc<Mutex<VecDeque<Id>>> = Arc::new(Mutex::new(VecDeque::new()));

    let db_path_t1 = db_path.clone();
    let jh_new_threads = tokio::spawn(async move {
        let interval = time::Duration::from_secs(1);
        let mut i = 0;
        let mut conn = match db_path_t1.exists() {
            true => db_open(db_path_t1.as_path()),
            false => db_create(db_path_t1.as_path()),
        };
        loop {
            thread::sleep(interval);
            i += 1;
            let client = Client::new();
            let listings = match client.newest() {
                Ok(list) => list,
                Err(err) => {
                    log::warn!("Client::listings got error response: {}", err);
                    continue;
                }
            };
            db_insert(&mut conn, &listings);
            log::debug!("New posts thread refresh {}. Got posts and inserted to db", i);
        }
    });

    let queue_t2 = queue.clone();
    let db_path_t2 = db_path.clone();
    let jh_update_queue = tokio::spawn(async move {
        let interval = time::Duration::from_secs(5);
        let mut i = 0;
        let mut conn = match db_path_t2.exists() {
            true => db_open(db_path_t2.as_path()),
            false => db_create(db_path_t2.as_path()),
        };
        loop {
            thread::sleep(interval);
            i += 1;
            let ids = match db_query_ids(&mut conn) {
                Ok(ids) => ids,
                Err(err) => {
                    log::warn!("db_query_ids returned error: {:?}", err);
                    continue;
                }
            };
            let mut q = queue_t2.lock().await;
            for id in ids {
                q.push_back(id);
            }
            log::debug!("Refreshed update queue {}", i);
        }
    });

    let queue_t3 = queue.clone();
    let db_path_t3 = db_path.clone();
    let jq_update_worker = tokio::spawn(async move {
        let interval = time::Duration::from_secs(0);
        let mut i = 0;
        let mut conn = match db_path_t3.exists() {
            true => db_open(db_path_t3.as_path()),
            false => db_create(db_path_t3.as_path()),
        };
        loop {
            thread::sleep(interval);
            i += 1;
            let mut update_id = None;
            {
                let mut q = queue_t3.lock().await;
                match q.pop_front() {
                    Some(id) => update_id = Some(id),
                    None => {
                        log::debug!("Attempted to update id from queue, but queue was empty");
                        continue;
                    }
                }
            }
            log::debug!("Refreshed update queue {}", i);
        }
    });


    jh_new_threads.await?;
    jh_update_queue.await?;

    Ok(())


}

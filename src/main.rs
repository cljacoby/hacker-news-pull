use hacker_news::client::html_client::Client;
use hacker_news::model::Listing;
use rusqlite::Connection;
use rusqlite::Transaction;
use rusqlite::TransactionBehavior;
use rusqlite::OpenFlags;
use rusqlite::params;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time;

fn db_create(path: &Path) -> Connection {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE Listings (id INTEGER PRIMARY KEY, title TEXT NOT NULL)",
        [],
    ).unwrap();

    conn
}

fn db_open(path: &Path) -> Connection {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .unwrap();

    conn
}

fn db_insert_many(conn: &mut Connection, listings: &Vec<Listing>) {
    let tx = Transaction::new(conn, TransactionBehavior::Deferred).unwrap();
    
    for listing in listings.iter() {
        tx.execute("INSERT OR REPLACE INTO Listings (id, title) VALUES (?1, ?2)",
            params![listing.id, listing.title.clone()]
        ).unwrap();
    }

    tx.commit().unwrap();
}


fn main() {
    let mut db_path = PathBuf::from(env::var("HOME").unwrap());
    db_path.push("hn.db");
    let mut conn = match db_path.exists() {
        true => db_open(db_path.as_path()),
        false => db_create(db_path.as_path())
    };
    let client = Client::new("", "");
    let interval = time::Duration::from_secs(1);
    let mut i = 0;

    loop {
        println!("Interval i={:?}", i);
        let listings = client.news().unwrap();
        db_insert_many(&mut conn, &listings);
        i += 1;
        thread::sleep(interval);
    }
}

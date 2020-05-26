use couch_hack_week::constants::*;
use couch_hack_week::fdb;
use foundationdb::tuple::{unpack, Bytes, Element};
use foundationdb::KeySelector;
use foundationdb::{RangeOption, Transaction};
use std::*;
use tokio::runtime::Runtime;
use foundationdb::future::FdbValue;

#[derive(Clone)]
struct Database {
    name: String,
    db_prefix: Vec<u8>,
}

impl Database {
    fn new(name: &Bytes, db_prefix: &[u8]) -> Database {
        Database {
            name: String::from_utf8_lossy(name.as_ref()).into(),
            db_prefix: db_prefix.to_vec(),
        }
    }
}

#[derive(Clone)]
struct Row {
    id: String,
    key: String,
    value: String
}

impl From<FdbValue> for Row {
    fn from(kv: FdbValue) -> Self {
        println!("kb {:?}", kv);
        let (_, _, raw_key): (Element, Element, Bytes) = unpack(kv.key()).unwrap();
        let key: String = String::from_utf8_lossy(raw_key.as_ref()).into();
        let value: String = String::from_utf8_lossy(kv.value()).into();

        Row {
            id: key.clone(),
            key,
            value
        }
    }
}

async fn get_dbs(trx: &Transaction) -> Result<Vec<Database>, Box<dyn std::error::Error>> {
    let couch_directory = trx.get(COUCHDB_PREFIX, false).await.unwrap().unwrap();

    let (start, end) = fdb::pack_range(&ALL_DBS, couch_directory.as_ref());

    let start_key = KeySelector::first_greater_or_equal(start);
    let end_key = KeySelector::first_greater_than(end);
    let opts = RangeOption {
        mode: foundationdb::options::StreamingMode::WantAll,
        limit: Some(5),
        ..RangeOption::from((start_key, end_key))
    };
    let iteration: usize = 1;
    let range = trx.get_range(&opts, iteration, false).await?;

    let dbs: Vec<Database> = range
        .iter()
        .map(|kv| {
            let (_, _, db_bytes): (Element, Element, Bytes) = unpack(kv.key()).unwrap();
            Database::new(&db_bytes, kv.value())
        })
        .collect();

    let db = dbs[3].clone();
    all_docs(trx, &db).await;
    Ok(dbs)
}

async fn all_docs(trx: &Transaction, db: &Database) -> Result<Vec<Row>, Box<dyn std::error::Error>> {

    let (start, end) = fdb::pack_range(&DB_ALL_DOCS, db.db_prefix.as_slice());
    let start_key = KeySelector::first_greater_or_equal(start);
    let end_key = KeySelector::first_greater_than(end);
    let opts = RangeOption {
        mode: foundationdb::options::StreamingMode::WantAll,
        ..RangeOption::from((start_key, end_key))
    };

    let range = trx.get_range(&opts, 1.into(), false).await?;
    let rows: Vec<Row> = range
        .iter()
        .map(|kv| {
            let key = &kv.key()[db.db_prefix.len()..];
            let (_, id_bytes):(i64, Bytes) = unpack(key).unwrap();

            let id: String = String::from_utf8_lossy(id_bytes.as_ref()).into();

            let (rev_num, raw_rev_str): (i16, Bytes) = unpack(kv.value()).unwrap();
            let rev_str = format!("{}-{}",rev_num, hex::encode(raw_rev_str.as_ref()));

            let row = Row {
                key: id.clone(),
                id,
                value: rev_str
            };
            println!("kv {:?} {:?}", row.id, row.value);
            row
        })
        .collect();

    Ok(rows)
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    let fdb = foundationdb::Database::default().unwrap();

    // write a value
    let trx = fdb.create_trx().unwrap();
    trx.set(b"hello", b"world"); // errors will be returned in the future result
    trx.commit().await.unwrap();

    // read a value
    let trx = fdb.create_trx().unwrap();
    let maybe_value = trx.get(b"hello", false).await.unwrap();

    let value = maybe_value.unwrap(); // unwrap the option

    let couch_directory = trx.get(COUCHDB_PREFIX, false).await.unwrap().unwrap();
    let s = String::from_utf8_lossy(&couch_directory.as_ref());
    println!("dd {:?}", s);

    let dbs = get_dbs(&trx).await.unwrap();

    dbs.iter().for_each(|db| println!("db: {:?}", db.name));
    // all_docs(&trx, &dbs[0]).await?;

    assert_eq!(b"world", &value.as_ref());
    Ok(())
}

fn main() {
    foundationdb::boot(|| {
        let mut rt = Runtime::new().unwrap();
        rt.block_on(async {
            println!("boom");
            async_main().await.unwrap();
        });
    });
}

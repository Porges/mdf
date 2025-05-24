#![feature(error_generic_member_access)] // required, see Compatibility below

use std::{path::PathBuf, time::Instant};

use errful::ExitResult;
use gedcomesque::entities::individual::{ActiveModel as IndividualActive, Entity as Individual};
use gedcomfy::parser::{
    encodings::SupportedEncoding, lines::LineValue, options::ParseOptions, Parser,
};
use sea_orm::{
    sea_query::TableCreateStatement, ActiveValue, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, Schema, TransactionTrait,
};

#[derive(derive_more::Display, errful::Error, derive_more::From, Debug)]
enum Error {
    #[display("I/O error")]
    Io {
        source: std::io::Error,
    },

    #[display("Database error")]
    Database {
        source: sea_orm::DbErr,
    },

    Parse(gedcomfy::parser::ParserError<'static>),

    FileLoad(gedcomfy::parser::FileLoadError),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitResult<Error> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../private/ITIS.ged");
    let filename = path.file_name().unwrap().to_string_lossy().to_string();

    let opts = ParseOptions::default().force_encoding(SupportedEncoding::Windows1252);
    let file_size = { std::fs::File::open(&path)?.metadata()?.len() };
    let start_time = Instant::now();
    let mut parser = Parser::with_options(opts).load_file(&path)?;
    let records = parser.raw_records().map_err(|e| e.to_static())?;
    let elapsed = start_time.elapsed().as_secs_f64();
    println!(
        "parsed {filename} in {}s: ({} bytes, {} records, {} records/s)",
        elapsed,
        file_size,
        records.len(),
        records.len() as f64 / elapsed,
    );

    // let target = "sqlite:gogogo.db?mode=rwc";
    let memtarget = "sqlite::memory:";
    let db: DatabaseConnection = Database::connect(memtarget).await?;

    // db.execute_unprepared("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
    //     .await
    //     .into_diagnostic()?;

    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);
    let stmt: TableCreateStatement = schema.create_table_from_entity(Individual);

    db.execute(db.get_database_backend().build(&stmt)).await?;

    let to_insert = Vec::from_iter(
        records
            .iter()
            .filter(|r| r.value.line.tag.value == "INDI")
            .map(|r| IndividualActive {
                name: ActiveValue::Set(
                    r.records
                        .iter()
                        .find_map(|r| {
                            if r.value.line.tag.value == "NAME" {
                                match r.value.line.line_value.value {
                                    LineValue::None | LineValue::Ptr(_) => todo!("unhandled"),
                                    LineValue::Str(s) => Some(s),
                                }
                            } else {
                                None
                            }
                        })
                        .unwrap_or("Unknown Name")
                        .to_string(),
                ),
                ..Default::default()
            }),
    );

    println!("{} records to insert", to_insert.len());

    let start_time = Instant::now();

    let txn = db.begin().await?;
    for chunk in to_insert.chunks(1000) {
        Individual::insert_many(chunk.to_owned()).exec(&txn).await?;
    }

    txn.commit().await?;

    println!(
        "inserted all records - elapsed {}s",
        start_time.elapsed().as_secs_f64()
    );

    let dudes = Individual::find().count(&db).await?;
    println!("found {dudes} records");

    ExitResult::success()
}

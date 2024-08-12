use std::{path::PathBuf, time::Instant};

use gedcomesque::entities::individual::{ActiveModel as IndividualActive, Entity as Individual};
use gedcomfy::parser::lines::LineValue;
use miette::{IntoDiagnostic, NamedSource};
use sea_orm::{
    sea_query::TableCreateStatement, ActiveValue, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, Schema, TransactionTrait,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> miette::Result<()> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../private/ITIS.ged");
    let filename = path.file_name().unwrap().to_string_lossy().to_string();
    let data = std::fs::read(path).into_diagnostic()?;
    let mut buffer = String::new();
    let records = gedcomfy::parser::parse(&data, &mut buffer).map_err(|e| {
        miette::Report::new(e).with_source_code(NamedSource::new(filename, data.clone()))
    })?;

    // let target = "sqlite:gogogo.db?mode=rwc";
    let memtarget = "sqlite::memory:";
    let db: DatabaseConnection = Database::connect(memtarget).await.into_diagnostic()?;

    // db.execute_unprepared("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
    //     .await
    //     .into_diagnostic()?;

    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);
    let stmt: TableCreateStatement = schema.create_table_from_entity(Individual);

    db.execute(db.get_database_backend().build(&stmt))
        .await
        .into_diagnostic()?;

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

    let txn = db.begin().await.into_diagnostic()?;
    for chunk in to_insert.chunks(1000) {
        Individual::insert_many(chunk.to_owned())
            .exec(&txn)
            .await
            .into_diagnostic()?;
    }

    txn.commit().await.into_diagnostic()?;

    println!(
        "inserted all records - elapsed {}s",
        start_time.elapsed().as_secs_f64()
    );

    let dudes = Individual::find().count(&db).await.into_diagnostic()?;
    println!("found {dudes} records");

    Ok(())
}

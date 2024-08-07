use std::{path::PathBuf, time::Instant};

use gedcomesque::entities::individual::{ActiveModel as IndividualActive, Entity as Individual};
use miette::{Error, IntoDiagnostic, NamedSource};
use sea_orm::{
    sea_query::TableCreateStatement, ActiveModelTrait, ActiveValue, ConnectionTrait, Database,
    DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait, Schema,
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

    let db: DatabaseConnection = Database::connect("sqlite:gogogo.db?mode=rwc")
        .await
        .into_diagnostic()?;

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
                                r.value.line.data.map(|d| d.value)
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

    for chunk in to_insert.chunks(1000) {
        Individual::insert_many(chunk.to_owned())
            .exec(&db)
            .await
            .into_diagnostic()?;
    }

    println!(
        "inserted all records - elapsed {}s",
        start_time.elapsed().as_secs_f64()
    );

    let guys = Individual::find().count(&db).await.into_diagnostic()?;
    println!("found {guys} records");

    Ok(())
}

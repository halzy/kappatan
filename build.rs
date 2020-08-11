fn main() {
    // Only re-run this build.rs script if the sql/db_schema.sql file changes:
    println!("cargo:rerun-if-changed=sql/db_schema.sql");

    // Remove the reference DB
    let _ = std::fs::remove_file("db/reference.db");

    // Set up the reference DB
    let fut = async move {
        simple_env_load::load_env_from(&[".env"]);
        let database_url = std::env::var("DATABASE_URL").unwrap();
        let pool = sqlx::Pool::new(&database_url).await.unwrap();
        sqlx::query_file!("sql/db_schema.sql")
            .execute(&pool)
            .await
            .unwrap();
    };

    tokio::runtime::Runtime::new().unwrap().block_on(fut);
}

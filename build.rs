fn main() {
    let fut = async move {
        dotenv::from_filename(".env").unwrap();
        let database_url = std::env::var("DATABASE_URL").unwrap();
        let pool = sqlx::Pool::new(&database_url).await.unwrap();
        sqlx::query_file!("sql/db_schema.sql")
        .execute(&pool)
        .await
        .unwrap();
    };

    tokio::runtime::Runtime::new().unwrap().block_on(fut);
}

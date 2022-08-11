
use sqlx::{
    mysql::{MySqlPoolOptions, MySqlQueryResult},
    Executor as _,
    MySql,
    Pool
};

// 本番DB（想定）のデータベース接続文字列
// 'static ライフタイムは、 プログラムが走っている間ずっと有効な値への参照に対してつけられる、最大のライフタイム
// &'static => 文字列リテラルへの 'static ライフタイムを持つ参照
pub const DB_STRING_PRODUCTION: &'static str = "mysql://user:pass@localhost:53306/production";
// rust_web_containerからアクセスする場合は上記URIをコンテナが解決できないので下記の接続文字列にする
// pub const DB_STRING_PRODUCTION: &'static str = "mysql://user:pass@mysql_container:3306/production";

// テストDB（想定）のデータベース接続文字列
pub const DB_STRING_TEST: &'static str = "mysql://user:pass@localhost:53306/test";
// rust_web_containerからアクセスする場合は上記URIをコンテナが解決できないので下記の接続文字列にする
// pub const DB_STRING_TEST: &'static str = "mysql://user:pass@mysql_container:53306/test";

// 非同期処理を実行するランタイムを作成
// Rustでは使用する非同期処理ランタイム（Tokioなど）を指定し、そのランタイムの実行コンテキスト内でのみasync/await構文を用いることができる
pub fn create_tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap() // unwrap()に失敗したら落ちる
}

// MySQL接続のためのクライアント
// コネクションプーリングにより動的なクライアント生成を回避
pub async fn create_pool(url: &str) -> Result<Pool<MySql>, sqlx::Error> {
    // .awaitを呼び出さないと、非同期処理は実行されない
    // https://zenn.dev/magurotuna/books/tokio-tutorial-ja/viewer/hello_tokio#async%2Fawait-%E3%82%92%E4%BD%BF%E3%81%86
    MySqlPoolOptions::new().connect(url).await
}

// DBに格納するデータとして、アヤメの測定データを定義
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct IrisMeasurement {
    pub id: Option<u64>,
    pub sepal_length: f64, // がくの長さ
    pub sepal_width: f64, // がくの幅
    pub petal_length: f64, // 花弁の長さ
    pub petal_width: f64, // 花弁の幅
    pub class: String, // 分別名
}

impl IrisMeasurement {
    const TABLE_NAME: &'static str = "iris_measurements";

    pub async fn create_table(pool: &Pool<MySql>) -> Result<MySqlQueryResult, sqlx::Error> {
        // include_str!マクロは外部ファイルをコンパイル時に文字列として読み込むことができるマクロで、
        // ファイルが存在しないとコンパイル段階でエラーとなるので非常に便利です。
        // ただし実行ファイルは事前に読み込む分大きくなるため、大きいファイルに対してはあまり良い手段ではありません。
        pool.execute(include_str!("../migrations/iris_measurements_create.sql"))
            .await
    }

    // SQLxがORM機能を持たないことを象徴するような書き方で、SQLをプリペアドステートメントにより構築しています。
    pub async fn insert(self, pool: &Pool<MySql>) -> Result<MySqlQueryResult, sqlx::Error> {
        // format!マクロの利用
        let sql = format!(
            r#"INSERT INTO {} (sepal_length, sepal_width, petal_length, petal_width, class) VALUES (?, ?, ?, ?, ?)"#,
            Self::TABLE_NAME
        );

        let result = sqlx::query(&sql)
            .bind(self.sepal_length)
            .bind(self.sepal_length)
            .bind(self.sepal_length)
            .bind(self.sepal_length)
            .bind(self.sepal_length)
            .execute(pool) // .executeの引数はexecutor
            .await;

        result
    }

    // アヤメの種類を指定しリストを取得
    pub async fn find_by_class(pool: &Pool<MySql>, class: &str) -> Result<Vec<IrisMeasurement>, sqlx::Error> {
        let sql = format!(
            r#"SELECT * FROM {} WHERE class = ?"#,
            Self::TABLE_NAME
        );

        // sqlx::FromRowトレイトが実装されているのでquery_asを使うことで、
        // MySQLの行データから、IrisMeasurement構造体にデシリアライズできる
        let rows = sqlx::query_as::<_, IrisMeasurement>(&sql)
            .bind(class)
            .fetch_all(pool)
            .await?;

        Ok(rows)
    }

}

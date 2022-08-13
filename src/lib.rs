
use sqlx::{
    mysql::{MySqlPoolOptions, MySqlQueryResult},
    Executor as _,
    MySql,
    Pool
};

// 本番DB（想定）のデ&ータベース接続文字列
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
            .bind(self.sepal_width)
            .bind(self.petal_length)
            .bind(self.petal_width)
            .bind(self.class)
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

#[cfg(test)]
mod test {
    // testsモジュールは、内部モジュールなので、外部モジュール内のテスト配下にあるコードを内部モジュールのスコープに持っていく必要があります。
    // ここではglobを使用して、外部モジュールで定義したもの全てがこのtestsモジュールでも使用可能になるようにしています。
    use super::*;

    pub async fn trucate_table(pool: &Pool<MySql>, name: &str) -> Result<MySqlQueryResult, sqlx::Error> {
        let sql = format!("TRUNCATE TABLE {}", name);
        pool.execute(sql.as_str()).await
    }

    // テスト用データ生成
    fn create_fake() -> IrisMeasurement {
        IrisMeasurement {
            id: None,
            sepal_length: 3.0,
            sepal_width: 4.0,
            petal_length: 5.0,
            petal_width: 6.0,
            class: "Iris-virginica".to_string()
        }
    }

    // テーブルの生成と初期化
    pub async fn setup_database(pool: &Pool<MySql>) {
        let _ = IrisMeasurement::create_table(pool).await.unwrap();
        let _ = trucate_table(pool, IrisMeasurement::TABLE_NAME).await.unwrap();
    }

    #[tokio::test]
    async fn create_and_select_ok() {
        // テスト用のデータベースに接続
        let pool = create_pool(DB_STRING_TEST).await.unwrap();
        // 前回のテスト実行による副作用を初期化
        // 何も返ってこないのでunwrap()していない
        let _ = setup_database(&pool).await;
        let measurement = create_fake();
        let insert_result = measurement.insert(&pool).await.unwrap();

        // insert文によってデータが記録されたかを確認する
        // assert_eq!マクロを利用する
        assert_eq!(
            "MySqlQueryResult { rows_affected: 1, last_insert_id: 1 }",
            format!("{:?}", insert_result)
        );

        let actual1 = IrisMeasurement::find_by_class(&pool, "Iris-virginica")
            .await
            .unwrap();

        // 条件を満たす登録データが取得できたか検証する
        assert_eq!(actual1.len(), 1);

        let actual2 = IrisMeasurement::find_by_class(&pool, "abc")
            .await
            .unwrap();

        // 条件を満たす登録データは取得できないことを検証する
        assert_eq!(actual2.len(), 0);
    }
}

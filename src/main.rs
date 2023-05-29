use serde_json::{json, Value};
use std::{collections::HashMap, net::SocketAddr};
use tokio_postgres::{Client, NoTls, Row, Config};
use dotenv::dotenv;
use std::env;

extern crate levenshtein;
use levenshtein::levenshtein;

use axum::{
    extract::Json,
    routing::{get, Router},
};
// implement the server to handle the requests and respond with json

#[tokio::main]
async fn main() {
    dotenv().ok();
    let addr = "0.0.0.0:3000";

    let app = Router::new().route("/", get(handler));
    axum::Server::bind(&addr.trim().parse().expect("Invalid address"))
        .serve(
            // Don't forget to add `ConnectInfo`
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
}

async fn client() -> Client {
    let host = env::var("HOST").expect("HOST must be set");
    let user = env::var("USER").expect("USER must be set");
    let password = env::var("PASSWORD").expect("PASSWORD must be set");
    let dbname = env::var("DBNAME").expect("DBNAME must be set");
    let config_string = format!("host={} user={} password='{}' dbname={}", host, user, password, dbname);
    let (client, monitor) = tokio_postgres::connect(
        config_string.as_str(),
        NoTls,
    )
    .await
    .unwrap();

    tokio::spawn(async move {
        if let Err(e) = monitor.await {
            eprintln!("Connection error: {}", e);
        }
    });

    client
}

async fn handler() -> Result<Json<Value>, Json<Value>> {
    let input = "London";
    let mut city = HashMap::<String, i64>::new();
    let client = client().await;
    for row in client
        .query("SELECT * FROM city_attributes", &[])
        .await
        .unwrap_or_else(|_| panic!("Error on query"))
    {
        city.insert(
            row.get::<_, String>("city_ascii").to_lowercase(),
            row.get::<_, i64>("id"),
        );
    }
    struct DerivedCities {
        similarity: f64,
        city: String,
    }
    let mut relevant_cities = Vec::<DerivedCities>::new();
    // check if the input is in the hashmap
    for (key, _value) in city.iter() {
        let similarity = levenshtein(&input.trim().to_lowercase(), key);
        if similarity < 3 {
            relevant_cities.push(DerivedCities {
                similarity: similarity as f64,
                city: key.to_string(),
            });
        }
    }

    // sort the relevant cities in ascending order
    relevant_cities.sort_by(|a, b| a.similarity.partial_cmp(&b.similarity).unwrap());
    let mut rows = Vec::<Vec<Row>>::new();
    for relevant_city in relevant_cities {
        rows.push(
            client
                .query(
                    "SELECT * FROM city_attributes WHERE id = $1 ",
                    &[city.get(&relevant_city.city).unwrap()],
                )
                .await
                .unwrap(),
        );
    }

    // convert the rows to json
    let mut response = Vec::<Value>::new();
    let mut lim = 0;
    for row in rows {
        for r in row {
            response.push(json!({
                "city": r.get::<_, String>("city_ascii"),
                "country": r.get::<_, String>("country"),
                "latitude": r.get::<_, f64>("lat"),
                "longitude": r.get::<_, f64>("lng"),
            }));
        }
        lim += 1;
        if lim == 20 {
            break;
        }
    }
    Ok(Json(json!(response)))
}

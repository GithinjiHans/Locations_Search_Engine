use serde_json::{json, Value};
use std::{collections::HashMap, net::SocketAddr};
use tokio_postgres::{Client, NoTls, Row};

use axum::{
    extract::Json,
    routing::{get, Router},
};
// implement the server to handle the requests and respond with json

#[tokio::main]
async fn main() {
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
    let (client, monitor) = tokio_postgres::connect(
        "host=localhost user=githinjihans password='' dbname=worldcities",
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
    let input = "Nai";
    let mut city = HashMap::<String, i64>::new();
    for row in client()
        .await
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
        let similarity = compare_strings(&input.trim().to_lowercase(), key);
        if similarity > 70.0 {
            relevant_cities.push(DerivedCities {
                similarity,
                city: key.to_string(),
            });
        }
    }

    // sort the relevant cities
    relevant_cities.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    let mut rows = Vec::<Vec<Row>>::new();
    for relevant_city in relevant_cities {
        rows.push(
            client()
                .await
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

fn compare_strings(input: &str, key: &str) -> f64 {
    let len1 = input.chars().count();
    let len2 = key.chars().count();
    let mut equality_percentage;

    if key.contains(input) {
        equality_percentage = 90.0;
        // check if the input is a the first part of the key
        if key == input {
            equality_percentage = 100.0;
            return equality_percentage;
        } else if key[0..len1] == input[..] {
            equality_percentage = 99.99;
            return equality_percentage;
        } else if key[len2 - len1..] == input[..] {
            equality_percentage = 95.0;
        }
        return equality_percentage;
    }

    // compare the characters in the input and the key
    let mut count = 0;
    for (i, char1) in input.chars().enumerate() {
        // get the ith character in the key
        let char2 = key.chars().nth(i).unwrap_or_else(|| ' ');
        if char1 == char2 {
            count += 1;
        }
    }
    equality_percentage = (count as f64 / len1 as f64) * 100.0;
    if equality_percentage > 60.0 && key.contains(input[..count].to_string().as_str()) {
        count = input.chars().count() - count;
        if count == 0 {
            equality_percentage = 92.0;
            return equality_percentage;
        }
        equality_percentage += (count as f64 / len1 as f64) * 20.0;
        return equality_percentage;
    }
    equality_percentage
}

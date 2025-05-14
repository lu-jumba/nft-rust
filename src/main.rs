use actix_web::{web, App, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Pool, Postgres};
use std::{collections::HashMap, env};
//use actix_cors::Cors;



mod data;
mod shop;
mod insurance;
mod repairs; // Assume all the previously implemented functions are in this module
mod police;

// Define the structure of incoming requests
#[derive(Debug, Serialize, Deserialize)]
struct Request {
    function: String,
    parameters: serde_json::Value, // Generic JSON object for parameters
}

// Map of function handlers
pub fn get_bc_functions() -> HashMap<&'static str, fn(&Pool<Postgres>, String) -> Result<(), sqlx::Error>> {
    let mut bc_functions = HashMap::new();

// Insurance Peer
bc_functions.insert("contract_type_ls", handlers::list_contract_types);
bc_functions.insert("contract_type_create", handlers::create_contract_type);
bc_functions.insert("contract_type_set_active", handlers::set_active_contract_type);
bc_functions.insert("contract_ls", handlers::list_contracts);
bc_functions.insert("claim_ls", handlers::list_claims);
bc_functions.insert("claim_file", handlers::file_claim);
bc_functions.insert("claim_process", handlers::process_claim);
bc_functions.insert("user_authenticate", handlers::auth_user);
bc_functions.insert("password_update", handlers::update_password);
bc_functions.insert("magic_authenticate", handlers::auth_magic);
bc_functions.insert("user_get_info", handlers::get_user);

// Shop Peer
bc_functions.insert("contract_create", handlers::create_contract);
bc_functions.insert("user_create", handlers::create_user);

// Repair Shop Peer
bc_functions.insert("repair_order_ls", handlers::list_repair_orders);
bc_functions.insert("repair_order_complete", handlers::complete_repair_order);

// Police Peer
bc_functions.insert("theft_claim_ls", handlers::list_theft_claims);
bc_functions.insert("theft_claim_process", handlers::process_theft_claim);

bc_functions
}

async fn invoke_function(
    pool: web::Data<PgPool>,
    request: web::Json<Request>,
) -> impl Responder {
    let function = request.function.as_str();
    let parameters = request.parameters.to_string();

    let mut bc_functions = handlers::get_bc_functions();

    match bc_functions.get(function) {
        Some(handler) => {
            if let Err(err) = handler(&pool, parameters).await {
                return format!("Error executing function '{}': {:?}", function, err);
            }
            format!("Function '{}' executed successfully.", function)
        }
        None => format!("Error: Invalid invoke function '{}'", function),
    }
}

// Start the server
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in the environment variables");

    let pool = PgPool::connect(&database_url).await.unwrap();
    println!("Connected to the database.");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .route("/invoke", web::post().to(invoke_function))
    })
    .bind("127.0.0.1:8080")? // Listen on localhost:8080
    .run()
    .await

/*
HttpServer::new(move || {
    App::new()
        .wrap(Cors::default()) // Allow any origin
        .app_data(web::Data::new(pool.clone()))
        .route("/invoke", web::post().to(invoke_function))
})*/
}


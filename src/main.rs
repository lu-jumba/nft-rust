


use sqlx::{PgPool, Pool, Postgres};
use std::collections::HashMap;
use std::env;
use serde_json::Value;

mod data;
mod shop;
mod insurance;
mod repairs; // Assume all the previously implemented functions are in this module

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load database connection details from environment variables
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://user:password@localhost/database".to_string());

    // Create a connection pool
    let pool: Pool<Postgres> = PgPool::connect(&database_url).await?;
    println!("Connected to the database.");

    // Map of function handlers
    let mut bc_functions: HashMap<&str, fn(&Pool<Postgres>, String) -> Result<(), sqlx::Error>> =
        HashMap::new();

    // Insurance Peer
    bc_functions.insert("contract_type_ls", handlers::list_contract_types);
    bc_functions.insert("contract_type_create", handlers::create_contract_type);
    bc_functions.insert("contract_type_set_active", handlers::set_active_contract_type);
    bc_functions.insert("contract_ls", handlers::list_contracts);
    bc_functions.insert("claim_ls", handlers::list_claims);
    bc_functions.insert("claim_file", handlers::file_claim);
    bc_functions.insert("claim_process", handlers::process_claim);
    bc_functions.insert("user_authenticate", handlers::auth_user);
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

    // Simulate input handling
    let input_function = "claim_ls"; // Replace with input or CLI argument
    let input_args = r#"{}"#.to_string(); // Replace with actual JSON string input

   

    match bc_functions.get(input_function) {
        Some(handler) => {
            handler(&pool, input_args).await?;
            println!("Function '{}' executed successfully.", input_function);
        }
        None => println!("Error: Invalid invoke function '{}'", input_function),
    }

    Ok(())
}


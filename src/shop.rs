use sqlx::{Error, Pool, Postgres};
use serde::{Deserialize, Serialize};
use bcrypt::{hash, verify, DEFAULT_COST};
use uuid::Uuid;
use chrono::NaiveDateTime;



#[derive(Debug, Serialize, Deserialize)]
pub struct CreateContractDto {
    pub uuid: Uuid,
    pub contract_type_uuid: Uuid,
    pub username: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub item: Item,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

pub async fn create_contract(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<String, Error> {
    // Parse the input JSON
    let dto: CreateContractDto = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Check if the user exists
    let user = sqlx::query_as!(
        User,
        r#"
        SELECT username, password, first_name, last_name
        FROM users
        WHERE username = $1
        "#,
        dto.username
    )
    .fetch_optional(pool)
    .await?;

    let mut user_password_hashed = String::new();

    if let Some(existing_user) = user {
        // Verify password for existing user
        if !verify(&dto.password, &existing_user.password).unwrap_or(false) {
            return Err(Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Invalid credentials.",
            ))));
        }
        user_password_hashed = existing_user.password;
    } else {
        // Hash the password for a new user
        user_password_hashed = hash(&dto.password, DEFAULT_COST).map_err(|err| {
            eprintln!("Password hashing failed: {:?}", err);
            Error::Decode(Box::new(err))
        })?;

        // Insert the new user into the database
        sqlx::query!(
            r#"
            INSERT INTO users (username, password, first_name, last_name)
            VALUES ($1, $2, $3, $4)
            "#,
            dto.username,
            user_password_hashed,
            dto.first_name,
            dto.last_name
        )
        .execute(pool)
        .await?;
    }

    // Create the contract
    let contract_id = dto.uuid;
    sqlx::query!(
        r#"
        INSERT INTO contracts (id, username, contract_type_uuid, item, start_date, end_date, void, claim_index)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        contract_id,
        dto.username,
        dto.contract_type_uuid,
        serde_json::to_value(dto.item).unwrap(), // Serialize the item to JSON
        dto.start_date,
        dto.end_date,
        false, // Contract is not void
        serde_json::to_value(Vec::<Uuid>::new()).unwrap() // Empty claim index
    )
    .execute(pool)
    .await?;

    // Respond with the created user details if a new user was created
    if user.is_none() {
        let response = serde_json::json!({
            "username": dto.username,
            "password": dto.password // Return the original password
        });
        return Ok(response.to_string());
    }

    Ok("Contract created successfully.".to_string())
}

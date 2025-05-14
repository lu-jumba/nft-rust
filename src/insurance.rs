use sqlx::{Error, Pool, Postgres};
use serde::{Deserialize, Serialize};
use serde_json::json;
use bcrypt::{hash, verify, DEFAULT_COST};
use uuid::Uuid;

use crate::data::{Claim, ClaimStatus, Contract, RepairOrder};


#[derive(Debug, Serialize, Deserialize)]
pub struct ContractTypeResult {
    pub uuid: String,
    #[serde(flatten)]
    pub contract_type: ContractType,
}

//Add an index to the shop_type column for faster filtering:
//:CREATE INDEX idx_contract_types_shop_type ON contract_types (shop_type);
pub async fn list_contract_types(
    pool: &Pool<Postgres>,
    args: Option<String>, // Optional JSON string for filtering
) -> Result<String, Error> {
    // Base query
    let mut query = String::from(
        "SELECT id AS uuid, shop_type, formula_per_day, max_sum_insured, theft_insured, 
        description, conditions, active, min_duration_days, max_duration_days 
        FROM contract_types",
    );
    let mut query_params: Vec<&(dyn sqlx::Encode<'_> + sqlx::Type<Postgres>)> = vec![];

    // Add filtering if called as a merchant
    if let Some(arg) = args {
        // Deserialize the input JSON
        let input: serde_json::Value = serde_json::from_str(&arg).map_err(|err| {
            eprintln!("Failed to parse input JSON: {:?}", err);
            Error::Decode(Box::new(err))
        })?;

        if let Some(shop_type) = input.get("shop_type").and_then(|v| v.as_str()) {
            query.push_str(" WHERE POSITION(UPPER($1) IN UPPER(shop_type)) > 0 AND active = TRUE");
            query_params.push(&shop_type);
        }
    }

    // Execute the query and fetch results
    let rows = sqlx::query_as::<_, ContractTypeResult>(&query)
        .bind_all(query_params) // Binds all parameters dynamically
        .fetch_all(pool)
        .await?;

    // Serialize results into JSON
    serde_json::to_string(&rows).map_err(|err| {
        eprintln!("Failed to serialize results to JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })
}


pub async fn create_contract_type(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<(), Error> {
    // Parse the input JSON into a partial struct to extract UUID
    let partial: serde_json::Value = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse partial JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    let uuid: Uuid = partial    //was let uuid: Uuid = partial

        .get("uuid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Missing or invalid UUID.",
            )))
        })?
        .parse()
        .map_err(|err| {
            eprintln!("Invalid UUID format: {:?}", err);
            Error::Decode(Box::new(err))
        })?;

    // Deserialize the full ContractType from the input JSON
    let ct: ContractType = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse ContractType JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Insert the contract type into the database
    sqlx::query!(
        r#"
        INSERT INTO contract_types (id, shop_type, formula_per_day, max_sum_insured, theft_insured, 
            description, conditions, active, min_duration_days, max_duration_days)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
        uuid,
        ct.shop_type,
        ct.formula_per_day,
        ct.max_sum_insured,
        ct.theft_insured,
        ct.description,
        ct.conditions,
        ct.active,
        ct.min_duration_days,
        ct.max_duration_days
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn set_active_contract_type(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<(), Error> {
    // Parse the input JSON
    let req: serde_json::Value = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Extract UUID and Active fields from input
    let uuid: Uuid = req
        .get("uuid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Missing or invalid UUID.",
            )))
        })?
        .parse()
        .map_err(|err| {
            eprintln!("Invalid UUID format: {:?}", err);
            Error::Decode(Box::new(err))
        })?;

    let active: bool = req
        .get("active")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| {
            Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Missing or invalid active flag.",
            )))
        })?;

    // Check if the contract type exists
    let contract_exists = sqlx::query!(
        "SELECT 1 FROM contract_types WHERE id = $1",
        uuid
    )
    .fetch_optional(pool)
    .await?;

    if contract_exists.is_none() {
        return Err(Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Contract Type could not be found.",
        ))));
    }

    // Update the active status
    sqlx::query!(
        "UPDATE contract_types SET active = $1 WHERE id = $2",
        active,
        uuid
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractResult {
    pub uuid: String,
    #[serde(flatten)]
    pub contract: Contract,
    pub claims: Option<Vec<Claim>>,
}

pub async fn list_contracts(
    pool: &Pool<Postgres>,
    args: Option<String>, // JSON input as a string for filtering
) -> Result<String, Error> {
    let mut filter_username: Option<String> = None;

    // Parse the input JSON if provided
    if let Some(arg) = args {
        let input: serde_json::Value = serde_json::from_str(&arg).map_err(|err| {
            eprintln!("Failed to parse input JSON: {:?}", err);
            Error::Decode(Box::new(err))
        })?;

        if let Some(username) = input.get("username").and_then(|v| v.as_str()) {
            filter_username = Some(username.to_string());
        }
    }

    let query = if let Some(ref username) = filter_username {
        // Filter contracts by username
        sqlx::query_as!(
            Contract,
            r#"
            SELECT id, username, item, start_date, end_date, void, contract_type_uuid, claim_index
            FROM contracts
            WHERE username = $1
            "#,
            username
        )
    } else {
        // Fetch all contracts
        sqlx::query_as!(
            Contract,
            r#"
            SELECT id, username, item, start_date, end_date, void, contract_type_uuid, claim_index
            FROM contracts
            "#
        )
    };

    let contracts: Vec<Contract> = query.fetch_all(pool).await?;

    // Construct results
    let mut results = Vec::new();

    for contract in contracts {
        let claims = if filter_username.is_some() {
            // Fetch claims associated with the contract
            sqlx::query_as!(
                Claim,
                r#"
                SELECT id, contract_uuid, date, description, is_theft, status, reimbursable, repaired, file_reference
                FROM claims
                WHERE contract_id = $1
                "#,
                contract.id
            )
            .fetch_all(pool)
            .await?
        } else {
            Vec::new()
        };

        results.push(ContractResult {
            uuid: contract.uuid.to_string(),
            contract,
            claims: Some(claims),
        });
    }

    // Serialize results into JSON
    serde_json::to_string(&results).map_err(|err| {
        eprintln!("Failed to serialize results to JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaimResult {
    pub uuid: String,
    #[serde(flatten)]
    pub claim: Claim,
}

pub async fn list_claims(
    pool: &Pool<Postgres>,
    args: Option<String>, // Optional JSON input for filtering
) -> Result<String, Error> {
    let mut filter_status: Option<ClaimStatus> = None;

    // Parse input JSON for filtering status if provided
    if let Some(arg) = args {
        let input: serde_json::Value = serde_json::from_str(&arg).map_err(|err| {
            eprintln!("Failed to parse input JSON: {:?}", err);
            Error::Decode(Box::new(err))
        })?;

        if let Some(status) = input.get("status").and_then(|v| v.as_str()) {
            filter_status = Some(ClaimStatus::from_str(status));
        }
    }

    // Query to fetch claims
    let query = if let Some(ref status) = filter_status {
        if *status != ClaimStatus::Unknown {
            // Filter claims by status
            sqlx::query_as!(
                Claim,
                r#"
                SELECT id, contract_uuid, date, description, is_theft, status, reimbursable, repaired, file_reference
                FROM claims
                WHERE status = $1
                "#,
                status.to_string()
            )
        } else {
            // Fetch all claims if status is "Unknown"
            sqlx::query_as!(
                Claim,
                r#"
                SELECT id, contract_uuid, date, description, is_theft, status, reimbursable, repaired, file_reference
                FROM claims
                "#
            )
        }
    } else {
        // Fetch all claims if no status is provided
        sqlx::query_as!(
            Claim,
            r#"
            SELECT id, contract_uuid, date, description, is_theft, status, reimbursable, repaired, file_reference
            FROM claims
            "#
        )
    };

    let claims: Vec<Claim> = query.fetch_all(pool).await?;

    // Construct results
    let results: Vec<ClaimResult> = claims
        .into_iter()
        .map(|claim| ClaimResult {
            uuid: claim.id.to_string(),
            claim,
        })
        .collect();

    // Serialize results into JSON
    serde_json::to_string(&results).map_err(|err| {
        eprintln!("Failed to serialize results to JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileClaimDto {
    pub uuid: Uuid,
    pub contract_uuid: Uuid,
    pub date: NaiveDateTime,
    pub description: String,
    pub is_theft: bool,
}

pub async fn file_claim(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<(), Error> {
    // Parse input JSON into DTO
    let dto: FileClaimDto = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Create the claim
    let claim_id = dto.uuid;
    let claim = Claim {
        id: claim_id,
        contract_uuid: dto.contract_uuid,
        date: dto.date,
        description: dto.description,
        is_theft: dto.is_theft,
        status: "New".to_string(), // ClaimStatusNew
        //reimbursable: 0.0,
        //repaired: false,
        //file_reference: String::new(),
    };

    // Check if the contract exists
   let contract = claim.Contract(pool);

   /* let contract = sqlx::query_as!(
        Contract,
        r#"
        SELECT id, username, claim_index
        FROM contracts
        WHERE id = $1
        "#,
        dto.contract_uuid
    )
    .fetch_optional(pool)
    .await?;*/

    let mut contract = match contract {
        Some(c) => c,
        None => {
            eprintln!("Contract with UUID {} not found.", dto.contract_uuid);
            return Err(Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Contract could not be found.",
            ))));
        }
    };

    // Insert the claim into the database
    //removed reimbursable, repaired, file_reference & values $7, $8, $9
    sqlx::query!(
        r#"
        INSERT INTO claims (id, contract_uuid, date, description, is_theft, status)  
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        claim.id,
        claim.contract_id,
        claim.date,
        claim.description,
        claim.is_theft,
        claim.status,
        //claim.reimbursable,
        //claim.repaired,
        //claim.file_reference
    )
    .execute(pool)
    .await?;

    // Update the claim index in the contract
    contract.claim_index.push(claim.id);

    // Update the contract in the database
    sqlx::query!(
        r#"
        UPDATE contracts
        SET claim_index = $1
        WHERE id = $2
        "#,
        serde_json::to_value(contract.claim_index).map_err(|err| {
            eprintln!("Failed to serialize claim_index to JSON: {:?}", err);
            Error::Decode(Box::new(err))
        })?,
        contract.id
    )
    .execute(pool)
    .await?;

    Ok(())
}


#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessClaimDto {
    pub uuid: Uuid,
    pub contract_uuid: Uuid,
    pub status: ClaimStatus,
    pub reimbursable: f32,
}

pub async fn process_claim(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<(), Error> {
    // Parse input JSON
    let input: ProcessClaimDto = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Fetch the claim
    let mut claim = sqlx::query_as!(
        Claim,
        r#"
        SELECT id, contract_uuid, date, description, is_theft, status, reimbursable, repaired, file_reference
        FROM claims
        WHERE id = $1 AND contract_uuid = $2
        "#,
        input.uuid,
        input.contract_uuid
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        eprintln!("Claim not found for UUID {}.", input.uuid);
        Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Claim cannot be found.",
        )))
    })?;

    // Validate the status transition
    if !claim.is_theft && claim.status != "New" && input.status != ClaimStatus::Rejected {
        return Err(Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Cannot change the status of a non-new claim.",
        ))));
    }
    if claim.is_theft && claim.status == "New" && input.status != ClaimStatus::Rejected {
        return Err(Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Theft must first be confirmed by authorities.",
        ))));
    }

    // Update the claim status
    claim.status = match input.status {
        ClaimStatus::Repair => "Repair".to_string(),
        ClaimStatus::Reimbursement => "Reimbursement".to_string(),
        ClaimStatus::Rejected => "Rejected".to_string(),
        _ => return Err(Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Unknown status change.",
        )))),
    };

    // Process based on the new status
    match input.status {
        ClaimStatus::Repair => {
            if claim.is_theft {
                return Err(Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Cannot repair stolen items.",
                ))));
            }

            //get the contract
            //let contract = fetch_contract(pool, claim.contract_uuid).await?;
            let contract = claim.Contract(pool);

            let mut contract = match contract {
                Some(c) => c,
                None => {
                    eprintln!("Contract with UUID {} not found.", input.contract_uuid);
                    return Err(Error::Decode(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Contract could not be found.",
                    ))));
                }
            };

            // Create a repair order
            let repair_order = RepairOrder {
                claim_uuid: claim.id,
                contract_id: claim.contract_uuid,
                item_id: contract.item,
                ready: false,
            };

            // Insert the repair order
            sqlx::query!(
                r#"
                INSERT INTO repair_orders (claim_uuid, contract_uuid, item, ready)
                VALUES ($1, $2, $3, $4)
                "#,
                repair_order.claim_uuid,
                repair_order.contract_uuid,
                repair_order.item,
                repair_order.ready
            )
            .execute(pool)
            .await?;
        }

        ClaimStatus::Reimbursement => {
            claim.reimbursable = input.reimbursable;

            // If theft was involved, mark the contract as void
            if claim.is_theft {
                sqlx::query!(
                    r#"
                    UPDATE contracts
                    SET void = TRUE
                    WHERE id = $1
                    "#,
                    claim.contract_uuid
                )
                .execute(pool)
                .await?;
            }
        }

        ClaimStatus::Rejected => {
            claim.reimbursable = 0.0;
        }

        _ => {}
    }

    // Persist the claim
    sqlx::query!(
        r#"
        UPDATE claims
        SET status = $1, reimbursable = $2
        WHERE id = $3
        "#,
        claim.status,
        claim.reimbursable,
        claim.id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/*// Fetch a contract by ID
async fn fetch_contract(pool: &Pool<Postgres>, contract_uuid: Uuid) -> Result<Contract, Error> {
    sqlx::query_as!(
        Contract,
        r#"
        SELECT id, username, item, claim_index, void
        FROM contracts
        WHERE id = $1
        "#,
        contract_uuid
    )
    .fetch_one(pool)
    .await
}*/

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthUserDto {
    pub username: String,
    pub password: String,
}

pub async fn auth_user(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<String, Error> {
    // Parse input JSON
    let input: AuthUserDto = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Fetch the user from the database
    let user = sqlx::query_as!(
        User,
        r#"
        SELECT username, password, first_name, last_name
        FROM users
        WHERE username = $1
        "#,
        input.username
    )
    .fetch_optional(pool)
    .await?;

    // Authenticate the user
    let authenticated = match user {
        Some(user) => verify(&input.password, &user.password).unwrap_or(false), // Verify bcrypt hash
        None => false,
    };

    // Serialize the result into JSON
    serde_json::to_string(&authenticated).map_err(|err| {
        eprintln!("Failed to serialize authentication result: {:?}", err);
        Error::Decode(Box::new(err))
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePasswordDto {
    pub username: String,
    pub new_password: String,
}

pub async fn update_password(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<String, Error> {
    // Parse input JSON
    let input: UpdatePasswordDto = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Check if the username exists
    let user_exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM users WHERE username = $1
        ) AS "exists!"
        "#,
        input.username
    )
    .fetch_one(pool)
    .await?;

    if !user_exists {
        return Err(Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Username does not exist.",
        ))));
    }

    // Hash the new password
    let hashed_password = hash(&input.new_password, DEFAULT_COST).map_err(|err| {
        eprintln!("Failed to hash password: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Update the user's password
    sqlx::query!(
        r#"
        UPDATE users
        SET password = $1
        WHERE username = $2
        "#,
        hashed_password,
        input.username
    )
    .execute(pool)
    .await?;

    Ok(format!("Password for user '{}' updated successfully.", input.username))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthMagicDto {
    pub username: String,
    //pub password: String,
}

pub async fn auth_magic(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<String, Error> {
    // Parse input JSON
    let input: AuthMagicDto = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Fetch the user from the database
    let user = sqlx::query_as!(
        User,
        r#"
        SELECT EXISTS(
            SELECT 1 FROM users WHERE username = $1
        ) AS "exists!"
        "#,
        input.username
    )
    .fetch_optional(pool)
    .await?;

    // Authenticate the username
    let username_authenticated = match user {
        Some(user) => verify(&input.username, &user.username).unwrap_or(false), 
        None => false,
    };

    // Serialize the result into JSON
    serde_json::to_string(&username_authenticated).map_err(|err| {
        eprintln!("Failed to serialize authentication result: {:?}", err);
        Error::Decode(Box::new(err))
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub username: String,
    pub first_name: String,
    pub last_name: String,
}

pub async fn get_user(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<Option<String>, Error> {
    // Parse input JSON
    let input: UserResponse = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Fetch the user from the database
    let user = sqlx::query_as!(
        UserResponse,
        r#"
        SELECT username, first_name, last_name
        FROM users
        WHERE username = $1
        "#,
        input.username
    )
    .fetch_optional(pool)
    .await?;

    // If the user is not found, return None
    if let Some(user) = user {
        // Serialize the user details into JSON
        let response_bytes = serde_json::to_string(&user).map_err(|err| {
            eprintln!("Failed to serialize user details: {:?}", err);
            Error::Decode(Box::new(err))
        })?;
        Ok(Some(response_bytes))
    } else {
        // Return None if the user does not exist
        Ok(None)
    }
}
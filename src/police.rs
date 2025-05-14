use sqlx::{Error, Pool, Postgres};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

//Add indexes to is_theft and status columns in the claims table for efficient filtering:
//CREATE INDEX idx_claims_is_theft_status ON claims (is_theft, status);


#[derive(Debug, Serialize, Deserialize)]
pub struct TheftClaimResult {
    pub uuid: String,
    pub contract_uuid: String,
    pub item: Item,
    pub description: String,
    pub name: String,
}

pub async fn list_theft_claims(pool: &Pool<Postgres>) -> Result<String, Error> {
    // Query to fetch claims flagged as theft and with a status of "New"
    let claims: Vec<Claim> = sqlx::query_as!(
        Claim,
        r#"
        SELECT id, contract_uuid, description, is_theft, status
        FROM claims
        WHERE is_theft = TRUE AND status = 'New'
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut results = Vec::new();

    for claim in claims {
        // Fetch the associated contract
        //get the contract
        contract = claim.Contract(pool);
        let mut contract = match contract {
            Some(c) => c,
            None => {
                eprintln!("Contract with UUID {} not found.", claim.contract_uuid);
                return Err(Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Contract could not be found.",
                ))));
            }
        };

        // Fetch the associated user
        user = contract.User(pool);
        let mut user = match user {
            Some(u) => u,
            None => {
                eprintln!("User not found.");
                return Err(Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "User could not be found.",
                ))));
            }
        };
       /* let user = sqlx::query_as!(
            User,
            r#"
            SELECT username, first_name, last_name
            FROM users
            WHERE username = $1
            "#,
            contract.username
        )
        .fetch_one(pool)
        .await?;*/

        // Construct the result
        results.push(TheftClaimResult {
            uuid: claim.id.to_string(),
            contract_uuid: contract.id.to_string(),
            item: serde_json::from_value(contract.item).unwrap(), // Deserialize JSON item field
            description: claim.description,
            name: format!("{} {}", user.first_name, user.last_name),
        });
    }

    // Serialize results into JSON
    serde_json::to_string(&results).map_err(|err| {
        eprintln!("Failed to serialize results: {:?}", err);
        Error::Decode(Box::new(err))
    })
}


#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessTheftClaimDto {
    pub uuid: Uuid,
    pub contract_uuid: Uuid,
    pub is_theft: bool,
    pub file_reference: String,
}

pub async fn process_theft_claim(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<(), Error> {
    // Parse the input JSON
    let dto: ProcessTheftClaimDto = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Fetch the claim
    let mut claim = sqlx::query_as!(
        Claim,
        r#"
        SELECT id, contract_uuid, is_theft, status, file_reference
        FROM claims
        WHERE id = $1 AND contract_uuid = $2
        "#,
        dto.uuid,
        dto.contract_uuid
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        eprintln!("Claim with UUID {} not found.", dto.uuid);
        Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Claim cannot be found.",
        )))
    })?;

    // Validate claim status and type
    if !claim.is_theft || claim.status != "New" {
        return Err(Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Claim is either not related to theft or has an invalid status.",
        ))));
    }

    // Update the claim's status and file reference
    claim.status = if dto.is_theft {
        "TheftConfirmed".to_string() // Status for confirmed theft
    } else {
        "Rejected".to_string() // Status for rejected claims
    };
    claim.file_reference = Some(dto.file_reference);

    // Persist the updated claim
    sqlx::query!(
        r#"
        UPDATE claims
        SET status = $1, file_reference = $2
        WHERE id = $3
        "#,
        claim.status,
        claim.file_reference,
        claim.id
    )
    .execute(pool)
    .await?;

    Ok(())
}



use serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;
use serde_json::{from_slice, from_value};
use uuid::Uuid;
use sqlx::{Error, Pool, Postgres, query_as};


#[derive(Serialize, Deserialize, Debug)]
pub struct ContractType {
    pub id: Uuid,
    pub shop_type: String,
    pub formula_per_day: String,
    pub max_sum_insured: f32,
    pub theft_insured: bool,
    pub description: String,
    pub conditions: String,
    pub active: bool,
    pub min_duration_days: i32,
    pub max_duration_days: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Item {
    pub id: i32,
    pub brand: String,
    pub model: String,
    pub price: f32,
    pub description: String,
    pub serial_no: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Contract {
    pub id: Uuid,
    pub username: String,
    pub item: Item,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
    pub void: bool,
    pub contract_type_uuid: Uuid,
    pub claim_index: Option<Vec<Uuid>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClaimStatus {
    Unknown,
    New,
    Rejected,
    Repair,
    Reimbursement,
    TheftConfirmed,
}

impl ClaimStatus {
    pub fn from_str(value: &str) -> Self {
        match value.to_uppercase().as_str() {
            "N" => ClaimStatus::New,
            "J" => ClaimStatus::Rejected,
            "R" => ClaimStatus::Repair,
            "F" => ClaimStatus::Reimbursement,
            "P" => ClaimStatus::TheftConfirmed,
            _ => ClaimStatus::Unknown,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            ClaimStatus::Unknown => "",
            ClaimStatus::New => "N",
            ClaimStatus::Rejected => "J",
            ClaimStatus::Repair => "R",
            ClaimStatus::Reimbursement => "F",
            ClaimStatus::TheftConfirmed => "P",
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Claim {
    pub id: Uuid,
    pub contract_uuid: Uuid,
    pub date: NaiveDateTime,
    pub description: String,
    pub is_theft: bool,
    pub status: String,
    pub reimbursable: f32,
    pub repaired: bool,
    pub file_reference: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub username: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub contract_index: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RepairOrder {
    //pub id: Uuid,
    pub claim_uuid: Uuid,
    pub contract_id: Uuid,
    pub item_id: i32,
    pub ready: bool,
}

impl User {
    pub async fn contracts(&self, pool: &Pool<Postgres>) -> Result<Vec<Contract>, sqlx::Error> {
        let mut contracts = Vec::new();

        for contract_id in &self.contract_index {
            // Fetch the contract data from the database
            let contract_row = sqlx::query!(
                "SELECT data FROM contracts WHERE id = $1",
                contract_id
            )
            .fetch_one(pool)
            .await;

            // If an error occurs, return it immediately
            let contract_row = match contract_row {
                Ok(row) => row,
                Err(e) => {
                    eprintln!("Failed to fetch contract {}: {:?}", contract_id, e);
                    return Err(e);
                }
            };

            // Parse the contract data from JSON
            let contract: Contract = match from_slice(&contract_row.data) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to parse contract {}: {:?}", contract_id, e);
                    return Err(sqlx::Error::ColumnDecode {
                        index: "data".into(),
                        source: Box::new(e),
                    });
                }
            };

            // Add the contract to the result vector
            contracts.push(contract);
        }

        Ok(contracts)
    }
}

//Ensure the claim_id column in the claim table is indexed for efficient lookups:
//CREATE INDEX idx_claims_claim_id ON claims (claim_id);
impl Contract {
    pub async fn claims(&self, pool: &Pool<Postgres>) -> Result<Vec<Claim>, Error> {
        let mut claims = Vec::new();

        for claim_id in &self.claim_index {
            // Query the database for the claim by its UUID
            let claim_row = sqlx::query_as!(
                Claim,
                r#"
                SELECT id, contract_uuid, date, description, is_theft, status, reimbursable, repaired, file_reference
                FROM claims
                WHERE id = $1
                "#,
                claim_id
            )
            .fetch_one(pool)
            .await;

            match claim_row {
                Ok(claim) => claims.push(claim),
                Err(e) => {
                    eprintln!("Error fetching claim {}: {:?}", claim_id, e);
                    return Err(e); // Return the error if any claim retrieval fails
                }
            }
        }

        Ok(claims)
    }
}

//Ensure the username column in the users table is indexed for efficient lookups:
//CREATE INDEX idx_users_username ON users (username);
impl Contract {
    pub async fn user(&self, pool: &Pool<Postgres>) -> Result<User, Error> {
        // Validate that the username is not empty
        if self.username.trim().is_empty() {
            return Err(Error::ColumnNotFound(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid username in contract.",
            ))));
        }

        // Query the database for the user with the specified username
        let row = sqlx::query!(
            r#"
            SELECT 
                username, 
                password, 
                first_name, 
                last_name, 
                contract_index 
            FROM users 
            WHERE username = $1
            "#,
            self.username
        )
        .fetch_one(pool)
        .await?;

        // Parse the user data
        let contract_index: Vec<Uuid> = match serde_json::from_value(row.contract_index) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("Failed to parse contract_index JSON for user {}: {:?}", self.username, e);
                return Err(Error::Decode(Box::new(e)));
            }
        };

        Ok(User {
            username: row.username,
            password: row.password,
            first_name: row.first_name,
            last_name: row.last_name,
            contract_index,
        })
    }
}

//Ensure the id column in the contracts table is indexed for fast lookups:
//CREATE INDEX idx_contracts_uuid ON contracts (id);

impl Claim {
    pub async fn contract(&self, pool: &Pool<Postgres>) -> Result<Option<Contract>, Error> {
        // If the contract_uuid is empty, return None
        if self.contract_uuid.is_nil() {
            return Ok(None);
        }

        // Query the database for the contract with the specified UUID
        let row = sqlx::query!(
            r#"
            SELECT 
                id, 
                username, 
                item, 
                start_date, 
                end_date, 
                void, 
                contract_type_uuid, 
                claim_index 
            FROM contracts 
            WHERE id = $1
            "#,
            self.contract_uuid
        )
        .fetch_one(pool)
        .await;

        match row {
            Ok(r) => {
                // Deserialize the `item` and `claim_index` fields from JSON
                let claim_index: Option<Vec<Uuid>> = r.claim_index
                    .map(|value| from_value(value).unwrap_or_else(|_| vec![])); // Safely deserialize
                let item: String = r.item;

                Ok(Some(Contract {
                    id: r.id,
                    username: r.username,
                    item,
                    start_date: r.start_date,
                    end_date: r.end_date,
                    void: r.void,
                    contract_type_uuid: r.contract_type_uuid,
                    claim_index,
                }))
            }
            Err(sqlx::Error::RowNotFound) => Ok(None), // Return None if no contract is found
            Err(e) => {
                eprintln!("Error fetching contract with UUID {}: {:?}", self.contract_uuid, e);
                Err(e)
            }
        }
    }
}



/*#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claim_status_conversion() {
        assert_eq!(ClaimStatus::from_str("N"), ClaimStatus::New);
        assert_eq!(ClaimStatus::New.to_str(), "N");
        assert_eq!(ClaimStatus::from_str("unknown"), ClaimStatus::Unknown);
    }
}*/

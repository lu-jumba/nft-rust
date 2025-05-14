use sqlx::{Error, Pool, Postgres};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

//Add an index to the ready column in the repair_orders table for efficient filtering:
//CREATE INDEX idx_repair_orders_ready ON repair_orders (ready);

#[derive(Debug, Serialize, Deserialize)]
pub struct RepairOrderResult {
    pub uuid: String,
    pub claim_uuid: String,
    pub contract_uuid: String,
    pub item: Item,
}

pub async fn list_repair_orders(pool: &Pool<Postgres>) -> Result<String, Error> {  //replaced id AS uuid with id AS claim_uuid
    // Query to fetch all repair orders where `ready` is false
    let repair_orders: Vec<RepairOrder> = sqlx::query_as!(
        RepairOrder,
        r#"
        SELECT id AS claim_uuid, contract_uuid, item, ready
        FROM repair_orders
        WHERE ready = FALSE
        "#
    )
    .fetch_all(pool)
    .await?;

    // Map the repair orders to the result structure
    let results: Vec<RepairOrderResult> = repair_orders
        .into_iter()
        .map(|order| RepairOrderResult {
            uuid: order.id.to_string(),
            claim_uuid: order.claim_id.to_string(),
            contract_uuid: order.contract_id.to_string(),
            item: serde_json::from_value(order.item).unwrap(), // Deserialize JSON item
        })
        .collect();

    // Serialize results into JSON
    serde_json::to_string(&results).map_err(|err| {
        eprintln!("Failed to serialize repair order results: {:?}", err);
        Error::Decode(Box::new(err))
    })
}


#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteRepairOrderDto {
    pub uuid: Uuid,
}

pub async fn complete_repair_order(
    pool: &Pool<Postgres>,
    args: String, // JSON input as a string
) -> Result<(), Error> {
    // Parse input JSON
    let input: CompleteRepairOrderDto = serde_json::from_str(&args).map_err(|err| {
        eprintln!("Failed to parse input JSON: {:?}", err);
        Error::Decode(Box::new(err))
    })?;

    // Fetch the repair order
    let mut repair_order = sqlx::query_as!(
        RepairOrder,
        r#"
        SELECT claim_uuid, contract_uuid, ready
        FROM repair_orders
        WHERE id = $1
        "#,
        input.uuid
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        eprintln!("Repair order with UUID {} not found.", input.uuid);
        Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find the repair order.",
        )))
    })?;

    // Mark the repair order as ready
    repair_order.ready = true;

    // Update the repair order in the database
    sqlx::query!(
        r#"
        UPDATE repair_orders
        SET ready = TRUE
        WHERE id = $1
        "#,
        repair_order.claim_uuid
    )
    .execute(pool)
    .await?;

    // Update the corresponding claim
    let claim = sqlx::query_as!(
        Claim,
        r#"
        SELECT id, repaired
        FROM claims
        WHERE id = $1 AND contract_uuid = $2
        "#,
        repair_order.claim_uuid,
        repair_order.contract_uuid
    )
    .fetch_optional(pool)
    .await?;

    if let Some(mut claim) = claim {
        claim.repaired = true;

        sqlx::query!(
            r#"
            UPDATE claims
            SET repaired = TRUE
            WHERE id = $1 AND contract_uuid = $2
            "#,
            repair_order.claim_uuid,
            repair_order.contract_uuid
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

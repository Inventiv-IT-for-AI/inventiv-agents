use anyhow::{Context, Result};
use reqwest;
use serde_json::json;
use std::env;

/// Script pour analyser une instance Scaleway créée manuellement via la console
/// et comparer sa configuration avec ce que notre code génère

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Analyzing working Scaleway instance created via console\n");

    // Load environment variables
    let secret_key = env::var("SCALEWAY_SECRET_KEY")
        .context("SCALEWAY_SECRET_KEY not set")?;
    let project_id = env::var("SCALEWAY_PROJECT_ID")
        .context("SCALEWAY_PROJECT_ID not set")?;
    let zone = env::var("SCALEWAY_ZONE").unwrap_or_else(|_| "fr-par-2".to_string());

    // Get server ID from command line or env
    let server_id = env::args()
        .nth(1)
        .or_else(|| env::var("SCALEWAY_SERVER_ID").ok())
        .context("Please provide server ID as argument or SCALEWAY_SERVER_ID env var")?;

    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "X-Auth-Token",
        reqwest::header::HeaderValue::from_str(&secret_key)?,
    );
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    println!("📋 Configuration:");
    println!("   Zone: {}", zone);
    println!("   Server ID: {}\n", server_id);

    // Get full server details
    let url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
        zone, server_id
    );

    println!("🔍 Fetching server details from API...\n");
    let resp = client.get(&url).headers(headers.clone()).send().await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(anyhow::anyhow!("Failed to get server: {}", error_text));
    }

    let server_json: serde_json::Value = resp.json().await?;
    let server = &server_json["server"];

    println!("{}", "=".repeat(80));
    println!("SERVER DETAILS");
    println!("{}", "=".repeat(80));
    println!("{}", serde_json::to_string_pretty(server)?);

    // Extract key fields for comparison
    println!("\n");
    println!("{}", "=".repeat(80));
    println!("KEY FIELDS FOR COMPARISON");
    println!("{}", "=".repeat(80));
    
    println!("\n📦 Volumes:");
    if let Some(volumes) = server["volumes"].as_object() {
        for (slot, vol) in volumes {
            println!("   Slot {}:", slot);
            println!("      ID: {}", vol["id"].as_str().unwrap_or("N/A"));
            println!("      Type: {}", vol["volume_type"].as_str().unwrap_or("N/A"));
            println!("      Size: {} bytes", vol["size"].as_u64().unwrap_or(0));
            println!("      Boot: {}", vol["boot"].as_bool().unwrap_or(false));
            println!("      Name: {}", vol["name"].as_str().unwrap_or("N/A"));
        }
    } else {
        println!("   No volumes (empty object)");
    }

    println!("\n🖥️  Instance Type:");
    println!("   Commercial Type: {}", server["commercial_type"].as_str().unwrap_or("N/A"));
    println!("   Boot Type: {}", server["boot_type"].as_str().unwrap_or("N/A"));

    println!("\n📝 Creation Details:");
    println!("   Image ID: {}", server["image"]["id"].as_str().unwrap_or("N/A"));
    println!("   Image Name: {}", server["image"]["name"].as_str().unwrap_or("N/A"));
    println!("   State: {}", server["state"].as_str().unwrap_or("N/A"));

    println!("\n🌐 Network:");
    if let Some(public_ip) = server["public_ip"].as_object() {
        println!("   Public IP: {}", public_ip["address"].as_str().unwrap_or("N/A"));
    }
    if let Some(ipv6) = server["ipv6"].as_str() {
        println!("   IPv6: {}", ipv6);
    }

    println!("\n🔒 Security Group:");
    if let Some(sg) = server["security_group"].as_object() {
        println!("   ID: {}", sg["id"].as_str().unwrap_or("N/A"));
        println!("   Name: {}", sg["name"].as_str().unwrap_or("N/A"));
    } else {
        println!("   None");
    }

    // Generate what our code would send
    println!("\n");
    println!("{}", "=".repeat(80));
    println!("WHAT OUR CODE WOULD SEND (for comparison)");
    println!("{}", "=".repeat(80));
    
    let our_create_body = json!({
        "name": "test-instance",
        "commercial_type": server["commercial_type"].as_str().unwrap_or("L4-1-24G"),
        "project": project_id,
        "image": server["image"]["id"].as_str().unwrap_or(""),
        "tags": ["test"],
        "dynamic_ip_required": true,
        "boot_type": server["boot_type"].as_str().unwrap_or("local"),
        "volumes": {}
    });
    
    println!("\n{}", serde_json::to_string_pretty(&our_create_body)?);

    println!("\n");
    println!("{}", "=".repeat(80));
    println!("DIFFERENCES TO CHECK");
    println!("{}", "=".repeat(80));
    println!("1. Compare 'volumes' structure");
    println!("2. Check if 'boot_type' is set correctly");
    println!("3. Verify image ID format");
    println!("4. Check for any missing fields");
    println!("5. Verify volume attachment method (API vs CLI)");

    Ok(())
}


use anyhow::{Context, Result};
use reqwest;
use serde_json::json;
use std::env;
use std::net::TcpStream;
use std::time::Duration;
use tokio::time::sleep;

/// Test pour reproduire EXACTEMENT la configuration de l'instance créée via console
/// Basé sur l'analyse de l'instance b44adbb6-7e4a-46a9-a78b-3393cfb11a5f

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Testing exact console configuration\n");

    // Load environment variables
    let secret_key = env::var("SCALEWAY_SECRET_KEY")
        .context("SCALEWAY_SECRET_KEY not set")?;
    let project_id = env::var("SCALEWAY_PROJECT_ID")
        .context("SCALEWAY_PROJECT_ID not set")?;
    let zone = env::var("SCALEWAY_ZONE").unwrap_or_else(|_| "fr-par-2".to_string());
    let access_key = env::var("SCALEWAY_ACCESS_KEY").ok();
    let org_id = env::var("SCALEWAY_ORGANIZATION_ID").ok();

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

    // Image qui fonctionne (Ubuntu Noble GPU OS 13 passthrough avec sbs_snapshot)
    let image_id = "5c3d28db-33ce-4997-8572-f49506339283";
    let instance_type = "L4-1-24G";

    println!("📋 Configuration:");
    println!("   Zone: {}", zone);
    println!("   Instance Type: {}", instance_type);
    println!("   Image ID: {} (Ubuntu Noble GPU OS 13 passthrough)", image_id);
    println!("   Project ID: {}\n", project_id);

    // Step 1: Get image snapshot ID
    println!("[1/6] Getting image snapshot ID...");
    let snapshot_id = get_image_snapshot_id(&client, &headers, &zone, image_id).await?;
    println!("✅ Image snapshot ID: {}\n", snapshot_id);

    // Step 2: Create Block Storage volume (empty for now - Scaleway might populate it from image)
    println!("[2/6] Creating Block Storage volume (200GB)...");
    let volume_name = format!("test-l4-exact-{}", uuid::Uuid::new_v4());
    let volume_id = create_block_storage(
        &client,
        &headers,
        &zone,
        &project_id,
        &volume_name,
        200_000_000_000, // 200GB
    )
    .await?;
    println!("✅ Block Storage created: {}\n", volume_id);
    println!("   Note: Using image with sbs_snapshot - Scaleway should populate volume from image snapshot\n");

    // Step 3: Create instance WITHOUT volumes first (like console might do)
    println!("[3/6] Creating instance WITHOUT volumes first...");
    let instance_name = format!("test-l4-exact-{}", uuid::Uuid::new_v4());
    
    // Create instance without volumes - console might do this first
    // Note: Using dynamic_ip_required: true to get IP automatically
    let create_body = json!({
        "name": instance_name,
        "commercial_type": instance_type,
        "project": project_id,
        "image": image_id,
        "tags": [],
        "dynamic_ip_required": true,  // Use true to get IP automatically
        "boot_type": "local"
        // NO volumes - attach via CLI after
    });

    let create_url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/servers",
        zone
    );
    
    println!("   Request URL: {}", create_url);
    println!("   Request body: {}", serde_json::to_string_pretty(&create_body)?);

    let create_resp = client
        .post(&create_url)
        .headers(headers.clone())
        .json(&create_body)
        .send()
        .await?;

    if !create_resp.status().is_success() {
        let error_text = create_resp.text().await?;
        println!("❌ Instance creation failed: {}", error_text);
        return Err(anyhow::anyhow!("Instance creation failed: {}", error_text));
    }

    let create_json: serde_json::Value = create_resp.json().await?;
    let server_id = create_json["server"]["id"]
        .as_str()
        .context("No server ID in response")?
        .to_string();
    
    let server_state = create_json["server"]["state"]
        .as_str()
        .unwrap_or("unknown");
    
    println!("✅ Instance created: {}", server_id);
    println!("   State: {}", server_state);
    println!("   Boot type: {}\n", create_json["server"]["boot_type"].as_str().unwrap_or("unknown"));

    // Step 4: Attach Block Storage via CLI (like console does)
    println!("[4/6] Attaching Block Storage via CLI (like console does)...");
    let access_key_val = access_key.as_ref().context("SCALEWAY_ACCESS_KEY required")?;
    let org_id_val = org_id.as_ref().context("SCALEWAY_ORGANIZATION_ID required")?;
    attach_block_storage_via_cli(
        &zone,
        &server_id,
        &volume_id,
        access_key_val,
        org_id_val,
        &project_id,
    ).await?;
    println!("✅ Block Storage attached via CLI\n");
    
    // Wait for attachment to propagate
    sleep(Duration::from_secs(3)).await;

    // Step 5: Check volumes attached
    println!("[5/6] Checking attached volumes...");
    let volumes_info = get_server_volumes(&client, &headers, &zone, &server_id).await?;
    println!("   Volumes attached: {}", volumes_info);
    println!();

    // Step 6: Start instance
    println!("[6/7] Starting instance...");
    if server_state != "running" {
        start_server(&client, &headers, &zone, &server_id).await?;
        
        // Wait for instance to be running
        println!("   Waiting for instance to be running...");
        let mut attempts = 0;
        loop {
            sleep(Duration::from_secs(2)).await;
            let state = get_server_state(&client, &headers, &zone, &server_id).await?;
            if let Some(s) = state {
                println!("   Current state: {}", s);
                if s == "running" {
                    break;
                }
            }
            attempts += 1;
            if attempts > 30 {
                return Err(anyhow::anyhow!("Instance failed to start within 60 seconds"));
            }
        }
    }
    println!("✅ Instance is running\n");

    // Step 7: Get IP and test SSH (wait up to 3 minutes)
    println!("[7/7] Getting IP address and testing SSH (waiting up to 3 minutes)...");
    let ip = get_server_ip(&client, &headers, &zone, &server_id).await?;
    println!("   IP address: {}", ip);
    
    // Wait for SSH to be ready (up to 3 minutes = 180 seconds)
    println!("   Waiting for SSH service to start (checking every 10 seconds, max 3 minutes)...");
    let mut ssh_ready = false;
    let max_attempts = 18; // 18 * 10s = 180s = 3 minutes
    for attempt in 1..=max_attempts {
        sleep(Duration::from_secs(10)).await;
        let ssh_test = test_ssh(&ip).await;
        match ssh_test {
            Ok(true) => {
                println!("✅ SSH is accessible after {} seconds!", attempt * 10);
                ssh_ready = true;
                break;
            }
            Ok(false) => {
                if attempt % 3 == 0 {
                    println!("   Still waiting for SSH... ({}s elapsed)", attempt * 10);
                }
            }
            Err(e) => {
                if attempt % 3 == 0 {
                    println!("   SSH test error (will retry): {}", e);
                }
            }
        }
    }
    
    if !ssh_ready {
        println!("❌ SSH not accessible after 3 minutes - there may be a problem");
        return Err(anyhow::anyhow!("SSH not accessible after 3 minutes"));
    }

    println!("\n✅ TEST COMPLETED");
    println!("   Instance ID: {}", server_id);
    println!("   IP: {}", ip);
    println!("   Block Storage: {}", volume_id);
    println!("\n💡 Next steps:");
    println!("   1. SSH into the instance: ssh root@{}", ip);
    println!("   2. Check Block Storage: lsblk");
    println!("   3. Clean up: Delete instance {} and volume {}", server_id, volume_id);

    Ok(())
}

async fn get_image_snapshot_id(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    image_id: &str,
) -> Result<String> {
    let url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/images/{}",
        zone, image_id
    );

    let resp = client.get(&url).headers(headers.clone()).send().await?;
    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(anyhow::anyhow!("Failed to get image: {}", error_text));
    }

    let json: serde_json::Value = resp.json().await?;
    let snapshot_id = json["image"]["root_volume"]["id"]
        .as_str()
        .context("Image does not have root_volume.sbs_snapshot")?
        .to_string();

    Ok(snapshot_id)
}

async fn create_block_storage(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    project_id: &str,
    name: &str,
    size_bytes: i64,
) -> Result<String> {
    let url = format!(
        "https://api.scaleway.com/block/v1/zones/{}/volumes",
        zone
    );

    let body = json!({
        "name": name,
        "project_id": project_id,
        "from_empty": {
            "size": size_bytes
        }
    });

    let resp = client
        .post(&url)
        .headers(headers.clone())
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(anyhow::anyhow!("Block Storage creation failed: {}", error_text));
    }

    let json_resp: serde_json::Value = resp.json().await?;
    let volume_id = json_resp["id"]
        .as_str()
        .context("No volume ID in response")?
        .to_string();

    Ok(volume_id)
}

async fn get_server_volumes(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    server_id: &str,
) -> Result<String> {
    let url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
        zone, server_id
    );

    let resp = client.get(&url).headers(headers.clone()).send().await?;
    let json: serde_json::Value = resp.json().await?;
    
    Ok(serde_json::to_string_pretty(&json["server"]["volumes"])?)
}

async fn get_server_state(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    server_id: &str,
) -> Result<Option<String>> {
    let url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
        zone, server_id
    );

    let resp = client.get(&url).headers(headers.clone()).send().await?;
    if !resp.status().is_success() {
        return Ok(None);
    }

    let json: serde_json::Value = resp.json().await?;
    let state = json["server"]["state"]
        .as_str()
        .map(|s| s.to_string());

    Ok(state)
}

async fn start_server(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    server_id: &str,
) -> Result<()> {
    let url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action",
        zone, server_id
    );

    let body = json!({
        "action": "poweron"
    });

    let resp = client
        .post(&url)
        .headers(headers.clone())
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(anyhow::anyhow!("Failed to start server: {}", error_text));
    }

    Ok(())
}

async fn get_server_ip(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    server_id: &str,
) -> Result<String> {
    let url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
        zone, server_id
    );

    let resp = client.get(&url).headers(headers.clone()).send().await?;
    let json: serde_json::Value = resp.json().await?;
    
    // Try different IP fields
    let ip = json["server"]["public_ip"]["address"]
        .as_str()
        .or_else(|| {
            json["server"]["public_ips"]
                .as_array()
                .and_then(|ips| ips.first())
                .and_then(|ip_obj| ip_obj["address"].as_str())
        })
        .or_else(|| json["server"]["ipv6"].as_str())
        .or_else(|| json["server"]["private_ip"].as_str())
        .context(format!("No IP address found. Full response: {}", serde_json::to_string(&json["server"])?))?
        .to_string();

    Ok(ip)
}

async fn attach_block_storage_via_cli(
    zone: &str,
    server_id: &str,
    volume_id: &str,
    access_key: &str,
    org_id: &str,
    project_id: &str,
) -> Result<()> {
    use std::process::Command;
    
    let output = Command::new("scw")
        .env("SCW_ACCESS_KEY", access_key)
        .env("SCW_SECRET_KEY", std::env::var("SCALEWAY_SECRET_KEY")?)
        .env("SCW_DEFAULT_PROJECT_ID", project_id)
        .env("SCW_DEFAULT_ORGANIZATION_ID", org_id)
        .arg("instance")
        .arg("server")
        .arg("update")
        .arg(server_id)
        .arg(format!("zone={}", zone))
        .arg(format!("volume-ids.0={}", volume_id))
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow::anyhow!("CLI attachment failed: stderr={}, stdout={}", stderr, stdout));
    }
    
    Ok(())
}

async fn test_ssh(ip: &str) -> Result<bool> {
    use std::net::ToSocketAddrs;
    let addr = format!("{}:22", ip)
        .to_socket_addrs()?
        .next()
        .context("Failed to resolve address")?;
    
    match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}


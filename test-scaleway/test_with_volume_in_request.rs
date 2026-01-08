use anyhow::{Context, Result};
use reqwest;
use serde_json::json;
use std::env;
use std::net::TcpStream;
use std::time::Duration;
use tokio::time::sleep;

/// Test : créer instance avec Block Storage attaché DIRECTEMENT dans la requête
/// Même si l'API dit qu'elle ne voit pas le Block Storage, peut-être que ça fonctionne quand même

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Testing instance creation with Block Storage in request\n");

    let secret_key = env::var("SCALEWAY_SECRET_KEY")
        .context("SCALEWAY_SECRET_KEY not set")?;
    let project_id = env::var("SCALEWAY_PROJECT_ID")
        .context("SCALEWAY_PROJECT_ID not set")?;
    let zone = env::var("SCALEWAY_ZONE").unwrap_or_else(|_| "fr-par-2".to_string());

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

    let image_id = "5c3d28db-33ce-4997-8572-f49506339283";
    let instance_type = "L4-1-24G";

    println!("📋 Configuration:");
    println!("   Zone: {}", zone);
    println!("   Instance Type: {}", instance_type);
    println!("   Image ID: {} (Ubuntu Noble GPU OS 13 passthrough)", image_id);
    println!("   Project ID: {}\n", project_id);

    // Step 1: Create Block Storage empty (200GB)
    println!("[1/5] Creating Block Storage volume (200GB)...");
    let volume_name = format!("test-l4-direct-{}", uuid::Uuid::new_v4());
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

    // Step 2: Create instance WITH Block Storage in volumes (even if API says it can't see it)
    println!("[2/5] Creating instance WITH Block Storage in volumes field...");
    let instance_name = format!("test-l4-direct-{}", uuid::Uuid::new_v4());
    
    let create_body = json!({
        "name": instance_name,
        "commercial_type": instance_type,
        "project": project_id,
        "image": image_id,
        "tags": [],
        "dynamic_ip_required": true,
        "boot_type": "local",
        "volumes": {
            "0": {
                "id": volume_id
            }
        }
    });

    let create_url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/servers",
        zone
    );
    
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
        println!("\n💡 This confirms that API Instance cannot see Block Storage created via API Block Storage");
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
    println!();

    // Check volumes
    println!("[3/5] Checking volumes...");
    let volumes_info = get_server_volumes(&client, &headers, &zone, &server_id).await?;
    println!("   Volumes: {}", volumes_info);
    
    // Verify volume size
    let volume_size_info = get_volume_size(&client, &headers, &zone, &volume_id).await?;
    println!("   Block Storage size: {}", volume_size_info);
    println!();

    // Start instance
    println!("[4/5] Starting instance...");
    if server_state != "running" {
        start_server(&client, &headers, &zone, &server_id).await?;
        
        println!("   Waiting for instance to be running...");
        let mut attempts = 0;
        loop {
            sleep(Duration::from_secs(2)).await;
            let state = get_server_state(&client, &headers, &zone, &server_id).await?;
            if let Some(s) = state {
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

    // Get IP and test SSH
    println!("[5/5] Getting IP and testing SSH (waiting up to 3 minutes)...");
    let ip = get_server_ip(&client, &headers, &zone, &server_id).await?;
    println!("   IP address: {}", ip);
    
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
        println!("❌ SSH not accessible after 3 minutes");
        return Err(anyhow::anyhow!("SSH not accessible after 3 minutes"));
    }

    println!("\n✅ TEST COMPLETED SUCCESSFULLY");
    println!("   Instance ID: {}", server_id);
    println!("   IP: {}", ip);
    println!("   Block Storage: {} (200GB)", volume_id);
    println!("\n💡 Next steps:");
    println!("   1. SSH into the instance: ssh root@{}", ip);
    println!("   2. Check volumes: lsblk");
    println!("   3. Clean up: Delete instance {}", server_id);

    Ok(())
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

async fn get_volume_size(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    volume_id: &str,
) -> Result<String> {
    let url = format!(
        "https://api.scaleway.com/block/v1/zones/{}/volumes/{}",
        zone, volume_id
    );

    let resp = client.get(&url).headers(headers.clone()).send().await?;
    if !resp.status().is_success() {
        return Ok("unknown (API error)".to_string());
    }

    let json: serde_json::Value = resp.json().await?;
    if let Some(size) = json["size"].as_u64() {
        let size_gb = size / 1_000_000_000;
        return Ok(format!("{}GB ({} bytes)", size_gb, size));
    }
    
    Ok("unknown".to_string())
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


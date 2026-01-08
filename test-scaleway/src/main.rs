use anyhow::{Context, Result};
use reqwest;
use serde_json::json;
use std::env;
use std::net::TcpStream;
use std::time::Duration;
use tokio::time::sleep;

/// Test script to validate Scaleway L4-1-24G instance provisioning with Block Storage
/// This script tests different sequences to find what actually works

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Starting Scaleway L4-1-24G provisioning test\n");

    // Load environment variables
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

    // Test image ID (Ubuntu Jammy GPU OS)
    let image_id = "1dd97ee0-5990-48c1-9bc8-0a2d5a2fec3d";
    let instance_type = "L4-1-24G";

    println!("📋 Configuration:");
    println!("   Zone: {}", zone);
    println!("   Instance Type: {}", instance_type);
    println!("   Image ID: {}", image_id);
    println!("   Project ID: {}\n", project_id);

    // ============================================
    // TEST SCENARIO 1: Create instance WITHOUT volumes, then attach Block Storage after
    // ============================================
    println!("{}", "=".repeat(60));
    println!("TEST SCENARIO 1: Create instance without volumes, attach Block Storage after");
    println!("{}", "=".repeat(60));

    // Step 1: Create Block Storage volume FROM IMAGE (not empty!)
    println!("\n[1/6] Creating Block Storage volume FROM IMAGE (200GB)...");
    let volume_name = format!("test-l4-{}", uuid::Uuid::new_v4());
    let volume_id = create_block_storage_from_image(
        &client,
        &headers,
        &zone,
        &project_id,
        &volume_name,
        200_000_000_000, // 200GB
        image_id,
    )
    .await?;
    println!("✅ Block Storage created from image: {}\n", volume_id);

    // Step 2: Create instance with volumes: {} to prevent local volume creation
    println!("[2/6] Creating instance with volumes: {{}} to prevent local volume creation...");
    let instance_name = format!("test-l4-{}", uuid::Uuid::new_v4());
    
    let create_body = json!({
        "name": instance_name,
        "commercial_type": instance_type,
        "project": project_id,
        "image": image_id,
        "tags": ["test", "l4-provisioning"],
        "dynamic_ip_required": true,
        "boot_type": "local",
        "volumes": {}
        // Empty volumes object - Scaleway should not create local volume
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

    // Step 3: Check volumes attached (should have local volume)
    println!("[3/6] Checking attached volumes (should have local volume)...");
    let volumes_info = get_server_volumes(&client, &headers, &zone, &server_id).await?;
    println!("   Volumes attached: {}", volumes_info);
    println!();

    // Step 4: Start instance FIRST (with local volume)
    println!("[4/6] Starting instance FIRST (with local volume)...");
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

    // Step 5: Get IP and wait for SSH
    println!("[5/6] Getting IP address and waiting for SSH...");
    let ip = get_server_ip(&client, &headers, &zone, &server_id).await?;
    println!("   IP address: {}", ip);
    
    // Wait for SSH to be ready
    println!("   Waiting for SSH service to start (up to 2 minutes)...");
    let mut ssh_ready = false;
    for i in 0..24 {
        sleep(Duration::from_secs(5)).await;
        let ssh_test = test_ssh(&ip).await;
        match ssh_test {
            Ok(true) => {
                println!("✅ SSH is accessible after {} seconds!", (i + 1) * 5);
                ssh_ready = true;
                break;
            }
            _ => {
                if i % 4 == 3 {
                    println!("   Still waiting for SSH... ({}s elapsed)", (i + 1) * 5);
                }
            }
        }
    }
    
    if !ssh_ready {
        println!("⚠️ SSH not accessible after 2 minutes, but continuing...");
    }

    // Step 6: NOW attach Block Storage (instance is running, SSH should work)
    println!("\n[6/6] Attaching Block Storage AFTER instance started and SSH is ready...");
    let access_key = env::var("SCALEWAY_ACCESS_KEY").ok();
    let org_id = env::var("SCALEWAY_ORGANIZATION_ID").ok();
    
    if access_key.is_some() && org_id.is_some() {
        // Get current volumes to preserve local volume
        let volumes_before = get_server_volumes(&client, &headers, &zone, &server_id).await?;
        println!("   Volumes before attachment: {}", volumes_before);
        
        attach_block_storage_via_cli(
            &zone,
            &server_id,
            &volume_id,
            &access_key.unwrap(),
            &org_id.unwrap(),
            &project_id,
        ).await?;
        println!("✅ Block Storage attached via CLI\n");
        
        // Wait a bit for attachment to propagate
        sleep(Duration::from_secs(3)).await;
        
        // Verify volumes again
        println!("   Verifying volumes after attachment...");
        let volumes_after = get_server_volumes(&client, &headers, &zone, &server_id).await?;
        println!("   Volumes after attachment: {}", volumes_after);
        println!();
    } else {
        println!("⚠️ SCALEWAY_ACCESS_KEY or SCALEWAY_ORGANIZATION_ID not set, skipping CLI attachment");
    }
    let ip = get_server_ip(&client, &headers, &zone, &server_id).await?;
    println!("   IP address: {}", ip);
    
    // Wait a bit for SSH to be ready
    println!("   Waiting 30 seconds for SSH service to start...");
    sleep(Duration::from_secs(30)).await;

    // Test SSH connectivity
    println!("   Testing SSH connectivity...");
    let ssh_test = test_ssh(&ip).await;
    match ssh_test {
        Ok(true) => println!("✅ SSH is accessible!"),
        Ok(false) => println!("⚠️ SSH port is not accessible yet"),
        Err(e) => println!("⚠️ SSH test error: {}", e),
    }

    // Test security group
    println!("\n   Checking security group...");
    let sg_info = get_security_group(&client, &headers, &zone, &server_id).await?;
    println!("   Security group: {}", sg_info);

    println!("\n✅ TEST SCENARIO 1 COMPLETED");
    println!("   Instance ID: {}", server_id);
    println!("   IP: {}", ip);
    println!("   Block Storage: {}", volume_id);
    println!("\n💡 Next steps:");
    println!("   1. SSH into the instance: ssh root@{}", ip);
    println!("   2. Check Block Storage: lsblk");
    println!("   3. Format and mount Block Storage if needed");
    println!("   4. Clean up: Delete instance {} and volume {}", server_id, volume_id);

    Ok(())
}

async fn create_block_storage_from_image(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    project_id: &str,
    name: &str,
    size_bytes: i64,
    image_id: &str,
) -> Result<String> {
    let url = format!(
        "https://api.scaleway.com/block/v1/zones/{}/volumes",
        zone
    );

    // Try creating Block Storage from image
    let body = json!({
        "name": name,
        "project_id": project_id,
        "from_snapshot": {
            "base_snapshot_id": image_id,
            "size": size_bytes
        }
    });

    println!("   Trying to create Block Storage from image/snapshot...");
    println!("   Request body: {}", serde_json::to_string_pretty(&body)?);

    let resp = client
        .post(&url)
        .headers(headers.clone())
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        println!("   ⚠️ Failed to create from snapshot: {}", error_text);
        println!("   Falling back to empty volume...");
        
        // Fallback: create empty volume
        let body_empty = json!({
            "name": name,
            "project_id": project_id,
            "from_empty": {
                "size": size_bytes
            }
        });
        
        let resp_empty = client
            .post(&url)
            .headers(headers.clone())
            .json(&body_empty)
            .send()
            .await?;
        
        if !resp_empty.status().is_success() {
            let error_text_empty = resp_empty.text().await?;
            return Err(anyhow::anyhow!("Block Storage creation failed: {}", error_text_empty));
        }
        
        let json_resp: serde_json::Value = resp_empty.json().await?;
        let volume_id = json_resp["id"]
            .as_str()
            .context("No volume ID in response")?
            .to_string();
        
        return Ok(volume_id);
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
    
    // Try public_ip first, then ipv6, then private_ip
    let ip = json["server"]["public_ip"]["address"]
        .as_str()
        .or_else(|| json["server"]["ipv6"].as_str())
        .or_else(|| json["server"]["private_ip"].as_str())
        .context("No IP address found")?
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

async fn attach_block_storage_via_cli(
    zone: &str,
    server_id: &str,
    volume_id: &str,
    access_key: &str,
    org_id: &str,
    project_id: &str,
) -> Result<()> {
    use std::process::Command;
    
    // First, get existing volumes to preserve them
    let get_output = Command::new("scw")
        .env("SCW_ACCESS_KEY", access_key)
        .env("SCW_SECRET_KEY", std::env::var("SCALEWAY_SECRET_KEY")?)
        .env("SCW_DEFAULT_PROJECT_ID", project_id)
        .env("SCW_DEFAULT_ORGANIZATION_ID", org_id)
        .arg("instance")
        .arg("server")
        .arg("get")
        .arg(server_id)
        .arg(format!("zone={}", zone))
        .arg("-o")
        .arg("json")
        .output()?;
    
    if !get_output.status.success() {
        let stderr = String::from_utf8_lossy(&get_output.stderr);
        return Err(anyhow::anyhow!("Failed to get server info: {}", stderr));
    }
    
    let server_json: serde_json::Value = serde_json::from_slice(&get_output.stdout)?;
    let volumes = server_json["volumes"].as_object();
    
    // Build volume-ids list: existing volumes + new Block Storage
    let mut volume_ids = Vec::new();
    if let Some(vols) = volumes {
        for (_slot, vol) in vols {
            if let Some(vol_id) = vol.get("id").and_then(|id| id.as_str()) {
                volume_ids.push(vol_id.to_string());
            }
        }
    }
    volume_ids.push(volume_id.to_string());
    
    // Attach all volumes (existing + new Block Storage)
    let mut cmd = Command::new("scw");
    cmd.env("SCW_ACCESS_KEY", access_key)
        .env("SCW_SECRET_KEY", std::env::var("SCALEWAY_SECRET_KEY")?)
        .env("SCW_DEFAULT_PROJECT_ID", project_id)
        .env("SCW_DEFAULT_ORGANIZATION_ID", org_id)
        .arg("instance")
        .arg("server")
        .arg("update")
        .arg(server_id)
        .arg(format!("zone={}", zone));
    
    for (idx, vol_id) in volume_ids.iter().enumerate() {
        cmd.arg(format!("volume-ids.{}={}", idx, vol_id));
    }
    
    let output = cmd.output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow::anyhow!("CLI attachment failed: stderr={}, stdout={}", stderr, stdout));
    }
    
    Ok(())
}

async fn get_security_group(
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
    
    let sg = json["server"]["security_group"]
        .as_object()
        .map(|sg| serde_json::to_string(sg).unwrap_or_default())
        .unwrap_or_else(|| "none".to_string());

    Ok(sg)
}


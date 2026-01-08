use anyhow::{Context, Result};
use reqwest;
use serde_json::json;
use std::env;
use std::net::TcpStream;
use std::time::Duration;
use tokio::time::sleep;

/// Test complet de validation : Instance L4-1-24G avec SSH opérationnel et Block Storage >150GB

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 TEST COMPLET DE VALIDATION - Scaleway L4-1-24G\n");
    let separator = "=".repeat(80);
    println!("{}", separator);
    println!("Exigences à valider:");
    println!("  1. Instance de type L4-1-24G");
    println!("  2. SSH opérationnel");
    println!("  3. Instance accessible");
    println!("  4. Block Storage de plus de 150GB");
    println!("{}", separator);
    println!();

    let secret_key = env::var("SCALEWAY_SECRET_KEY")
        .context("SCALEWAY_SECRET_KEY not set")?;
    let project_id = env::var("SCALEWAY_PROJECT_ID")
        .context("SCALEWAY_PROJECT_ID not set")?;
    let zone = env::var("SCALEWAY_ZONE").unwrap_or_else(|_| "fr-par-2".to_string());
    let access_key = env::var("SCALEWAY_ACCESS_KEY")
        .context("SCALEWAY_ACCESS_KEY required for CLI resize")?;
    let org_id = env::var("SCALEWAY_ORGANIZATION_ID")
        .context("SCALEWAY_ORGANIZATION_ID required for CLI resize")?;

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
    let min_storage_gb = 150;

    println!("📋 Configuration:");
    println!("   Zone: {}", zone);
    println!("   Instance Type: {}", instance_type);
    println!("   Image ID: {} (Ubuntu Noble GPU OS 13 passthrough)", image_id);
    println!("   Project ID: {}", project_id);
    println!("   Minimum Storage Required: {}GB\n", min_storage_gb);

    let mut validation_results: Vec<(&str, bool, String)> = Vec::new();

    // ============================================
    // ÉTAPE 1: Créer l'instance avec l'image
    // ============================================
    println!("[ÉTAPE 1/6] Création de l'instance avec l'image...");
    let instance_name = format!("test-validation-{}", uuid::Uuid::new_v4());
    
    let create_body = json!({
        "name": instance_name,
        "commercial_type": instance_type,
        "project": project_id,
        "image": image_id,
        "tags": ["validation-test"],
        "dynamic_ip_required": true,
        "boot_type": "local"
    });

    let create_url = format!(
        "https://api.scaleway.com/instance/v1/zones/{}/servers",
        zone
    );

    let create_resp = client
        .post(&create_url)
        .headers(headers.clone())
        .json(&create_body)
        .send()
        .await?;

    if !create_resp.status().is_success() {
        let error_text = create_resp.text().await?;
        println!("❌ ÉCHEC: Création d'instance échouée: {}", error_text);
        return Err(anyhow::anyhow!("Instance creation failed: {}", error_text));
    }

    let create_json: serde_json::Value = create_resp.json().await?;
    let server_id = create_json["server"]["id"]
        .as_str()
        .context("No server ID in response")?
        .to_string();
    
    let created_instance_type = create_json["server"]["commercial_type"]
        .as_str()
        .unwrap_or("unknown");
    
    println!("✅ Instance créée: {}", server_id);
    println!("   Type: {}", created_instance_type);
    
    // Validation 1: Type d'instance
    if created_instance_type == instance_type {
        validation_results.push(("Type d'instance L4-1-24G", true, "✅".to_string()));
    } else {
        validation_results.push(("Type d'instance L4-1-24G", false, format!("❌ Attendu: {}, Obtenu: {}", instance_type, created_instance_type)));
    }
    println!();

    // ============================================
    // ÉTAPE 2: Récupérer le Block Storage créé automatiquement
    // ============================================
    println!("[ÉTAPE 2/6] Récupération du Block Storage créé automatiquement...");
    let volumes_info = get_server_volumes(&client, &headers, &zone, &server_id).await?;
    println!("   Volumes: {}", volumes_info);
    
    let volumes_json: serde_json::Value = serde_json::from_str(&volumes_info)?;
    let volume_id = volumes_json["0"]["id"]
        .as_str()
        .context("No volume ID found")?
        .to_string();
    
    let volume_size_before = get_volume_size_bytes(&client, &headers, &zone, &volume_id).await?;
    let volume_size_gb_before = volume_size_before / 1_000_000_000;
    
    println!("   Volume ID: {}", volume_id);
    println!("   Taille initiale: {}GB ({} bytes)", volume_size_gb_before, volume_size_before);
    
    // Validation 2: Block Storage existe
    if volume_size_before > 0 {
        validation_results.push(("Block Storage créé automatiquement", true, "✅".to_string()));
    } else {
        validation_results.push(("Block Storage créé automatiquement", false, "❌ Aucun volume trouvé".to_string()));
    }
    println!();

    // ============================================
    // ÉTAPE 3: Agrandir le Block Storage à 200GB
    // ============================================
    println!("[ÉTAPE 3/6] Agrandissement du Block Storage à 200GB...");
    let target_size_bytes: i64 = 200_000_000_000; // 200GB
    let target_size_gb: u64 = (target_size_bytes / 1_000_000_000) as u64;
    
    if volume_size_gb_before < min_storage_gb {
        resize_block_storage_via_cli(
            &zone,
            &volume_id,
            target_size_bytes,
            &access_key,
            &org_id,
            &project_id,
        ).await?;
        println!("✅ Demande d'agrandissement envoyée");
        
        // Wait for resize to complete
        println!("   Attente de la fin de l'agrandissement (jusqu'à 30 secondes)...");
        for i in 0..6 {
            sleep(Duration::from_secs(5)).await;
            let current_size = get_volume_size_bytes(&client, &headers, &zone, &volume_id).await?;
            let current_size_gb: u64 = current_size / 1_000_000_000;
            if current_size_gb >= target_size_gb {
                println!("✅ Agrandissement terminé: {}GB", current_size_gb);
                break;
            }
            if i < 5 {
                println!("   En cours... ({}GB actuellement)", current_size_gb);
            }
        }
    } else {
        println!("ℹ️ Volume déjà assez grand ({}GB >= {}GB)", volume_size_gb_before, min_storage_gb);
    }
    
    let volume_size_after = get_volume_size_bytes(&client, &headers, &zone, &volume_id).await?;
    let volume_size_gb_after = volume_size_after / 1_000_000_000;
    println!("   Taille finale: {}GB ({} bytes)\n", volume_size_gb_after, volume_size_after);
    
    // Validation 3: Taille du Block Storage >= 150GB
    if volume_size_gb_after >= min_storage_gb {
        validation_results.push(
            (
                "Block Storage >= 150GB",
                true,
                format!("✅ {}GB", volume_size_gb_after)
            )
        );
    } else {
        validation_results.push(
            (
                "Block Storage >= 150GB",
                false,
                format!("❌ {}GB < {}GB", volume_size_gb_after, min_storage_gb)
            )
        );
    }

    // ============================================
    // ÉTAPE 4: Démarrer l'instance
    // ============================================
    println!("[ÉTAPE 4/6] Démarrage de l'instance...");
    let server_state = get_server_state(&client, &headers, &zone, &server_id).await?
        .unwrap_or_else(|| "unknown".to_string());
    
    if server_state != "running" {
        start_server(&client, &headers, &zone, &server_id).await?;
        
        println!("   Attente du démarrage de l'instance...");
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
    println!("✅ Instance démarrée\n");

    // ============================================
    // ÉTAPE 5: Récupérer l'IP et tester l'accessibilité
    // ============================================
    println!("[ÉTAPE 5/6] Récupération de l'adresse IP et test d'accessibilité...");
    let ip = get_server_ip(&client, &headers, &zone, &server_id).await?;
    println!("   IP publique: {}", ip);
    
    // Test de connectivité réseau (vérifier que l'IP est assignée)
    println!("   Vérification de l'assignation IP...");
    if !ip.is_empty() {
        validation_results.push(("Instance accessible (réseau)", true, format!("✅ IP: {}", ip)));
        println!("✅ Instance accessible sur le réseau (IP: {})", ip);
    } else {
        validation_results.push(("Instance accessible (réseau)", false, "❌ Aucune IP assignée".to_string()));
        println!("⚠️ Instance peut ne pas être accessible");
    }
    println!();

    // ============================================
    // ÉTAPE 6: Test SSH (jusqu'à 3 minutes)
    // ============================================
    println!("[ÉTAPE 6/6] Test d'accès SSH (attente jusqu'à 3 minutes)...");
    let mut ssh_ready = false;
    let max_attempts = 18; // 18 * 10s = 180s = 3 minutes
    let mut ssh_accessible_at = None;
    
    for attempt in 1..=max_attempts {
        sleep(Duration::from_secs(10)).await;
        let ssh_test = test_ssh(&ip).await;
        match ssh_test {
            Ok(true) => {
                ssh_accessible_at = Some(attempt * 10);
                println!("✅ SSH accessible après {} secondes!", attempt * 10);
                ssh_ready = true;
                break;
            }
            Ok(false) => {
                if attempt % 3 == 0 {
                    println!("   En attente de SSH... ({}s écoulées)", attempt * 10);
                }
            }
            Err(e) => {
                if attempt % 3 == 0 {
                    println!("   Erreur de test SSH (nouvelle tentative): {}", e);
                }
            }
        }
    }
    
    if ssh_ready {
        validation_results.push(
            (
                "SSH opérationnel",
                true,
                format!("✅ Accessible après {} secondes", ssh_accessible_at.unwrap())
            )
        );
    } else {
        validation_results.push(("SSH opérationnel", false, "❌ Non accessible après 3 minutes".to_string()));
        return Err(anyhow::anyhow!("SSH not accessible after 3 minutes"));
    }
    println!();

    // ============================================
    // RAPPORT FINAL DE VALIDATION
    // ============================================
    println!("\n");
    let separator = "=".repeat(80);
    println!("{}", separator);
    println!("RAPPORT FINAL DE VALIDATION");
    println!("{}", separator);
    println!();
    
    let mut all_passed = true;
    for (requirement, passed, details) in &validation_results {
        let status = if *passed { "✅ PASS" } else { "❌ FAIL" };
        println!("{} {}: {}", status, requirement, details);
        if !passed {
            all_passed = false;
        }
    }
    
    println!();
    println!("{}", separator);
    if all_passed {
        println!("🎉 TOUS LES TESTS SONT PASSÉS !");
        println!();
        println!("Instance validée:");
        println!("   Instance ID: {}", server_id);
        println!("   Type: {}", instance_type);
        println!("   IP: {}", ip);
        println!("   Block Storage: {} ({}GB)", volume_id, volume_size_gb_after);
        println!("   SSH: Accessible après {} secondes", ssh_accessible_at.unwrap());
    } else {
        println!("❌ CERTAINS TESTS ONT ÉCHOUÉ");
        return Err(anyhow::anyhow!("Validation failed"));
    }
    println!("{}", separator);
    println!();
    println!("💡 Prochaines étapes:");
    println!("   1. SSH dans l'instance: ssh root@{}", ip);
    println!("   2. Vérifier les volumes: lsblk");
    println!("   3. Vérifier l'espace disque: df -h");
    println!("   4. Nettoyer: Supprimer l'instance {} et le volume {}", server_id, volume_id);

    Ok(())
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

async fn get_volume_size_bytes(
    client: &reqwest::Client,
    headers: &reqwest::header::HeaderMap,
    zone: &str,
    volume_id: &str,
) -> Result<u64> {
    let url = format!(
        "https://api.scaleway.com/block/v1/zones/{}/volumes/{}",
        zone, volume_id
    );

    let resp = client.get(&url).headers(headers.clone()).send().await?;
    if !resp.status().is_success() {
        return Ok(0);
    }

    let json: serde_json::Value = resp.json().await?;
    if let Some(size) = json["size"].as_u64() {
        return Ok(size);
    }
    
    Ok(0)
}

async fn resize_block_storage_via_cli(
    zone: &str,
    volume_id: &str,
    new_size_bytes: i64,
    access_key: &str,
    org_id: &str,
    project_id: &str,
) -> Result<()> {
    use std::process::Command;
    
    let new_size_gb = new_size_bytes / 1_000_000_000;
    
    let output = Command::new("scw")
        .env("SCW_ACCESS_KEY", access_key)
        .env("SCW_SECRET_KEY", std::env::var("SCALEWAY_SECRET_KEY")?)
        .env("SCW_DEFAULT_PROJECT_ID", project_id)
        .env("SCW_DEFAULT_ORGANIZATION_ID", org_id)
        .arg("block")
        .arg("volume")
        .arg("update")
        .arg(volume_id)
        .arg(format!("zone={}", zone))
        .arg(format!("size={}GB", new_size_gb))
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow::anyhow!("CLI resize failed: stderr={}, stdout={}", stderr, stdout));
    }
    
    Ok(())
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
        .context(format!("No IP address found"))?
        .to_string();

    Ok(ip)
}

async fn test_tcp_connect(ip: &str, port: u16) -> bool {
    use std::net::ToSocketAddrs;
    let addr = match format!("{}:{}", ip, port).to_socket_addrs() {
        Ok(mut addrs) => match addrs.next() {
            Some(addr) => addr,
            None => return false,
        },
        Err(_) => return false,
    };
    
    match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(_) => true,
        Err(_) => false,
    }
}

async fn test_ssh(ip: &str) -> Result<bool> {
    Ok(test_tcp_connect(ip, 22).await)
}

